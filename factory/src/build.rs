//! **Building and unbuilding**: the keys that choose what is in hand and the click that puts it
//! down.
//!
//! F1 has no window furniture at all — the editor, the HUD and the guide are F5 — so what is in
//! hand is said in the log and in `docs/factory.md`, and the checks forge the clicks rather than
//! press anything. The one rule that is not "put it where the mouse was" is that **a miner has to
//! stand on ore**, which is what makes a patch worth finding.

use bevy::prelude::*;
use games_shell::camera::WorldClick;

use crate::belts::Lanes;
use crate::grid::{Building, Dir, Flow, Grid, Ore, What};
use crate::Map;

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
    pub fn word(&self) -> &'static str {
        match self.what {
            Some(what) => what.word(),
            None => "the wrecking ball",
        }
    }
}

/// **The keys.** `1` `2` `3` choose a belt, a miner and a chest, `0` chooses taking things away,
/// and `R` turns what is in hand a quarter turn anticlockwise.
pub fn choose(keys: Res<ButtonInput<KeyCode>>, mut hand: ResMut<Hand>) {
    let mut said = false;
    for (key, what) in [
        (KeyCode::Digit1, Some(What::Belt)),
        (KeyCode::Digit2, Some(What::Miner)),
        (KeyCode::Digit3, Some(What::Chest)),
        (KeyCode::Digit0, None),
    ] {
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
        info!("in hand: {} facing {}", hand.word(), hand.dir.word());
    }
}

/// **The click.** One tile, one building — or one tile emptied.
///
/// The items on a belt that is taken away go with it. They have to: a lane belongs to its tile,
/// and leaving them there would mean a tile with no belt on it carrying things.
pub fn clicks(
    mut clicked: MessageReader<WorldClick>,
    map: Res<Map>,
    hand: Res<Hand>,
    ore: Res<Ore>,
    mut grid: ResMut<Grid>,
    mut lanes: ResMut<Lanes>,
) {
    for click in clicked.read() {
        if click.button != MouseButton::Left {
            continue;
        }
        let Some(tile) = map.tile_at(click.at) else {
            info!("clicked {:.1}, {:.1} — off the map", click.at.x, click.at.y);
            continue;
        };
        let at = grid.index(tile);
        match hand.what {
            None => match grid.remove(at) {
                Some(gone) => {
                    lanes.of[at].clear();
                    info!("took away the {} at {}, {}", gone.what.word(), tile.x, tile.y);
                }
                None => info!("nothing at {}, {} to take away", tile.x, tile.y),
            },
            // **a miner has to stand on ore**, which is the whole reason a patch is somewhere in
            // particular rather than everywhere
            Some(What::Miner) if ore.left[at] == 0 => {
                info!("no ore at {}, {}: a miner needs some", tile.x, tile.y);
            }
            Some(what) => {
                lanes.of[at].clear();
                grid.place(at, Building::new(what, hand.dir));
                info!("built a {} at {}, {} facing {}", what.word(), tile.x, tile.y, hand.dir.word());
            }
        }
    }
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
