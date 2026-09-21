//! **The conveyors, and everything that moves on them.**
//!
//! One rule, in one place, with no Bevy in it beyond the resources it is kept in: the tests at the
//! bottom run the whole of it — carrying, jamming, merging, digging, delivering — with no `App`
//! at all.
//!
//! **Where an item is.** A belt tile is [`TILE_PX`] **steps** long and an item on it is a whole
//! number of them: how far through that tile it has come — and, since F2, **which item it is**
//! ([`OnBelt`], two fields and eight bytes). A lane is the items on one tile, **front first**, so
//! `lanes[t][0]` is the one nearest the end of the tile and every item is at least
//! [`Rules::spacing`] behind the one in front of it. That invariant is the whole model: carrying
//! is adding a step to each number, jamming is a number that cannot grow, and merging is two
//! tiles handing items to one and each finding the gap the other left.
//!
//! **Why steps and not a fraction of a tile.** F1 held the position as an `f32` in `0..=1` and
//! measured what that costs: with `items_per_tile` at 3, a jammed tile holds three items and not
//! four, because three gaps of `f32(1/3)` come to a hair over a tile — and *which* way the
//! rounding falls is a chain of subtractions and not something to work out on paper
//! (`docs/worklog/2026-09-21-factory-F1.md` §5.2). F2a made the position a whole number of steps
//! instead, **one step to one pixel of the art**, so that a tile's worth of gaps is a tile
//! exactly, a jammed tile holds one more than the gaps that fit at *every* gap a belt can have,
//! and the drawing goes from a position to a pixel with nothing to round ([`crate::grid::item_at`]).
//! What it costs is that `items_per_tile` has to divide [`TILE_PX`] — the data stage refuses the
//! rest at the line they are written on (`crate::data`).
//!
//! **Why the positions are in a `Vec` and not a component on an entity.** Because the rule above
//! needs *the item in front*, and an ECS query has no such thing. F1 built both ways round
//! and measured them at thousands of items before choosing (`src/items.rs`,
//! `docs/worklog/2026-09-21-factory-F1.md` §3).
//!
//! **Where the numbers are.** [`Rules`] is what `ruby/data.rb` says — F1 kept these four in
//! `factory.settings.txt` because there was no Ruby yet, and F2 moved them to where the data
//! stage was always meant to hold them. Nothing in here is a `const`.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::data::{Data, ItemId, RecipeId};
use crate::grid::{Building, Dir, Grid, Ore, What};
use crate::machines;
use crate::TILE_PX;

/// **How far along a belt something is, in steps**, and the one number the conveyors are made of.
///
/// There are [`TILE_PX`] steps to a tile, which is one step to one pixel of the art — the sheet's
/// own 16, not a number chosen here, and the reason the drawing has nothing to scale.
///
/// **Signed**, because the arithmetic runs backwards as well as forwards: an item is held behind
/// the one in front of it by subtracting a gap over and over ([`hold`]), and a queue longer than
/// the tile it is on runs off the back of it into negative numbers — which is where an item being
/// fed in from the belt behind really is. **Thirty-two bits**, because the cap on how far a front
/// item may go is *two* tiles ahead (the hand-over moves one tile at a time, so the second tile is
/// the far end of anything reachable) and a frame's own step is `tiles_per_second × TILE_PX ×
/// seconds`, which a data file may write as large as it likes: the conversion saturates and the
/// adding below saturates, so no number a player can write is an overflow.
pub type Steps = i32;

/// **The numbers of play**, every one of them a line of `ruby/data.rb` — the belt's two, the
/// miner's one and what it brings up, the chest's one, and (since F2a) how much ore a tile of the
/// ground holds. `docs/numbers.md` §9.3 says where each default came from, and a player changes
/// them by editing the file.
///
/// **Every one of them is more than zero**, and whoever fills this in is what makes that true:
/// the data stage refuses a declaration whose number is not, at the line it is written on
/// (`crate::data::more_than_zero`). There is no floor inside the arithmetic below, because a
/// floor is a number, and a number that is only there to stop a division would have nowhere it
/// came from.
///
/// **The one number that is not here** — how wide a patch of ore is — is [`crate::Map`]'s,
/// because the smallest map a patch fits on is derived from it in `main`, before there is a VM to
/// have read any Ruby with. How much ore a tile holds was the map's too until F2a and is a line
/// of `data.rb` now: nothing needs it before the first frame.
#[derive(Resource, Debug, Clone)]
pub struct Rules {
    /// How many tiles an item is carried in a second.
    pub belt_tiles_per_second: f32,
    /// **How many items fit on one tile of belt**, and one of the divisors of [`TILE_PX`]: the
    /// gap between two items is a tile's steps divided by this, and a gap that is not a whole
    /// number of steps is not a gap. The data stage is what makes that true
    /// (`crate::data::fits_a_tile`), at the line the number is written on.
    pub items_per_tile: u32,
    /// How long a miner takes over one item, in seconds.
    pub mine_seconds: f32,
    /// How many items a chest holds, of all kinds together, before it stops taking them.
    pub chest_capacity: u32,
    /// What a miner brings up out of the ground.
    pub digs: ItemId,
    /// **How long one swing of an inserter's arm takes, in seconds** — `inserter :arm,
    /// seconds_per_item:` in `ruby/data.rb`, and the same word the miner uses for the same
    /// meaning: how long this thing takes over one item.
    ///
    /// It is also what an idle inserter's script waits between looks ([`crate::inserters`]), and
    /// that is not a second number: an arm that is already busy could not have acted sooner, so
    /// looking more often than it can swing is looking for nothing.
    pub swing_seconds: f32,
    /// **How many items can be dug out of one tile of ore.** It is here rather than in
    /// [`crate::Map`] because it is a number of *play* — how long a patch lasts is how long a line
    /// of miners is worth building — and because `Ore::laid_out` runs in `Startup`, after the data
    /// stage, so there is a VM to have read it with. How *wide* a patch is stayed a setting: the
    /// smallest map is derived from that one in `main`, where there is not.
    pub ore_per_tile: u32,
}

impl Rules {
    /// **The gap between two items on a belt, in steps.** It comes out exactly because
    /// [`Rules::items_per_tile`] divides [`TILE_PX`] — which is the data stage's promise and not
    /// something guarded here, for the same reason there is no floor in the arithmetic: a guard
    /// would be a number with nowhere to have come from.
    pub fn spacing(&self) -> Steps {
        (TILE_PX / self.items_per_tile) as Steps
    }

    /// How long one tile of belt is, in steps — the whole of what a tile means to the conveyors.
    pub fn tile(&self) -> Steps {
        TILE_PX as Steps
    }

    /// **How long one frame of the belt's two-frame animation lasts**, derived rather than
    /// chosen: the pack draws a chevron every 8 px and the second frame is the first with the
    /// chevrons moved half of that (`tools/factory-belts.py`), so the picture only reads as one
    /// moving belt if a swap happens every 4 px the belt travels. One tile is 16 px.
    pub fn belt_frame_seconds(&self) -> f32 {
        4.0 / (self.belt_tiles_per_second * crate::TILE_PX as f32)
    }

    /// How many items a belt delivers a second when it is full — the number a miner's rate and a
    /// chest's size are set against (`docs/numbers.md` §9).
    pub fn belt_items_per_second(&self) -> f32 {
        self.belt_tiles_per_second * self.items_per_tile as f32
    }
}

/// **One item on a belt**: how far through its tile it has come, and which item it is.
///
/// F1 had one kind of thing in the world and an item was the number on its own. `data.rb`
/// declares as many kinds as it likes, so a furnace can refuse a plate and a chest can hold a
/// mixture, and the kind has to travel with the position — it is the only place an item exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnBelt {
    /// `0..=TILE_PX`: how far through the tile, in steps. `0` is the tile's entry edge, which is
    /// the same point in the world as [`TILE_PX`] on the tile before it. It goes negative while
    /// something is being fed in from behind, which is where those items really are.
    pub along: Steps,
    pub item: ItemId,
}

/// **The items on the belts**: one lane per tile of the map, front first.
///
/// Tiles with nothing on them keep an empty lane rather than being left out, because a lane is
/// looked up by tile index from three places and a map of 4,096 empty `VecDeque`s is 128 KB that
/// is never walked (the stepping walks [`Grid::built`], not the map).
#[derive(Resource, Debug, Default)]
pub struct Lanes {
    pub of: Vec<VecDeque<OnBelt>>,
    /// Scratch: where each lane's last item was at the *start* of the frame, or [`Steps::MAX`] for
    /// a lane with nothing on it — which is "as far ahead as you like: nothing is in the way", and
    /// is what F1 wrote as `f32::INFINITY` before the positions were whole numbers. Every belt
    /// asks its neighbour how much room there is, and asking one snapshot makes the answer the
    /// same whatever order the tiles happen to be walked in.
    ///
    /// **A sentinel rather than an `Option`**, which is what F1's `f32::INFINITY` was: this is
    /// written once per tile of the map and read once per belt every frame, and `Option<i32>` is
    /// eight bytes where the number is four, with a branch to unwrap it in the middle of the
    /// carrying. The arithmetic that reads it widens, so the sentinel needs no special case — a
    /// tail that far ahead lands on the cap by itself.
    tails: Vec<Steps>,
    /// Scratch: the tiles with something on them, copied out of the grid at the start of the
    /// step. A copy because the passes below reach into the grid to fill a chest and to move a
    /// miner on, and a list borrowed from it cannot be walked while that happens.
    order: Vec<u32>,
    /// **The part of a step the belts have not taken yet**, always less than one.
    ///
    /// A frame moves the belts `tiles_per_second × TILE_PX × seconds` steps, and that is hardly
    /// ever a whole number: at the speed `data.rb` is played at it is **half a step a frame**, so
    /// a factory that threw the fraction away would not move at all, and one that rounded it would
    /// run at a speed its own frame rate chose. What is left over is kept here and taken next
    /// frame, so the distance covered in a second is the distance the speed says whether the
    /// frames are a sixtieth of a second or a fifth.
    ///
    /// **One of these for the whole factory, not one per belt**, because what has to be true is
    /// that belts of the same speed move the same amount in the same frame — and every belt in
    /// this game has the same speed (`data.rb` declares exactly one `belt`). A remainder per tile
    /// would say the same thing more expensively while there is one speed; a second kind of belt
    /// would want one remainder per *speed*, which is still not one per tile.
    part_of_a_step: f32,
}

impl Lanes {
    pub fn for_map(tiles: u32) -> Lanes {
        let n = (tiles * tiles) as usize;
        Lanes {
            of: vec![VecDeque::new(); n],
            tails: vec![Steps::MAX; n],
            order: Vec::new(),
            part_of_a_step: 0.0,
        }
    }

    /// **How many whole steps the belts take this frame**, with the fraction that is left over
    /// kept for the next one ([`Lanes::part_of_a_step`]).
    ///
    /// The conversion from a float to an integer saturates in Rust, and a frame that saturated has
    /// asked for more than the far end of the map; there is nothing sensible to carry out of one,
    /// so the remainder is clamped back into the `0..1` it is defined to be in.
    fn take_a_step(&mut self, rules: &Rules, seconds: f32) -> Steps {
        let wanted = self.part_of_a_step
            + rules.belt_tiles_per_second * TILE_PX as f32 * seconds.max(0.0);
        let whole = wanted.floor() as Steps;
        self.part_of_a_step = (wanted - whole as f32).clamp(0.0, 1.0);
        whole.max(0)
    }

    pub fn count(&self) -> usize {
        self.of.iter().map(|lane| lane.len()).sum()
    }

    /// Everything on the tile, front first.
    pub fn on(&self, tile: usize) -> &VecDeque<OnBelt> {
        &self.of[tile]
    }
}

/// **What happened to the items this step**, in the order it happened.
///
/// The counters on the other end of it are what a HUD and the checks read. It is a list rather
/// than three numbers because F1's measurement had a second way of holding an item that replayed
/// it to keep an entity per item in step with the lanes (`src/items.rs`), and an order is what
/// that needed; the shape is worth keeping for the next thing that wants to watch the items
/// rather than count them.
#[derive(Debug, Default)]
pub struct Moves(pub Vec<Move>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// The front item of `from` became the last item of `to`.
    Carried { from: usize, to: usize },
    /// The front item of `from` went into a chest and is gone.
    ///
    /// **Only items that were on a belt are `Taken`.** A miner standing next to a chest puts its
    /// dig straight in, and that is one [`Move::Made`] and nothing else: it was never carried, so
    /// counting it here as well would make `carried + taken` stop meaning "what the belts did".
    Taken { from: usize },
    /// A miner or a machine put a new item into `at` — the back of its lane if `at` is a belt, or
    /// the chest or machine itself otherwise. **Whatever it went into, because what this counts
    /// is things delivered**, and a miner that happens to stand next to a chest is digging just
    /// as much as one feeding a belt.
    Made { at: usize },
    /// A machine at `at` finished a craft of `recipe` and is holding what it made.
    Crafted { at: usize, recipe: RecipeId },
    /// **An inserter's arm arrived**, and either put down what it was carrying or found the tile
    /// in front would not take it. It is the one [`Move`] a *script* is waiting on: the system
    /// that stepped the factory turns it into the answer to that inserter's `move`
    /// ([`crate::inserters`]).
    Swung { at: usize, placed: bool },
}

impl Moves {
    pub fn carried(&self) -> usize {
        self.0.iter().filter(|m| matches!(m, Move::Carried { .. })).count()
    }
    pub fn taken(&self) -> usize {
        self.0.iter().filter(|m| matches!(m, Move::Taken { .. })).count()
    }
    pub fn made(&self) -> usize {
        self.0.iter().filter(|m| matches!(m, Move::Made { .. })).count()
    }
    pub fn crafted(&self) -> usize {
        self.0.iter().filter(|m| matches!(m, Move::Crafted { .. })).count()
    }
    /// Every arm that arrived this step, and whether it put down what it was carrying.
    pub fn swung(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        self.0.iter().filter_map(|m| match m {
            &Move::Swung { at, placed } => Some((at, placed)),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------------------------

/// **One step of the whole factory**, and the only thing in the game that moves an item.
///
/// In six passes, and the order of them is the model:
///
/// 1. **the tails**, as they were before anything moved — what a belt is allowed to push into its
///    neighbour is read from this, so two belts merging into a third both see the same picture;
/// 2. **carry**: every item moves forward by a step, none closer to the one in front than the gap;
/// 3. **hand over**: an item that has reached the end of its tile goes to the next one, *if* there
///    is still room when its turn comes — this is where a merge is decided, the tile with the
///    lower index gets the gap, and where a chest or a machine takes something off a belt;
/// 4. **dig**: a miner that has finished puts an item into what it faces, or holds it and waits;
/// 5. **swing**: an inserter whose arm is crossing moves it on, and puts down what it is carrying
///    when it arrives — the half of an inserter that is not Ruby;
/// 6. **craft**: a machine with nothing waiting to go out works at a recipe it has the parts for.
///
/// The last three are [`crate::machines`], because what they are about is a machine and not a
/// belt; what is here is the order, which is the part that is the model.
pub fn step(
    grid: &mut Grid,
    ore: &mut Ore,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    seconds: f32,
) -> Moves {
    let mut moves = Moves::default();
    let spacing = rules.spacing();
    let tile = rules.tile();
    let forward = lanes.take_a_step(rules, seconds);
    lanes.order.clear();
    lanes.order.extend_from_slice(grid.built());

    // ---- 1. the tails as they were ---------------------------------------------------------
    for tail in &mut lanes.tails {
        *tail = Steps::MAX;
    }
    for i in 0..lanes.order.len() {
        let t = lanes.order[i] as usize;
        if grid.at(t).is_some_and(|b| b.what == What::Belt) {
            lanes.tails[t] = lanes.of[t].back().map_or(Steps::MAX, |i| i.along);
        }
    }

    // ---- 2. carry ---------------------------------------------------------------------------
    for i in 0..lanes.order.len() {
        let t = lanes.order[i] as usize;
        let Some(&Building { what: What::Belt, dir, .. }) = grid.at(t) else { continue };
        // How far the front item may get. Past the end of the tile only if the next tile is a
        // belt with room in it; up to the end of the tile otherwise, which is what a chest, a
        // machine, the edge of the map and a belt facing back at this one all look like.
        //
        // **Two tiles is the far end of the next tile**, and it is a cap rather than a number: an
        // empty neighbour has nothing in the way at all, so without it the front would run as far
        // as a step can carry it in one frame. The hand-over below moves an item one tile at a
        // time, so at the speeds this is played at (half a step a frame) nothing ever reaches it.
        // At a tile a frame it would, and then *whether* the second join is crossed in the same
        // frame depends on the neighbour's index — see the F1 worklog's notes.
        let room = match room_ahead(grid, t, dir) {
            // in 64 bits, so that the empty lane's sentinel tail lands on the cap rather than
            // wrapping round to a tile with no room in it at all
            Some(next) => ((tile as i64 + lanes.tails[next] as i64 - spacing as i64)
                .min(2 * tile as i64)) as Steps,
            None => tile,
        };
        carry(&mut lanes.of[t], forward, spacing, room.max(0));
    }

    // ---- 3. hand over -----------------------------------------------------------------------
    for i in 0..lanes.order.len() {
        let t = lanes.order[i] as usize;
        let Some(&Building { what: What::Belt, dir, .. }) = grid.at(t) else { continue };
        let next = grid.step_from(t, dir);
        while lanes.of[t].front().is_some_and(|f| f.along >= tile) {
            let front = lanes.of[t][0];
            let handed = match next.and_then(|n| grid.at(n).map(|b| (n, b.what))) {
                // onto the next belt, if it is not the one this belt is being fed by and the gap
                // is still there now that it is this tile's turn
                Some((n, What::Belt)) if grid.at(n).is_some_and(|b| b.dir != dir.back()) => {
                    let arriving = OnBelt { along: front.along - tile, item: front.item };
                    let room =
                        lanes.of[n].back().is_none_or(|tail| tail.along - arriving.along >= spacing);
                    if room {
                        lanes.of[t].pop_front();
                        lanes.of[n].push_back(arriving);
                        moves.0.push(Move::Carried { from: t, to: n });
                        true
                    } else {
                        false
                    }
                }
                // into a chest that has room. A machine is in the list because it is what the
                // belt runs into, and `machines::hand_to` is what refuses it: nothing goes into
                // a machine but through an inserter's hand, so a belt running into one jams
                // (F3, and the table at the head of `crate::machines`).
                Some((n, What::Chest | What::Machine(_) | What::Covered { .. })) => {
                    let took =
                        machines::hand_to(grid, lanes, rules, data, n, front.item, machines::Offer::Direct);
                    if took {
                        lanes.of[t].pop_front();
                        moves.0.push(Move::Taken { from: t });
                    }
                    took
                }
                _ => false,
            };
            if !handed {
                // nowhere to go: the item waits at the very end of the tile and everything
                // behind it closes up to a gap's distance
                hold(&mut lanes.of[t], spacing, tile);
                break;
            }
        }
    }

    // ---- 4, 5, 6: the miners, the arms and the machines ---------------------------------------
    let order = core::mem::take(&mut lanes.order);
    machines::dig(grid, ore, lanes, rules, data, &order, seconds, &mut moves);
    machines::swing(grid, lanes, rules, data, &order, seconds, &mut moves);
    machines::craft(grid, data, &order, seconds, &mut moves);
    lanes.order = order;

    moves
}

/// The tile a belt may push items into, or `None` when the end of its tile is as far as they go.
fn room_ahead(grid: &Grid, tile: usize, dir: Dir) -> Option<usize> {
    let next = grid.step_from(tile, dir)?;
    match grid.at(next) {
        // **two belts facing each other jam** rather than passing the same item back and forth
        Some(Building { what: What::Belt, dir: theirs, .. }) if *theirs != dir.back() => Some(next),
        _ => None,
    }
}

/// One lane carried forward: each item moves by `forward`, the front one no further than
/// `head_max` and none closer than `spacing` to the one in front of it.
///
/// It is the one piece of arithmetic the conveyors are, which is why it is a plain function over a
/// lane and not a system.
pub fn carry(lane: &mut VecDeque<OnBelt>, forward: Steps, spacing: Steps, head_max: Steps) {
    // **The chain is worked out in 64 bits**, which is what makes it exact rather than careful:
    // a data file may ask for any speed it likes, so a frame's step is as large as it likes, and
    // a queue longer than its tile runs off the back of it by a gap at a time. Neither sum can
    // leave the range of an `i64`, and what lands back in the item is `min`ed against a limit of
    // at most two tiles, so the narrowing is exact. Saturating the 32-bit arithmetic instead says
    // the same thing and costs a test and a move per item (`worklog/…-F2a.md` §7).
    let mut limit = head_max as i64;
    for p in lane.iter_mut() {
        let at = (p.along as i64 + forward as i64).min(limit);
        p.along = at as Steps;
        limit = at - spacing as i64;
    }
}

/// The lane brought back behind a front item that could not go on.
fn hold(lane: &mut VecDeque<OnBelt>, spacing: Steps, head: Steps) {
    let mut limit = head as i64;
    for p in lane.iter_mut() {
        let at = (p.along as i64).min(limit);
        p.along = at as Steps;
        limit = at - spacing as i64;
    }
}

// ---------------------------------------------------------------------------------------------


#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Building;
    use crate::machines::Stock;

    /// **The data file the tests run on.** Not the game's own: a test that would have to be read
    /// again every time a default moves is not a test of the rule. It goes through the same door
    /// the game's file does, because a `Data` built by hand here would be a second way of making
    /// one that nothing else uses.
    const DATA: &str = concat!(
        "item :ore, icon: 0\n",
        "item :plate, icon: 1\n",
        "item :gear, icon: 2\n",
        "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",
        "machine :works, size: [2, 2], sprite: [1, 2, 3, 4], speed: 2.0\n",
        "recipe :plate, in: { ore: 1 }, out: { plate: 1 }, time: 1.0, made_in: :furnace\n",
        "recipe :gear, in: { plate: 2 }, out: { gear: 1 }, time: 1.0, made_in: :works\n",
        "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n",
        "miner :drill, seconds_per_item: 1.0, digs: :ore\n",
        "chest :crate, capacity: 4\n",
        "ore :patch, per_tile: 10\n",
        "inserter :arm, seconds_per_item: 1.0\n",
    );

    /// The three item numbers the file above declares, in the order it declares them.
    const ORE: ItemId = 0;
    const PLATE: ItemId = 1;
    const GEAR: ItemId = 2;

    fn world() -> (Rules, Data) {
        let (data, rules) = crate::data::for_a_test(DATA);
        (rules, data)
    }

    /// An item of the kind a miner brings up, somewhere along a tile.
    fn a_rock(along: Steps) -> OnBelt {
        OnBelt { along, item: ORE }
    }

    /// One tile, in steps — what the whole model measures in.
    const TILE: Steps = TILE_PX as Steps;

    /// A map with a line of belts along row 1, running east from (1, 1).
    fn line(length: u32) -> (Grid, Ore, Lanes) {
        let tiles = length + 4;
        let mut grid = Grid::new(tiles);
        for x in 1..=length {
            let at = grid.index(UVec2::new(x, 1));
            grid.place(at, Building::new(What::Belt, Dir::East));
        }
        (grid, Ore { left: vec![0; (tiles * tiles) as usize], changed: false }, Lanes::for_map(tiles))
    }

    /// One sixtieth of a second, which is the frame the game steps by.
    const FRAME: f32 = 1.0 / 60.0;

    /// **A mind that never misses**, in Rust: every arm that is not already swinging is told to
    /// move. It is `ruby/inserter.rb`'s loop with the reading and the deciding taken out.
    ///
    /// It is here because the two halves of an inserter are tested in two places. What is tested
    /// here is the **arm** — how long a swing takes, what it picks up, what happens when the tile
    /// in front will not take it — and that is a fact about the factory, which is what this file
    /// is. What the *script* does is tested by the run's own checks, where there is a VM.
    fn drive_the_arms(grid: &mut Grid, lanes: &mut Lanes) {
        let arms: Vec<u32> = grid
            .built()
            .iter()
            .copied()
            .filter(|&t| grid.at(t as usize).map(|b| b.what) == Some(What::Inserter))
            .collect();
        for t in arms {
            let t = t as usize;
            if grid.at(t).is_some_and(|b| !b.swinging) {
                crate::machines::start_swing(grid, lanes, t);
            }
        }
    }

    fn run(
        grid: &mut Grid,
        ore: &mut Ore,
        lanes: &mut Lanes,
        rules: &Rules,
        data: &Data,
        frames: u32,
    ) -> Moves {
        let mut all = Moves::default();
        for _ in 0..frames {
            drive_the_arms(grid, lanes);
            all.0.extend(step(grid, ore, lanes, rules, data, FRAME).0);
        }
        all
    }

    /// What is in a chest, so that a test can say "one plate" rather than "one thing".
    fn inside(grid: &Grid, tile: usize) -> Stock {
        grid.at(tile).map(|b| b.held.clone()).unwrap_or_default()
    }

    /// **It carries.** An item put on the first tile of a line arrives at the last one, and it
    /// takes the time the speed says it should.
    #[test]
    fn a_belt_carries_an_item_at_the_speed_it_is_given() {
        let (rules, data) = world();
        let (mut grid, mut ore, mut lanes) = line(4);
        let first = grid.index(UVec2::new(1, 1));
        let last = grid.index(UVec2::new(4, 1));
        lanes.of[first].push_back(a_rock(0));

        // four tiles at two tiles a second is two seconds, less the tile it starts on
        let frames = (3.0 / rules.belt_tiles_per_second / FRAME).ceil() as u32;
        let moves = run(&mut grid, &mut ore, &mut lanes, &rules, &data, frames);
        assert_eq!(lanes.count(), 1, "the item is still on the belt and there is only one");
        assert_eq!(lanes.of[last].len(), 1, "and it is on the last tile: {:?}", lanes.of);
        assert_eq!(lanes.of[last][0].item, ORE, "and it is still the same item");
        assert_eq!(moves.carried(), 3, "it crossed three joins to get there");
        // and it is at the far end of it, where it stops: nothing to hand it to
        run(&mut grid, &mut ore, &mut lanes, &rules, &data, 60);
        assert_eq!(lanes.of[last][0].along, TILE, "{:?}", lanes.of[last]);
    }

    /// **The same seconds carry an item the same distance, whatever the frames are.**
    ///
    /// This is the whole of why a frame's leftover fraction of a step is kept
    /// ([`Lanes::part_of_a_step`]): at the speed this is played at a sixtieth of a second is
    /// **half a step**, so a factory that dropped the fraction would not move at all, and one
    /// that rounded each frame up would run at a speed its own frame rate chose. A browser drawing
    /// five frames a second and a PC drawing sixty are the two ends measured here.
    ///
    /// **Within one step**, and not nearer: the position is a whole number of steps, so where a
    /// frame boundary falls can leave the two a step apart and there is no finer answer to ask
    /// for. One step is one pixel of the art.
    #[test]
    fn the_same_seconds_carry_an_item_the_same_way_at_any_frame_rate() {
        let (rules, data) = world();
        let seconds = 2.0f32;
        let mut went = Vec::new();
        for frames_a_second in [60.0f32, 5.0] {
            let (mut grid, mut ore, mut lanes) = line(8);
            let first = grid.index(UVec2::new(1, 1));
            lanes.of[first].push_back(a_rock(0));
            let frame = 1.0 / frames_a_second;
            for _ in 0..(seconds * frames_a_second) as u32 {
                step(&mut grid, &mut ore, &mut lanes, &rules, &data, frame);
            }
            // where it got to, counted from the start of the line rather than per tile
            let (at, on) = lanes
                .of
                .iter()
                .enumerate()
                .find_map(|(t, lane)| lane.front().map(|item| (t, item.along)))
                .expect("it is somewhere");
            went.push((at - first) as Steps * TILE + on);
        }
        let expected = (rules.belt_tiles_per_second * seconds) as Steps * TILE;
        assert_eq!(expected, 64, "two seconds at two tiles a second is four tiles of sixteen");
        for (rate, &got) in [60, 5].iter().zip(went.iter()) {
            assert!(
                (got - expected).abs() <= 1,
                "{rate} frames a second carried it {got} steps and the speed says {expected}"
            );
        }
        assert!(
            (went[0] - went[1]).abs() <= 1,
            "sixty frames a second carried it {} steps and five carried it {}",
            went[0],
            went[1]
        );
    }

    /// **It jams.** A belt running into nothing fills up from the end, one item every gap, and
    /// then the tile behind it fills too — and no item is ever lost or overlapped.
    ///
    /// **A jammed tile holds one more item than `items_per_tile`**, and that is not an off-by-one:
    /// the item at `0` is at the tile's entry edge, which is the same point in the world as
    /// `TILE_PX` on the tile before it. `items_per_tile` is how many *gaps* fit in a tile's
    /// length, and a queue of N gaps has N + 1 things in it. Since F2a it is **every** gap a belt
    /// can have and not only the ones a float held exactly
    /// ([`a_jam_holds_one_more_than_the_gaps_that_fit`]).
    ///
    /// [`a_jam_holds_one_more_than_the_gaps_that_fit`]:
    ///     self::a_jam_holds_one_more_than_the_gaps_that_fit
    #[test]
    fn items_pile_up_behind_a_belt_that_runs_into_nothing() {
        let (rules, data) = world();
        let (mut grid, mut o, mut lanes) = line(2);
        let first = grid.index(UVec2::new(1, 1));
        let last = grid.index(UVec2::new(2, 1));
        // six items, which is more than the two tiles hold: the ones at negative positions are
        // where something feeding this belt would be pushing them in from
        for i in 0..6 {
            lanes.of[first].push_back(a_rock(-i * rules.spacing()));
        }
        run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);

        assert_eq!(lanes.count(), 6, "nothing was lost");
        assert_eq!(lanes.of[last].len(), 3, "the far tile: 16, 8, 0 — {:?}", lanes.of[last]);
        assert_eq!(lanes.of[first].len(), 3, "and the rest are waiting behind: {:?}", lanes.of);
        // the front one is at the end of the tile and the gap is kept everywhere
        assert_eq!(lanes.of[last][0].along, TILE);
        for lane in [&lanes.of[first], &lanes.of[last]] {
            for pair in lane.iter().collect::<Vec<_>>().windows(2) {
                assert!(pair[0].along - pair[1].along >= rules.spacing(), "{lane:?}");
            }
        }
        // and the two tiles' items keep the gap across the join as well
        assert!(TILE + lanes.of[last][2].along - lanes.of[first][0].along >= rules.spacing());
        // nothing has moved for a while: this is a jam and not a slow queue
        let before: Vec<Steps> = lanes.of[first].iter().map(|i| i.along).collect();
        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 60);
        assert_eq!(moves.carried(), 0, "a jam does not hand anything on");
        assert_eq!(lanes.of[first].iter().map(|i| i.along).collect::<Vec<Steps>>(), before);
    }

    /// **Two belts merging into one both get through**, and neither starves the other: the
    /// items end up alternating, because each hands over only when the gap the other left is
    /// wide enough.
    #[test]
    fn two_belts_merge_into_one_and_keep_the_gap() {
        let (rules, data) = world();
        let tiles = 8;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        // one belt running east into (4, 4), one running north into it, and one carrying on east
        let from_west = grid.index(UVec2::new(3, 4));
        let from_south = grid.index(UVec2::new(4, 3));
        let join = grid.index(UVec2::new(4, 4));
        let away = grid.index(UVec2::new(5, 4));
        grid.place(from_west, Building::new(What::Belt, Dir::East));
        grid.place(from_south, Building::new(What::Belt, Dir::North));
        grid.place(join, Building::new(What::Belt, Dir::East));
        grid.place(away, Building::new(What::Belt, Dir::East));
        // three items waiting on each feeder, nose to tail
        for i in 0..3 {
            lanes.of[from_west].push_back(a_rock(-i * rules.spacing()));
            lanes.of[from_south].push_back(a_rock(-i * rules.spacing()));
        }

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
        assert_eq!(lanes.count(), 6, "all six are still there: {:?}", lanes.of);
        // **which side got through** is the whole question, and the moves say it where the
        // positions cannot: an item carries no note of where it came from
        let through = |source: usize| {
            moves.0.iter().filter(|m| matches!(m, Move::Carried { from, to } if *from == source && *to == join)).count()
        };
        assert!(through(from_west) >= 2, "the belt from the west got {} through", through(from_west));
        assert!(through(from_south) >= 2, "the belt from the south got {} through", through(from_south));
        for lane in [&lanes.of[join], &lanes.of[away]] {
            for pair in lane.iter().collect::<Vec<_>>().windows(2) {
                assert!(pair[0].along - pair[1].along >= rules.spacing(), "{lane:?}");
            }
        }
        // and the gap is kept across the join the two of them merge into
        let tail = lanes.of[away][lanes.of[away].len() - 1].along;
        assert!(TILE + tail - lanes.of[join][0].along >= rules.spacing());
    }

    /// Two belts pointing at each other are a jam and not a game of catch.
    #[test]
    fn belts_facing_each_other_do_not_pass_an_item_back_and_forth() {
        let (rules, data) = world();
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let left = grid.index(UVec2::new(2, 2));
        let right = grid.index(UVec2::new(3, 2));
        grid.place(left, Building::new(What::Belt, Dir::East));
        grid.place(right, Building::new(What::Belt, Dir::West));
        lanes.of[left].push_back(a_rock(0));

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 120);
        assert_eq!(moves.carried(), 0, "it never crossed");
        assert_eq!(lanes.of[left].len(), 1);
        assert_eq!(lanes.of[left][0].along, TILE, "it waits at the end of its own tile");
    }

    /// **The line end to end**: ore in the ground, a miner on it, a belt, a chest. The chest
    /// fills, the ore runs down by exactly as much, and when the chest is full the belt jams
    /// rather than losing anything.
    #[test]
    fn a_miner_digs_onto_a_belt_and_the_chest_at_the_end_fills_up() {
        let (rules, data) = world();
        let tiles = 8;
        let mut grid = Grid::new(tiles);
        let mut lanes = Lanes::for_map(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let pit = grid.index(UVec2::new(1, 1));
        let belt = grid.index(UVec2::new(2, 1));
        let chest = grid.index(UVec2::new(3, 1));
        o.left[pit] = 10;
        grid.place(pit, Building::new(What::Miner, Dir::East));
        grid.place(belt, Building::new(What::Belt, Dir::East));
        grid.place(chest, Building::new(What::Chest, Dir::East));

        // long enough to dig more than the chest holds
        run(&mut grid, &mut o, &mut lanes, &rules, &data, 60 * 8);
        assert_eq!(inside(&grid, chest).count(), rules.chest_capacity, "the chest filled");
        assert_eq!(inside(&grid, chest).of(ORE), rules.chest_capacity, "with what the miner digs");
        let dug = 10 - o.left[pit];
        assert_eq!(
            dug as usize,
            rules.chest_capacity as usize + lanes.count(),
            "everything dug is either in the chest or still on the belt"
        );
        assert!(lanes.count() > 0, "and the belt jammed rather than the items going nowhere");
        // the miner is holding a finished dig rather than banking them up
        assert!((grid.at(pit).unwrap().work - rules.mine_seconds).abs() < 1e-5);
    }

    /// A miner facing a wall keeps its finished dig and takes nothing out of the ground.
    #[test]
    fn a_miner_with_nowhere_to_put_it_takes_nothing_out_of_the_ground() {
        let (rules, data) = world();
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut lanes = Lanes::for_map(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let pit = grid.index(UVec2::new(2, 2));
        o.left[pit] = 5;
        grid.place(pit, Building::new(What::Miner, Dir::East));

        run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert_eq!(o.left[pit], 5, "the ore is still in the ground");
        assert_eq!(lanes.count(), 0);
        assert!((grid.at(pit).unwrap().work - rules.mine_seconds).abs() < 1e-5);

        // and the moment there is somewhere to put it, it goes: the wait was not lost
        let chest = grid.index(UVec2::new(3, 2));
        grid.place(chest, Building::new(What::Chest, Dir::East));
        let moves = step(&mut grid, &mut o, &mut lanes, &rules, &data, FRAME);
        assert_eq!(inside(&grid, chest).of(ORE), 1);
        assert_eq!(o.left[pit], 4);
        // **a dig straight into a chest is still a dig**: it is counted, and it is not counted
        // twice by also being something a belt took
        assert_eq!(moves.made(), 1, "{:?}", moves.0);
        assert_eq!(moves.taken(), 0, "it was never on a belt: {:?}", moves.0);
    }

    /// **Every gap a belt can have**, and a jammed tile holds one more than the gaps that fit —
    /// exactly, at all five of them.
    ///
    /// This is F1's seven-value test taken again after F2a. F1 ran it at 1, 2, 3, 4, 5, 7 and 8
    /// and could only assert *the invariant*: nothing lost, nothing closer than a gap, and a count
    /// that was `items_per_tile` or one more. At 3 it was three and not four, because three gaps
    /// of `f32(1/3)` come to a hair over a tile — and at 5 it *was* six, because the chain of
    /// subtractions rounded the other way. Which is to say the count was not a fact about the
    /// factory at all (`docs/worklog/2026-09-21-factory-F1.md` §5.2).
    ///
    /// With the position a whole number of steps there is no chain and no rounding: a gap is
    /// `TILE_PX ÷ items_per_tile` steps and `items_per_tile` of them are a tile, so the count is
    /// `N + 1` and the positions are the exact multiples. The values run are the ones a data file
    /// can now hold — the divisors of `TILE_PX` — because the rest are refused at their line
    /// (`crate::data::fits_a_tile`), which is the other half of this test and lives in `data.rs`.
    #[test]
    fn a_jam_holds_one_more_than_the_gaps_that_fit() {
        let (base, data) = world();
        for items_per_tile in [1u32, 2, 4, 8, 16] {
            let rules = Rules { items_per_tile, ..base.clone() };
            let gap = rules.spacing();
            let (mut grid, mut o, mut lanes) = line(2);
            let first = grid.index(UVec2::new(1, 1));
            let last = grid.index(UVec2::new(2, 1));
            // three tiles' worth, so that the far tile fills, the near one fills behind it, and
            // there are still some waiting off the back of the line
            let all = (items_per_tile as usize + 1) * 3;
            for i in 0..all as Steps {
                lanes.of[first].push_back(a_rock(-i * gap));
            }
            run(&mut grid, &mut o, &mut lanes, &rules, &data, 1200);

            let held = lanes.of[last].len();
            assert_eq!(lanes.count(), all, "nothing was lost ({items_per_tile})");
            assert_eq!(
                held,
                items_per_tile as usize + 1,
                "{items_per_tile}: a queue of {items_per_tile} gaps holds one more — {:?}",
                lanes.of[last]
            );
            // and every one of them is where the arithmetic says, to the step
            let places: Vec<Steps> = lanes.of[last].iter().map(|i| i.along).collect();
            let wanted: Vec<Steps> = (0..held as Steps).map(|k| TILE - k * gap).collect();
            assert_eq!(places, wanted, "{items_per_tile}: the front at the end, a gap apart");
            for pair in lanes.of[first].iter().collect::<Vec<_>>().windows(2) {
                assert!(pair[0].along - pair[1].along >= gap, "{items_per_tile}: {:?}", lanes.of[first]);
            }
            assert!(
                TILE + lanes.of[last][held - 1].along - lanes.of[first][0].along >= gap,
                "the gap is kept across the join too ({items_per_tile})"
            );
        }
    }

    /// The gap is a number a player can move, and moving it moves how much a tile holds.
    #[test]
    fn how_many_fit_on_a_tile_is_the_number_that_says_so() {
        let (base, data) = world();
        for items_per_tile in [1u32, 2, 4, 8] {
            let rules = Rules { items_per_tile, ..base.clone() };
            let (mut grid, mut o, mut lanes) = line(1);
            let only = grid.index(UVec2::new(1, 1));
            for i in 0..16 as Steps {
                lanes.of[only].push_back(a_rock(-i * rules.spacing()));
            }
            run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
            let on_the_tile = lanes.of[only].iter().filter(|i| i.along > 0).count();
            assert_eq!(on_the_tile, items_per_tile as usize, "{items_per_tile}: {:?}", lanes.of[only]);
        }
    }

    // ------------------------------------------------------------------------------------------
    // F2: the machines
    // ------------------------------------------------------------------------------------------

    /// **A furnace turns ore into a plate, in the time the recipe and the two arms say.**
    ///
    /// The belt carries the ore up to an inserter, which lifts it into the furnace; the furnace
    /// works for `time / speed`; a second inserter lifts the plate out onto the belt, which
    /// carries it to the chest. **Both arms are new at F3** — before it, the belt fed the machine
    /// and the machine pushed its plate out, and the line needed no script at all.
    ///
    /// The time is checked as a number of frames rather than as a wall clock: the whole factory
    /// is a function of `seconds`.
    #[test]
    fn a_furnace_makes_what_the_recipe_says_in_the_time_it_says() {
        let (rules, data) = world();
        let tiles = 10;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let arm_in = grid.index(UVec2::new(2, 1));
        let furnace = grid.index(UVec2::new(3, 1));
        let arm_out = grid.index(UVec2::new(4, 1));
        let away = grid.index(UVec2::new(5, 1));
        let chest = grid.index(UVec2::new(6, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        grid.place(arm_in, Building::new(What::Inserter, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(What::Machine(kind), Dir::East));
        grid.place(arm_out, Building::new(What::Inserter, Dir::East));
        grid.place(away, Building::new(What::Belt, Dir::East));
        grid.place(chest, Building::new(What::Chest, Dir::East));
        lanes.of[feed].push_back(a_rock(0));

        // a swing in, a craft, a swing out, and one tile of belt into the chest
        let recipe = &data.recipes[0];
        let seconds = rules.swing_seconds
            + recipe.time / data.machines[kind as usize].speed
            + rules.swing_seconds
            + 1.0 / rules.belt_tiles_per_second;
        let moves = run(
            &mut grid,
            &mut o,
            &mut lanes,
            &rules,
            &data,
            (seconds / FRAME).ceil() as u32 + 8,
        );
        assert_eq!(moves.crafted(), 1, "one craft, and not two: {:?}", moves.0);
        assert_eq!(inside(&grid, chest).of(PLATE), 1, "a plate came out of it");
        assert_eq!(inside(&grid, chest).of(ORE), 0, "and the ore went in, not through");
        assert_eq!(lanes.count(), 0, "nothing is left on the belts");
    }

    /// **Without an inserter the line does not join up**, which is the whole of what F3 changed:
    /// a belt running into a machine jams, and the machine never sees the ore.
    ///
    /// It is the same little factory as above with the two arms left out, run for long enough
    /// that a factory which was going to work would have.
    #[test]
    fn a_belt_running_into_a_machine_jams_because_nothing_goes_in_but_through_an_arm() {
        let (rules, data) = world();
        let tiles = 8;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let furnace = grid.index(UVec2::new(2, 1));
        let away = grid.index(UVec2::new(3, 1));
        let chest = grid.index(UVec2::new(4, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(What::Machine(kind), Dir::East));
        grid.place(away, Building::new(What::Belt, Dir::East));
        grid.place(chest, Building::new(What::Chest, Dir::East));
        lanes.of[feed].push_back(a_rock(0));

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
        assert_eq!(moves.crafted(), 0, "nothing was made: {:?}", moves.0);
        assert!(grid.at(furnace).unwrap().held.is_empty(), "and nothing went in");
        assert_eq!(lanes.of[feed].len(), 1, "the ore is still on the belt");
        assert_eq!(lanes.of[feed][0].along, TILE, "waiting at the end of its tile");
        assert_eq!(inside(&grid, chest).count(), 0);
    }

    /// **One arm, one item a swing**, and the item is in the hand for the whole of it.
    ///
    /// This is the arm on its own: a belt behind, a chest in front, and nothing to decide. What
    /// is measured is the three things a swing is — it takes a swing's worth of seconds, the item
    /// is out of the belt and in the hand while it crosses, and it is in the chest at the end.
    #[test]
    fn an_arm_carries_one_thing_a_swing_and_holds_it_on_the_way() {
        let (rules, data) = world();
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let arm = grid.index(UVec2::new(2, 1));
        let chest = grid.index(UVec2::new(3, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        grid.place(arm, Building::new(What::Inserter, Dir::East));
        grid.place(chest, Building::new(What::Chest, Dir::East));
        for i in 0..2 {
            lanes.of[feed].push_back(a_rock(-i * rules.spacing()));
        }

        // one frame is enough to start the swing and not to finish it
        drive_the_arms(&mut grid, &mut lanes);
        step(&mut grid, &mut o, &mut lanes, &rules, &data, FRAME);
        assert_eq!(grid.at(arm).unwrap().held.of(ORE), 1, "the ore is in the hand");
        assert!(grid.at(arm).unwrap().swinging, "and the arm is on its way");
        assert_eq!(lanes.of[feed].len(), 1, "and it is off the belt: it is not in two places");
        assert_eq!(inside(&grid, chest).count(), 0, "and not in the chest yet");

        // the rest of the swing, and a frame's grace either side of it
        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, (rules.swing_seconds / FRAME).ceil() as u32);
        assert_eq!(inside(&grid, chest).of(ORE), 1, "one thing arrived, in one swing");
        assert_eq!(moves.swung().filter(|&(_, placed)| placed).count(), 1, "{:?}", moves.0);
        // and the second one follows a swing behind it
        run(&mut grid, &mut o, &mut lanes, &rules, &data, (rules.swing_seconds / FRAME).ceil() as u32 + 2);
        assert_eq!(inside(&grid, chest).of(ORE), 2);
        assert_eq!(lanes.count(), 0, "the belt is empty");
    }

    /// **An arm that carried something over and found nowhere to put it keeps it**, and puts it
    /// down the moment there is room — the miner's "blocked is not lost", kept by the hand.
    #[test]
    fn an_arm_refused_keeps_what_it_picked_up() {
        let (base, data) = world();
        // a chest that is full before the arm ever reaches it
        let rules = Rules { chest_capacity: 1, ..base };
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let arm = grid.index(UVec2::new(2, 1));
        let chest = grid.index(UVec2::new(3, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        grid.place(arm, Building::new(What::Inserter, Dir::East));
        grid.place(chest, Building::new(What::Chest, Dir::East));
        if let Some(full) = grid.at_mut(chest) {
            full.held.add(PLATE, 1);
        }
        lanes.of[feed].push_back(a_rock(0));

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert!(moves.swung().any(|(at, placed)| at == arm && !placed), "a swing was refused");
        assert_eq!(grid.at(arm).unwrap().held.of(ORE), 1, "and the hand still has it");
        assert_eq!(lanes.count(), 0, "it is not on the belt either: nothing was made or lost");
        assert_eq!(inside(&grid, chest).of(ORE), 0);

        // room, and it goes
        if let Some(emptied) = grid.at_mut(chest) {
            emptied.held.take(PLATE, 1);
        }
        run(&mut grid, &mut o, &mut lanes, &rules, &data, (rules.swing_seconds / FRAME).ceil() as u32 + 2);
        assert_eq!(inside(&grid, chest).of(ORE), 1, "the wait was not lost");
        assert!(grid.at(arm).unwrap().held.is_empty());
    }

    /// **A furnace refuses what no recipe of its kind wants**, and the belt jams rather than the
    /// item disappearing into it.
    #[test]
    fn a_machine_will_not_take_what_it_has_no_recipe_for() {
        let (rules, data) = world();
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let furnace = grid.index(UVec2::new(2, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(What::Machine(kind), Dir::East));
        lanes.of[feed].push_back(OnBelt { along: 0, item: GEAR });

        run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert_eq!(lanes.of[feed].len(), 1, "the gear is still on the belt");
        assert_eq!(lanes.of[feed][0].along, TILE, "waiting at the end of its tile");
        assert!(grid.at(furnace).unwrap().held.is_empty(), "and the furnace took nothing");
    }

    /// **A machine of four tiles is reached through any of them, either way round.**
    ///
    /// This is the whole of what covering several tiles means: an arm reaching into the far
    /// corner of a 2 by 2 machine is reaching into the machine, and so is one reaching *out* of
    /// another corner. Nothing about which way the machine faces comes into it — since F3 a
    /// machine's direction says nothing at all (`crate::data::Data::footprint`).
    #[test]
    fn a_machine_of_four_tiles_is_fed_through_any_of_them() {
        let (rules, data) = world();
        let tiles = 10;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let kind = data.machine("works").expect("declared");
        let origin = UVec2::new(3, 3);
        let at = grid.index(origin);
        grid.place(at, Building::new(What::Machine(kind), Dir::East));
        for tile in data.footprint(kind, origin).into_iter().skip(1) {
            let covered = grid.index(tile);
            grid.place(covered, Building::new(What::Covered { origin: at as u32 }, Dir::East));
        }
        // a belt running east into an arm that reaches into the machine's *upper* row, which is
        // not the origin's row — and an arm on the other side reaching out of a covered tile
        let feed = grid.index(UVec2::new(1, 4));
        let arm_in = grid.index(UVec2::new(2, 4));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        grid.place(arm_in, Building::new(What::Inserter, Dir::East));
        let arm_out = grid.index(UVec2::new(5, 4));
        let out = grid.index(UVec2::new(6, 4));
        grid.place(arm_out, Building::new(What::Inserter, Dir::East));
        grid.place(out, Building::new(What::Chest, Dir::East));
        for i in 0..2 {
            lanes.of[feed].push_back(OnBelt { along: -i * rules.spacing(), item: PLATE });
        }

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
        assert_eq!(moves.crafted(), 1, "two plates make one gear: {:?}", moves.0);
        assert_eq!(inside(&grid, out).of(GEAR), 1, "and an arm took it out of a covered tile");
        assert_eq!(lanes.count(), 0, "both plates went in through the covered tile");
    }

    /// **A machine with nowhere to put what it made stops**, and starts again the moment there
    /// is somewhere — which is the miner's rule, kept by the machines as well.
    #[test]
    fn a_machine_holding_what_it_made_does_not_start_another_craft() {
        let (rules, data) = world();
        let tiles = 6;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let arm_in = grid.index(UVec2::new(2, 1));
        let furnace = grid.index(UVec2::new(3, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        grid.place(arm_in, Building::new(What::Inserter, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(What::Machine(kind), Dir::East));
        for i in 0..3 {
            lanes.of[feed].push_back(a_rock(-i * rules.spacing()));
        }

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
        assert_eq!(moves.crafted(), 1, "one plate made, and it is still in there: {:?}", moves.0);
        let held = grid.at(furnace).unwrap();
        assert_eq!(held.made.of(PLATE), 1, "what it made is waiting");
        assert_eq!(held.held.of(ORE), 1, "and one craft's worth of ore is waiting behind it");
        // the third ore is either still on the belt or in the arm's hand, which has nowhere to
        // put it: the furnace is holding one craft's worth already
        assert_eq!(
            lanes.of[feed].len() + grid.at(arm_in).unwrap().held.count() as usize,
            1,
            "the third is waiting: {:?}",
            lanes.of[feed]
        );

        // an arm out of it and somewhere to put what it carries, and it goes — and then the next
        // craft runs
        let arm_out = grid.index(UVec2::new(4, 1));
        let out = grid.index(UVec2::new(5, 1));
        grid.place(arm_out, Building::new(What::Inserter, Dir::East));
        grid.place(out, Building::new(What::Chest, Dir::East));
        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 900);
        assert_eq!(
            inside(&grid, out).of(PLATE),
            3,
            "the one it was holding, the one it had the ore for, and the one that was waiting"
        );
        assert_eq!(moves.crafted(), 2, "two more crafts, not three: {:?}", moves.0);
        assert_eq!(lanes.count(), 0, "and the belt emptied");
    }

    // ------------------------------------------------------------------------------------------
    // F2a: nothing is made and nothing is lost, over minutes
    // ------------------------------------------------------------------------------------------

    /// **What an item is worth in ore**, worked out from the recipes: what comes out of a craft is
    /// worth what went into it, and something no recipe makes is worth itself.
    ///
    /// It is what lets a whole factory be added up in one unit — two plates are in a gear, so a
    /// gear in the chest is two ore out of the ground — and it is worked out rather than written
    /// down so that the sum follows the data file rather than this test's memory of it.
    fn in_ore(data: &Data, item: ItemId) -> u32 {
        let Some(recipe) = data.recipes.iter().find(|r| r.outputs.iter().any(|&(i, _)| i == item))
        else {
            return 1;
        };
        let out = recipe.outputs.iter().find(|&&(i, _)| i == item).map(|&(_, n)| n).unwrap_or(1);
        recipe.inputs.iter().map(|&(i, n)| in_ore(data, i) * n).sum::<u32>() / out
    }

    /// Everything that is anywhere but in the ground, in ore: on the belts, in the chests, and
    /// inside the machines — **including what a machine is part way through**, which it took out
    /// of its own store when it started and has not made into anything yet.
    fn everywhere_else(grid: &Grid, lanes: &Lanes, data: &Data) -> u32 {
        let mut total = 0;
        for lane in lanes.of.iter() {
            total += lane.iter().map(|i| in_ore(data, i.item)).sum::<u32>();
        }
        for &t in grid.built() {
            let Some(building) = grid.at(t as usize) else { continue };
            for stock in [&building.held, &building.made] {
                total += stock.0.iter().map(|&(i, n)| in_ore(data, i) * n).sum::<u32>();
            }
            if let Some(r) = building.making
                && let Some(recipe) = data.recipes.get(r as usize)
            {
                total += recipe.inputs.iter().map(|&(i, n)| in_ore(data, i) * n).sum::<u32>();
            }
        }
        total
    }

    /// **Minutes of a whole factory, and not one item made or lost in any of them.**
    ///
    /// Two miners on two patches, two belts merging into one, **four inserters**, a furnace that
    /// can only take half of what they bring up — so the belts behind it are jammed for the whole
    /// run — an assembler two tiles by two, and a chest. Every kind of thing one step of the
    /// factory does is in it at once: carrying, merging, jamming, digging into a jam, lifting in,
    /// crafting, lifting out, and **an arm holding something it has nowhere to put**, which is
    /// the state F3 added and the one an equality is the only honest test of.
    ///
    /// **What is asserted is an equality and not a tolerance.** Before F2a the positions were
    /// floats and the count of items was still whole, so this was already exact; what is new is
    /// that it can be *asked* at every one of ten thousand frames rather than at the end, because
    /// nothing drifts. Five minutes of the game's own time at a sixtieth of a second is 18,000
    /// steps, which is about what a player leaves a factory running for while they build the next
    /// one.
    #[test]
    fn nothing_is_made_and_nothing_is_lost_over_minutes_of_a_whole_factory() {
        let (base, data) = world();
        // a chest nothing fills, so that the line keeps flowing for the whole run rather than
        // backing up into a stopped factory after the first few seconds
        let rules = Rules { chest_capacity: 10_000, ..base };
        let tiles = 16;
        let mut grid = Grid::new(tiles);
        let mut lanes = Lanes::for_map(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };

        let at = |x: u32, y: u32| (y * tiles + x) as usize;
        let put = |grid: &mut Grid, x: u32, y: u32, what: What, dir: Dir| {
            grid.place(at(x, y), Building::new(what, dir));
        };
        // two miners, each on a tile with plenty in it
        for y in [2, 4] {
            o.left[at(1, y)] = 400;
            put(&mut grid, 1, y, What::Miner, Dir::East);
            put(&mut grid, 2, y, What::Belt, Dir::East);
            put(&mut grid, 3, y, What::Belt, Dir::East);
        }
        // and the two lines turn towards each other into one
        put(&mut grid, 4, 2, What::Belt, Dir::North);
        put(&mut grid, 4, 4, What::Belt, Dir::South);
        put(&mut grid, 4, 3, What::Belt, Dir::East);
        put(&mut grid, 5, 3, What::Belt, Dir::East);
        // **four arms, because a machine has no other door** (F3)
        put(&mut grid, 6, 3, What::Inserter, Dir::East);
        let furnace = data.machine("furnace").expect("declared");
        put(&mut grid, 7, 3, What::Machine(furnace), Dir::East);
        put(&mut grid, 8, 3, What::Inserter, Dir::East);
        put(&mut grid, 9, 3, What::Belt, Dir::East);
        put(&mut grid, 10, 3, What::Inserter, Dir::East);
        // the assembler covers four tiles, and the arm at (13, 3) reaches into a covered one
        let works = data.machine("works").expect("declared");
        put(&mut grid, 11, 3, What::Machine(works), Dir::East);
        for (x, y) in [(12, 3), (11, 4), (12, 4)] {
            put(&mut grid, x, y, What::Covered { origin: at(11, 3) as u32 }, Dir::East);
        }
        put(&mut grid, 13, 3, What::Inserter, Dir::East);
        put(&mut grid, 14, 3, What::Chest, Dir::East);

        let ore_at_the_start = o.total();
        // five minutes of the game's own time
        for frame in 0..(60 * 5 * 60) {
            drive_the_arms(&mut grid, &mut lanes);
            step(&mut grid, &mut o, &mut lanes, &rules, &data, FRAME);
            // asked every second of it, so that a step that lost something says which one
            if frame % 60 == 0 {
                let dug = (ore_at_the_start - o.total()) as u32;
                assert_eq!(
                    dug,
                    everywhere_else(&grid, &lanes, &data),
                    "frame {frame}: what came out of the ground is not what the factory holds"
                );
            }
        }

        // and the run was a factory and not a stalled one
        let chest = inside(&grid, at(14, 3));
        assert!(chest.of(GEAR) > 50, "the chest filled with gears: {chest:?}");
        assert_eq!(chest.of(ORE), 0, "and nothing went through unsmelted");
        assert!(lanes.count() > 0, "the belts behind the furnace are jammed, as they should be");
        let dug = (ore_at_the_start - o.total()) as u32;
        assert_eq!(dug, everywhere_else(&grid, &lanes, &data), "and at the end of it too");
        assert!(dug > 200, "five minutes of two miners past an arm that lifts one a second");
    }

    /// A machine's speed divides the recipe's time, and the `works` in the test data runs at 2.
    #[test]
    fn a_machines_speed_divides_the_recipes_time() {
        let (rules, data) = world();
        let tiles = 8;
        let mut grid = Grid::new(tiles);
        let mut o = Ore { left: vec![0; (tiles * tiles) as usize], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let kind = data.machine("works").expect("declared");
        let origin = UVec2::new(2, 2);
        let at = grid.index(origin);
        grid.place(at, Building::new(What::Machine(kind), Dir::East));
        for tile in data.footprint(kind, origin).into_iter().skip(1) {
            let covered = grid.index(tile);
            grid.place(covered, Building::new(What::Covered { origin: at as u32 }, Dir::East));
        }
        if let Some(machine) = grid.at_mut(at) {
            machine.held.add(PLATE, 2);
        }
        let recipe = data.recipes.iter().find(|r| r.name == "gear").expect("declared");
        let wanted = recipe.time / data.machines[kind as usize].speed;
        assert_eq!(wanted, 0.5, "a second's recipe in a machine of speed two");

        let mut frames = 0;
        loop {
            let moves = step(&mut grid, &mut o, &mut lanes, &rules, &data, FRAME);
            frames += 1;
            if moves.crafted() > 0 {
                break;
            }
            assert!(frames < 600, "it never finished");
        }
        let took = frames as f32 * FRAME;
        assert!(
            (took - wanted).abs() <= FRAME,
            "it took {took} s and the recipe and the speed say {wanted} s"
        );
    }
}
