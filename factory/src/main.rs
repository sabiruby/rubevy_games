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
mod data;
mod draw;
mod grid;
mod items;
mod machines;
mod platform;

use std::path::PathBuf;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::sprite_render::{TilemapChunk, TilemapChunkTileData};
use games_shell::camera::{CameraControls, CameraPlugin, CameraSet, WorldClick};
use rubevy::{RubevyPlugin, ScriptWorld};

use belts::{Lanes, OnBelt, Rules};
use build::Hand;
use data::Data;
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
/// **The count is the trap.** wgpu's OpenGL backend guesses a texture's bind target from its
/// shape, and a `D2` texture with square layers and a count that is a multiple of six is guessed
/// to be a *cube map array* (`wgpu-hal-29.0.4/src/gles/mod.rs:458`). Kenney's 132 square tiles are
/// 6 × 22, so a browser bound the tileset as `TEXTURE_CUBE_MAP_ARRAY`, which WebGL2 does not have,
/// and drew a black page with no page error at all — while the same code drew the floor correctly
/// on a PC, where the backend is Vulkan.
///
/// F1 added two ore tiles, which took the count to 138 and straight back onto a multiple of six,
/// and the script put a blank on the end: 139. **F2 added four** — the assembler's, two tiles by
/// two — and 142 is not a multiple of six, so the blank was not needed and is not there. The
/// script decides that; nobody edits it. The check below reads the count back out of the loaded
/// image so that it cannot stop working quietly.
const TILESET_LAYERS: u32 = 142;

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
/// end the run the moment they are all answered, and they build **two** little factories one
/// after the other:
///
/// * F1's — a miner, three belts and a chest — `mine_seconds + 4 ÷ belt_tiles_per_second`, which
///   is 3.0 s with what `data.rb` says today (measured: 2.3 to 2.4 s);
/// * F2's — a belt into a furnace into a belt into a chest, and the same for an assembler —
///   `2 ÷ belt + time ÷ speed + 1 ÷ belt`, which is 3.5 s for the furnace's.
///
/// Six and a half seconds of the game's own time, then, and twenty is three times it.
const HEADLESS_SECONDS: f32 = 20.0;
const SHOT_FILE: &str = "shot.png";
/// `--shot` with no seconds. The picture wants the factory working, not only laid out: the two
/// little factories above are finished by 6.5 s of the game's own time, so this is just past the
/// second of them.
const SHOT_SECONDS: f32 = 8.0;

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
    /// **How many items can be dug out of one tile of ore**, and **how wide a patch is**, in
    /// tiles. They are the map's rather than [`Rules`]'s because they are how the world is *laid
    /// out* before anybody plays it: nothing in one step of the factory reads them, `Ore::laid_out`
    /// reads them once, and the smallest map a patch fits on is derived from the second of them in
    /// `main` — **before there is a VM to have read any Ruby with**. That is the line between the
    /// numbers that stayed in `factory.settings.txt` and the four that moved into `ruby/data.rb`.
    pub ore_per_tile: u32,
    pub ore_patch_radius: f32,
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

    // **The floor under the map's size is derived and not chosen.** F1 wrote `.max(8.0)` and
    // could not say where the 8 came from; what says it is the ore, because a map too small for
    // the four patches to clear its border ring is a map the game cannot be played on
    // (`Ore::smallest_map`, which is 15 tiles at the default radius). It is read before the map
    // so that the map can be clamped by it, which is the whole reason `ore_patch_radius` stays a
    // setting rather than moving into `data.rb` with the rest of the numbers of play: this is
    // wanted here, before there is a VM to have read any Ruby.
    let ore_patch_radius = positive(&settings, "ore_patch_radius", 3.0);
    let smallest_map = Ore::smallest_map(ore_patch_radius);
    let map = Map {
        tiles: settings.number("map_tiles").unwrap_or(MAP_TILES).max(smallest_map as f32) as u32,
        ore_per_tile: counted(&settings, "ore_per_tile", 60.0),
        ore_patch_radius,
    };
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
    let stress = args
        .value("--stress")
        .and_then(|n| n.parse::<f32>().ok())
        .or_else(|| games_shell::checks::asked_number("FACTORY_STRESS"))
        .or_else(|| settings.number("stress_items"))
        .unwrap_or(0.0)
        .max(0.0) as usize;

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
            .add_systems(
                Startup,
                (draw::start_drawing.run_if(resource_exists::<Rules>), say_the_trouble)
                    .after(lay_the_land),
            )
            // **the keys are the window's**: a run with no window has no `ButtonInput` at all
            // (it is `InputPlugin`'s, and `MinimalPlugins` is not that), and the checks work the
            // hand directly rather than pressing anything
            .add_systems(Update, build::choose.before(build::clicks).run_if(the_factory_is_up))
            .add_systems(
                Update,
                (draw::draw_floor, draw::draw_buildings)
                    .after(FactorySet::Step)
                    .run_if(resource_exists::<draw::Chunks>),
            )
            .add_systems(Update, draw::snap_zoom.after(CameraSet::Drive));
            app.add_systems(
                Update,
                draw::draw_items.after(FactorySet::Step).run_if(resource_exists::<draw::Chunks>),
            );
        }
    }

    app.insert_resource(map)
        .insert_resource(settings)
        .insert_resource(RubyDir(ruby))
        .init_resource::<Hand>()
        .init_resource::<Flow>()
        .init_resource::<Tally>()
        // **The data stage is the first thing that happens**, and everything else in `Startup` is
        // after it — the world is not laid out, the chunks are not spawned and no `Update` runs
        // at all unless it gave the game a [`Rules`] and a [`Data`]. That is the Battle's rule
        // from S5b-2 (`Match::MODEL`: no default model in Rust, nothing starts until one arrives)
        // applied to a factory: **there are no numbers of play in this binary**.
        .add_systems(Startup, read_the_data_stage)
        .add_systems(Startup, lay_the_land.after(read_the_data_stage).run_if(resource_exists::<Rules>))
        .add_systems(
            Update,
            (build::clicks, build::follow_the_flow)
                .chain()
                .before(FactorySet::Step)
                .run_if(the_factory_is_up),
        );
    app.add_systems(
        Update,
        items::run_the_factory.in_set(FactorySet::Step).run_if(the_factory_is_up),
    );
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
        .add_systems(
            Startup,
            size_the_map_for_the_stress_run
                .after(read_the_data_stage)
                .before(lay_the_land)
                .run_if(resource_exists::<Rules>),
        )
        .add_systems(Startup, lay_the_snake.after(lay_the_land).run_if(the_factory_is_up))
        .add_systems(
            Update,
            watch_the_frames.after(FactorySet::Step).run_if(the_factory_is_up),
        );
    }

    if platform::selftest_asked() {
        app.init_resource::<SelfTest>()
            .add_systems(Update, selftest.before(FactorySet::Step).run_if(the_factory_is_up));
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    app.run();
}

/// Where `ruby/` is, for the one system that reads it.
#[derive(Resource, Debug)]
struct RubyDir(PathBuf);

/// **The data file**, and the name its errors are reported under — in a browser the compiler calls
/// every program `playground.rb`, and this is the name a player would recognise
/// (`crate::data::from_compiler`).
const DATA_FILE: &str = "data.rb";

/// **What a data file that will not do left behind.** The game does not start; a window says this
/// on the screen and every run says it in the log.
#[derive(Resource, Debug)]
struct DataTrouble(String);

/// Whether there is a factory at all — which there is not until the data stage has given the game
/// its tables and `lay_the_land` has made a grid out of them.
fn the_factory_is_up(grid: Option<Res<Grid>>) -> bool {
    grid.is_some()
}

/// **The data stage** (plan §3.2): `ruby/data.rb`, compiled and run in the VM rubevy keeps, and
/// its declarations collected into [`Data`] and [`Rules`] — all of it inside one `Startup`
/// system, so that **the tables are there before the first `Update`**.
///
/// The VM is `ScriptWorld::vm`, the one every script in this game will share (plan §2: one VM,
/// so that a constant the data stage defines is a constant the control stage can read). Nothing
/// here touches `Vm::set_host_state`, which is rubevy's (plan §5); `declare` keeps its tables in
/// the VM's host store, which is a different place.
fn read_the_data_stage(
    mut commands: Commands,
    ruby: Res<RubyDir>,
    mut world: ResMut<ScriptWorld>,
) {
    let file = ruby.0.join(DATA_FILE);
    let source = match platform::read(&file) {
        Ok(text) => text,
        Err(why) => {
            let says = format!("{DATA_FILE}: {why}");
            error!("the factory has no data: {says}");
            commands.insert_resource(DataTrouble(says));
            return;
        }
    };
    match data::read_the_declarations(&mut world.vm, DATA_FILE, &source, platform::compile) {
        Ok((tables, rules)) => {
            info!(
                "{DATA_FILE}: {} items, {} recipes, {} machines; a full belt carries {} items a second",
                tables.items.len(),
                tables.recipes.len(),
                tables.machines.len(),
                rules.belt_items_per_second(),
            );
            // and the other direction, for F3's inserters and F4's control stage
            data::expose_the_tables(&mut world.vm, &tables);
            commands.insert_resource(rules);
            commands.insert_resource(tables);
        }
        Err(trouble) => {
            let says = trouble.say(DATA_FILE);
            error!("the factory has no data: {says}");
            error!("fix {DATA_FILE} and start it again (F5 adds an editor and a reload)");
            commands.insert_resource(DataTrouble(says));
        }
    }
}

/// **A window with no factory in it says why.** One line of text in the middle of the screen, and
/// the same sentence the log has. There is no egui here until F5, and `bevy_ui`'s default font is
/// already in the build for the two games that have one.
fn say_the_trouble(mut commands: Commands, trouble: Option<Res<DataTrouble>>) {
    let Some(trouble) = trouble else { return };
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        },
        children![(
            Text::new(format!("{}\n\nfix it and start the game again", trouble.0)),
            TextFont { font_size: 20.0.into(), ..default() },
            TextColor(Color::srgb(1.0, 0.85, 0.6)),
        )],
    ));
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
fn lay_the_land(mut commands: Commands, map: Res<Map>, data: Res<Data>) {
    let ore = Ore::laid_out(map.tiles, map.ore_patch_radius, map.ore_per_tile);
    info!(
        "a map of {} by {} tiles, {} of them with ore in ({} in the ground)",
        map.tiles,
        map.tiles,
        ore.tiles_with_ore(),
        ore.total(),
    );
    let machines: Vec<String> = data
        .machines
        .iter()
        .enumerate()
        .map(|(i, m)| format!("{} {}", i + 4, m.name))
        .collect();
    info!(
        "keys: 1 belt, 2 miner, 3 chest, {}, 0 take away, R turn; click to build",
        machines.join(", ")
    );
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
            lanes.of[t as usize].push_back(OnBelt { along, item: rules.digs });
            along -= spacing;
            laid += 1;
        }
    }
    info!(
        "stress: a loop of {} belts, {} items asked for, {} laid ({} is the most they hold)",
        belts, stress.items, laid, most
    );
}

/// **A stress run sizes its own map**, once the data stage has said how many items fit on a tile.
///
/// The loop it lays holds `items_per_tile` to a tile, so the map it needs is the square root of
/// the items asked for. It is worked out here rather than left to whoever runs it, because a page
/// has no way of setting `map_tiles` and a measurement that cannot be taken in a browser is not a
/// measurement of the browser — and it is worked out *here*, in `Startup`, rather than in `main`,
/// because `items_per_tile` is `data.rb`'s now and `main` has no VM yet.
fn size_the_map_for_the_stress_run(
    stress: Res<Stress>,
    rules: Res<Rules>,
    mut map: ResMut<Map>,
    controls: Option<ResMut<CameraControls>>,
) {
    let side = (stress.items as f32 / rules.items_per_tile).sqrt().ceil() as u32;
    // an even number of rows, so that the serpentine comes home (`lay_the_snake`)
    let tiles = map.tiles.max(side + side % 2 + 2);
    if tiles == map.tiles {
        return;
    }
    map.tiles = tiles;
    // the camera was given the old map's edges in `main`; a bigger world needs bigger ones
    if let Some(mut controls) = controls {
        controls.bounds =
            Some(Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(map.span() / 2.0)));
    }
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
    /// F2's two little factories: where each machine is, the belt that feeds it, the chest it
    /// fills, what it is meant to make, and what the numbers say each takes
    machines: Vec<MachineCheck>,
    /// **What is left to build, one tile a frame.** A click is answered by a system that reads
    /// [`Hand`] when it runs, not when the click was written, so a frame that writes five clicks
    /// with five different things in hand builds five of the last one. F1 never noticed because
    /// it laid one belt a frame; this is the same thing said out loud.
    to_build: Vec<(UVec2, Option<What>)>,
    done: bool,
}

/// One of F2's two little factories: a belt into a machine into a belt into a chest.
#[derive(Debug, Clone)]
struct MachineCheck {
    what: String,
    makes: data::ItemId,
    made_of: String,
    chest: usize,
    /// what the recipe and the belt say this takes, in seconds
    needs: f32,
    /// when its chest first held one, on the game's clock — so that each line says **its own**
    /// time and not the time the slowest of them took
    done_at: Option<f32>,
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
    data: Res<Data>,
    grid: Res<Grid>,
    ore: Res<Ore>,
    mut lanes: ResMut<Lanes>,
    tally: Res<Tally>,
    chunks: Query<(&TilemapChunk, &TilemapChunkTileData)>,
    images: Res<Assets<Image>>,
    mut world: ResMut<ScriptWorld>,
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
        // ---- the data stage was done before this, the first `Update` --------------------------
        0 => {
            // **`test.frames` is 1 here**, which is the whole of this check: this system runs in
            // `Update`, `read_the_data_stage` runs in `Startup`, and the tables being readable on
            // the first frame is the plan's "the tables are there before the first `Update`". A
            // run where they were not never gets here at all — nothing in `Update` is added
            // without them (`the_factory_is_up`) — so the count is what is worth printing.
            let ready = test.frames == 1
                && !data.items.is_empty()
                && !data.recipes.is_empty()
                && !data.machines.is_empty();
            say(
                if ready { "ok  " } else { "FAIL" },
                &format!(
                    "the data stage was done before Update {}: {} items, {} recipes, {} machines, a belt of {} a second",
                    test.frames,
                    data.items.len(),
                    data.recipes.len(),
                    data.machines.len(),
                    rules.belt_items_per_second()
                ),
            );
            let patches = ore.tiles_with_ore();
            let ok = patches > 0 && ore.total() == patches as u64 * map.ore_per_tile as u64;
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
            let held = grid.at(grid.index(chest)).map(|b| b.held.count()).unwrap_or(0);
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
                .map(|b| b.held.count() as u64)
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
        // ---- F2: the data stage's tables, and what they say ---------------------------------
        15 => {
            // the two little factories: a belt, a machine, a belt and a chest each
            match lay_out_the_machine_lines(&map, &data, &rules, &mut test) {
                true => test.step = 16,
                false => {
                    say("FAIL", "there was nowhere to build the machines the data file declares");
                    test.step = 18;
                }
            }
        }
        // one tile a frame, because the hand is read when the click is answered
        16 if !test.to_build.is_empty() => {
            let (tile, what) = test.to_build.remove(0);
            hand.what = what;
            hand.dir = Dir::East;
            clicks.write(click_on(&map, tile));
        }
        16 => {
            // the frame after the clicks: the buildings are there, so the feeding belts can be
            // loaded with what each machine eats
            seed_the_machine_lines(&map, &data, &rules, &grid, &mut lanes);
            test.started = time.elapsed_secs();
            test.step = 17;
        }
        17 => {
            let waited = time.elapsed_secs() - test.started;
            let longest = test.machines.iter().map(|m| m.needs).fold(0.0f32, f32::max);
            // each one's own moment, caught on the frame it happens rather than read off at the
            // end: two machines that finish two seconds apart are two different measurements
            for m in test.machines.iter_mut() {
                if m.done_at.is_none() && grid.at(m.chest).map(|b| b.held.of(m.makes)).unwrap_or(0) > 0
                {
                    m.done_at = Some(waited);
                }
            }
            if test.machines.iter().any(|m| m.done_at.is_none()) && waited < longest * CHECK_SLACK {
                return;
            }
            for m in test.machines.clone() {
                let held = grid.at(m.chest).map(|b| b.held.of(m.makes)).unwrap_or(0);
                say(
                    if m.done_at.is_some() { "ok  " } else { "FAIL" },
                    &format!(
                        "the {} turned {} into {} {} after {:.1} s (the numbers say {:.1} s)",
                        m.what,
                        m.made_of,
                        held,
                        data.item_name(m.makes),
                        m.done_at.unwrap_or(waited),
                        m.needs
                    ),
                );
            }
            test.step = 18;
        }
        // ---- a data file that is wrong says which line it is wrong on ------------------------
        18 => {
            // **The same door the real file went through**, on the game's own VM, in whatever
            // build this is — which is the whole point: a browser's compiler names every program
            // `playground.rb` and this proves that what a player is told is still `data.rb:4`.
            let mut right = 0;
            for (source, line, sort) in wrong_data_files() {
                let trouble = data::read_the_declarations(
                    &mut world.vm,
                    DATA_FILE,
                    &source,
                    platform::compile,
                );
                match trouble {
                    Err(trouble) if trouble.at == Some(line) => right += 1,
                    Err(trouble) => info!(
                        "selftest: {sort} was refused at the wrong line: {}",
                        trouble.say(DATA_FILE)
                    ),
                    Ok(_) => info!("selftest: {sort} was not refused at all"),
                }
            }
            let all = wrong_data_files().len();
            say(
                if right == all { "ok  " } else { "FAIL" },
                &format!(
                    "a wrong {DATA_FILE} is refused with the line it is wrong on ({right}/{all})"
                ),
            );
            test.step = 19;
        }
        // ---- and the tables can be read back from Ruby ----------------------------------------
        19 => {
            let asked = read_the_tables_back(&mut world.vm, &data);
            say(
                if asked.is_some() { "ok  " } else { "FAIL" },
                &format!(
                    "a script reads the tables back: {}",
                    asked.unwrap_or_else(|| "it could not".into())
                ),
            );
            test.step = 20;
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

/// **F2's two little factories, built with clicks**: for each machine the data file declares, a
/// belt, the machine, a belt and a chest, in a row.
///
/// The rows are worked out from the map rather than written down, so that a map of another size
/// still has somewhere to put them; `Ore::laid_out` keeps the middle of the map bare, which is
/// where they go. The machine with the largest footprint decides how far apart the rows are.
fn lay_out_the_machine_lines(
    map: &Map,
    data: &Data,
    rules: &Rules,
    test: &mut SelfTest,
) -> bool {
    test.machines.clear();
    test.to_build.clear();
    let tall = data.machines.iter().map(|m| m.size.y).max().unwrap_or(1);
    let wide = data.machines.iter().map(|m| m.size.x).max().unwrap_or(1);
    // a row per machine, `tall` apart so that a machine's footprint never reaches the next row,
    // starting in the middle of the map and going up
    let first_row = map.tiles / 2;
    let left = map.tiles / 2 - (wide + 4);
    for (kind, machine) in data.machines.iter().enumerate() {
        let row = first_row + kind as u32 * (tall + 1);
        // the recipe it will run: the first one made in it, which is the one it will pick
        let Some(&recipe) = machine.recipes.first() else { continue };
        let recipe = &data.recipes[recipe as usize];
        let Some(&(makes, _)) = recipe.outputs.first() else { continue };
        let out = left + 2 + machine.size.x;
        if row + tall >= map.tiles || out + 1 >= map.tiles {
            return false;
        }
        for (x, what) in [
            (left, Some(What::Belt)),
            (left + 1, Some(What::Belt)),
            (left + 2, Some(What::Machine(kind as data::MachineId))),
            (out, Some(What::Belt)),
            (out + 1, Some(What::Chest)),
        ] {
            test.to_build.push((UVec2::new(x, row), what));
        }
        let made_of: Vec<String> = recipe
            .inputs
            .iter()
            .map(|&(item, n)| format!("{n} {}", data.item_name(item)))
            .collect();
        // **what the game's own numbers say this takes**: two tiles of belt to reach it, one
        // craft at the machine's own speed, and one tile of belt out of it into the chest
        let needs = 2.0 / rules.belt_tiles_per_second
            + recipe.time / machine.speed
            + 1.0 / rules.belt_tiles_per_second;
        test.machines.push(MachineCheck {
            what: machine.name.clone(),
            makes,
            made_of: made_of.join(" and "),
            chest: (row * map.tiles + out + 1) as usize,
            needs,
            done_at: None,
        });
    }
    !test.machines.is_empty()
}

/// **What each machine eats, put on the belt that feeds it** — which is what a miner up the line
/// would have put there, and what F3's inserters will hand over.
fn seed_the_machine_lines(
    map: &Map,
    data: &Data,
    rules: &Rules,
    grid: &Grid,
    lanes: &mut Lanes,
) {
    let tall = data.machines.iter().map(|m| m.size.y).max().unwrap_or(1);
    let wide = data.machines.iter().map(|m| m.size.x).max().unwrap_or(1);
    let first_row = map.tiles / 2;
    let left = map.tiles / 2 - (wide + 4);
    for (kind, machine) in data.machines.iter().enumerate() {
        let row = first_row + kind as u32 * (tall + 1);
        let Some(&recipe) = machine.recipes.first() else { continue };
        let recipe = &data.recipes[recipe as usize];
        let feed = grid.index(UVec2::new(left, row));
        let mut along = 0.0;
        for &(item, n) in &recipe.inputs {
            for _ in 0..n {
                lanes.of[feed].push_back(OnBelt { along, item });
                along -= rules.spacing();
            }
        }
    }
}

/// **The wrong data files the checks put through the door**, and the line each is wrong on.
///
/// They are written here rather than kept as files because a file that has to be wrong is a file
/// somebody will fix; and because a page has no directory to put one in. Each is the same little
/// data file with one thing changed, and every one of them covers a different road to the error:
/// serde refusing a field, serde refusing a number, a reference between two declarations, and the
/// compiler refusing to parse it at all.
fn wrong_data_files() -> Vec<(String, u32, &'static str)> {
    // 1 item, 2 machine, 3 recipe, 4 belt, 5 miner, 6 chest
    let good = concat!(
        "item :rock, icon: 0\n",
        "machine :oven, size: [1, 1], sprite: [109], speed: 1.0\n",
        "recipe :rock, in: {}, out: { rock: 1 }, time: 1.0, made_in: :oven\n",
        "belt :line, tiles_per_second: 1.0, items_per_tile: 1.0\n",
        "miner :drill, seconds_per_item: 1.0, digs: :rock\n",
        "chest :box, capacity: 1\n",
    );
    vec![
        (good.replace("item :rock, icon: 0", "item :rock, icon: 0, colour: :grey"), 1, "an unknown field"),
        (good.replace("time: 1.0, made_in: :oven", "time: 0.0, made_in: :oven"), 3, "a time of zero"),
        (good.replace("out: { rock: 1 }", "out: { pebble: 1 }"), 3, "an item nothing declares"),
        (good.replace("size: [1, 1]", "size: [0, 1]"), 2, "a machine no tiles wide"),
        (good.replace("tiles_per_second: 1.0", "tiles_per_second: -1.0"), 4, "a belt that runs backwards"),
        // **last, and on purpose**: a half-written line is reported where the parser gives up,
        // which is the *next* token — so `icon:` on line 1 of a file with six more lines is
        // reported at line 2. At the end of the file the next token is the end of the file, and
        // the line is the line. That is the compiler's reading and not something to work around.
        (format!("{good}item :half, icon:\n"), 7, "Ruby that will not parse"),
    ]
}

/// **The other direction**: a script asking the tables what they hold, in the game's own VM, with
/// the methods `data::expose_the_tables` put there at startup. It answers what it read, for the
/// line to print, or `None` if anything went wrong.
fn read_the_tables_back(vm: &mut sabiruby::Vm, data: &Data) -> Option<String> {
    let first_recipe = data.recipes.first()?;
    let last_item = data.items.last()?;
    let script = format!(
        "$r = recipe_of(:{})[:made_in]\n$i = item_of(:{})[:icon]\n",
        first_recipe.name, last_item.name
    );
    let bytes = platform::compile(&script, "selftest.rb").ok()?;
    vm.load_and_run(&bytes).ok()?;
    let made_in = vm.global_get("$r");
    let made_in = vm.str_bytes(made_in).map(|b| String::from_utf8_lossy(b).into_owned())?;
    let icon = match vm.global_get("$i") {
        sabiruby::Value::Int(n) => n,
        _ => return None,
    };
    (made_in == data.machines[first_recipe.made_in as usize].name && icon == last_item.icon as i64)
        .then(|| {
            format!(
                "recipe_of(:{})[:made_in] is {made_in} and item_of(:{})[:icon] is {icon}",
                first_recipe.name, last_item.name
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one piece of arithmetic the building is built on. It is checked here as well as in the
    /// run because a run needs a window for one of its checks and this needs nothing.
    #[test]
    fn a_tiles_middle_is_in_that_tile() {
        let map = Map { tiles: 32, ore_per_tile: 60, ore_patch_radius: 3.0 };
        for tile in [UVec2::ZERO, UVec2::new(31, 0), UVec2::new(0, 31), UVec2::new(31, 31), UVec2::new(8, 11)] {
            assert_eq!(map.tile_at(map.tile_centre(tile)), Some(tile), "tile {tile}");
        }
    }

    /// The corners belong to exactly one tile each: the bottom left of the map is tile (0, 0) and
    /// a hair below it is off the map, which is what keeps a click on the edge from placing
    /// something outside the world.
    #[test]
    fn the_edges_belong_to_one_tile_and_outside_is_outside() {
        let map = Map { tiles: 32, ore_per_tile: 60, ore_patch_radius: 3.0 };
        let half = map.span() / 2.0;
        assert_eq!(map.tile_at(Vec2::new(-half, -half)), Some(UVec2::ZERO));
        assert_eq!(map.tile_at(Vec2::new(-half - 0.01, -half)), None);
        assert_eq!(map.tile_at(Vec2::new(half - 0.01, half - 0.01)), Some(UVec2::splat(31)));
        assert_eq!(map.tile_at(Vec2::new(half, half)), None, "the far edge is the next tile, which is not there");
    }

    /// **The data stage is done before the first `Update`.**
    ///
    /// The run's own checks say so as well (`the data stage was done before Update 1`), but a
    /// check inside a run only ever runs in a run that started, and what this is about is a game
    /// that would not have. So: an `App` with nothing in it but the data stage and one `Update`
    /// system that writes down what it could see the first time it ran.
    #[test]
    fn the_tables_are_there_before_the_first_update() {
        #[derive(Resource, Default)]
        struct Saw(Option<(bool, usize, f32)>);

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin {
                file_path: platform::assets_dir(),
                meta_check: AssetMetaCheck::Never,
                ..default()
            },
            RubevyPlugin::default(),
        ))
        .init_asset::<Image>()
        .insert_resource(RubyDir(platform::ruby_dir()))
        .init_resource::<Saw>()
        .add_systems(Startup, read_the_data_stage)
        .add_systems(
            Update,
            |data: Option<Res<Data>>, rules: Option<Res<Rules>>, mut saw: ResMut<Saw>| {
                if saw.0.is_none() {
                    saw.0 = Some((
                        rules.is_some(),
                        data.as_ref().map(|d| d.items.len()).unwrap_or(0),
                        rules.map(|r| r.belt_tiles_per_second).unwrap_or(0.0),
                    ));
                }
            },
        );
        app.update();

        let saw = app.world().resource::<Saw>().0.expect("the Update system ran");
        assert!(saw.0, "the first Update had no Rules: the data stage was not done before it");
        assert!(saw.1 >= 1, "and no items either");
        assert!(saw.2 > 0.0, "and the belt's speed came out of the file");
        // and the real `ruby/data.rb` is a file the game will start on, which is the other half
        // of this: the test above would pass on a file with one item in it
        let data = app.world().resource::<Data>();
        assert!(!data.recipes.is_empty(), "ruby/data.rb declares recipes");
        assert!(!data.machines.is_empty(), "and machines to make them in");
        for machine in &data.machines {
            assert_eq!(
                machine.sprite.len() as u32,
                machine.tiles(),
                "{}: one picture per tile",
                machine.name
            );
            for &tile in &machine.sprite {
                assert!(tile < TILESET_LAYERS as u16, "{}: tile {tile} is off the sheet", machine.name);
            }
        }
        for item in &data.items {
            assert!(item.icon < draw::ITEM_ICONS, "{}: icon {} is off the strip", item.name, item.icon);
        }
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
