//! **Building and unbuilding**: the keys that choose what is in hand, the click that puts it down,
//! and — since F3 — the **order** the two of them write.
//!
//! F1 has no window furniture at all — the editor, the HUD and the guide are F5 — so what is in
//! hand is said in the log and in `docs/factory.md`, and the checks forge the orders rather than
//! press anything. The one rule that is not "put it where the mouse was" is that **a miner has to
//! stand on ore**, which is what makes a patch worth finding.
//!
//! # Why there is an [`Order`] between the click and the building
//!
//! There are two halves and F1 and F2 had them as one system. [`clicks`] turns a mouse — a
//! [`WorldClick`], which is the shared crate's and carries a point in the world and nothing else
//! — together with what is in [`Hand`] into an order: **a tile, a thing, and a way round**.
//! [`orders`] is the only thing that builds, and it knows nothing about a mouse or a hand.
//!
//! **What that is worth is that the order carries what was meant.** With one system, what got
//! built was `Hand` *as it stood when that system ran*, not as it stood when the click was made,
//! and the two are different things the moment anything writes both in one frame. F2's checks did,
//! and a window run built a chest where it meant a belt — an hour after the same binary had run
//! clean, because Bevy's multi-threaded scheduler is free to order two systems that have not said
//! (`worklog/2026-09-21-factory-F2.md` §8.4a). F2 fixed that run by writing one ordering line;
//! what is here fixes the shape, which is what the plan asked for (§7, 09-21 F2): **the checks
//! write an order, and so does the editor, and so would a blueprint.**
//!
//! One ordering line is still wanted and its reason is a different one, written where it is said
//! (`crate::main`'s `selftest`): a check that gives an order and then looks at what it did has to
//! be on the same side of [`orders`] in every frame, or it looks a frame too early. That is a
//! property of *checking*, not of building, and nothing a player does is affected by it.

use bevy::prelude::*;
use games_shell::camera::WorldClick;

use crate::belts::Lanes;
use crate::data::Data;
use crate::grid::{Building, Dir, Flow, Grid, Ore, What};
use crate::Map;

/// **Build this here, facing this way** — or, where `what` is `None`, take away whatever is there.
///
/// It is the game's own message and not [`WorldClick`], which is the shared crate's and cannot
/// carry a tile or a building: this crate's vocabulary has no business in a camera. Everything
/// that puts something on the map writes one of these — the mouse ([`clicks`]), the checks, and
/// whatever F5's editor and a later blueprint want.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct Order {
    /// Which tile, which is already the answer to "where" — a point in the world is the click's
    /// business and is turned into this before the order is written.
    pub at: UVec2,
    /// What to put there. `None` is the wrecking ball.
    pub what: Option<What>,
    /// Which way round. A thing with no direction keeps it anyway, so that turning one is not a
    /// special case ([`Building::dir`]).
    pub dir: Dir,
}

/// What a click will put down, and which way round.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Hand {
    /// `None` is the wrecking ball: a click takes away whatever is there.
    pub what: Option<What>,
    pub dir: Dir,
}

impl Default for Hand {
    fn default() -> Hand {
        Hand { what: Some(What::Belt), dir: Dir::East }
    }
}

impl Hand {
    pub fn word(&self, data: &Data) -> String {
        word_for(self.what, data)
    }
}

/// **What to call a thing in a sentence.** A machine is called whatever `data.rb` called it, which
/// is the only name a player has for one; the three fittings have words of their own; nothing in
/// hand is the wrecking ball.
///
/// It is a function over [`What`] rather than a method on [`Hand`] because [`orders`] says these
/// sentences and has no hand — the order carries what was meant (see the module's note).
pub fn word_for(what: Option<What>, data: &Data) -> String {
    match what {
        Some(What::Machine(kind)) => data
            .machines
            .get(kind as usize)
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "a machine".into()),
        Some(what) => what.word().to_string(),
        None => "the wrecking ball".into(),
    }
}

/// **What each digit puts in hand**, worked out from the data file: `1` `2` `3` are the belt, the
/// miner and the chest, and the machines follow in the order they were declared.
///
/// `0` is the wrecking ball, which is why the machines start at 4 and not at 3, and why there is
/// room for six of them.
pub fn what_the_digits_hold(data: &Data) -> Vec<(KeyCode, Option<What>)> {
    let mut keys = vec![
        (KeyCode::Digit1, Some(What::Belt)),
        (KeyCode::Digit2, Some(What::Miner)),
        (KeyCode::Digit3, Some(What::Chest)),
        (KeyCode::Digit0, None),
    ];
    let digits = [KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
    for (i, digit) in digits.into_iter().enumerate() {
        match data.machines.get(i) {
            Some(_) => keys.push((digit, Some(What::Machine(i as crate::data::MachineId)))),
            None => break,
        }
    }
    keys
}

/// **The keys.** `1` `2` `3` choose a belt, a miner and a chest, `4` onwards choose the machines
/// `data.rb` declares, `0` chooses taking things away, and `R` turns what is in hand a quarter
/// turn anticlockwise.
pub fn choose(
    keys: Res<ButtonInput<KeyCode>>,
    data: Res<Data>,
    mut hand: ResMut<Hand>,
    mut digits: Local<Vec<(KeyCode, Option<What>)>>,
) {
    if digits.is_empty() {
        *digits = what_the_digits_hold(&data);
    }
    let mut said = false;
    for &(key, what) in digits.iter() {
        if keys.just_pressed(key) {
            hand.what = what;
            said = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        hand.dir = hand.dir.left();
        said = true;
    }
    if said {
        info!("in hand: {} facing {}", hand.word(&data), hand.dir.word());
    }
}

/// **The mouse, as an order**: the tile the click was in, and what was in hand at the moment the
/// click is read. Nothing is built here.
///
/// A click off the map is said and dropped — there is no tile to name in an order, and the
/// sentence is the useful half of it.
pub fn clicks(
    mut clicked: MessageReader<WorldClick>,
    map: Res<Map>,
    hand: Res<Hand>,
    mut orders: MessageWriter<Order>,
) {
    for click in clicked.read() {
        if click.button != MouseButton::Left {
            continue;
        }
        match map.tile_at(click.at) {
            Some(tile) => {
                orders.write(Order { at: tile, what: hand.what, dir: hand.dir });
            }
            None => info!("clicked {:.1}, {:.1} — off the map", click.at.x, click.at.y),
        }
    }
}

/// **The orders, carried out.** One tile, one building — or one machine's worth of tiles, or one
/// tile emptied.
///
/// The items on a belt that is taken away go with it. They have to: a lane belongs to its tile,
/// and leaving them there would mean a tile with no belt on it carrying things.
pub fn orders(
    mut asked: MessageReader<Order>,
    ore: Res<Ore>,
    data: Res<Data>,
    mut grid: ResMut<Grid>,
    mut lanes: ResMut<Lanes>,
) {
    for &Order { at: tile, what, dir } in asked.read() {
        if tile.x >= grid.tiles || tile.y >= grid.tiles {
            info!("nowhere to build at {}, {}: that is off the map", tile.x, tile.y);
            continue;
        }
        let at = grid.index(tile);
        match what {
            None => match take_away(&mut grid, &mut lanes, at) {
                Some(gone) => info!("took away the {} at {}, {}", gone, tile.x, tile.y),
                None => info!("nothing at {}, {} to take away", tile.x, tile.y),
            },
            // **a miner has to stand on ore**, which is the whole reason a patch is somewhere in
            // particular rather than everywhere
            Some(What::Miner) if ore.left[at] == 0 => {
                info!("no ore at {}, {}: a miner needs some", tile.x, tile.y);
            }
            Some(What::Machine(kind)) => {
                match build_a_machine(&mut grid, &mut lanes, &data, kind, tile, dir) {
                    true => info!(
                        "built a {} at {}, {} facing {}",
                        word_for(what, &data),
                        tile.x,
                        tile.y,
                        dir.word()
                    ),
                    false => info!(
                        "a {} does not fit at {}, {}: it would go off the map",
                        word_for(what, &data),
                        tile.x,
                        tile.y
                    ),
                }
            }
            // a covered tile is not something a player can put down; only a machine makes one
            Some(What::Covered { .. }) => {}
            Some(one) => {
                take_away(&mut grid, &mut lanes, at);
                lanes.of[at].clear();
                grid.place(at, Building::new(one, dir));
                info!("built a {} at {}, {} facing {}", one.word(), tile.x, tile.y, dir.word());
            }
        }
    }
}

/// **Takes away whatever is on a tile, all of it.** Clicking any tile of a machine takes the
/// whole machine, which is the only thing a player could mean by it — and what it was holding
/// goes with it, the way a belt's items do.
pub fn take_away(grid: &mut Grid, lanes: &mut Lanes, at: usize) -> Option<String> {
    let origin = match grid.at(at).map(|b| b.what) {
        Some(What::Covered { origin }) => origin as usize,
        Some(_) => at,
        None => return None,
    };
    let word = grid.at(origin).map(|b| b.what.word().to_string())?;
    // the covered tiles first, so that none of them is left pointing at a tile with nothing on it
    let covered: Vec<u32> = grid
        .built()
        .iter()
        .copied()
        .filter(|&t| grid.at(t as usize).map(|b| b.what) == Some(What::Covered { origin: origin as u32 }))
        .collect();
    for t in covered {
        grid.remove(t as usize);
        lanes.of[t as usize].clear();
    }
    grid.remove(origin);
    lanes.of[origin].clear();
    Some(word)
}

/// **Puts a machine down**, which is one building on its origin tile and a marker on the rest of
/// its footprint. It answers whether it fitted; a machine that would go off the map is not built
/// at all rather than built with a corner missing.
pub fn build_a_machine(
    grid: &mut Grid,
    lanes: &mut Lanes,
    data: &Data,
    kind: crate::data::MachineId,
    origin: UVec2,
    dir: Dir,
) -> bool {
    let footprint = data.footprint(kind, origin);
    if footprint.iter().any(|t| t.x >= grid.tiles || t.y >= grid.tiles) {
        return false;
    }
    let at = grid.index(origin);
    for tile in &footprint {
        let t = grid.index(*tile);
        take_away(grid, lanes, t);
    }
    for (i, tile) in footprint.iter().enumerate() {
        let t = grid.index(*tile);
        lanes.of[t].clear();
        let what = if i == 0 { What::Machine(kind) } else { What::Covered { origin: at as u32 } };
        grid.place(t, Building::new(what, dir));
    }
    true
}

/// Which way items arrive at each tile, worked out again when something has been built.
///
/// It runs after the clicks and before the factory steps, because a belt laid this frame is a
/// corner for its neighbour this frame.
pub fn follow_the_flow(grid: Res<Grid>, mut flow: ResMut<Flow>) {
    if flow.seen != grid.changes || flow.came_in.len() != (grid.tiles * grid.tiles) as usize {
        flow.refresh(&grid);
    }
}
