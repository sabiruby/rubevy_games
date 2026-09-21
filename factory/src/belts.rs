//! **The conveyors, and everything that moves on them.**
//!
//! One rule, in one place, with no Bevy in it beyond the resources it is kept in: the tests at the
//! bottom run the whole of it — carrying, jamming, merging, digging, delivering — with no `App`
//! at all.
//!
//! **Where an item is.** A belt tile is one unit long and an item on it is a number in `0..=1`:
//! how far through that tile it has come — and, since F2, **which item it is**
//! ([`OnBelt`], two fields and eight bytes). A lane is the items on one tile, **front first**, so
//! `lanes[t][0]` is the one nearest the end of the tile and every item is at least
//! [`Rules::spacing`] behind the one in front of it. That invariant is the whole model: carrying
//! is adding a step to each number, jamming is a number that cannot grow, and merging is two
//! tiles handing items to one and each finding the gap the other left.
//!
//! **Why the positions are `f32` in a `Vec` and not a component on an entity.** Because the rule
//! above needs *the item in front*, and an ECS query has no such thing. F1 built both ways round
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

/// **The numbers of play**, every one of them a line of `ruby/data.rb` — the belt's two, the
/// miner's one and what it brings up, and the chest's one. `docs/numbers.md` §9.3 says where each
/// default came from, and a player changes them by editing the file.
///
/// **Every one of them is more than zero**, and whoever fills this in is what makes that true:
/// the data stage refuses a declaration whose number is not, at the line it is written on
/// (`crate::data::more_than_zero`). There is no floor inside the arithmetic below, because a
/// floor is a number, and a number that is only there to stop a division would have nowhere it
/// came from.
///
/// **The two numbers that are not here** — how much ore a tile holds and how wide a patch is —
/// are [`crate::Map`]'s, because they are how the world is *laid out* before anybody plays, and
/// the map's own smallest size is derived from the second of them in `main`, before there is a
/// VM to have read any Ruby.
#[derive(Resource, Debug, Clone)]
pub struct Rules {
    /// How many tiles an item is carried in a second.
    pub belt_tiles_per_second: f32,
    /// How many items fit on one tile of belt. The gap between two items is one over this.
    pub items_per_tile: f32,
    /// How long a miner takes over one item, in seconds.
    pub mine_seconds: f32,
    /// How many items a chest holds, of all kinds together, before it stops taking them.
    pub chest_capacity: u32,
    /// What a miner brings up out of the ground.
    pub digs: ItemId,
}

impl Rules {
    /// The gap between two items on a belt, as a fraction of a tile.
    pub fn spacing(&self) -> f32 {
        1.0 / self.items_per_tile
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
        self.belt_tiles_per_second * self.items_per_tile
    }
}

/// **One item on a belt**: how far through its tile it has come, and which item it is.
///
/// F1 had one kind of thing in the world and an item was the number on its own. `data.rb`
/// declares as many kinds as it likes, so a furnace can refuse a plate and a chest can hold a
/// mixture, and the kind has to travel with the position — it is the only place an item exists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnBelt {
    /// `0..=1`: how far through the tile. `0.0` is the tile's entry edge, which is the same point
    /// in the world as `1.0` on the tile before it.
    pub along: f32,
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
    /// Scratch: where each lane's last item was at the *start* of the frame. Every belt asks its
    /// neighbour how much room there is, and asking one snapshot makes the answer the same
    /// whatever order the tiles happen to be walked in.
    tails: Vec<f32>,
    /// Scratch: the tiles with something on them, copied out of the grid at the start of the
    /// step. A copy because the passes below reach into the grid to fill a chest and to move a
    /// miner on, and a list borrowed from it cannot be walked while that happens.
    order: Vec<u32>,
}

impl Lanes {
    pub fn for_map(tiles: u32) -> Lanes {
        let n = (tiles * tiles) as usize;
        Lanes { of: vec![VecDeque::new(); n], tails: vec![f32::NAN; n], order: Vec::new() }
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
/// 5. **deliver**: a machine pushes what it has made into what it faces;
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
    let forward = (rules.belt_tiles_per_second * seconds).max(0.0);
    lanes.order.clear();
    lanes.order.extend_from_slice(grid.built());

    // ---- 1. the tails as they were ---------------------------------------------------------
    for tail in &mut lanes.tails {
        *tail = f32::NAN;
    }
    for i in 0..lanes.order.len() {
        let t = lanes.order[i] as usize;
        if grid.at(t).is_some_and(|b| b.what == What::Belt) {
            // an empty lane is "as far ahead as you like": nothing is in the way
            lanes.tails[t] = lanes.of[t].back().map(|i| i.along).unwrap_or(f32::INFINITY);
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
        // **`2.0` is the far end of the next tile**, and it is a cap rather than a number: an
        // empty neighbour's tail is `INFINITY` (nothing is in the way), so without it the front
        // would run off to infinity in one step. The hand-over below moves an item one tile at a
        // time, so at the speeds this is played at (a thirtieth of a tile a frame) nothing ever
        // reaches it. At a tile a frame it would, and then *whether* the second join is crossed
        // in the same frame depends on the neighbour's index — see the F1 worklog's notes.
        let room = match room_ahead(grid, t, dir) {
            Some(next) => (1.0 + lanes.tails[next] - spacing).min(2.0),
            None => 1.0,
        };
        carry(&mut lanes.of[t], forward, spacing, room.max(0.0));
    }

    // ---- 3. hand over -----------------------------------------------------------------------
    for i in 0..lanes.order.len() {
        let t = lanes.order[i] as usize;
        let Some(&Building { what: What::Belt, dir, .. }) = grid.at(t) else { continue };
        let next = grid.step_from(t, dir);
        while lanes.of[t].front().is_some_and(|f| f.along >= 1.0) {
            let front = lanes.of[t][0];
            let handed = match next.and_then(|n| grid.at(n).map(|b| (n, b.what))) {
                // onto the next belt, if it is not the one this belt is being fed by and the gap
                // is still there now that it is this tile's turn
                Some((n, What::Belt)) if grid.at(n).is_some_and(|b| b.dir != dir.back()) => {
                    let arriving = OnBelt { along: front.along - 1.0, item: front.item };
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
                // into a chest that has room, or into a machine that wants it. Both are what
                // `machines::hand_to` answers, and it is the only place that knows.
                Some((n, What::Chest | What::Machine(_) | What::Covered { .. })) => {
                    let took = machines::hand_to(grid, lanes, rules, data, n, front.item);
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
                hold(&mut lanes.of[t], spacing, 1.0);
                break;
            }
        }
    }

    // ---- 4, 5, 6: the machines ---------------------------------------------------------------
    let order = core::mem::take(&mut lanes.order);
    machines::dig(grid, ore, lanes, rules, data, &order, seconds, &mut moves);
    machines::deliver(grid, lanes, rules, data, &order, &mut moves);
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
pub fn carry(lane: &mut VecDeque<OnBelt>, forward: f32, spacing: f32, head_max: f32) {
    let mut limit = head_max;
    for p in lane.iter_mut() {
        p.along = (p.along + forward).min(limit);
        limit = p.along - spacing;
    }
}

/// The lane brought back behind a front item that could not go on.
fn hold(lane: &mut VecDeque<OnBelt>, spacing: f32, head: f32) {
    let mut limit = head;
    for p in lane.iter_mut() {
        p.along = p.along.min(limit);
        limit = p.along - spacing;
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
        "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2.0\n",
        "miner :drill, seconds_per_item: 1.0, digs: :ore\n",
        "chest :crate, capacity: 4\n",
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
    fn a_rock(along: f32) -> OnBelt {
        OnBelt { along, item: ORE }
    }

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
        lanes.of[first].push_back(a_rock(0.0));

        // four tiles at two tiles a second is two seconds, less the tile it starts on
        let frames = (3.0 / rules.belt_tiles_per_second / FRAME).ceil() as u32;
        let moves = run(&mut grid, &mut ore, &mut lanes, &rules, &data, frames);
        assert_eq!(lanes.count(), 1, "the item is still on the belt and there is only one");
        assert_eq!(lanes.of[last].len(), 1, "and it is on the last tile: {:?}", lanes.of);
        assert_eq!(lanes.of[last][0].item, ORE, "and it is still the same item");
        assert_eq!(moves.carried(), 3, "it crossed three joins to get there");
        // and it is at the far end of it, where it stops: nothing to hand it to
        run(&mut grid, &mut ore, &mut lanes, &rules, &data, 60);
        assert!((lanes.of[last][0].along - 1.0).abs() < 1e-5, "{:?}", lanes.of[last]);
    }

    /// **It jams.** A belt running into nothing fills up from the end, one item every gap, and
    /// then the tile behind it fills too — and no item is ever lost or overlapped.
    ///
    /// **A jammed tile holds one more item than `items_per_tile`**, and that is not an off-by-one:
    /// the item at `0.0` is at the tile's entry edge, which is the same point in the world as
    /// `1.0` on the tile before it. `items_per_tile` is how many *gaps* fit in a tile's length,
    /// and a queue of N gaps has N + 1 things in it. (With the gap this game is played at. When
    /// the gap is not a number `f32` holds exactly it can be one fewer, which is what
    /// [`a_jam_keeps_the_gap_even_when_the_gap_is_not_an_exact_number`] is about.)
    ///
    /// [`a_jam_keeps_the_gap_even_when_the_gap_is_not_an_exact_number`]:
    ///     self::a_jam_keeps_the_gap_even_when_the_gap_is_not_an_exact_number
    #[test]
    fn items_pile_up_behind_a_belt_that_runs_into_nothing() {
        let (rules, data) = world();
        let (mut grid, mut o, mut lanes) = line(2);
        let first = grid.index(UVec2::new(1, 1));
        let last = grid.index(UVec2::new(2, 1));
        // six items, which is more than the two tiles hold: the ones at negative positions are
        // where something feeding this belt would be pushing them in from
        for i in 0..6 {
            lanes.of[first].push_back(a_rock(-(i as f32) * rules.spacing()));
        }
        run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);

        assert_eq!(lanes.count(), 6, "nothing was lost");
        assert_eq!(lanes.of[last].len(), 3, "the far tile: 1.0, 0.5, 0.0 — {:?}", lanes.of[last]);
        assert_eq!(lanes.of[first].len(), 3, "and the rest are waiting behind: {:?}", lanes.of);
        // the front one is at the end of the tile and the gap is kept everywhere
        assert!((lanes.of[last][0].along - 1.0).abs() < 1e-5);
        for lane in [&lanes.of[first], &lanes.of[last]] {
            for pair in lane.iter().collect::<Vec<_>>().windows(2) {
                assert!(pair[0].along - pair[1].along >= rules.spacing() - 1e-5, "{lane:?}");
            }
        }
        // and the two tiles' items keep the gap across the join as well
        assert!(1.0 + lanes.of[last][2].along - lanes.of[first][0].along >= rules.spacing() - 1e-5);
        // nothing has moved for a while: this is a jam and not a slow queue
        let before: Vec<f32> = lanes.of[first].iter().map(|i| i.along).collect();
        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 60);
        assert_eq!(moves.carried(), 0, "a jam does not hand anything on");
        assert_eq!(lanes.of[first].iter().map(|i| i.along).collect::<Vec<f32>>(), before);
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
            lanes.of[from_west].push_back(a_rock(-(i as f32) * rules.spacing()));
            lanes.of[from_south].push_back(a_rock(-(i as f32) * rules.spacing()));
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
                assert!(pair[0].along - pair[1].along >= rules.spacing() - 1e-5, "{lane:?}");
            }
        }
        // and the gap is kept across the join the two of them merge into
        let tail = lanes.of[away][lanes.of[away].len() - 1].along;
        assert!(1.0 + tail - lanes.of[join][0].along >= rules.spacing() - 1e-5);
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
        lanes.of[left].push_back(a_rock(0.0));

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 120);
        assert_eq!(moves.carried(), 0, "it never crossed");
        assert_eq!(lanes.of[left].len(), 1);
        assert!((lanes.of[left][0].along - 1.0).abs() < 1e-5, "it waits at the end of its own tile");
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

    /// **What a jam does when the gap is not a number `f32` can hold exactly.**
    ///
    /// The test above says a jammed tile holds `items_per_tile + 1`, and that is a statement about
    /// arithmetic with no rounding in it: it wants `items_per_tile` gaps to add up to exactly one
    /// tile. They only do when `1 / items_per_tile` is exact, which is when it is a power of two.
    /// `1 / 3` rounds **up**, so three of them are a hair longer than a tile and the fourth item
    /// does not fit; and the position of a jammed item is not `1 - k × gap` but a chain of
    /// subtractions, each rounded again, so which way the last one falls is not something to work
    /// out on paper. This runs it.
    ///
    /// **What is being asserted is the invariant and not the count**: no item is ever lost, and no
    /// two are ever closer than a gap. How many end up on the tile is a *consequence* of the gap,
    /// and it is either `items_per_tile` or one more — a tile holding one item's worth less of
    /// buffer than the whole number suggests is not a wrong factory, it is the number 1/3.
    ///
    /// Measured 2026-09-21: 1, 2, 4, 5, 7 and 8 all hold one more than their number, and **3
    /// holds three**. It is not "the exact ones behave and the rest do not": three gaps of
    /// `f32(1/3)` come to a hair *over* a tile so the fourth item is turned away at the join,
    /// while five gaps of `f32(1/5)` come to a hair over as well and the chain of subtractions
    /// rounds back far enough to let the sixth in. Nothing is lost either way.
    #[test]
    fn a_jam_keeps_the_gap_even_when_the_gap_is_not_an_exact_number() {
        let (base, data) = world();
        for items_per_tile in [1.0f32, 2.0, 3.0, 4.0, 5.0, 7.0, 8.0] {
            let rules = Rules { items_per_tile, ..base.clone() };
            let (mut grid, mut o, mut lanes) = line(2);
            let first = grid.index(UVec2::new(1, 1));
            let last = grid.index(UVec2::new(2, 1));
            // three tiles' worth, so that the far tile fills, the near one fills behind it, and
            // there are still some waiting off the back of the line
            let all = (items_per_tile as usize + 1) * 3;
            for i in 0..all {
                lanes.of[first].push_back(a_rock(-(i as f32) * rules.spacing()));
            }
            run(&mut grid, &mut o, &mut lanes, &rules, &data, 1200);

            let held = lanes.of[last].len();
            println!("items_per_tile {items_per_tile}: a jammed tile holds {held}");
            assert_eq!(lanes.count(), all, "nothing was lost ({items_per_tile})");
            assert!(
                (lanes.of[last][0].along - 1.0).abs() < 1e-5,
                "the front is at the end of the tile ({items_per_tile}): {:?}",
                lanes.of[last]
            );
            for lane in [&lanes.of[first], &lanes.of[last]] {
                for pair in lane.iter().collect::<Vec<_>>().windows(2) {
                    assert!(
                        pair[0].along - pair[1].along >= rules.spacing() - 1e-5,
                        "{items_per_tile}: {lane:?}"
                    );
                }
            }
            assert!(
                1.0 + lanes.of[last][held - 1].along - lanes.of[first][0].along
                    >= rules.spacing() - 1e-5,
                "the gap is kept across the join too ({items_per_tile})"
            );
            let whole = items_per_tile as usize;
            assert!(
                held == whole || held == whole + 1,
                "{items_per_tile}: a tile holds its own number of gaps' worth, or one more — {held}"
            );
        }
    }

    /// The gap is a number a player can move, and moving it moves how much a tile holds.
    #[test]
    fn how_many_fit_on_a_tile_is_the_number_that_says_so() {
        let (base, data) = world();
        for items_per_tile in [1.0, 2.0, 4.0] {
            let rules = Rules { items_per_tile, ..base.clone() };
            let (mut grid, mut o, mut lanes) = line(1);
            let only = grid.index(UVec2::new(1, 1));
            for i in 0..8 {
                lanes.of[only].push_back(a_rock(-(i as f32) * rules.spacing()));
            }
            run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
            let on_the_tile = lanes.of[only].iter().filter(|i| i.along > 0.0).count();
            assert_eq!(on_the_tile, items_per_tile as usize, "{items_per_tile}: {:?}", lanes.of[only]);
        }
    }

    // ------------------------------------------------------------------------------------------
    // F2: the machines
    // ------------------------------------------------------------------------------------------

    /// **A furnace turns ore into a plate, in the time the recipe says.**
    ///
    /// The belt feeds it, the furnace takes the ore in, works for `time / speed` and puts a plate
    /// on the belt in front of it, which carries it to the chest. The time is checked as a number
    /// of frames rather than as a wall clock: the whole factory is a function of `seconds`.
    #[test]
    fn a_furnace_makes_what_the_recipe_says_in_the_time_it_says() {
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
        lanes.of[feed].push_back(a_rock(0.0));

        // one tile of belt at two tiles a second, then a craft of one second, then two tiles
        let recipe = &data.recipes[0];
        let seconds = 1.0 / rules.belt_tiles_per_second
            + recipe.time / data.machines[kind as usize].speed
            + 2.0 / rules.belt_tiles_per_second;
        let moves = run(
            &mut grid,
            &mut o,
            &mut lanes,
            &rules,
            &data,
            (seconds / FRAME).ceil() as u32 + 4,
        );
        assert_eq!(moves.crafted(), 1, "one craft, and not two: {:?}", moves.0);
        assert_eq!(inside(&grid, chest).of(PLATE), 1, "a plate came out of it");
        assert_eq!(inside(&grid, chest).of(ORE), 0, "and the ore went in, not through");
        assert_eq!(lanes.count(), 0, "nothing is left on the belts");
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
        lanes.of[feed].push_back(OnBelt { along: 0.0, item: GEAR });

        run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert_eq!(lanes.of[feed].len(), 1, "the gear is still on the belt");
        assert!((lanes.of[feed][0].along - 1.0).abs() < 1e-5, "waiting at the end of its tile");
        assert!(grid.at(furnace).unwrap().held.is_empty(), "and the furnace took nothing");
    }

    /// **A machine of four tiles is fed through any of them, and puts what it makes outside.**
    ///
    /// This is the whole of what covering several tiles means: a belt running into the far corner
    /// of a 2 by 2 machine is running into the machine, and what it makes comes out one step the
    /// way it faces from the corner it was built on.
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
        // a belt running east into the machine's *upper* row, which is not the origin's row
        let feed = grid.index(UVec2::new(2, 4));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        // and the tile the output goes to: one east of the footprint, in the origin's row
        let out = grid.index(UVec2::new(5, 3));
        grid.place(out, Building::new(What::Chest, Dir::East));
        for i in 0..2 {
            lanes.of[feed].push_back(OnBelt { along: -(i as f32) * rules.spacing(), item: PLATE });
        }

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert_eq!(moves.crafted(), 1, "two plates make one gear: {:?}", moves.0);
        assert_eq!(inside(&grid, out).of(GEAR), 1, "and it came out at the tile it faces");
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
        let furnace = grid.index(UVec2::new(2, 1));
        grid.place(feed, Building::new(What::Belt, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(What::Machine(kind), Dir::East));
        for i in 0..3 {
            lanes.of[feed].push_back(a_rock(-(i as f32) * rules.spacing()));
        }

        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 600);
        assert_eq!(moves.crafted(), 1, "one plate made, and it is still in there: {:?}", moves.0);
        let held = grid.at(furnace).unwrap();
        assert_eq!(held.made.of(PLATE), 1, "what it made is waiting");
        assert_eq!(held.held.of(ORE), 1, "and one craft's worth of ore is waiting behind it");
        assert_eq!(lanes.of[feed].len(), 1, "the third is jammed on the belt");

        // somewhere to put it, and it goes — and then the next craft runs
        let out = grid.index(UVec2::new(3, 1));
        grid.place(out, Building::new(What::Chest, Dir::East));
        let moves = run(&mut grid, &mut o, &mut lanes, &rules, &data, 300);
        assert_eq!(
            inside(&grid, out).of(PLATE),
            3,
            "the one it was holding, the one it had the ore for, and the one that was jammed"
        );
        assert_eq!(moves.crafted(), 2, "two more crafts, not three: {:?}", moves.0);
        assert_eq!(lanes.count(), 0, "and the belt emptied");
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
