//! **The two ways of holding an item, and the measurement that chose between them.**
//!
//! The plan (§3.1, §3.7) would not settle this on paper: *"items are entities, or items are a lane
//! per belt with sprites only for the drawing — measure both at thousands of items and decide"*.
//! So the game can be built either way and `--holding entities` / `--holding lanes` says which;
//! the numbers are in `docs/worklog/2026-09-21-factory-F1.md`.
//!
//! **The rule is not duplicated.** Both ways run the same [`belts::step`] over the same
//! `VecDeque<f32>` lanes, because the rule needs *the item in front* and that is an order. The
//! difference is where that order and those numbers live between frames:
//!
//! | | lanes | entities |
//! |---|---|---|
//! | where a position lives | the lane | a component on the item |
//! | where the order lives | the lane | nowhere — it is rebuilt every frame by bucketing the query by tile and sorting each bucket |
//! | what one item costs | 4 bytes | an entity, its archetype row, a `Transform`, a `Sprite` |
//! | what the drawing does | writes a pool of sprites, one per item on screen | writes each item's own `Transform` |
//!
//! [`belts::step`]: crate::belts::step

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::belts::{self, Lanes, Move, Rules};
use crate::grid::{Flow, Grid, Ore};
use crate::Map;

/// Which of the two the run was built with.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holding {
    /// The items are numbers in a lane per tile; the drawing borrows sprites from a pool.
    Lanes,
    /// Every item is an entity carrying its own position.
    Entities,
}

impl Holding {
    pub fn from_word(word: &str) -> Option<Holding> {
        match word {
            "lanes" | "lane" => Some(Holding::Lanes),
            "entities" | "entity" => Some(Holding::Entities),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Holding::Lanes => "lanes",
            Holding::Entities => "entities",
        }
    }
}

/// One item, where the items are entities.
#[derive(Component, Debug, Clone, Copy)]
pub struct OnBelt {
    /// The tile it is on.
    pub tile: u32,
    /// How far through that tile it has come, `0..=1`.
    pub along: f32,
}

/// The order the entities are in, rebuilt every frame — see the table at the top of the file.
/// It is kept as a resource rather than a `Local` so that the buffers are reused rather than
/// allocated, which is the fastest this way round can be made without stopping being this way
/// round.
#[derive(Resource, Debug, Default)]
pub struct Sorting {
    buckets: Vec<Vec<(f32, Entity)>>,
    lanes: Vec<VecDeque<Entity>>,
}

/// **How much the factory did this frame**, for the HUD F5 will have, for the checks, and for the
/// log line the stress run prints.
#[derive(Resource, Debug, Default)]
pub struct Tally {
    pub items: usize,
    pub carried: u64,
    pub taken: u64,
    pub made: u64,
    /// What one step of the factory took, in microseconds. Measured with
    /// `bevy::platform::time::Instant`, which is the clock that exists in a browser as well
    /// (`std::time::Instant` panics there, which is what made the published garden a black page
    /// on 2026-09-18).
    pub last_step_us: f32,
    /// The same, averaged over about a second, for a number that can be read off a screen.
    pub step_us: f32,
}

// ---------------------------------------------------------------------------------------------
// Lanes
// ---------------------------------------------------------------------------------------------

/// One step of the factory, with the lanes as the only copy of where anything is.
pub fn run_with_lanes(
    time: Res<Time>,
    rules: Res<Rules>,
    mut grid: ResMut<Grid>,
    mut ore: ResMut<Ore>,
    mut lanes: ResMut<Lanes>,
    mut tally: ResMut<Tally>,
) {
    let started = bevy::platform::time::Instant::now();
    let moves = belts::step(&mut grid, &mut ore, &mut lanes, &rules, time.delta_secs());
    note(&mut tally, &moves, lanes.count(), started);
}

/// The sprites the lanes borrow to be seen with. Only a run with a window has any.
#[derive(Resource, Debug, Default)]
pub struct Pool {
    pub sprites: Vec<Entity>,
}

// ---------------------------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------------------------

/// One step of the factory, with each item's position on the item itself.
///
/// Three passes rather than one, and the two extra ones are what this way round costs: the query
/// has to be **bucketed by tile and each bucket sorted** before the rule can be applied — an ECS
/// has no "the item in front of this one" — and then every position has to be written back.
pub fn run_with_entities(
    time: Res<Time>,
    rules: Res<Rules>,
    mut grid: ResMut<Grid>,
    mut ore: ResMut<Ore>,
    mut lanes: ResMut<Lanes>,
    mut sorting: ResMut<Sorting>,
    mut tally: ResMut<Tally>,
    mut items: Query<(Entity, &mut OnBelt)>,
    mut commands: Commands,
) {
    let started = bevy::platform::time::Instant::now();
    let cells = lanes.of.len();
    let sorting = &mut *sorting;
    if sorting.buckets.len() != cells {
        sorting.buckets = vec![Vec::new(); cells];
        sorting.lanes = vec![VecDeque::new(); cells];
    }

    // ---- gather: every item into the bucket of the tile it is on ---------------------------
    for bucket in &mut sorting.buckets {
        bucket.clear();
    }
    for (entity, on) in &items {
        if let Some(bucket) = sorting.buckets.get_mut(on.tile as usize) {
            bucket.push((on.along, entity));
        }
    }
    // ---- order: front first, which the lanes keep for nothing -------------------------------
    for (tile, bucket) in sorting.buckets.iter_mut().enumerate() {
        lanes.of[tile].clear();
        sorting.lanes[tile].clear();
        if bucket.is_empty() {
            continue;
        }
        bucket.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
        for &(along, entity) in bucket.iter() {
            lanes.of[tile].push_back(along);
            sorting.lanes[tile].push_back(entity);
        }
    }

    // ---- the same rule as the other way round ------------------------------------------------
    let moves = belts::step(&mut grid, &mut ore, &mut lanes, &rules, time.delta_secs());

    // ---- the entities follow the moves, in the order they happened ---------------------------
    for one in &moves.0 {
        match *one {
            Move::Carried { from, to } => {
                if let Some(entity) = sorting.lanes[from].pop_front() {
                    sorting.lanes[to].push_back(entity);
                    if let Ok((_, mut on)) = items.get_mut(entity) {
                        on.tile = to as u32;
                    }
                }
            }
            Move::Taken { from } => {
                if let Some(entity) = sorting.lanes[from].pop_front() {
                    commands.entity(entity).despawn();
                }
            }
            Move::Made { at } => {
                // **Spawned with the position it is about to have**: the write-back below cannot
                // reach it, because a command has not been applied yet and the component is not
                // there to be written.
                let along = *lanes.of[at].back().unwrap_or(&0.0);
                let entity = commands.spawn(OnBelt { tile: at as u32, along }).id();
                sorting.lanes[at].push_back(entity);
            }
        }
    }

    // ---- write back --------------------------------------------------------------------------
    for (tile, lane) in lanes.of.iter().enumerate() {
        for (i, &along) in lane.iter().enumerate() {
            let Some(&entity) = sorting.lanes[tile].get(i) else { continue };
            if let Ok((_, mut on)) = items.get_mut(entity) {
                on.tile = tile as u32;
                on.along = along;
            }
        }
    }
    // **The lanes are left as they are.** They are scratch this way round — the truth is on the
    // items — but they are also the only place anything else can read the factory from without
    // knowing which way round it is held, so the checks and the counters read them and the cost
    // of leaving them is nothing: they have just been filled.
    let count = lanes.count();
    note(&mut tally, &moves, count, started);
}

fn note(tally: &mut Tally, moves: &belts::Moves, items: usize, started: bevy::platform::time::Instant) {
    tally.items = items;
    tally.carried += moves.carried() as u64;
    tally.taken += moves.taken() as u64;
    tally.made += moves.made() as u64;
    let us = started.elapsed().as_secs_f32() * 1e6;
    tally.last_step_us = us;
    // an average over about a second, so that a number read off the screen is not one frame's
    tally.step_us = if tally.step_us == 0.0 { us } else { tally.step_us * 0.95 + us * 0.05 };
}

// ---------------------------------------------------------------------------------------------
// Where an item is, which both ways round need and neither owns
// ---------------------------------------------------------------------------------------------

/// Every item's place in the world, in the order the lanes hold them. The drawing uses it and so
/// do the checks; it is a function rather than a system so that a run with no window can ask.
pub fn places(map: &Map, grid: &Grid, flow: &Flow, lanes: &Lanes, out: &mut Vec<Vec2>) {
    out.clear();
    for &t in grid.built() {
        let t = t as usize;
        let lane = lanes.on(t);
        if lane.is_empty() {
            continue;
        }
        let Some(building) = grid.at(t) else { continue };
        let tile = grid.tile_of(t);
        for &along in lane {
            out.push(crate::grid::item_at(map, tile, flow.came_in[t], building.dir, along));
        }
    }
}
