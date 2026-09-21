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
//! 5. **deliver** — a machine pushes what it has made into what it faces, one item at a time;
//! 6. **craft** — a machine with nothing waiting to go out takes a recipe it has the parts for,
//!    consumes them and works at it.
//!
//! Deliver comes before craft so that a machine that finished last frame is empty again when it
//! is asked whether it can start, which is what makes a machine with somewhere to put things run
//! back to back rather than every other craft.
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
        let placed = put_into(grid, lanes, rules, data, t, dir, rules.digs);
        if let Some(miner) = grid.at_mut(t) {
            // **blocked is not lost**: the dig stays finished and is delivered the moment there
            // is somewhere to put it, and it does not bank a second one meanwhile
            miner.work = if placed.is_some() { work - rules.mine_seconds } else { rules.mine_seconds };
        }
        if let Some(at) = placed {
            moves.0.push(Move::Made { at });
            ore.left[t] -= 1;
            if ore.left[t] == 0 {
                ore.changed = true;
            }
        }
    }
}

/// **Pass 5: deliver.** What a machine made goes into what it faces — a belt, a chest, or another
/// machine that wants it — one item at a time, and it stops at the first one that is refused.
pub fn deliver(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    order: &[u32],
    moves: &mut Moves,
) {
    for &t in order {
        let t = t as usize;
        let Some(&Building { what: What::Machine(kind), dir, .. }) = grid.at(t) else { continue };
        let tile = grid.tile_of(t);
        let out = data.output_of(kind, tile, dir);
        let last = grid.tiles as i32;
        if out.x < 0 || out.y < 0 || out.x >= last || out.y >= last {
            continue;
        }
        let into = (out.y as u32 * grid.tiles + out.x as u32) as usize;
        while let Some(item) = grid.at(t).and_then(|b| b.made.first()) {
            if !hand_to(grid, lanes, rules, data, into, item) {
                break;
            }
            if let Some(machine) = grid.at_mut(t) {
                machine.made.take(item, 1);
            }
            moves.0.push(Move::Made { at: into });
        }
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

/// **Puts one item into the tile `dir` of `from`**, which is what a miner does with a dig. It
/// answers the tile it went into, so that the caller can say where.
fn put_into(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    from: usize,
    dir: crate::grid::Dir,
    item: ItemId,
) -> Option<usize> {
    let into = grid.step_from(from, dir)?;
    hand_to(grid, lanes, rules, data, into, item).then_some(into)
}

/// **Whether a tile took an item**, and it takes it if it did: the one place that knows what each
/// kind of thing does with something handed to it.
pub fn hand_to(
    grid: &mut Grid,
    lanes: &mut Lanes,
    rules: &Rules,
    data: &Data,
    into: usize,
    item: ItemId,
) -> bool {
    match grid.at(into).map(|b| b.what) {
        Some(What::Belt) => {
            let spacing = rules.spacing();
            let room = lanes.of[into].back().is_none_or(|last| last.along >= spacing);
            if room {
                lanes.of[into].push_back(OnBelt { along: 0.0, item });
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
        Some(What::Machine(_)) | Some(What::Covered { .. }) => {
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
