//! **The machines**: what they hold, what they make, and how a thing gets in and out of one.
//!
//! F1's factory had one machine — the miner — and one kind of item, so an item was a position on
//! a belt and a chest was a number. F2 declares items in Ruby, so an item on a belt carries
//! *which* item it is ([`crate::belts::OnBelt`]) and anything that holds several is a [`Stock`].
//!
//! Three of the six passes of one step of the factory are here, and the order of them is the
//! model as much as the belts' four are ([`crate::belts::step`]):
//!
//! 4. **dig** — a miner that has finished puts an item of [`Rules::digs`] into what it faces;
//! 5. **swing** — an inserter whose arm is on its way puts down what it is carrying when it
//!    arrives ([`crate::inserters`] is the half of that a script drives);
//! 6. **craft** — a machine with nothing waiting to go out takes a recipe it has the parts for,
//!    consumes them and works at it.
//!
//! **F2's fifth pass was `deliver`** — a machine pushing what it made into what it faces — and F3
//! took it out. Nothing goes into or out of a machine now but through an inserter's hand, which
//! is what makes the player's Ruby the thing that joins a factory up (plan §4).
//!
//! # Who may hand a thing to what
//!
//! One table, and it is the whole of the rule F3 added ([`hand_to`], [`Offer`]):
//!
//! | into | a belt or a miner offering | an inserter offering |
//! |---|---|---|
//! | a belt | yes, if there is a gap | yes, if there is a gap |
//! | a chest | yes, if it has room | yes, if it has room |
//! | a machine | **no** | yes, if some recipe of its kind wants it |
//!
//! **A chest takes from a belt and a machine does not**, and that is a decision rather than an
//! oversight (`docs/worklog/2026-09-21-factory-F3.md`): Factorio needs an inserter for both, and
//! taking the chest away as well would mean the first thing a player builds — a miner, a belt, a
//! chest, which is what F1's checks build and what the game opens on — could not be built without
//! writing Ruby. The line that has to be joined by a script is the one with a *machine* in it,
//! because that is the line the game is about.
//!
//! # How much a machine holds
//!
//! **One craft's worth in and one craft's worth out**, and neither is a number anybody chose. A
//! machine takes an item in while it has less of it than one craft needs, so it can fill up for
//! the next craft while it is working on this one — the smallest buffer that lets it run without
//! a gap. It refuses to start another craft while it is still holding what it made, which is
//! exactly what F1's miner does with a finished dig: **blocked is not lost**.
//!
//! # A machine of more than one tile
//!
//! The grid holds [`What::Machine`] on the machine's **origin** — the bottom-left tile of its
//! footprint, the one that was clicked — and [`What::Covered`] on the rest, each pointing back at
//! the origin. Everything that happens to a machine happens at its origin; a belt handing an item
//! to a covered tile is handing it to the machine, which is what lets a belt feed a big machine
//! from any of its sides.
//!
//! The footprint is in the map's own axes and **does not turn** with the machine
//! ([`crate::data::Data::footprint`]). What the direction says is where the machine puts what it
//! makes.

use bevy::prelude::*;

use crate::belts::{Lanes, Move, Moves, OnBelt, Rules};
use crate::data::{Data, ItemId, RecipeId};
use crate::grid::{Building, Grid, Ore, What};

/// **A few items of a few kinds**: what a chest holds, what a machine has taken in, and what it
/// has made.
///
/// A `Vec` of pairs rather than one number per item, because how many kinds of item there are is
/// a question `data.rb` answers and this file cannot: a chest that had a slot per declared item
/// would be a chest whose size depends on the data file. Nothing here holds more than a handful
/// of kinds, so a scan is a scan of two or three.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stock(pub Vec<(ItemId, u32)>);

impl Stock {
    /// How many of one kind.
    pub fn of(&self, item: ItemId) -> u32 {
        self.0.iter().find(|(i, _)| *i == item).map(|(_, n)| *n).unwrap_or(0)
    }

    /// How many things altogether, which is what a chest's capacity counts.
    pub fn count(&self) -> u32 {
        self.0.iter().map(|(_, n)| n).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|(_, n)| *n == 0)
    }

    pub fn add(&mut self, item: ItemId, n: u32) {
        match self.0.iter_mut().find(|(i, _)| *i == item) {
            Some(entry) => entry.1 += n,
            None => self.0.push((item, n)),
        }
    }

    /// Takes `n` away, and answers whether there were that many. Nothing is taken when there
    /// were not, so a recipe that cannot be paid for in full pays nothing.
    pub fn take(&mut self, item: ItemId, n: u32) -> bool {
        match self.0.iter_mut().find(|(i, _)| *i == item) {
            Some(entry) if entry.1 >= n => {
                entry.1 -= n;
                true
            }
            _ => false,
        }
    }

    /// The first kind there is any of, which is what a machine hands out next.
    pub fn first(&self) -> Option<ItemId> {
        self.0.iter().find(|(_, n)| *n > 0).map(|(i, _)| *i)
    }
}

// ---------------------------------------------------------------------------------------------

/// **Who is holding the thing out**, which is the whole of what F3 added to [`hand_to`]: a
/// machine takes from an inserter's hand and from nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// A belt carrying something into the next tile, or a miner putting a dig down.
    Direct,
    /// An inserter's hand at the end of its swing.
    Inserter,
}

/// **The machine a tile belongs to**: itself if it is an origin, the origin if it is covered.
pub fn machine_at(grid: &Grid, tile: usize) -> Option<usize> {
    match grid.at(tile).map(|b| b.what) {
        Some(What::Machine(_)) => Some(tile),
        Some(What::Covered { origin }) => Some(origin as usize),
        _ => None,
    }
}

/// **Whether a machine would take this item**, and the whole of what a belt or a miner has to ask
/// before handing one over: some recipe of its kind wants it, and it has less than one craft's
/// worth of it already.
pub fn wants(grid: &Grid, data: &Data, origin: usize, item: ItemId) -> bool {
    let Some(Building { what: What::Machine(kind), held, .. }) = grid.at(origin) else {
        return false;
    };
    let Some(machine) = data.machines.get(*kind as usize) else { return false };
    let needed = machine
        .recipes
        .iter()
        .filter_map(|&r| data.recipes.get(r as usize))
        .filter_map(|r| r.inputs.iter().find(|(i, _)| *i == item).map(|(_, n)| *n))
        .max();
    match needed {
        Some(needed) => held.of(item) < needed,
        None => false,
    }
}

/// Puts an item into a machine. The caller has already asked [`wants`].
pub fn take_in(grid: &mut Grid, origin: usize, item: ItemId) {
    if let Some(building) = grid.at_mut(origin) {
        building.held.add(item, 1);
    }
}

// ---------------------------------------------------------------------------------------------
// The three passes
// ---------------------------------------------------------------------------------------------

/// **Pass 4: dig.** F1's fourth pass, with the item it brings up now a number out of `data.rb`.
// eight of them, and each is one of the things a dig touches: the grid it stands on, the ground
// under it, the belt in front of it, the two tables, the walk order, the frame and the record
#[allow(clippy::too_many_arguments)]
pub fn dig(
    grid: &mut Grid,
    ore: &mut Ore,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    order: &[u32],
    seconds: f32,
    moves: &mut Moves,
) {
    for &t in order {
        let t = t as usize;
        let Some(&Building { what: What::Miner, dir, work, .. }) = grid.at(t) else { continue };
        let work = work + seconds;
        if work < rules.mine_seconds || ore.left[t] == 0 {
            // **A miner over nothing is not broken, it is idle**: it keeps its work so that
            // laying ore under it later starts it at once rather than a dig late.
            if let Some(miner) = grid.at_mut(t) {
                miner.work = work.min(rules.mine_seconds);
            }
            continue;
        }
        let placed = put_into(grid, lanes, rules, data, t, dir, rules.digs, Offer::Direct);
        if let Some(miner) = grid.at_mut(t) {
            // **blocked is not lost**: the dig stays finished and is delivered the moment there
            // is somewhere to put it, and it does not bank a second one meanwhile
            miner.work = if placed.is_some() { work - rules.mine_seconds } else { rules.mine_seconds };
        }
        if let Some(at) = placed {
            moves.0.push(Move::Made { at, item: rules.digs });
            ore.left[t] -= 1;
            if ore.left[t] == 0 {
                ore.changed = true;
            }
        }
    }
}

/// **What a `move` does at the start of a swing**: an empty hand reaches into the tile behind, a
/// full one carries on with what it was refused, and either way the arm sets off. It answers
/// whether it is now swinging — `false` is an arm with an empty hand and nothing behind it.
///
/// **The picking up is now and not at the end**, which is what makes the item travel with the
/// arm: it is out of the belt and in the hand from this moment, which is where the picture draws
/// it and where the sum that says nothing is lost counts it.
///
/// It is here, beside the arm, rather than in the system that answers a `move`, so that a test of
/// the arm can drive one with no Ruby and no `App` — and so that there is one place that says
/// what a swing begins with.
pub fn start_swing(grid: &mut Grid, lanes: &mut Lanes, tile: usize) -> bool {
    let Some(&Building { what: What::Inserter, dir, .. }) = grid.at(tile) else { return false };
    if grid.at(tile).is_none_or(|b| b.held.is_empty()) {
        let picked =
            grid.step_from(tile, dir.back()).and_then(|from| take_from(grid, lanes, from));
        match picked {
            Some(item) => {
                if let Some(arm) = grid.at_mut(tile) {
                    arm.held.add(item, 1);
                }
            }
            None => return false,
        }
    }
    if let Some(arm) = grid.at_mut(tile) {
        arm.swinging = true;
        arm.work = 0.0;
    }
    true
}

/// **Pass 5: swing.** An inserter whose arm is on its way moves it on, and puts down what it is
/// carrying the moment the swing is over.
///
/// The *decision* to start a swing is not here: it is a line of Ruby, answered in
/// [`crate::inserters`]. What is here is the arm — how long it takes and what happens at the end
/// of it — because an arm is physics and physics is Rust (plan §1).
///
/// **A swing that ends over a full belt is not a swing wasted.** The hand keeps what it picked
/// up, the arm stops, and the script is told the answer was no; the same "blocked is not lost"
/// rule a miner keeps with a finished dig. What the script does about it is the script's.
pub fn swing(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    order: &[u32],
    seconds: f32,
    moves: &mut Moves,
) {
    for &t in order {
        let t = t as usize;
        let Some(&Building { what: What::Inserter, dir, work, swinging, .. }) = grid.at(t) else {
            continue;
        };
        if !swinging {
            continue;
        }
        let work = work + seconds;
        if work < rules.swing_seconds {
            if let Some(arm) = grid.at_mut(t) {
                arm.work = work;
            }
            continue;
        }
        // the arm has arrived: put down what it is carrying, if the tile in front will take it
        let carrying = grid.at(t).and_then(|b| b.held.first());
        let placed = match carrying {
            Some(item) => {
                put_into(grid, lanes, rules, data, t, dir, item, Offer::Inserter).is_some()
            }
            // nothing in the hand at the end of a swing is an arm that was told to move when
            // there was nothing behind it: the swing happened and moved nothing
            None => false,
        };
        if let Some(arm) = grid.at_mut(t) {
            arm.swinging = false;
            arm.work = 0.0;
            if placed && let Some(item) = carrying {
                arm.held.take(item, 1);
            }
        }
        if placed
            && let Some(into) = grid.step_from(t, dir)
            && let Some(item) = carrying
        {
            moves.0.push(Move::Made { at: into, item });
        }
        moves.0.push(Move::Swung { at: t, placed });
    }
}

/// **Pass 6: craft.** A machine with nothing waiting to go out takes the first recipe of its kind
/// it has the parts for, and works at it for the recipe's own time divided by its speed.
pub fn craft(
    grid: &mut Grid,
    data: &Data,
    order: &[u32],
    seconds: f32,
    moves: &mut Moves,
) {
    for &t in order {
        let t = t as usize;
        let Some(building) = grid.at(t) else { continue };
        let What::Machine(kind) = building.what else { continue };
        // **holding what it made is what stops it** — the same rule F1's miner keeps
        if !building.made.is_empty() {
            // **and if it has the parts for another craft, that is what this game calls a jam**
            // (F4): the machine is not waiting for anything a player has to bring it, it is
            // waiting for somebody to take what it has already made. It is said every frame it
            // is true; the control stage is told the frame it *became* true
            // (`crate::control::Happenings`).
            if could_start(grid, data, t, kind) {
                moves.0.push(Move::Jammed { at: t });
            }
            continue;
        }
        let (making, work) = (building.making, building.work);
        let Some(machine) = data.machines.get(kind as usize) else { continue };
        let speed = machine.speed;
        let Some(id) = making.or_else(|| start(grid, data, t, kind)) else { continue };
        let Some(recipe) = data.recipes.get(id as usize) else { continue };
        let work = work + seconds * speed;
        if work < recipe.time {
            if let Some(b) = grid.at_mut(t) {
                b.work = work;
            }
            continue;
        }
        let outputs = recipe.outputs.clone();
        if let Some(b) = grid.at_mut(t) {
            for (item, n) in outputs {
                b.made.add(item, n);
            }
            b.work = 0.0;
            b.making = None;
        }
        moves.0.push(Move::Crafted { at: t, recipe: id });
    }
}

/// **Whether a machine has the parts for a craft**, without starting one — [`start`]'s first half,
/// which is what tells a machine that is waiting for parts from one that is waiting for somebody
/// to take what it made ([`Move::Jammed`]).
pub fn could_start(grid: &Grid, data: &Data, tile: usize, kind: u16) -> bool {
    let Some(machine) = data.machines.get(kind as usize) else { return false };
    let Some(held) = grid.at(tile).map(|b| &b.held) else { return false };
    machine.recipes.iter().any(|&r| {
        data.recipes
            .get(r as usize)
            .is_some_and(|recipe| recipe.inputs.iter().all(|&(i, n)| held.of(i) >= n))
    })
}

/// Which recipe a machine can begin: the first of its kind whose inputs it is holding. The parts
/// are taken out of the machine's store here, which is what makes a craft something that has
/// begun rather than something being considered every frame.
fn start(grid: &mut Grid, data: &Data, tile: usize, kind: u16) -> Option<RecipeId> {
    let machine = data.machines.get(kind as usize)?;
    let held = grid.at(tile).map(|b| b.held.clone())?;
    let ready = machine.recipes.iter().copied().find(|&r| {
        data.recipes
            .get(r as usize)
            .is_some_and(|recipe| recipe.inputs.iter().all(|&(i, n)| held.of(i) >= n))
    })?;
    let recipe = data.recipes.get(ready as usize)?;
    let building = grid.at_mut(tile)?;
    for &(item, n) in &recipe.inputs {
        building.held.take(item, n);
    }
    building.making = Some(ready);
    building.work = 0.0;
    Some(ready)
}

// ---------------------------------------------------------------------------------------------

/// **Puts one item into the tile `dir` of `from`**, which is what a miner does with a dig and
/// what an inserter's arm does at the end of a swing. It answers the tile it went into, so that
/// the caller can say where.
#[allow(clippy::too_many_arguments)]
fn put_into(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    from: usize,
    dir: crate::grid::Dir,
    item: ItemId,
    by: Offer,
) -> Option<usize> {
    let into = grid.step_from(from, dir)?;
    hand_to(grid, lanes, rules, data, into, item, by).then_some(into)
}

/// **Whether a tile took an item**, and it takes it if it did: the one place that knows what each
/// kind of thing does with something handed to it, and — since F3 — **who is allowed to hand it**
/// (the table at the head of this file).
pub fn hand_to(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    into: usize,
    item: ItemId,
    by: Offer,
) -> bool {
    match grid.at(into).map(|b| b.what) {
        Some(What::Belt) => {
            let spacing = rules.spacing();
            // it goes on at the tile's entry edge, which is step 0, and it only goes on if what
            // is already there has moved a gap's worth away from it
            let room = lanes.on(into).back().is_none_or(|last| last.along >= spacing);
            if room {
                lanes.put_on(into, OnBelt { along: 0, item });
            }
            room
        }
        Some(What::Chest) => {
            let full = grid.at(into).is_some_and(|b| b.held.count() >= rules.chest_capacity);
            if !full && let Some(chest) = grid.at_mut(into) {
                chest.held.add(item, 1);
                return true;
            }
            false
        }
        // **an inserter's hand and nothing else** (F3). A belt running into a machine jams, which
        // is what makes a factory with no inserters in it a factory that does not run.
        Some(What::Machine(_)) | Some(What::Covered { .. }) if by == Offer::Inserter => {
            let Some(origin) = machine_at(grid, into) else { return false };
            if wants(grid, data, origin, item) {
                take_in(grid, origin, item);
                return true;
            }
            false
        }
        _ => false,
    }
}

/// **What the tile at `from` would let an inserter take**, without taking it: the item on the
/// front of a belt, what a chest holds, or what a machine has made and not got rid of.
///
/// It is the question an inserter's `behind` asks and the first half of what its `move` does, so
/// the two cannot disagree about what is there. A miner is not in the list: a miner puts its own
/// dig down (`dig`), and an arm reaching into one would be a second way for the same item to
/// leave the ground.
pub fn would_give(grid: &Grid, lanes: &Lanes, from: usize) -> Option<ItemId> {
    match grid.at(from).map(|b| b.what) {
        // the one nearest the end of the tile, which is the one a belt would hand on next
        Some(What::Belt) => lanes.on(from).front().map(|on| on.item),
        Some(What::Chest) => grid.at(from).and_then(|b| b.held.first()),
        Some(What::Machine(_)) | Some(What::Covered { .. }) => {
            machine_at(grid, from).and_then(|origin| grid.at(origin)).and_then(|b| b.made.first())
        }
        _ => None,
    }
}

/// **Takes what [`would_give`] said was there**, and answers it. Nothing is taken where nothing
/// was offered.
pub fn take_from(grid: &mut Grid, lanes: &mut Lanes, from: usize) -> Option<ItemId> {
    let item = would_give(grid, lanes, from)?;
    match grid.at(from).map(|b| b.what) {
        Some(What::Belt) => {
            lanes.take_off(from);
        }
        Some(What::Chest) => {
            if let Some(chest) = grid.at_mut(from) {
                chest.held.take(item, 1);
            }
        }
        Some(What::Machine(_)) | Some(What::Covered { .. }) => {
            let origin = machine_at(grid, from)?;
            if let Some(machine) = grid.at_mut(origin) {
                machine.made.take(item, 1);
            }
        }
        _ => return None,
    }
    Some(item)
}

/// **Whether the tile at `into` would take this item from an inserter**, without giving it one:
/// the question an inserter's `front_takes?` asks.
pub fn would_take(grid: &Grid, lanes: &Lanes, rules: &Rules, data: &Data, into: usize, item: ItemId) -> bool {
    match grid.at(into).map(|b| b.what) {
        Some(What::Belt) => {
            lanes.on(into).back().is_none_or(|last| last.along >= rules.spacing())
        }
        Some(What::Chest) => {
            grid.at(into).is_some_and(|b| b.held.count() < rules.chest_capacity)
        }
        Some(What::Machine(_)) | Some(What::Covered { .. }) => {
            machine_at(grid, into).is_some_and(|origin| wants(grid, data, origin, item))
        }
        _ => false,
    }
}
