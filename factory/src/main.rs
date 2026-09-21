//! **Factory** — a small top-down factory, the third sample of [rubevy].
//!
//! The plan is `docs/plans/factory-plan.md`. F0 was the skeleton and one question — *can Bevy's
//! own `TilemapChunk` draw in a browser?* — and **F1 is the factory itself, in Rust, with no Ruby
//! in it at all**: a grid, building and unbuilding with the mouse, conveyors that carry and jam
//! and merge, ore in the ground, miners that dig it and chests that hold it. The data stage, the
//! control stage and the inserters' Ruby are F2 onwards.
//!
//! **Where everything is.** This file is the wiring: the arguments, the settings, which systems
//! run in which order, and the checks. The factory is elsewhere and none of it knows about Bevy's
//! renderer, which is what lets a run with no window have the whole thing in it:
//!
//! | file | what is in it |
//! |---|---|
//! | `src/grid.rs` | the tiles, what is built on them, the ore, and which way an item arrives |
//! | `src/belts.rs` | **the rule**: one step of the whole factory, and the tests of it |
//! | `src/items.rs` | where an item lives, and the measurement that settled it |
//! | `src/build.rs` | the keys and the click |
//! | `src/draw.rs` | the two chunks, the sprites, and the zoom |
//!
//! **Where the numbers are.** Every number the factory is played by is a line of
//! `factory.settings.txt` ([`belts::Rules`]), and F2 moves the ones that are numbers of *play*
//! into `ruby/data.rb`, which is what the data stage is for. The `const`s in this file are the
//! names of the defaults and two invariants: the pack's 16 px and how many tiles the sheet has.
//! `docs/numbers.md` §9 has the table, with where each default came from.
//!
//! [rubevy]: https://github.com/sabiruby/rubevy

mod belts;
mod build;
mod draw;
mod grid;
mod items;
mod platform;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::sprite_render::{TilemapChunk, TilemapChunkTileData};
use games_shell::camera::{CameraControls, CameraPlugin, CameraSet, WorldClick};
use rubevy::RubevyPlugin;

use belts::{Lanes, Rules};
use build::Hand;
use grid::{Building, Dir, Flow, Grid, Ore, What};
use items::Tally;

// ---------------------------------------------------------------------------------------------
// The two numbers that are not settings
// ---------------------------------------------------------------------------------------------

/// **How big one tile of the art is.** Kenney's Tiny Factory is 16 px × 16 px
/// (`factory/art/Tilesheet-tiny-factory.txt`: "Tile size • 16px × 16px"). It is not a setting:
/// another number here does not draw the same picture differently, it cuts the sheet in the
/// wrong places.
pub const TILE_PX: u32 = 16;

/// **How many tiles the sheet has.** The sheet is a vertical strip of 16 px squares built by
/// `tools/factory-tileset.py`: Kenney's 132 in the pack's own order, the four
/// `tools/factory-belts.py` draws (a straight and a corner seen from straight above, two frames
/// each — the style the author chose), the two `tools/factory-ore.py` draws, and a blank.
///
/// **The blank is not decoration.** wgpu's OpenGL backend guesses a texture's bind target from
/// its shape, and a `D2` texture with square layers and a count that is a multiple of six is
/// guessed to be a *cube map array* (`wgpu-hal-29.0.4/src/gles/mod.rs:458`). Kenney's 132 square
/// tiles are 6 × 22, so a browser bound the tileset as `TEXTURE_CUBE_MAP_ARRAY`, which WebGL2
/// does not have, and drew a black page with no page error at all — while the same code drew the
/// floor correctly on a PC, where the backend is Vulkan. F1 added two ore tiles, which took the
/// count to 138 and straight back onto a multiple of six, and the script put a blank on the end:
/// **this is the trap working as designed**, and the check below reads the count back out of the
/// loaded image so that it cannot stop working quietly.
const TILESET_LAYERS: u32 = 139;

// ---------------------------------------------------------------------------------------------
// The defaults, every one of which `factory.settings.txt` can move
// ---------------------------------------------------------------------------------------------

/// **How big the map is, in tiles each way. Still F0's provisional value**, and F1 could not
/// settle it honestly — the plan (§3.7) says it comes from "how many machines have to fit", and
/// two of the three things that would say are not known yet:
///
/// * **what the drawing can carry.** F1 measured the factory's own step at 16,000 items (0.3 ms,
///   `docs/worklog/2026-09-21-factory-F1.md` §3), which is nowhere near a frame; what runs out
///   first is the picture. Both renderers on the machine this was written on are **software**
///   (lavapipe in the container, SwiftShader in the browser) and neither says anything about a
///   real one: a browser there drew 900 belts at the same five frames a second with two items on
///   them as with eighteen hundred.
/// * **how many scripted inserters a frame holds**, which is F3's measurement.
///
/// So it stays where F0 put it, with the reason written down rather than a number invented to
/// replace it (`docs/numbers.md` §9.2). A stress run sizes its own map and ignores this.
const MAP_TILES: f32 = 32.0;

/// **Half of how much world the window holds, top to bottom, in world units** — and one world
/// unit is one pixel of the art. Derived: F0's provisional 128 was 3.515625 screen pixels to one
/// pixel of the art in the default 900 px window, and a zoom that is not a whole number is what
/// makes the seams between tiles shimmer. This is that view at the **whole-number zoom below it**
/// — 900 / (2 × 3), 18¾ tiles from top to bottom rather than F0's 16 — because between the two
/// whole numbers either side of it, the one that shows more of the factory is the one to have.
/// [`draw::snap_zoom`] keeps it whole from there on.
const CAMERA_HALF_HEIGHT: f32 = 150.0;

/// The window the game opens. **The same as the other two games here open**, and the number
/// itself has no recorded source in either of them (`sabibots/src/main.rs`: "Source unknown").
/// It is a setting, so nobody has to accept it.
const WINDOW: [f32; 2] = [1600.0, 900.0];

/// `--headless` with no number, and `--shot` with no file or seconds.
///
/// **The headless default is an upper bound and not a wait.** The checks wait for conditions and
/// end the run the moment they are all answered; the longest of them waits for a miner to dig and
/// a belt to carry four tiles, which is `mine_seconds + 4 ÷ belt_tiles_per_second` of the game's
/// own time — 3.0 s with the defaults, and 2.3 to 2.4 s measured. Ten seconds is three times that
/// and a bit.
const HEADLESS_SECONDS: f32 = 10.0;
const SHOT_FILE: &str = "shot.png";
/// `--shot` with no seconds. The picture wants the factory working, not only laid out: the checks'
/// little line takes 3.0 s of the game's own time to put its first item in its chest (above), so
/// this is just past it.
const SHOT_SECONDS: f32 = 3.5;

/// **How long the checks wait for the tileset to arrive, in frames.** They wait for the thing
/// itself rather than for a number of seconds (S7's rule), but a wait with no end is a check that
/// can never fail, and in a page there is no run ending underneath it to stop it. The bound is in
/// frames because what is being waited for is the asset server getting a turn, which is once a
/// frame. Measured: the tileset was in `Assets<Image>` on **frame 3** in the container and
/// **frame 4** in the browser, so this is a hundred times the worst of the two.
const TILESET_WAIT_FRAMES: u32 = 400;

/// How far the checks are allowed past the time the game's own numbers say their little factory
/// needs, before they call it stuck. Two, for a frame's granularity at each end and for a browser
/// whose frames are not a sixtieth of a second.
const CHECK_SLACK: f32 = 2.0;

// ---------------------------------------------------------------------------------------------
// The map
// ---------------------------------------------------------------------------------------------

/// **The grid, and the arithmetic between it and the world.** A tile is `(column, row)` with
/// `(0, 0)` at the bottom left, which is how [`TilemapChunk`] itself counts
/// (`TilemapChunk::calculate_tile_transform`, and the shader flips the row for the picture), so
/// there is one convention here and not two.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Map {
    /// How many tiles across and down. Square, for now.
    pub tiles: u32,
}

impl Map {
    /// How wide the whole map is, in world units.
    fn span(&self) -> f32 {
        (self.tiles * TILE_PX) as f32
    }

    /// **The tile a world point is in**, or `None` if the point is off the map. The map is
    /// centred on the origin, which is where the chunk puts itself.
    fn tile_at(&self, world: Vec2) -> Option<UVec2> {
        let half = self.span() / 2.0;
        let col = ((world.x + half) / TILE_PX as f32).floor();
        let row = ((world.y + half) / TILE_PX as f32).floor();
        let last = self.tiles as f32;
        (col >= 0.0 && col < last && row >= 0.0 && row < last)
            .then(|| UVec2::new(col as u32, row as u32))
    }

    /// **The middle of a tile, in world units.** The same arithmetic
    /// [`TilemapChunk::calculate_tile_transform`] does, written here because the game needs it
    /// before anything has been spawned.
    pub fn tile_centre(&self, tile: UVec2) -> Vec2 {
        let half = self.span() / 2.0;
        Vec2::new(
            tile.x as f32 * TILE_PX as f32 + TILE_PX as f32 / 2.0 - half,
            tile.y as f32 * TILE_PX as f32 + TILE_PX as f32 / 2.0 - half,
        )
    }
}

/// `--headless`: when the run gives up waiting for its checks.
#[derive(Resource)]
struct Headless {
    until: f32,
}

/// `--shot FILE SECONDS`: a window, one picture of it, and out.
#[derive(Resource)]
struct Shot {
    path: String,
    after: f32,
    taken: bool,
}

/// **The measurement the plan asks F1 for** (§3.1, §3.7): a map laid out as one long snake of
/// belt, filled with this many items, and the frame times printed.
///
/// `--stress N` on a PC, `FACTORY_STRESS=N` in a shell, `?stress=N` in a page — the last of the
/// three is why it is a number the checks' own reader understands rather than an argument: a page
/// has no command line, and a measurement that cannot be taken in a browser is not a measurement
/// of the browser. It stays after F1 has used it: the next stage to wonder what a number costs
/// can lay a loop and watch it rather than arguing.
#[derive(Resource, Debug)]
struct Stress {
    items: usize,
    /// Frames to watch before saying anything, and again between each saying.
    every: u32,
    seen: Vec<f32>,
    steps: Vec<f32>,
    said: u32,
}

/// How many times the stress run reports before it stops. Three, so that the first one — which
/// has the window opening and the assets loading in it — can be thrown away and the other two
/// compared with each other.
const STRESS_REPORTS: u32 = 3;

// ---------------------------------------------------------------------------------------------

fn main() {
    let ruby = platform::ruby_dir();
    let args = games_shell::Args::from_env();
    // The store, read before either branch: the window's size comes out of it, and so do the
    // flags' own defaults, so a headless run reads it too.
    let (settings, _lang) = games_shell::remembered(
        platform::SETTINGS_FILE,
        "Factory: what the game remembers. Delete a line for the default.",
        platform::read,
        platform::write,
        args.value("--lang").as_deref(),
    );

    let mut map = Map { tiles: settings.number("map_tiles").unwrap_or(MAP_TILES).max(8.0) as u32 };
    let half_height = positive(&settings, "camera_half_height", CAMERA_HALF_HEIGHT);
    let window = [
        settings.number("window_width").unwrap_or(WINDOW[0]),
        settings.number("window_height").unwrap_or(WINDOW[1]),
    ];
    let headless = args.headless(settings.number("headless_seconds").unwrap_or(HEADLESS_SECONDS));
    let shot = args.shot(
        settings.get("shot_file").unwrap_or(SHOT_FILE),
        settings.number("shot_seconds").unwrap_or(SHOT_SECONDS),
    );
    let rules = Rules {
        belt_tiles_per_second: positive(&settings, "belt_tiles_per_second", 2.0),
        items_per_tile: positive(&settings, "items_per_tile", 2.0),
        mine_seconds: positive(&settings, "mine_seconds", 1.0),
        chest_capacity: counted(&settings, "chest_capacity", 60.0),
        ore_per_tile: counted(&settings, "ore_per_tile", 60.0),
        ore_patch_radius: positive(&settings, "ore_patch_radius", 3.0),
    };
    let stress = args
        .value("--stress")
        .and_then(|n| n.parse::<f32>().ok())
        .or_else(|| games_shell::checks::asked_number("FACTORY_STRESS"))
        .or_else(|| settings.number("stress_items"))
        .unwrap_or(0.0)
        .max(0.0) as usize;
    if stress > 0 {
        // **A stress run sizes its own map.** The loop it lays holds `items_per_tile` to a tile,
        // so the map it needs is the square root of the items asked for — worked out here rather
        // than left to whoever runs it, because a page has no way of setting `map_tiles` and a
        // measurement that cannot be taken in a browser is not a measurement of the browser.
        let side = (stress as f32 / rules.items_per_tile.max(0.001)).sqrt().ceil() as u32;
        // an even number of rows, so that the serpentine comes home (`lay_the_snake`)
        map.tiles = map.tiles.max(side + side % 2 + 2);
    }

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            // No renderer, the same factory. There is no image loader without `ImagePlugin`, so
            // the tileset is never fetched here and the checks that are about the picture say so
            // rather than waiting for something that cannot come.
            app.add_plugins((
                MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    core::time::Duration::from_secs_f32(1.0 / 60.0),
                )),
                bevy::log::LogPlugin { filter: "info,bevy_asset=off".into(), ..default() },
                bevy::asset::AssetPlugin {
                    file_path: platform::assets_dir(),
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                },
                RubevyPlugin::default(),
            ))
            .init_asset::<Image>()
            // **The click still exists here**, it is just that nobody writes one: `WorldClick`
            // is registered by `CameraPlugin`, which a run with no window does not add, and
            // without the message the reader below would not even start. Registering it keeps
            // one code path for the click in both modes — which is what lets the checks forge
            // one and build a factory where there is no mouse.
            .add_message::<WorldClick>()
            .insert_resource(Headless { until: seconds })
            .add_systems(Update, stop_when_over);
        }
        None => {
            app.add_plugins((
                DefaultPlugins
                    .set(AssetPlugin {
                        file_path: platform::assets_dir(),
                        // the tiles have no .meta files; in the browser each would be a 404
                        meta_check: AssetMetaCheck::Never,
                        ..default()
                    })
                    // **Pixel art is not filtered.** Bevy's default is linear, which turns a
                    // 16 px tile blown up three times into a smear.
                    .set(ImagePlugin::default_nearest())
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "Factory".into(),
                            resolution: (window[0].max(1.0) as u32, window[1].max(1.0) as u32)
                                .into(),
                            // in the browser: the page's canvas, as large as its box
                            canvas: Some("#factory".into()),
                            fit_canvas_to_parent: true,
                            ..default()
                        }),
                        ..default()
                    }),
                // The camera the player drives (S3). The world has an edge and this crate could
                // not have guessed where, so the game says: the map, and no further.
                CameraPlugin::showing(half_height).with(CameraControls {
                    bounds: Some(Rect::from_center_half_size(
                        Vec2::ZERO,
                        Vec2::splat(map.span() / 2.0),
                    )),
                    ..default()
                }),
                RubevyPlugin::default(),
            ))
            .init_resource::<items::Pool>()
            .init_resource::<draw::Animation>()
            .insert_resource(draw::SnapZoom(
                settings.number("camera_snap_zoom").unwrap_or(1.0) != 0.0,
            ))
            .add_systems(Startup, draw::start_drawing)
            // **the keys are the window's**: a run with no window has no `ButtonInput` at all
            // (it is `InputPlugin`'s, and `MinimalPlugins` is not that), and the checks work the
            // hand directly rather than pressing anything
            .add_systems(Update, build::choose.before(build::clicks))
            .add_systems(
                Update,
                (draw::draw_floor, draw::draw_buildings).after(FactorySet::Step),
            )
            .add_systems(Update, draw::snap_zoom.after(CameraSet::Drive));
            app.add_systems(Update, draw::draw_items.after(FactorySet::Step));
        }
    }

    app.insert_resource(map)
        .insert_resource(settings)
        .insert_resource(rules)
        .init_resource::<Hand>()
        .init_resource::<Flow>()
        .init_resource::<Tally>()
        .add_systems(Startup, lay_the_land)
        .add_systems(
            Update,
            (build::clicks, build::follow_the_flow).chain().before(FactorySet::Step),
        );
    app.add_systems(Update, items::run_the_factory.in_set(FactorySet::Step));
    if stress > 0 {
        app.insert_resource(Stress {
            items: stress,
            // a second of frames at a sixtieth each, which is long enough for the numbers to
            // stop being the first frame's and short enough to see three of them in a run
            every: 60,
            seen: Vec::new(),
            steps: Vec::new(),
            said: 0,
        })
        .add_systems(Startup, lay_the_snake.after(lay_the_land))
        .add_systems(Update, watch_the_frames.after(FactorySet::Step));
    }

    if platform::selftest_asked() {
        app.init_resource::<SelfTest>()
            .add_systems(Update, selftest.before(FactorySet::Step));
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    // F2 onwards reads this; F1 only says where it is, so that a run in the wrong directory says
    // so now rather than in a stage's time.
    if !ruby.is_dir() && platform::RUBY_FILES.is_empty() {
        warn!("no Ruby at {ruby:?}: nothing to run there yet, but F2 will want it");
    }
    app.run();
}

/// **A setting that has to be more than zero**, and the reason there is no floor inside the
/// arithmetic that uses one ([`belts::Rules`]). A speed of zero is not a slow belt, it is a
/// division; a gap of zero is not a crowded tile, it is every item in the same place. There is no
/// sensible number to clamp such a setting to — the smallest belt speed that still means anything
/// is not something anybody measured — so the store is told it is wrong and the default stands.
fn positive(settings: &games_shell::Settings, key: &str, default: f32) -> f32 {
    match settings.number(key) {
        Some(n) if n > 0.0 => n,
        Some(wrong) => {
            warn!("{key} = {wrong} is not more than zero; using {default}");
            default
        }
        None => default,
    }
}

/// The same, for a setting that counts **things**. Items are whole and there is at least one of
/// them, which is not a threshold anybody chose: half an item in a chest is not a smaller chest.
fn counted(settings: &games_shell::Settings, key: &str, default: f32) -> u32 {
    let asked = positive(settings, key, default).round();
    if asked < 1.0 {
        warn!("{key} counts items, so it cannot round to less than one; using {default}");
        return default as u32;
    }
    asked as u32
}

/// Where one step of the factory happens, so that everything else can say whether it is before or
/// after it. Building is before (a belt laid this frame carries this frame) and the picture is
/// after (what is drawn is where things are now).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactorySet {
    Step,
}

/// **The world, before anything is built on it**: the grid, the ore and the empty lanes.
fn lay_the_land(mut commands: Commands, map: Res<Map>, rules: Res<Rules>) {
    let ore = Ore::laid_out(map.tiles, rules.ore_patch_radius, rules.ore_per_tile);
    info!(
        "a map of {} by {} tiles, {} of them with ore in ({} in the ground); a belt carries {} items a second",
        map.tiles,
        map.tiles,
        ore.tiles_with_ore(),
        ore.total(),
        rules.belt_items_per_second(),
    );
    info!("keys: 1 belt, 2 miner, 3 chest, 0 take away, R turn; click to build");
    commands.insert_resource(Grid::new(map.tiles));
    commands.insert_resource(Lanes::for_map(map.tiles));
    commands.insert_resource(ore);
}

/// **The stress test's factory**: nearly the whole map laid as **one closed loop of belt**, with
/// the items asked for spread evenly round it.
///
/// A loop rather than a line, because a line ends at a wall and a factory that has jammed is the
/// *cheap* case: nothing hands anything on. Round a loop with room in it every tile carries and
/// every join is crossed, which is the case the measurement is about. It is laid as a serpentine
/// — east along the bottom, then back and forth across the rows above it — with the left-hand
/// column kept as the way home.
fn lay_the_snake(
    stress: Res<Stress>,
    rules: Res<Rules>,
    mut grid: ResMut<Grid>,
    mut lanes: ResMut<Lanes>,
) {
    // the inside of the map, 1..=n each way, and an even number of rows so that the serpentine
    // comes back to the left-hand column rather than to the right
    let n = grid.tiles - 2;
    let rows = if n % 2 == 0 { n } else { n - 1 };
    let lay = |grid: &mut Grid, x: u32, y: u32, dir: Dir| {
        let at = grid.index(UVec2::new(x, y));
        grid.place(at, Building::new(What::Belt, dir));
    };
    // the bottom row, east, turning north at the far end
    for x in 1..=n {
        lay(&mut grid, x, 1, if x == n { Dir::North } else { Dir::East });
    }
    // the rows above it, back and forth over columns 2..=n
    for y in 2..=rows {
        for x in 2..=n {
            let dir = match (y % 2 == 0, x) {
                // west, turning north at column 2 — except on the top row, which carries on west
                // into the column that goes home
                (true, 2) if y < rows => Dir::North,
                (true, _) => Dir::West,
                (false, _) if x == n => Dir::North,
                (false, _) => Dir::East,
            };
            lay(&mut grid, x, y, dir);
        }
    }
    // the way home: the left-hand column, south, into the bottom row
    for y in 2..=rows {
        lay(&mut grid, 1, y, Dir::South);
    }

    // **spread, not piled up**: items nose to tail in one place would be a jam with an empty
    // loop in front of it, and a jam is the case this is not measuring
    let spacing = rules.spacing();
    let belts = grid.built().len();
    let most = (belts as f32 * rules.items_per_tile) as usize;
    let wanted = stress.items.min(most);
    let mut laid = 0usize;
    for (i, &t) in grid.built().iter().enumerate() {
        // how many this tile gets, so that the remainder is spread rather than all at the end
        let upto = (wanted * (i + 1)) / belts.max(1);
        let mut along = 1.0;
        while laid < upto {
            lanes.of[t as usize].push_back(along);
            along -= spacing;
            laid += 1;
        }
    }
    info!(
        "stress: a loop of {} belts, {} items asked for, {} laid ({} is the most they hold)",
        belts, stress.items, laid, most
    );
}

/// The stress run's own report: the frame times and what the factory's own step took inside them.
fn watch_the_frames(
    time: Res<Time<Real>>,
    tally: Res<Tally>,
    grid: Res<Grid>,
    mut stress: ResMut<Stress>,
    mut exit: MessageWriter<AppExit>,
) {
    stress.seen.push(time.delta_secs() * 1000.0);
    let step = tally.last_step_us;
    stress.steps.push(step);
    if stress.seen.len() < stress.every as usize {
        return;
    }
    let frame = spread(&mut stress.seen);
    let inner = spread(&mut stress.steps);
    info!(
        "stress: items={} belts={} frame ms p50 {:.2} p95 {:.2} (about {:.0} fps) | step us p50 {:.0} p95 {:.0}",
        tally.items,
        grid.built().len(),
        frame.0,
        frame.1,
        1000.0 / frame.0.max(0.001),
        inner.0,
        inner.1,
    );
    stress.seen.clear();
    stress.steps.clear();
    stress.said += 1;
    if stress.said >= STRESS_REPORTS && platform::CHECKS_EXIT_WHEN_DONE {
        exit.write(AppExit::Success);
    }
}

/// The middle and the ninety-fifth of a list of samples. It sorts the list, which is why it takes
/// it by `&mut`: the caller is about to throw it away.
fn spread(samples: &mut [f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    samples.sort_by(f32::total_cmp);
    let at = |q: f32| samples[((samples.len() as f32 - 1.0) * q).round() as usize];
    (at(0.5), at(0.95))
}

fn take_shot(
    mut commands: Commands,
    time: Res<Time>,
    mut shot: ResMut<Shot>,
    mut exit: MessageWriter<AppExit>,
) {
    if shot.taken {
        if time.elapsed_secs() > shot.after + 1.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if time.elapsed_secs() < shot.after {
        return;
    }
    shot.taken = true;
    let path = shot.path.clone();
    info!("screenshot -> {path}");
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}

/// `--headless N`: the bound on a run whose checks never answered. A run that finished them has
/// already exited (`games_shell::checks::CHECKS_EXIT_WHEN_DONE`).
fn stop_when_over(time: Res<Time>, headless: Res<Headless>, mut exit: MessageWriter<AppExit>) {
    if time.elapsed_secs() >= headless.until {
        info!("headless: {:.1} s, done", time.elapsed_secs());
        exit.write(AppExit::Success);
    }
}

// ---------------------------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------------------------

/// `FACTORY_SELFTEST=1` on a PC, `factory/?selftest` in a page. One line per thing proved, in the
/// shape the other two games print and `tools/fixedlines.sh` compares
/// (`docs/verification/selftest-lines.md`).
///
/// **It builds its factory with clicks**, because that is the road a player takes and the only
/// road there is: the checks write a `WorldClick` and the same system that answers a mouse
/// answers them. A run with no window has no mouse and this is the whole of why the message is
/// registered there too.
#[derive(Resource, Default)]
struct SelfTest {
    step: u8,
    frames: u32,
    /// where the little factory was built, so that what is said about it can name the tiles
    line: Vec<UVec2>,
    /// what the game's own numbers say the line needs, in seconds
    needs: f32,
    /// when the line was finished, on the game's clock
    started: f32,
    /// how much ore was in the ground when it started
    ore_before: u64,
    done: bool,
}

/// One line, in the shape the other two games print and `tools/fixedlines.sh` matches: the
/// verdict padded to four columns, one space, the sentence. `ok  `, `FAIL`, `--  `.
fn say(verdict: &str, what: &str) {
    info!("selftest: {verdict} {what}");
}

/// **Why this is a few steps and not one:** half of what is being checked is about something
/// arriving — the tileset, an item at the end of a belt — and each is waited for by asking
/// whether it is there yet, not by sleeping. The two bounds are derived: the tileset's is
/// [`TILESET_WAIT_FRAMES`] frames, because what is waited on happens once a frame, and the
/// factory's is the time its own numbers say the work takes, times [`CHECK_SLACK`].
#[allow(clippy::too_many_arguments)]
fn selftest(
    mut test: ResMut<SelfTest>,
    time: Res<Time>,
    map: Res<Map>,
    rules: Res<Rules>,
    grid: Res<Grid>,
    ore: Res<Ore>,
    lanes: Res<Lanes>,
    tally: Res<Tally>,
    chunks: Query<(&TilemapChunk, &TilemapChunkTileData)>,
    images: Res<Assets<Image>>,
    mut hand: ResMut<Hand>,
    shot: Option<Res<Shot>>,
    mut clicks: MessageWriter<WorldClick>,
    mut exit: MessageWriter<AppExit>,
) {
    if test.done {
        return;
    }
    test.frames += 1;
    match test.step {
        // ---- the world was laid out ---------------------------------------------------------
        0 => {
            let patches = ore.tiles_with_ore();
            let ok = patches > 0 && ore.total() == patches as u64 * rules.ore_per_tile as u64;
            say(
                if ok { "ok  " } else { "FAIL" },
                &format!(
                    "the map is {} by {} tiles with {} of them holding {} of ore",
                    map.tiles,
                    map.tiles,
                    patches,
                    ore.total()
                ),
            );
            test.step = 1;
        }
        // ---- the arithmetic the building is built on -----------------------------------------
        1 => {
            let last_tile = map.tiles - 1;
            let corners = [
                UVec2::new(0, 0),
                UVec2::new(last_tile, 0),
                UVec2::new(0, last_tile),
                UVec2::new(last_tile, last_tile),
                UVec2::splat(map.tiles / 2),
            ];
            let agreed =
                corners.iter().filter(|&&t| map.tile_at(map.tile_centre(t)) == Some(t)).count();
            let off = map.tile_at(Vec2::splat(map.span())).is_none();
            say(
                if agreed == corners.len() && off { "ok  " } else { "FAIL" },
                &format!(
                    "a world point and the tile it is in agree at the corners and the middle ({}/{}), and a point off the map is off it",
                    agreed,
                    corners.len()
                ),
            );
            test.step = 2;
        }
        // ---- the tileset arrived, cut into layers ------------------------------------------
        2 => {
            // A headless run spawns no chunk and has no image loader, so there is nothing here to
            // measure and saying `ok` would be claiming a check that never ran. It is known on
            // the first frame — there is no chunk and there never will be — so that case says so
            // at once rather than sitting out the bound below, which is for a chunk whose picture
            // has not arrived *yet*.
            let Some((chunk, _)) = chunks.iter().next() else {
                say("--  ", "no floor was drawn (this run has no renderer)");
                test.step = 3;
                return;
            };
            match images.get(&chunk.tileset) {
                Some(image) => {
                    let size = image.texture_descriptor.size;
                    let layers = size.depth_or_array_layers;
                    // the count as well as the size: it is what keeps the sheet off a
                    // multiple of six, which is what a browser draws a black page over
                    let ok = layers == TILESET_LAYERS
                        && layers % 6 != 0
                        && size.width == TILE_PX
                        && size.height == TILE_PX;
                    say(
                        if ok { "ok  " } else { "FAIL" },
                        &format!(
                            "the tileset arrived as an array of {} layers of {} by {} px (frame {})",
                            layers, size.width, size.height, test.frames
                        ),
                    );
                    test.step = 3;
                }
                // Nothing yet — wait another frame, up to the bound. Running out of it is a
                // **FAIL** and not a `--`: a chunk was spawned, so the picture was asked for and
                // did not come.
                None if test.frames >= TILESET_WAIT_FRAMES => {
                    say(
                        "FAIL",
                        &format!("the tileset never arrived ({TILESET_WAIT_FRAMES} frames)"),
                    );
                    test.step = 3;
                }
                None => {}
            }
        }
        // ---- a miner cannot stand anywhere but on ore -----------------------------------------
        3 => {
            // the middle of the map, which `Ore::laid_out` leaves bare on purpose
            let bare = UVec2::splat(map.tiles / 2);
            hand.what = Some(What::Miner);
            hand.dir = Dir::East;
            clicks.write(click_on(&map, bare));
            test.line = vec![bare];
            test.step = 4;
        }
        4 => {
            let bare = test.line[0];
            let at = grid.index(bare);
            say(
                if grid.at(at).is_none() && ore.left[at] == 0 { "ok  " } else { "FAIL" },
                &format!("a miner cannot be built where there is no ore ({}, {})", bare.x, bare.y),
            );
            test.step = 5;
        }
        // ---- the line: a miner on ore, belts, a chest -------------------------------------
        5 => {
            // the middle of the first patch of ore, which is where `Ore::laid_out` puts one
            let pit = UVec2::splat(map.tiles / 4);
            test.line = vec![pit];
            hand.what = Some(What::Miner);
            hand.dir = Dir::East;
            clicks.write(click_on(&map, pit));
            test.ore_before = ore.total();
            test.step = 6;
        }
        6 | 7 | 8 => {
            // three belts, one a frame, running east from the miner
            let next = UVec2::new(test.line[0].x + test.step as u32 - 5, test.line[0].y);
            hand.what = Some(What::Belt);
            hand.dir = Dir::East;
            clicks.write(click_on(&map, next));
            test.line.push(next);
            test.step += 1;
        }
        9 => {
            let chest = UVec2::new(test.line[0].x + 4, test.line[0].y);
            hand.what = Some(What::Chest);
            clicks.write(click_on(&map, chest));
            test.line.push(chest);
            // **what the game's own numbers say this takes**: one dig, then three tiles of belt
            // and the step into the chest
            test.needs = rules.mine_seconds + 4.0 / rules.belt_tiles_per_second;
            test.started = time.elapsed_secs();
            test.step = 10;
        }
        10 => {
            let built: Vec<&str> = test
                .line
                .iter()
                .filter_map(|&t| grid.at(grid.index(t)).map(|b| b.what.word()))
                .collect();
            say(
                if built.len() == test.line.len() { "ok  " } else { "FAIL" },
                &format!("a click built each of the {} things the line needs ({})", test.line.len(), built.join(", ")),
            );
            test.step = 11;
        }
        // ---- it works: the chest fills ---------------------------------------------------------
        11 => {
            let chest = *test.line.last().unwrap();
            let held = grid.at(grid.index(chest)).map(|b| b.held).unwrap_or(0);
            let waited = time.elapsed_secs() - test.started;
            if held == 0 && waited < test.needs * CHECK_SLACK {
                return;
            }
            say(
                if held > 0 { "ok  " } else { "FAIL" },
                &format!(
                    "the miner dug, the belts carried and the chest holds {} after {:.1} s (the numbers say {:.1} s)",
                    held, waited, test.needs
                ),
            );
            test.step = 12;
        }
        // ---- and nothing was made out of nothing ------------------------------------------------
        12 => {
            let dug = test.ore_before - ore.total();
            let in_chests: u64 = grid
                .built()
                .iter()
                .filter_map(|&t| grid.at(t as usize))
                .filter(|b| b.what == What::Chest)
                .map(|b| b.held as u64)
                .sum();
            let on_belts = tally.items as u64;
            say(
                if dug == in_chests + on_belts { "ok  " } else { "FAIL" },
                &format!(
                    "every item is either in a chest or on a belt: {} dug, {} held, {} carried",
                    dug, in_chests, on_belts
                ),
            );
            test.step = 13;
        }
        // ---- and a click takes it away again ------------------------------------------------
        13 => {
            let belt = test.line[1];
            hand.what = None;
            clicks.write(click_on(&map, belt));
            test.step = 14;
        }
        14 => {
            let belt = test.line[1];
            let at = grid.index(belt);
            say(
                if grid.at(at).is_none() && lanes.on(at).is_empty() { "ok  " } else { "FAIL" },
                &format!(
                    "a click with nothing in hand takes the belt at {}, {} away, and what was on it goes with it",
                    belt.x, belt.y
                ),
            );
            test.step = 15;
        }
        _ => {
            test.done = true;
            // **A run that was asked for a picture is not over when the checks are.** F0 noticed
            // that `--shot` and the checks could not be used together — the checks end the run
            // before the picture's moment comes — and it is the checks' little factory that is
            // worth a picture, so the one waiting for a camera wins.
            if platform::CHECKS_EXIT_WHEN_DONE && shot.is_none() {
                exit.write(AppExit::Success);
            } else {
                info!("selftest: done — the factory keeps running (a page has nothing to exit to)");
            }
        }
    }
}

/// A click in the middle of a tile, as the camera would have written it.
fn click_on(map: &Map, tile: UVec2) -> WorldClick {
    WorldClick { at: map.tile_centre(tile), button: MouseButton::Left, cursor: Vec2::ZERO }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one piece of arithmetic the building is built on. It is checked here as well as in the
    /// run because a run needs a window for one of its checks and this needs nothing.
    #[test]
    fn a_tiles_middle_is_in_that_tile() {
        let map = Map { tiles: 32 };
        for tile in [UVec2::ZERO, UVec2::new(31, 0), UVec2::new(0, 31), UVec2::new(31, 31), UVec2::new(8, 11)] {
            assert_eq!(map.tile_at(map.tile_centre(tile)), Some(tile), "tile {tile}");
        }
    }

    /// The corners belong to exactly one tile each: the bottom left of the map is tile (0, 0) and
    /// a hair below it is off the map, which is what keeps a click on the edge from placing
    /// something outside the world.
    #[test]
    fn the_edges_belong_to_one_tile_and_outside_is_outside() {
        let map = Map { tiles: 32 };
        let half = map.span() / 2.0;
        assert_eq!(map.tile_at(Vec2::new(-half, -half)), Some(UVec2::ZERO));
        assert_eq!(map.tile_at(Vec2::new(-half - 0.01, -half)), None);
        assert_eq!(map.tile_at(Vec2::new(half - 0.01, half - 0.01)), Some(UVec2::splat(31)));
        assert_eq!(map.tile_at(Vec2::new(half, half)), None, "the far edge is the next tile, which is not there");
    }

    /// The middle and the ninety-fifth, which the stress run's numbers are.
    #[test]
    fn the_spread_is_the_middle_and_the_ninety_fifth() {
        let mut one = [1.0f32];
        assert_eq!(spread(&mut one), (1.0, 1.0));
        let mut ten: Vec<f32> = (1..=100).map(|n| n as f32).collect();
        let (middle, high) = spread(&mut ten);
        assert_eq!(middle, 51.0, "the hundred numbers 1..100");
        assert_eq!(high, 95.0);
        assert_eq!(spread(&mut []), (0.0, 0.0));
    }
}
