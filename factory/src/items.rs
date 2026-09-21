//! **Where an item lives, and the measurement that settled it.**
//!
//! The plan (§3.1, §3.7) would not settle this on paper: *"items are entities, or items are a lane
//! per belt with sprites only for the drawing — measure both at thousands of items and decide"*.
//! F1 built both, ran the same rule over both at 1,000, 4,000 and 16,000 items on a PC and in a
//! browser, and **kept the lanes**: `docs/worklog/2026-09-21-factory-F1.md` §3 has the numbers and
//! `git show e3af41a:factory/src/items.rs` has the code that was measured.
//!
//! **Why the lanes win, and it is not only the 2.2 to 2.9 times.** The rule needs *the item in
//! front* ([`belts::step`]), and that is an order. A lane is one. An ECS query is not: the way
//! round where every item is an entity has to **rebuild the order every frame** — bucket the query
//! by tile, sort each bucket, apply the rule, write every position back — which is work the model
//! does not ask for and the lane does not do. What is left here is the lane's side of it: one
//! step a frame, and a pool of sprites to be seen with.
//!
//! [`belts::step`]: crate::belts::step

use bevy::prelude::*;

use crate::belts::{self, Lanes, Rules};
use crate::data::{Data, ItemId};
use crate::grid::{Flow, Grid, Ore};
use crate::Map;

/// **How much the factory did this frame**, for the HUD F5 will have, for the checks, and for the
/// log line the stress run prints.
#[derive(Resource, Debug, Default)]
pub struct Tally {
    pub items: usize,
    pub carried: u64,
    pub taken: u64,
    pub made: u64,
    /// How many crafts the machines have finished (F2). A miner's dig is not one: what it does is
    /// counted by `made`, which is the same word the checks use for both.
    pub crafted: u64,
    /// What one step of the factory took, in microseconds. Measured with
    /// `bevy::platform::time::Instant`, which is the clock that exists in a browser as well
    /// (`std::time::Instant` panics there, which is what made the published garden a black page
    /// on 2026-09-18). **A browser rounds it to 100 µs**, which is worth knowing before reading
    /// one off a page.
    pub last_step_us: f32,
    /// The same, averaged over about a second, for a number that can be read off a screen.
    pub step_us: f32,
}

/// One step of the factory. The lanes are where every item is, so this is the whole of it.
pub fn run_the_factory(
    time: Res<Time>,
    rules: Res<Rules>,
    data: Res<Data>,
    mut grid: ResMut<Grid>,
    mut ore: ResMut<Ore>,
    mut lanes: ResMut<Lanes>,
    mut tally: ResMut<Tally>,
    mut arms: ResMut<crate::inserters::Arms>,
) {
    let started = bevy::platform::time::Instant::now();
    let moves = belts::step(&mut grid, &mut ore, &mut lanes, &rules, &data, time.delta_secs());
    // **the arms that arrived are what a script is waiting on** — the step is the only thing that
    // knows, and `crate::inserters::finish_swings` is the only thing that can answer
    arms.finished.extend(moves.swung());
    tally.items = lanes.count();
    tally.carried += moves.carried() as u64;
    tally.taken += moves.taken() as u64;
    tally.made += moves.made() as u64;
    tally.crafted += moves.crafted() as u64;
    let us = started.elapsed().as_secs_f32() * 1e6;
    tally.last_step_us = us;
    // an average over about a second, so that a number read off the screen is not one frame's
    tally.step_us = if tally.step_us == 0.0 { us } else { tally.step_us * 0.95 + us * 0.05 };
}

/// The sprites the lanes borrow to be seen with. Only a run with a window has any.
#[derive(Resource, Debug, Default)]
pub struct Pool {
    pub sprites: Vec<Entity>,
}

// ---------------------------------------------------------------------------------------------

/// Every item's place in the world, in the order the lanes hold them. The drawing uses it; it is
/// a function rather than a system so that a run with no window could ask too.
pub fn places(map: &Map, grid: &Grid, flow: &Flow, lanes: &Lanes, out: &mut Vec<(Vec2, ItemId)>) {
    out.clear();
    for &t in grid.built() {
        let t = t as usize;
        let lane = lanes.on(t);
        if lane.is_empty() {
            continue;
        }
        let Some(building) = grid.at(t) else { continue };
        let tile = grid.tile_of(t);
        for on in lane {
            let at = crate::grid::item_at(map, tile, flow.came_in[t], building.dir, on.along);
            out.push((at, on.item));
        }
    }
}
