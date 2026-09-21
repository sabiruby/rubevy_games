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
//! | `src/build.rs` | the keys, the click, and the order they write |
//! | `src/inserters.rs` | **the one machine with a mind**: the scripts, the questions, the swing |
//! | `src/draw.rs` | the two chunks, the sprites, and the zoom |
//!
//! **Where the numbers are.** Every number the factory is *played by* is a line of `ruby/data.rb`
//! ([`belts::Rules`]), which is what the data stage is for; what is left in `factory.settings.txt`
//! moves the game rather than being played by it — the window, the camera, the map's size, and
//! how wide a patch of ore is, which is the one number of the world's layout that is wanted here
//! in `main` before there is a VM. The `const`s in this file are the names of the defaults and
//! two invariants: the pack's 16 px (which is also the steps a belt tile is long) and how many
//! tiles the sheet has. `docs/numbers.md` §9 has the table, with where each default came from.
//!
//! [rubevy]: https://github.com/sabiruby/rubevy

mod belts;
mod build;
mod control;
mod data;
mod draw;
mod grid;
mod inserters;
mod items;
mod guide_text;
mod machines;
mod platform;
mod save;
mod window;

use std::path::PathBuf;

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::sprite_render::{TilemapChunk, TilemapChunkTileData};
use games_shell::camera::{CameraControls, CameraPlugin, CameraSet, WorldClick};
use rubevy::{HoldRequests, RubevyPlugin, RubevySet, ScriptWorld};
use rubevy_egui::{VmInspector, VmInspectorPlugin};

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
/// image so that it cannot stop working quietly. **F3 added one** — the inserter's base — and 143
/// is not a multiple of six either.
const TILESET_LAYERS: u32 = 143;

// ---------------------------------------------------------------------------------------------
// The defaults, every one of which `factory.settings.txt` can move
// ---------------------------------------------------------------------------------------------

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
/// end the run the moment they are all answered; what this has to clear is the sum of the bounds
/// they would wait to, which is what a run where something is wrong takes. Each bound is the
/// game's own numbers times [`CHECK_SLACK`]:
///
/// * F1's line — a miner, three belts and a chest — `mine_seconds + 4 ÷ belt`, 3.0 s, so 6;
/// * the jam — two tiles of belt running into a machine with no arm beside it, 1.0 s, so 1
///   (it is a condition that is true in one tile's time and the bound is barely reached);
/// * the two machine lines — `swing (the script's first look) + one swing per thing the recipe
///   eats + time ÷ speed + swing + 1 ÷ belt`, 5.5 s, so 11;
/// * an arm whose script raises — a swing, so 2;
/// * **F4's three**, each of them the miner's line putting one more thing in its chest
///   (`mine_seconds + 4 ÷ belt`, 3.0 s, so 6 each): the goal being reached, the goal that cannot
///   be reached not being reached, and the factory still running with a broken `control.rb` —
///   **18**.
///
/// **Forty-four seconds of the game's own time**, then, and sixty-six is half as much again. What
/// a run where nothing is wrong takes is 15 s, measured 2026-09-22 (it was 10.5 s at F3).
const HEADLESS_SECONDS: f32 = 66.0;
const SHOT_FILE: &str = "shot.png";
/// `--shot` with no seconds. The picture wants the factory **working**: the two machine lines
/// start at about 4 s of the game's own time and their first item is in the chest at 9.5 s
/// (measured 2026-09-21), so this is in the middle of them, with arms mid-swing.
const SHOT_SECONDS: f32 = 8.0;

/// **How long the checks wait for the tileset to arrive, in frames.** They wait for the thing
/// itself rather than for a number of seconds (S7's rule), but a wait with no end is a check that
/// can never fail, and in a page there is no run ending underneath it to stop it. The bound is in
/// frames because what is being waited for is the asset server getting a turn, which is once a
/// frame. Measured: the tileset was in `Assets<Image>` on **frame 3** in the container and
/// **frame 4** in the browser, so this is a hundred times the worst of the two.
const TILESET_WAIT_FRAMES: u32 = 400;

/// **How long the checks wait for the panel and what it applied, in frames.** What is waited for
/// happens once a frame, so the bound is in frames rather than in seconds (S7's rule), and three
/// things are waited for in a row:
///
/// * the panel opening — an order is read on the frame it is written or the one after (a Bevy
///   message is readable for two) and the panel is filled in that frame;
/// * the button being taken — the system that does it runs after the checks, so the same frame;
/// * **the program being in the VM** — the crew is put in step with the texts on the next frame,
///   the new `Script` lands at the sync point after that, and rubevy loads it at the head of the
///   frame after *that*.
///
/// Three frames is the longest of the three, and six is twice it.
const PANEL_WAIT_FRAMES: u32 = 6;

/// **How many instructions this game's VM may run in a frame**, and the one number here that came
/// out of an instrument rather than out of an argument.
///
/// The recipe is rubevy's own (`docs/host-api.md`, "Choosing `budget` and `frame_time`") and it
/// has three steps, none of which is a multiplier somebody liked:
///
/// 1. **measure the rate.** `--arms 3000`, with the scripts' first look spread as it is in play,
///    runs **10,800 instructions in 1,102 µs** of tick at the median — about **9,800 instructions
///    a millisecond** (2026-09-21, release, this machine; the same instrument at 1,000 arms says
///    13,800, and the lower of the two is the one to be conservative at because it is the busier
///    factory). The garden measured 9,300 for its rules, which is the same kind of Ruby doing the
///    same kind of thing.
/// 2. **decide the share of the frame.** A sixtieth of a second is 16.7 ms and this game has one
///    VM, so a quarter of the frame is 4.2 ms of it.
/// 3. **what that time buys**: 4 ms × 9,800 ≈ **39,000**.
///
/// **What it leaves for a player.** The factory's own inserters at the biggest chain the default
/// map holds — about 340 arms (`docs/numbers.md` §9.6) — cost about 2,000 instructions in the
/// worst frame measured, so this is twenty times what the scripts that ship use. At three
/// thousand arms, which no map here is big enough for, the worst frame measured was 14,040 and
/// this is still 2.8 times it.
///
/// **The budget is what bites and the clock is the guard**, which is the way round rubevy
/// recommends for a game that wants its scripts to behave the same everywhere: instructions are a
/// fact about the scripts and are the same number in a browser several times slower, where
/// milliseconds are a fact about the machine. `frame_time` is left at rubevy's 8 ms, which at the
/// rate above is about 78,000 instructions — twice this.
const SCRIPT_BUDGET: f32 = 39_000.0;

/// **The tick's wall-clock bound, in milliseconds.** rubevy's own default, kept rather than
/// chosen: the measurement above says the scripts that ship use 1.1 ms of it at three thousand
/// arms and 0.2 ms at the three hundred a map holds, so it is a guard with room and not a
/// reservation. Zero means no guard at all, which is rubevy's rule and not this game's.
const SCRIPT_FRAME_TIME_MS: f32 = 8.0;

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
    /// **How many tiles across and up**, which since F3a is two numbers and `ruby/data.rb`'s:
    /// `map :world, size: [32, 32]`. It was a square in `factory.settings.txt` before that, read
    /// in `main` before there was a VM — and a page, which has no settings file to write, could
    /// not say it at all.
    pub tiles: UVec2,
}

impl Map {
    /// How wide and how tall the whole map is, in world units.
    fn span(&self) -> Vec2 {
        (self.tiles * TILE_PX).as_vec2()
    }

    /// **The tile a world point is in**, or `None` if the point is off the map. The map is
    /// centred on the origin, which is where the chunk puts itself.
    fn tile_at(&self, world: Vec2) -> Option<UVec2> {
        let half = self.span() / 2.0;
        let at = ((world + half) / TILE_PX as f32).floor();
        let last = self.tiles.as_vec2();
        (at.x >= 0.0 && at.x < last.x && at.y >= 0.0 && at.y < last.y)
            .then(|| at.as_uvec2())
    }

    /// **The middle of a tile, in world units.** The same arithmetic
    /// [`TilemapChunk::calculate_tile_transform`] does, written here because the game needs it
    /// before anything has been spawned.
    pub fn tile_centre(&self, tile: UVec2) -> Vec2 {
        tile.as_vec2() * TILE_PX as f32 + TILE_PX as f32 / 2.0 - self.span() / 2.0
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
    /// **How many inserters to stand beside it** (F3): each one is a chest with something in it,
    /// an arm, and an empty chest, so every one of them has work to do for as long as the stock
    /// lasts. It is the scripts' side of the measurement, where `items` is the belts'.
    arms: usize,
    /// Frames to watch before saying anything, and again between each saying.
    every: u32,
    seen: Vec<f32>,
    steps: Vec<f32>,
    /// What the VM's own tick came to, frame by frame ([`ScriptWorld::last_frame`]).
    instructions: Vec<f32>,
    ticks: Vec<f32>,
    woke: Vec<f32>,
    carried: Vec<f32>,
    dropped: u64,
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
    let (settings, lang) = games_shell::remembered(
        platform::SETTINGS_FILE,
        "Factory: what the game remembers. Delete a line for the default.",
        platform::read,
        platform::write,
        args.value("--lang").as_deref(),
    );

    // **There is no map here any more** (F3a). How big the world is, how wide a patch of ore is
    // and how many patches there are are all `ruby/data.rb`'s now, so the world is laid out in
    // `Startup` after the data stage (`lay_the_land`), and everything that wanted the map's size
    // in `main` went with it: the grid, the lanes, the chunk and the camera's edges. What F1 had
    // here was a `.max` that raised a too-small map without saying so; what says no now is the
    // data stage, at the line the size is written on. (What a store written by an older build is
    // told is in `say_where_the_map_went`, which is a `Startup` system rather than a line here:
    // **there is no log yet in `main`** — `LogPlugin` is one of the plugins below — so a `warn!`
    // written here goes nowhere at all, which is how it was found.)
    // **Since S11 the refusals are the shared crate's** (`games_shell::Settings::positive` and
    // `counted`): what F1 wrote here for this game, three games wanted. A refusal is kept rather
    // than said on the spot for the reason in the paragraph above — there is no log in `main` —
    // and `SettingsRefusalsPlugin` says all of them in `Startup`.
    let half_height = settings.positive("camera_half_height", CAMERA_HALF_HEIGHT);
    let window = [
        settings.counted("window_width", 1, WINDOW[0] as u64) as f32,
        settings.counted("window_height", 1, WINDOW[1] as u64) as f32,
    ];
    let headless = args.headless(settings.positive("headless_seconds", HEADLESS_SECONDS));
    let shot = args.shot(
        settings.get("shot_file").unwrap_or(SHOT_FILE),
        settings.positive("shot_seconds", SHOT_SECONDS),
    );
    // **a stress run of no items is the ordinary game**, so nought is what these mean when
    // nobody asks and a fraction of an item is not a thing to lay on a belt
    let stress = args
        .value("--stress")
        .and_then(|n| n.parse::<f32>().ok())
        .map(|n| n.max(0.0) as u64)
        .or_else(|| games_shell::checks::asked_number("FACTORY_STRESS").map(|n| n.max(0.0) as u64))
        .unwrap_or_else(|| settings.counted("stress_items", 0, 0)) as usize;
    let arms = args
        .value("--arms")
        .and_then(|n| n.parse::<f32>().ok())
        .map(|n| n.max(0.0) as u64)
        .or_else(|| games_shell::checks::asked_number("FACTORY_ARMS").map(|n| n.max(0.0) as u64))
        .unwrap_or_else(|| settings.counted("stress_arms", 0, 0)) as usize;
    // **the measuring instrument's one knob that is not a size**: zero takes the spreading of
    // the scripts' first look away, which is how the stress run shows what it is worth.
    // It keeps its `.max(0.0)` where the other settings lost theirs (S11), and the reason is that
    // **a spread of minus a second and a spread of none are the same run**: there is nothing for
    // the store to be told, because nothing was silently substituted for what it asked for.
    let stagger = inserters::Stagger(
        games_shell::checks::asked_number("FACTORY_STAGGER")
            .map(|n| n.max(0.0))
            .unwrap_or_else(|| settings.number("inserter_stagger").unwrap_or(1.0).max(0.0)),
    );

    // **`--save PATH` / `--load PATH`** (F5): the same two things F5 and F9 do in a window, for a
    // run that has no keyboard. A `--save` is written as the run ends, which is what makes
    // "save, load in a second process, save again, compare the two files" one shell line each.
    let save_to = args.value("--save");
    let load_from = args.value("--load");
    // **Where a save goes**, settled before the store is handed to the app: `--save`'s path, or
    // `save_file` in the store, or the game's own name. In a browser it is a `localStorage` key
    // and not a file (`crate::platform`).
    let save_file = save_to
        .clone()
        .unwrap_or_else(|| settings.get("save_file").unwrap_or(platform::SAVE_FILE).to_string());

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
            // without the message `build::clicks` would not even start. It is registered so
            // that there is one code path for the mouse in both modes — and since F3 the checks
            // do not write one at all: they write the order a click turns into (`build::Order`),
            // which is the message that carries what was meant.
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
                            resolution: (window[0] as u32, window[1] as u32).into(),
                            // in the browser: the page's canvas, as large as its box
                            canvas: Some("#factory".into()),
                            fit_canvas_to_parent: true,
                            ..default()
                        }),
                        ..default()
                    }),
                // The camera the player drives (S3). **Where the world's edge is is not known
                // here any anymore** — the map is a declaration since F3a — so the plugin is
                // given the view and [`point_the_camera_at_the_map`] gives it the edges in
                // `Startup`, once the data stage has said how big the world is.
                CameraPlugin::showing(half_height),
                RubevyPlugin::default(),
                // **F5: the window, all of it.** The editor an inserter's script is edited in
                // (and `data.rb`, and `control.rb` — `crate::window` says what Apply means for
                // each), the VM panel behind `F2`, the `H` guide in English and Japanese, and the
                // wiring that lets every one of them be resized out of `factory.settings.txt`.
                window::the_editor(&settings),
                VmInspectorPlugin,
                games_shell::GuidePlugin::default(),
                games_shell::PanelSettingsPlugin,
            ))
            // A picture is asked for one thing and the guide sits over the middle of the window,
            // so a `--shot` run starts with it shut unless `--guide` says otherwise — the
            // garden's rule, and how the picture that shows the Japanese is not tofu is taken.
            .insert_resource(
                guide_text::guide().opening(lang, shot.is_none() || args.has("--guide")),
            )
            // **Closed** (the garden's G9): a debugger thrown over the middle of the window is
            // not what somebody came to look at a factory for. `F2` opens it, and `--vm` does for
            // a picture.
            .insert_resource(VmInspector::following().opened(args.has("--vm")))
            .init_resource::<window::Watched>()
            .init_resource::<window::Paused>()
            .add_systems(
                Update,
                (
                    window::follow_the_orders.before(build::orders),
                    window::panel_keys,
                    window::show_code,
                    window::show_vm,
                    window::do_editor_actions,
                    window::watch_the_programs,
                )
                    .chain()
                    .run_if(the_factory_is_up),
            )
            // F5 and F9, and the two buttons in the HUD that mean the same thing
            .add_systems(Update, save_load_keys.before(save::save_the_factory))
            // the window's half of a rebuild: the floor's chunks are the size of the map, and the
            // camera's edges are the map's (`draw_the_new_world`, then the two `Startup` systems
            // again — there is one road, and this is it being walked a second time)
            .add_systems(
                Update,
                (
                    draw_the_new_world,
                    draw::start_drawing.run_if(not(resource_exists::<draw::Chunks>)),
                    point_the_camera_at_the_map,
                )
                    .chain()
                    .after(lay_the_land_again)
                    .before(FactorySet::Step)
                    .run_if(on_message::<RebuildTheWorld>)
                    .run_if(resource_exists::<Map>),
            )
            .add_systems(bevy_egui::EguiPrimaryContextPass, window::draw_hud)
            .init_resource::<items::Pool>()
            .init_resource::<draw::Animation>()
            .insert_resource(draw::SnapZoom(
                settings.number("camera_snap_zoom").unwrap_or(1.0) != 0.0,
            ))
            .add_systems(
                Startup,
                (
                    draw::start_drawing.run_if(resource_exists::<Map>),
                    point_the_camera_at_the_map.run_if(resource_exists::<Map>),
                    say_the_trouble,
                )
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
            .add_systems(Update, draw::snap_zoom.after(CameraSet::Drive))
            // **Ruby moving the camera** (F5): after the scripts' writes have landed and after
            // the player's own driving, so that a `look_at` in `control.rb` is the last word on
            // where the camera is that frame and a hand on the mouse is the last word otherwise.
            .add_systems(
                Update,
                let_a_script_move_the_camera.after(CameraSet::Drive).after(RubevySet::Answer),
            );
            app.add_systems(
                Update,
                draw::draw_items.after(FactorySet::Step).run_if(resource_exists::<draw::Chunks>),
            );
        }
    }

    // **What the store asked for and did not get** (S11). The settings are read at the top of
    // `main`, where a `warn!` has no log to go to yet (F3a, which found that out by writing one
    // there and seeing nothing), so the refusals are kept and said in `Startup`.
    app.add_plugins(games_shell::SettingsRefusalsPlugin);
    app.hold_requests(inserters::MOVE)
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
        .init_resource::<inserters::Arms>()
        .insert_resource(stagger)
        .add_systems(Startup, set_the_budget)
        // **The camera, in the words a script uses about one** — rubevy's optional layer (R9).
        // It is Ruby the crate carries and does not run, over `Rubevy.find` and
        // `e[:Transform] =`, so loading it is what gives `control.rb` a `Rubevy::Camera` and an
        // app that does not load it has none. A headless run loads it too and finds no camera,
        // which is exactly what `look_at` is written for.
        .add_systems(Startup, take_up_the_camera_layer)
        .add_systems(Startup, say_where_the_map_went)
        .add_systems(Startup, read_the_data_stage)
        .add_systems(Startup, lay_the_land.after(read_the_data_stage).run_if(resource_exists::<Rules>))
        // **the inserters' half**, and every line of it is after the data stage: what an arm is
        // written in is compiled with the item names and the swing in front of it, and both of
        // those are `ruby/data.rb`'s
        .add_systems(
            Startup,
            (inserters::read_the_scripts, inserters::install_answers).after(read_the_data_stage),
        )
        // **the control stage** (F4): one script, after the data stage, because the names it is
        // told about are `data.rb`'s
        .init_resource::<control::Happenings>()
        .add_message::<control::Rewrite>()
        .add_systems(
            Update,
            control::follow_the_rewrites
                .before(FactorySet::Step)
                .run_if(resource_exists::<control::TheControl>),
        )
        .add_systems(
            Startup,
            control::read_the_control_stage
                .after(read_the_data_stage)
                .run_if(resource_exists::<Data>),
        )
        .add_systems(
            Update,
            (
                // what happened is published after the step, so a script hears about a frame at
                // the head of the frame after it; what the script says is read back in the same
                // order, so the line on the screen is never half a frame old
                control::tell_the_control_stage,
                control::hear_the_control_stage,
                control::watch_the_control_ending,
                control::say_if_anything_was_dropped,
            )
                .chain()
                .after(FactorySet::Step)
                .run_if(resource_exists::<control::TheControl>),
        )
        .add_systems(
            Update,
            inserters::keep_the_crew
                .after(build::orders)
                .before(FactorySet::Step)
                .run_if(the_factory_is_up),
        )
        // **`move` is a question rubevy keeps for the game** (`hold_requests`): it arrives as a
        // `Held` on the arm that asked rather than out of `take_requests`, and every way the
        // waiting can end — the arm taken away, its script replaced, its script raising — takes
        // the request with it. F3 wrote that index and that tidying-up by hand
        // (`Arms::waiting`); this line is what is left of it.
        .add_systems(
            Update,
            (inserters::start_swings, inserters::answer_the_rest).in_set(RubevySet::Answer),
        )
        .add_systems(
            Update,
            (inserters::finish_swings, inserters::watch_endings)
                .after(FactorySet::Step)
                .run_if(the_factory_is_up),
        )
        // **F5: the world written down and read back.** Four systems and an order that matters:
        // a load is put in before the step (so nothing ever steps half a world), the memories are
        // handed over after it (a script started this frame has reached `run` by then), and the
        // save is last, so a run that loads and saves in one go writes the world it read rather
        // than that world plus a frame.
        .insert_resource(save::SaveFile {
            path: save_file,
            on_exit: save_to.is_some(),
        })
        .init_resource::<save::Asked>()
        .init_resource::<save::SaveNote>()
        .add_message::<window::Reread>()
        .add_message::<RebuildTheWorld>()
        .init_resource::<window::DataFile>()
        // **one road from "the declarations changed" to "the factory is this world now"**, and
        // it is after the re-reading and before anything steps
        .add_systems(
            Update,
            lay_the_land_again
                .after(reread_the_data_stage)
                .before(FactorySet::Step)
                .before(build::orders)
                .run_if(resource_exists::<Rules>),
        )
        .add_systems(Startup, read_the_data_file_text.after(read_the_data_stage))
        .add_systems(
            Update,
            reread_the_data_stage
                .before(FactorySet::Step)
                .before(build::orders)
                .run_if(resource_exists::<Rules>),
        )
        .add_systems(
            Update,
            save::load_the_factory
                .before(FactorySet::Step)
                .before(inserters::keep_the_crew)
                .run_if(the_factory_is_up),
        )
        .add_systems(
            Update,
            (save::restore_memories.run_if(resource_exists::<save::Restoring>), save::save_the_factory)
                .chain()
                .after(FactorySet::Step)
                .run_if(the_factory_is_up)
                .run_if(resource_exists::<control::TheControl>),
        )
        // **The game's own message for "build this here"** (F3). A mouse becomes one of these and
        // so does a check; `build::orders` is the only thing that builds, and it is ordered before
        // the step so that a belt laid this frame carries this frame.
        .add_message::<build::Order>()
        .add_systems(
            Update,
            (build::clicks, build::orders, build::follow_the_flow)
                .chain()
                .before(FactorySet::Step)
                .run_if(the_factory_is_up),
        );
    // **The step is after the scripts have run and been answered**, and that is the whole of the
    // order an inserter rests on: a script reads the world in `RubevySet::Tick` (its three
    // questions are answered there, out of the world as it stands), the `move` it asked for is
    // taken in `RubevySet::Answer` and picks the item up, and *then* the factory moves. So what a
    // script saw is what its arm picked up, with nothing in between.
    app.add_systems(
        Update,
        items::run_the_factory
            .in_set(FactorySet::Step)
            .after(RubevySet::Answer)
            .run_if(the_factory_is_up),
    );
    if stress > 0 || arms > 0 {
        app.insert_resource(Stress {
            items: stress,
            arms,
            // a second of frames at a sixtieth each, which is long enough for the numbers to
            // stop being the first frame's and short enough to see three of them in a run
            every: 60,
            seen: Vec::new(),
            steps: Vec::new(),
            instructions: Vec::new(),
            ticks: Vec::new(),
            woke: Vec::new(),
            carried: Vec::new(),
            dropped: 0,
            said: 0,
        })
        .add_systems(
            Startup,
            size_the_map_for_the_stress_run
                .after(read_the_data_stage)
                .before(lay_the_land)
                .run_if(resource_exists::<Rules>),
        )
        .add_systems(
            Startup,
            (lay_the_snake, lay_the_arms).chain().after(lay_the_land).run_if(the_factory_is_up),
        )
        .add_systems(
            Update,
            watch_the_frames.after(FactorySet::Step).run_if(the_factory_is_up),
        );
    }

    if platform::selftest_asked() {
        // **Before the orders are carried out, and the reason changed at F3.**
        //
        // F2 wrote `.before(build::clicks)` because the checks set what was in hand and forged a
        // `WorldClick` in the same frame, and the system that answered a click read the hand
        // *when it ran*: a frame in which it ran first built this frame's hand at last frame's
        // place, and a window run built a chest where it meant a belt (`worklog/…-F2.md` §8.4a).
        // **That hole is gone**: a check writes a `build::Order`, which carries the tile, the
        // thing and the way round, so nothing about it depends on when anything else runs.
        //
        // What is left is a property of *checking* and not of building: a check gives an order
        // on one frame and looks at what it did on the next, so it has to be on the same side of
        // `build::orders` every frame or it looks a frame early. Hence this line, and hence it
        // names `orders` rather than `clicks` — what a mouse does is no longer its business.
        app.init_resource::<SelfTest>()
            .add_systems(
                Update,
                selftest.before(build::orders).before(FactorySet::Step).run_if(the_factory_is_up),
            )
            // **F4's checks are a system of their own** and on the same side of the building as
            // the rest ([`CONTROL_CHECKS`])
            .add_systems(
                Update,
                control_checks
                    .before(build::orders)
                    .before(control::follow_the_rewrites)
                    .before(FactorySet::Step)
                    .run_if(the_factory_is_up)
                    .run_if(resource_exists::<control::TheControl>),
            );
    }
    // **What this run was asked for besides being a factory, and the end of the run** (S11).
    // Both errands are known here — the checks are the `if` above and the picture is the line
    // below — and neither of them ends the run itself any more:
    // `games_shell::checks::end_the_run` does, when nothing is left.
    app.add_plugins(platform::Errands::of(
        "the factory",
        platform::selftest_asked(),
        shot.is_some(),
    ));
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    // **`--load PATH`**: the file is read here and put into the world by `load_the_factory` on the
    // first frame, which is the same door F9 uses. A file that will not be read is said and the
    // game starts an ordinary factory — the world is built by `lay_the_land` either way, because
    // a load fills a grid rather than making one.
    if let Some(path) = &load_from {
        match save::read_save(path) {
            Ok(file) => {
                app.insert_resource(save::Loading(file));
            }
            Err(e) => {
                error!("{e}");
                app.insert_resource(save::SaveNote { text: e, at: 0.0, bad: true });
            }
        }
    }
    app.run();
}

/// `Startup`: the camera layer, so that `ruby/control_prelude.rb`'s `look_at` has a
/// `Rubevy::Camera` to talk to.
fn take_up_the_camera_layer(mut world: ResMut<ScriptWorld>) {
    if let Err(why) = world.load_and_run(rubevy::layers::CAMERA) {
        error!("the camera layer would not load: {why}");
    }
}

/// **What a script wrote to the camera, taken as where the player is looking.**
///
/// The camera here is driven by a resource and not by its `Transform` — `games_shell`'s
/// `CameraPlugin` writes the transform out of [`CameraView`] every frame in `PostUpdate` — so a
/// `Rubevy::Camera#move_to`, which writes the `Transform`, would be painted over before anybody
/// saw it. This is the two halves joined: what the transform says now, *minus* what the panels
/// cover, becomes the view.
///
/// **It settles rather than drifts.** After a script has moved the camera this sets
/// `focus = t - shift`, `place_camera` writes `t = focus + shift` back, and the next frame finds
/// the transform where it left it and does nothing — so a window with an editor open does not
/// walk the camera sideways by the width of the panel every frame. The remembered value is what
/// tells "somebody else moved this" from "this is what we wrote", and it is the whole of the
/// arrangement.
fn let_a_script_move_the_camera(
    mut view: ResMut<games_shell::camera::CameraView>,
    insets: Res<rubevy_egui::ViewInsets>,
    windows: Query<&Window>,
    cameras: Query<&Transform, With<Camera2d>>,
    mut seen: Local<Option<Vec2>>,
) {
    let Some(at) = cameras.iter().next().map(|t| t.translation.truncate()) else { return };
    let was = seen.replace(at);
    if was != Some(at) && was.is_some() {
        let height = windows.iter().next().map(|w| w.height()).unwrap_or(1.0).max(1.0);
        let world_per_px = (view.half_height * 2.0) / height;
        view.focus = at - insets.shift(world_per_px);
    }
}

/// **What the scripts may spend in a frame**, said by the game rather than inherited.
///
/// Both numbers are settings, so a run that writes another one in `factory.settings.txt` is a run
/// the rustdoc on [`SCRIPT_BUDGET`] is not about — which is the whole of what "measured" buys and
/// the whole of what changing it costs. A `script_frame_time_ms` of zero or less is rubevy's
/// "no clock at all", so it is passed through rather than refused.
fn set_the_budget(settings: Res<games_shell::Settings>, mut world: ResMut<ScriptWorld>) {
    // **a budget of nothing is a VM that runs nothing**, which is a thing to ask for; a negative
    // one is not (S11)
    world.budget = settings.counted("script_budget", 0, SCRIPT_BUDGET as u64);
    let ms = settings.number("script_frame_time_ms").unwrap_or(SCRIPT_FRAME_TIME_MS);
    world.frame_time =
        (ms > 0.0).then(|| std::time::Duration::from_secs_f32(ms / 1000.0));
    info!(
        "the scripts may run {} instructions a frame, and the tick stops at {} ms",
        world.budget,
        ms.max(0.0)
    );
}

/// Where `ruby/` is, for the two systems that read it.
#[derive(Resource, Debug)]
pub struct RubyDir(pub PathBuf);

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
    // **How long the data stage takes is the page's problem**, so it is measured rather than
    // guessed: in a browser the compiler is a wasm module called synchronously and the whole of
    // this happens before the first frame is drawn, so whatever it costs is time the page is
    // blank for (plan §3.2). `bevy::platform::time::Instant` is the clock that exists there.
    let started = bevy::platform::time::Instant::now();
    match data::read_the_declarations(&mut world.vm, DATA_FILE, &source, platform::compile) {
        Ok((tables, rules)) => {
            info!(
                "{DATA_FILE}: {} items, {} recipes, {} machines; a full belt carries {} items a second ({:.1} ms to compile and run)",
                tables.items.len(),
                tables.recipes.len(),
                tables.machines.len(),
                rules.belt_items_per_second(),
                started.elapsed().as_secs_f32() * 1e3,
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

/// **`F5` writes the factory down and `F9` reads it back**, and the two buttons in the HUD mean
/// the same thing — which is why both go through [`save::Asked`] rather than one of them being a
/// key and the other a call.
///
/// Neither is behind `EguiWantsInput::wants_keyboard_input()`, and that is the garden's finding
/// rather than an oversight: egui does not give keyboard focus back when the pointer clicks the
/// world, so a guarded `F5` is a save key that stops working for the rest of the session after
/// the first edit. `F5` and `F9` are keys egui never wants; `P` is a letter and is guarded
/// ([`window::panel_keys`]).
fn save_load_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut asked: ResMut<save::Asked>,
    file: Res<save::SaveFile>,
    time: Res<Time>,
    mut note: ResMut<save::SaveNote>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::F5) {
        asked.save = true;
    }
    if asked.load || keys.just_pressed(KeyCode::F9) {
        asked.load = false;
        match save::read_save(&file.path) {
            Ok(read) => commands.insert_resource(save::Loading(read)),
            Err(e) => {
                error!("{e}");
                note.say(time.elapsed_secs(), true, e);
            }
        }
    }
}

/// `Startup`: the text of `ruby/data.rb` as it was read, so that the editor has something to show
/// and Revert has something to go back to. It is read again rather than kept by
/// [`read_the_data_stage`], because that one runs whether or not there is a window and a string
/// nobody reads is a string nobody should be holding.
fn read_the_data_file_text(mut file: ResMut<window::DataFile>, ruby: Res<RubyDir>) {
    file.text = platform::read(&ruby.0.join(DATA_FILE)).unwrap_or_default();
}

/// **`data.rb`, read again while the game runs** — the stage's own point, and what makes the size
/// of the map something a player can change in a page without restarting anything.
///
/// The three outcomes are written out on [`window`]; what is here is the arithmetic of the third,
/// which is **what the world is indexed by**. A tile is a place on a map of a particular size, an
/// item on a belt is a number into `data.rb`'s items, and a machine on the grid is a number into
/// its machines. Change any of those and every number in the world means something else, so the
/// world has to be laid out again — and laying it out again loses what was built, which is not a
/// thing to do to somebody who pressed a button once. Everything else (a belt's speed, a recipe's
/// time, a chest's capacity) is read out of [`Rules`] every frame, so swapping the tables is the
/// whole of applying it.
#[allow(clippy::too_many_arguments)]
fn reread_the_data_stage(
    mut commands: Commands,
    mut asked: MessageReader<window::Reread>,
    mut file: ResMut<window::DataFile>,
    mut world: ResMut<ScriptWorld>,
    mut rules: ResMut<Rules>,
    mut data: ResMut<Data>,
    mut editor: Option<ResMut<rubevy_egui::Editor>>,
    mut rebuild: MessageWriter<RebuildTheWorld>,
) {
    let Some(window::Reread(text)) = asked.read().last().cloned() else { return };
    let read = data::read_the_declarations(&mut world.vm, DATA_FILE, &text, platform::compile);
    let (tables, new_rules) = match read {
        Ok(both) => both,
        Err(trouble) => {
            // **the world is not touched at all**: the tables in use are still the ones that
            // built it, and what is wrong is said with the line of the player's own file it is on
            let says = trouble.say(DATA_FILE);
            error!("{says}");
            file.trouble = Some(says.clone());
            file.confirming = None;
            if let Some(editor) = editor.as_mut() {
                editor.message = format!("not applied — {}", says.lines().next().unwrap_or(&says));
            }
            return;
        }
    };
    let changes_the_world = what_the_world_is_built_on(&rules, &data, &new_rules, &tables);
    if let Some(why) = &changes_the_world
        && file.confirming.as_deref() != Some(text.as_str())
    {
        // **one click of confirmation**, and it is asked for rather than assumed: a player who
        // widened the map does mean to rebuild, and a player who changed a recipe's time and
        // happened to add an item does not mean to lose the factory
        file.confirming = Some(text.clone());
        let says = format!("this rebuilds the world and loses what is built — {why}. Apply again to do it");
        warn!("{DATA_FILE}: {says}");
        if let Some(editor) = editor.as_mut() {
            editor.message = says;
        }
        return;
    }
    file.confirming = None;
    file.trouble = None;
    file.text = text.clone();
    file.in_memory = true;
    data::expose_the_tables(&mut world.vm, &tables);
    *rules = new_rules;
    *data = tables;
    let says = match &changes_the_world {
        Some(why) => {
            rebuild.write(RebuildTheWorld);
            format!("read again, and the world is laid out anew — {why}")
        }
        None => "read again; the factory goes on with the new numbers".into(),
    };
    info!("{DATA_FILE}: {says}");
    if let Some(editor) = editor.as_mut() {
        editor.applied(says);
    }
    // the arms' programs carry the item names and the swing, which the game writes in front of
    // the prelude out of these tables — so every one of them is compiled again
    commands.run_system_cached(forget_the_compiled_scripts);
}

/// Every arm's program is thrown away, because the block the game writes in front of the prelude
/// is made of `data.rb`'s own names and numbers (`inserters::names_and_numbers`).
fn forget_the_compiled_scripts(mut minds: ResMut<inserters::Minds>) {
    minds.forget_the_programs();
}

/// **What the world is built on**, and the sentence saying which of it moved — or `None` where
/// nothing did, which is the case a factory goes on running through.
fn what_the_world_is_built_on(
    rules: &Rules,
    data: &Data,
    new_rules: &Rules,
    new_data: &Data,
) -> Option<String> {
    if rules.map_tiles != new_rules.map_tiles {
        return Some(format!(
            "the map was {} by {} and is now {} by {}",
            rules.map_tiles.x, rules.map_tiles.y, new_rules.map_tiles.x, new_rules.map_tiles.y
        ));
    }
    let ore_of = |r: &Rules| (r.ore_per_tile, r.ore_patch_radius.to_bits(), r.ore_patches);
    if ore_of(rules) != ore_of(new_rules) {
        return Some("the ore in the ground is laid out differently".into());
    }
    let names = |d: &Data| {
        (
            d.items.iter().map(|i| i.name.clone()).collect::<Vec<_>>(),
            d.machines.iter().map(|m| m.name.clone()).collect::<Vec<_>>(),
            d.recipes.iter().map(|r| r.name.clone()).collect::<Vec<_>>(),
        )
    };
    (names(data) != names(new_data))
        .then(|| "the items, machines or recipes are not the same list".into())
}

/// The game's own message for "lay the land again". It is a message rather than a call because
/// what it does — despawn the chunks, remake the grid, point the camera — is several systems'
/// work and only a window has two of them.
#[derive(Message, Debug, Clone, Copy)]
pub struct RebuildTheWorld;

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

/// Where one step of the factory happens, so that everything else can say whether it is before or
/// after it. Building is before (a belt laid this frame carries this frame) and the picture is
/// after (what is drawn is where things are now).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactorySet {
    Step,
}

/// **The world, before anything is built on it**: the grid, the ore and the empty lanes.
///
/// It runs after the data stage and only when that left a [`Rules`] behind, which is what lets the
/// ore in the ground be a number of play: `ore_per_tile` is `data.rb`'s since F2a, and this is the
/// one place that reads it.
fn lay_the_land(mut commands: Commands, rules: Res<Rules>, data: Res<Data>) {
    let map = Map { tiles: rules.map_tiles };
    let ore =
        Ore::laid_out(map.tiles, rules.ore_patch_radius, rules.ore_patches, rules.ore_per_tile);
    info!(
        "a map of {} by {} tiles, {} of them with ore in ({} in the ground, in {} patches of radius {})",
        map.tiles.x,
        map.tiles.y,
        ore.tiles_with_ore(),
        ore.total(),
        rules.ore_patches.x * rules.ore_patches.y,
        rules.ore_patch_radius,
    );
    // **What a map of this size costs to hold**, said rather than refused: how much memory there
    // is is a fact about the machine — a page in a phone's browser and a PC are not the same
    // number — so the data stage's ceiling is the drawing's alone (`draw::MOST_TILES_ACROSS`) and
    // this is how a player finds out what they asked for. It is counted from the types rather
    // than written down, so it cannot go stale: every one of these is one per tile.
    let per_tile = size_of::<Option<Building>>()
        + size_of::<std::collections::VecDeque<OnBelt>>()
        + size_of::<belts::Steps>()
        + size_of::<u32>()
        + size_of::<Dir>();
    info!(
        "{} tiles is about {:.1} MB of grid, lanes and ore ({} bytes a tile)",
        grid::how_many(map.tiles),
        (grid::how_many(map.tiles) * per_tile) as f32 / (1024.0 * 1024.0),
        per_tile,
    );
    let machines: Vec<String> = data
        .machines
        .iter()
        .enumerate()
        .map(|(i, m)| format!("{} {}", i + 5, m.name))
        .collect();
    info!(
        "keys: 1 belt, 2 miner, 3 chest, 4 inserter, {}, 0 take away, R turn; click to build, and click an inserter to write its Ruby",
        machines.join(", ")
    );
    commands.insert_resource(Grid::new(map.tiles));
    commands.insert_resource(Lanes::for_map(map.tiles));
    commands.insert_resource(ore);
    commands.insert_resource(map);
}

/// **The world, laid out again** — one system, so that there is one road from "the declarations
/// changed" to "the factory is this world now" (F3a's rule: `lay_the_land` is the only thing that
/// makes a world).
///
/// **It loses what was built**, which is why it is asked for twice ([`reread_the_data_stage`]).
/// The pieces it takes away are the three things whose length is the map's: the grid, the lanes
/// and the ore. In a window two more follow it and they are separate systems because only a
/// window has them at all — the chunks the floor is drawn into ([`draw::start_drawing`]) and the
/// camera's edges ([`point_the_camera_at_the_map`]).
fn lay_the_land_again(
    mut commands: Commands,
    mut asked: MessageReader<RebuildTheWorld>,
    rules: Res<Rules>,
    data: Res<Data>,
) {
    if asked.read().next().is_none() {
        return;
    }
    // the arms go with the grid: `keep_the_crew` despawns every entity whose tile is not an
    // inserter any more, and after this none of them is
    lay_the_land(commands.reborrow(), rules, data);
}

/// The window's half of a rebuild: the floor's chunks are the size of the map, so the old ones go
/// and [`draw::start_drawing`] makes new ones, and the camera is told where the new edges are.
fn draw_the_new_world(
    mut commands: Commands,
    mut asked: MessageReader<RebuildTheWorld>,
    chunks: Option<Res<draw::Chunks>>,
    mut pool: ResMut<items::Pool>,
) {
    if asked.read().next().is_none() {
        return;
    }
    if let Some(chunks) = chunks {
        commands.entity(chunks.floor).despawn();
        commands.entity(chunks.buildings).despawn();
        commands.remove_resource::<draw::Chunks>();
    }
    // the item sprites are borrowed from a pool that is refilled as the lanes need it; the
    // entities are still good, but the pool is rebuilt with the rest so that nothing is holding a
    // sprite for an item that is not there any more
    for sprite in pool.sprites.drain(..) {
        commands.entity(sprite).despawn();
    }
}

/// **The camera, given a world to look at** — which since F3a is not known until the data stage
/// has run, so it is said here rather than where the plugin is added.
///
/// Three things, and each of them is the map's and not a number this game preferred:
///
/// * **how far the camera may be walked**: the map, and no further. `CameraControls::bounds` is
///   the shared crate's way of being told, because a world's edge is not something it can guess.
/// * **how much world the window holds at rest**: `camera_half_height`, which is a setting and is
///   derived from the *window* (whole-number zoom, see [`CAMERA_HALF_HEIGHT`]) — **except that it
///   never shows more world than there is**. A map sixteen tiles tall is 128 world units, and
///   showing 150 of them would be a strip of nothing above and below the world. That `min` is a
///   fact about the map, which is why it is here.
/// * **how far the wheel may go out**: the shared camera's limits are a multiple of the view at
///   rest (`games_shell::camera::zoom_by`), which on a big map would mean never being able to see
///   where anything is. The limit is raised — never lowered — until the whole map fits, so that
///   a player on a 512-tile map can zoom out to the whole of it and a player on the default map
///   has exactly what they had before.
fn point_the_camera_at_the_map(
    map: Res<Map>,
    mut controls: ResMut<CameraControls>,
    mut home: ResMut<games_shell::camera::CameraHome>,
    mut view: ResMut<games_shell::camera::CameraView>,
) {
    let half = map.span() / 2.0;
    controls.bounds = Some(Rect::from_center_half_size(Vec2::ZERO, half));
    home.half_height = home.half_height.min(half.y);
    view.half_height = home.half_height;
    // the wheel measures in multiples of the view at rest, so "the whole map" is that ratio —
    // and the taller of the two sides, because the window's own shape decides which one runs out
    // first and showing a little more than the map is not a fault
    let to_the_whole_map = half.max_element() / home.half_height.max(f32::MIN_POSITIVE);
    controls.zoom_out_limit = controls.zoom_out_limit.max(to_the_whole_map);
    info!(
        "the camera shows {:.0} world units top to bottom and may be walked over {} by {} tiles",
        home.half_height * 2.0,
        map.tiles.x,
        map.tiles.y
    );
}

/// **The keys that have moved out of the store**, and the declaration each is now — the table the
/// sentence below is built from, so that a key that moves later is one line here.
const MOVED_TO_THE_DATA_FILE: [(&str, &str); 2] = [
    ("map_tiles", "map :world, size: [w, h]"),
    ("ore_patch_radius", "ore :iron_ore, patch_radius:"),
];

/// **A setting that has moved says so once, rather than being ignored in silence** (F3a).
///
/// `map_tiles` and `ore_patch_radius` were keys of `factory.settings.txt` until F3a and are
/// declarations now, so a store written by an older build has lines in it that no longer do
/// anything. A player who wrote one of them meant it, and a number quietly ignored is the worst
/// of the three things that can happen to it.
///
/// The line is left where it is: nothing here writes to the store, and a file rewritten behind
/// somebody's back is worse again. What is said is where the number lives now.
fn moved_settings(settings: &games_shell::Settings) -> Vec<String> {
    MOVED_TO_THE_DATA_FILE
        .iter()
        .filter_map(|(key, now)| {
            let value = settings.get(key)?;
            Some(format!(
                "{key}={value} in {} does nothing any more: it is `{now}` in {DATA_FILE} now, which is where the size of the world is said since F3a",
                platform::SETTINGS_FILE,
            ))
        })
        .collect()
}

fn say_where_the_map_went(settings: Res<games_shell::Settings>) {
    for says in moved_settings(&settings) {
        warn!("{says}");
    }
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
    // a run that asked only for arms gets only arms: a loop of belt round them would be the
    // belts' measurement and the scripts' measuring each other
    if stress.items == 0 {
        return;
    }
    // the inside of the map, and an even number of rows so that the serpentine comes back to the
    // left-hand column rather than to the right
    let n = grid.tiles.x - 2;
    let tall = grid.tiles.y - 2;
    let rows = if tall % 2 == 0 { tall } else { tall - 1 };
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
    let most = belts * rules.items_per_tile as usize;
    let wanted = stress.items.min(most);
    let mut laid = 0usize;
    for (i, &t) in grid.built().iter().enumerate() {
        // how many this tile gets, so that the remainder is spread rather than all at the end
        let upto = (wanted * (i + 1)) / belts.max(1);
        let mut along = rules.tile();
        while laid < upto {
            lanes.put_on(t as usize, OnBelt { along, item: rules.digs });
            along -= spacing;
            laid += 1;
        }
    }
    info!(
        "stress: a loop of {} belts, {} items asked for, {} laid ({} is the most they hold)",
        belts, stress.items, laid, most
    );
}

/// **The stress run's inserters**: `arms` little shuttles, each one a stocked chest, an arm and an
/// empty chest, laid in rows above the snake.
///
/// A shuttle rather than a belt because what is being measured is the **scripts**: every arm in it
/// has something behind it and room in front of it for as long as the stock lasts, so every one of
/// them runs the busy path of `ruby/inserter.rb` — read, decide, swing — rather than the idle one.
/// How long that lasts is the chest's own capacity over the arm's own rate, which at what
/// `data.rb` says today is sixty seconds, and a stress run says its piece three times in three.
///
/// **They are not in the snake.** An arm standing in the loop would be a hole in it, and then the
/// belts' measurement and the scripts' would be measuring each other.
fn lay_the_arms(stress: Res<Stress>, rules: Res<Rules>, mut grid: ResMut<Grid>) {
    if stress.arms == 0 {
        return;
    }
    // a shuttle is three tiles and a gap, and a row of them has a row's gap above it, so that
    // nothing reaches into its neighbour
    let across = ((grid.tiles.x.saturating_sub(2)) / 4).max(1);
    let mut laid = 0usize;
    'rows: for row in (1..grid.tiles.y - 1).step_by(2) {
        for unit in 0..across {
            if laid == stress.arms {
                break 'rows;
            }
            let x = 1 + unit * 4;
            if x + 2 >= grid.tiles.x {
                break;
            }
            let from = grid.index(UVec2::new(x, row));
            let arm = grid.index(UVec2::new(x + 1, row));
            let to = grid.index(UVec2::new(x + 2, row));
            let mut stocked = Building::new(What::Chest, Dir::East);
            stocked.held.add(rules.digs, rules.chest_capacity);
            grid.place(from, stocked);
            grid.place(arm, Building::new(What::Inserter, Dir::East));
            grid.place(to, Building::new(What::Chest, Dir::East));
            laid += 1;
        }
    }
    info!(
        "stress: {laid} inserters asked for {}, each with {} things to move",
        stress.arms, rules.chest_capacity
    );
}

/// **A stress run sizes its own map**, once the data stage has said how many items fit on a tile.
///
/// The loop it lays holds `items_per_tile` to a tile, so the map it needs is the square root of
/// the items asked for. It is worked out here rather than left to whoever runs it, because a page
/// has no command line and a measurement that cannot be taken in a browser is not a measurement
/// of the browser — and it is worked out *here*, in `Startup`, rather than in `main`, because
/// `items_per_tile` is `data.rb`'s and `main` has no VM yet.
///
/// **It writes the size the data file asked for**, before [`lay_the_land`] reads it, so the grid,
/// the lanes, the chunk and the camera are all made once, at the size that is going to be used.
/// F3 had it write into the `Map` afterwards and patch the camera up; there is nothing to patch
/// now, because nothing has been built from the old number yet.
fn size_the_map_for_the_stress_run(stress: Res<Stress>, mut rules: ResMut<Rules>) {
    let side = (stress.items as f32 / rules.items_per_tile as f32).sqrt().ceil() as u32;
    // **and the arms want room too** (F3): a shuttle is three tiles and a gap across and takes a
    // row of its own with a row's gap above it, so `arms` of them fit on a square of side
    // √(8 × arms) — four tiles by two, per arm
    let for_arms = (stress.arms as f32 * 8.0).sqrt().ceil() as u32;
    let side = side.max(for_arms);
    // an even number of rows, so that the serpentine comes home (`lay_the_snake`)
    let wanted = UVec2::splat(side + side % 2 + 2).max(rules.map_tiles);
    // and no larger than a map may be at all, which is the drawing's limit and applies to a
    // measurement exactly as it applies to a game
    rules.map_tiles = wanted.min(UVec2::splat(draw::MOST_TILES_ACROSS));
}

/// The stress run's own report: the frame times and what the factory's own step took inside them.
#[allow(clippy::too_many_arguments)]
fn watch_the_frames(
    time: Res<Time<Real>>,
    tally: Res<Tally>,
    grid: Res<Grid>,
    world: Res<ScriptWorld>,
    crew: inserters::Crew,
    mut stress: ResMut<Stress>,
    errands: Res<platform::Errands>,
    mut exit: MessageWriter<AppExit>,
) {
    stress.seen.push(time.delta_secs() * 1000.0);
    let step = tally.last_step_us;
    stress.steps.push(step);
    // **the VM's own frame, kept per frame rather than read off at the end** (`FrameStats` is the
    // last tick and not a total, rubevy `docs/host-api.md`). Instructions are the number to watch:
    // they are a fact about the scripts and are the same on a machine several times slower,
    // where milliseconds are a fact about the machine.
    let f = world.last_frame();
    stress.instructions.push(f.instructions as f32);
    stress.ticks.push(f.time_ns as f32 / 1000.0);
    stress.woke.push((f.reflect_answers + f.in_tick_answers) as f32);
    stress.carried.push((f.carried_reflect + f.carried_in_tick) as f32);
    stress.dropped += f.dropped;
    if stress.seen.len() < stress.every as usize {
        return;
    }
    let frame = spread(&mut stress.seen);
    let inner = spread(&mut stress.steps);
    let insn = spread(&mut stress.instructions);
    let tick = spread(&mut stress.ticks);
    let woke = spread(&mut stress.woke);
    let carried = spread(&mut stress.carried);
    info!(
        "stress: items={} belts={} arms={} frame ms p50 {:.2} p95 {:.2} (about {:.0} fps) | step us p50 {:.0} p95 {:.0}",
        tally.items,
        grid.built().len(),
        crew.how_many(),
        frame.0,
        frame.1,
        1000.0 / frame.0.max(0.001),
        inner.0,
        inner.1,
    );
    info!(
        "stress: vm insn p50 {:.0} p95 {:.0} of {} | tick us p50 {:.0} p95 {:.0} | answers p50 {:.0} p95 {:.0} | carried p50 {:.0} p95 {:.0} | dropped {} | programs {}",
        insn.0,
        insn.1,
        world.budget,
        tick.0,
        tick.1,
        woke.0,
        woke.1,
        carried.0,
        carried.1,
        stress.dropped,
        world.loaded_programs(),
    );
    stress.seen.clear();
    stress.steps.clear();
    stress.instructions.clear();
    stress.ticks.clear();
    stress.woke.clear();
    stress.carried.clear();
    stress.said += 1;
    // **A stress run is a third thing a run can be asked for, and it ends the same way**: when it
    // has said its reports and nothing else this run was told to do is unfinished (S11). Before
    // that it asked only whether the platform could exit, which in a run that was also asked for
    // a picture is the same lie `--shot` and the checks told each other (`platform::Errands`).
    if stress.said >= STRESS_REPORTS && errands.left().is_none() {
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

/// `--shot FILE SECONDS`: the picture, and then — since S11 — nothing else.
///
/// **A run that is also being checked does not end until the checks have finished.** F1 made the
/// checks wait for the camera (they used to end the run before the picture's moment came); F2
/// found the other half of the same thing — its checks take thirteen seconds of the game's own
/// time and the camera's moment is at eight, so the picture was ending the run with four of the
/// lines unsaid. Whichever of the two is still working keeps the run alive, and since S11 it is
/// `games_shell::checks::Errands` that knows which, for all three games rather than this one.
fn take_shot(
    mut commands: Commands,
    time: Res<Time>,
    mut shot: ResMut<Shot>,
    mut errands: ResMut<platform::Errands>,
) {
    if shot.taken {
        if time.elapsed_secs() > shot.after + 1.0 {
            errands.the_picture_is_taken();
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
fn stop_when_over(
    time: Res<Time>,
    headless: Res<Headless>,
    file: Res<save::SaveFile>,
    mut asked: ResMut<save::Asked>,
    mut exit: MessageWriter<AppExit>,
) {
    if time.elapsed_secs() < headless.until {
        return;
    }
    // **`--save PATH` is written as the run ends**, on this frame: the save system is ordered
    // after the step and this is `Update`, so what reaches the file is the world of the last
    // frame this run had rather than that world plus one.
    if file.on_exit && !asked.save {
        asked.save = true;
        return;
    }
    info!("headless: {:.1} s, done", time.elapsed_secs());
    exit.write(AppExit::Success);
}

// ---------------------------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------------------------

/// `FACTORY_SELFTEST=1` on a PC, `factory/?selftest` in a page. One line per thing proved, in the
/// shape the other two games print and `tools/fixedlines.sh` compares
/// (`docs/verification/selftest-lines.md`).
///
/// **It builds its factory the way a player does**, because that is the only road there is: the
/// checks write a [`build::Order`] — the tile, the thing, the way round — and the same system
/// that carries out a mouse's order carries out theirs. F2 forged the mouse itself (a
/// `WorldClick` and a `Hand` in the same frame) and paid for it with an ordering line and a
/// flaky window run; F3 moved the seam one system along, to where what was meant is written down.
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
    /// **What is left to build.** One tile a frame, which since F3 is a convenience and not a
    /// rule: an order carries what was meant, so five of them in one frame build five different
    /// things. It stays one a frame because a check that builds a tile a frame is a check whose
    /// log says which tile went wrong.
    to_build: Vec<(UVec2, Option<What>)>,
    /// **The arms, kept back on purpose.** The lines are built without them, watched until they
    /// stop at the machine's edge, and only then joined up — which is the whole of F3 said as a
    /// check.
    arms_to_build: Vec<(UVec2, Option<What>)>,
    /// The tiles the inserters ended up on, for the checks that are about them.
    arms: Vec<UVec2>,
    /// The one arm given a script that will not do (step 20), and then taken away (step 22).
    broken: Option<usize>,
    /// How many arms had a script before one of them was taken away — and, later, how many
    /// programs the VM was holding before the editor applied one.
    before: usize,
    /// What every arm but the one in the panel was running, so that "and left the others alone"
    /// is a comparison and not a hope.
    others: Vec<String>,
    /// Frames spent waiting for the panel to catch up ([`PANEL_WAIT_FRAMES`]).
    waited: u32,
    /// **F4**: what the control stage's own subscriptions had lost while one was running, read
    /// before the checks break it on purpose.
    dropped_in_the_script: u64,
    done: bool,
}

/// **The steps the control stage's checks are**, which are a system of their own
/// ([`control_checks`]): a Bevy system may take sixteen parameters and [`selftest`] takes sixteen.
/// The first of them is where [`selftest`] hands over and the last is where it takes the run back
/// to say it is done.
const CONTROL_CHECKS: std::ops::Range<u8> = 31..40;

/// **A script that does nothing but wait**, for the editor to apply. It compiles and runs, which
/// is what is being measured — in a browser that means the page's own compiler was called
/// synchronously and answered — and what it does is of no interest here.
const AN_IDLE_SCRIPT: &str = concat!(
    "inserter \"Idle\" do\n",
    "  def run\n",
    "    loop { idle }\n",
    "  end\n",
    "end\n",
);

/// **A script that will not do**, for the check that says one of those stops one arm and nothing
/// else. It raises rather than failing to compile because a raise is the harder half: the program
/// loads, the task starts, and what goes wrong goes wrong in the middle of a frame with the rest
/// of the factory running.
const A_BROKEN_SCRIPT: &str = concat!(
    "inserter \"Broken\" do\n",
    "  def run\n",
    "    loop do\n",
    "      nothing.at.all\n",
    "    end\n",
    "  end\n",
    "end\n",
);

/// One of the two little factories: three belts, an arm, a machine, an arm, a belt and a chest.
///
/// **The two arms are F3's**, and the reason there are five tiles here rather than two is that a
/// line is looked at twice — once with the machine standing next to a belt that will not feed it,
/// and once with the arms in place.
#[derive(Debug, Clone)]
struct MachineCheck {
    what: String,
    makes: data::ItemId,
    /// which recipe it will run, so that the belt that feeds it can be given what it eats
    /// without working out where the line is a second time (`seed_the_machine_lines`)
    recipe: data::RecipeId,
    made_of: String,
    /// the machine's own tile
    at: usize,
    /// the belt that runs up to it, which is what jams while there is no arm
    feed: usize,
    /// where the arm that feeds it goes — the same tile the feeding belt is on, because the belt
    /// is replaced by the arm
    arm_in: usize,
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
    mut crew: inserters::Crew,
    mut orders: MessageWriter<build::Order>,
    // S11: what this run was asked for, and what is left of it — the checks are one errand of
    // two and no longer end the run themselves (`games_shell::checks::Errands`)
    mut errands: ResMut<platform::Errands>,
) {
    if test.done {
        return;
    }
    // **F4's are next door** ([`control_checks`]), because this system already takes the sixteen
    // parameters a Bevy system may take. It carries the run on from where this one left it and
    // hands it back at the end of [`CONTROL_CHECKS`], where the `_` arm below says it is done.
    if CONTROL_CHECKS.contains(&test.step) {
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
            let ok = patches > 0 && ore.total() == patches as u64 * rules.ore_per_tile as u64;
            say(
                if ok { "ok  " } else { "FAIL" },
                &format!(
                    "the map is {} by {} tiles with {} of them holding {} of ore",
                    map.tiles.x,
                    map.tiles.y,
                    patches,
                    ore.total()
                ),
            );
            test.step = 1;
        }
        // ---- the arithmetic the building is built on -----------------------------------------
        1 => {
            let last_tile = map.tiles - UVec2::ONE;
            let corners = [
                UVec2::new(0, 0),
                UVec2::new(last_tile.x, 0),
                UVec2::new(0, last_tile.y),
                last_tile,
                map.tiles / 2,
            ];
            let agreed =
                corners.iter().filter(|&&t| map.tile_at(map.tile_centre(t)) == Some(t)).count();
            let off = map.tile_at(map.span()).is_none();
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
            // **a tile with no ore in it, asked for rather than worked out** (F4). It was the
            // middle of the map, which is bare for an even number of patches and is the middle of
            // one for an odd number — and how many there are is a line of `ruby/data.rb`.
            let Some(bare) = somewhere_clear(&map, &grid, &ore, UVec2::ONE) else {
                say("FAIL", "there is nowhere on this map without ore in it");
                test.step = 5;
                return;
            };
            orders.write(build::Order { at: bare, what: Some(What::Miner), dir: Dir::East });
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
            // **the middle of the first patch of ore**, asked of the thing that put it there
            // rather than written out as "a quarter of the map": how many patches there are is
            // `data.rb`'s since F3a, and a check that knows where the ore is because it has done
            // the same arithmetic is a check that stops being true when the arithmetic changes
            let pit =
                Ore::patch_middle(map.tiles, rules.ore_patches, UVec2::ZERO).floor().as_uvec2();
            test.line = vec![pit];
            orders.write(build::Order { at: pit, what: Some(What::Miner), dir: Dir::East });
            test.ore_before = ore.total();
            test.step = 6;
        }
        6 | 7 | 8 => {
            // three belts, one a frame, running east from the miner
            let next = UVec2::new(test.line[0].x + test.step as u32 - 5, test.line[0].y);
            orders.write(build::Order { at: next, what: Some(What::Belt), dir: Dir::East });
            test.line.push(next);
            test.step += 1;
        }
        9 => {
            let chest = UVec2::new(test.line[0].x + 4, test.line[0].y);
            orders.write(build::Order { at: chest, what: Some(What::Chest), dir: Dir::East });
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
            orders.write(build::Order { at: belt, what: None, dir: Dir::East });
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
            // the two little factories: two belts, a machine, a belt and a chest each — and the
            // two arms each needs, which are built later, on purpose (step 17)
            match lay_out_the_machine_lines(&map, &data, &rules, &grid, &ore, &mut test) {
                true => test.step = 16,
                false => {
                    say("FAIL", "there was nowhere to build the machines the data file declares");
                    test.step = 20;
                }
            }
        }
        // one tile a frame: five orders in one frame would build in one frame, but a check whose
        // log says which tile went wrong is worth more than four frames
        16 if !test.to_build.is_empty() => {
            let (tile, what) = test.to_build.remove(0);
            orders.write(build::Order { at: tile, what, dir: Dir::East });
        }
        16 => {
            // the frame after the orders: the buildings are there, so the feeding belts can be
            // loaded with what each machine eats
            let lines = test.machines.clone();
            seed_the_machine_lines(&data, &rules, &mut lanes, &lines);
            test.started = time.elapsed_secs();
            test.step = 17;
        }
        // ---- F3: with no arm beside it, a machine takes nothing ------------------------------
        17 => {
            // **waited for as a condition and not as a length of time**: the claim is that the
            // belt in front of the machine stops, so what is waited for is a belt whose front
            // item has reached the end of its tile with the machine still empty. The bound is
            // what the belt's own numbers say two tiles take.
            let waited = time.elapsed_secs() - test.started;
            let jammed = test
                .machines
                .iter()
                .filter(|m| {
                    lanes.on(m.feed).front().is_some_and(|on| on.along >= rules.tile())
                        && grid.at(m.at).is_some_and(|b| b.held.is_empty() && b.made.is_empty())
                })
                .count();
            let bound = 2.0 / rules.belt_tiles_per_second * CHECK_SLACK;
            if jammed < test.machines.len() && waited < bound {
                return;
            }
            say(
                if jammed == test.machines.len() { "ok  " } else { "FAIL" },
                &format!(
                    "with no inserter in the gap nothing reaches the machine: {}/{} lines are jammed on the belt with the machine empty",
                    jammed,
                    test.machines.len()
                ),
            );
            test.step = 18;
        }
        // and now the arms, which are what joins the line up
        18 if !test.arms_to_build.is_empty() => {
            let (tile, what) = test.arms_to_build.remove(0);
            test.arms.push(tile);
            orders.write(build::Order { at: tile, what, dir: Dir::East });
        }
        18 => {
            test.started = time.elapsed_secs();
            test.step = 19;
        }
        19 => {
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
            // every arm the lines needed has a script, and no more than that
            say(
                if crew.how_many() == test.arms.len() { "ok  " } else { "FAIL" },
                &format!(
                    "every inserter on the map has a script of its own: {} arms, {} scripts",
                    test.arms.len(),
                    crew.how_many()
                ),
            );
            test.step = 20;
        }
        // ---- F3: a script that will not do stops one arm and nothing else --------------------
        20 => {
            // **the first line's feeding arm**, given a script of its own that raises. It is one
            // arm and not all of them on purpose: what is being measured is that the other line
            // goes on working.
            let broken = test.machines[0].arm_in;
            crew.minds.give_to_one(broken, A_BROKEN_SCRIPT.to_string());
            test.broken = Some(broken);
            test.started = time.elapsed_secs();
            test.step = 21;
        }
        21 => {
            let Some(broken) = test.broken else {
                test.step = 22;
                return;
            };
            let waited = time.elapsed_secs() - test.started;
            // it has to be started, run and raised, which is a frame or three; the bound is a
            // swing, which is the slowest thing an arm does
            if !crew.arms.has_stopped(broken) && waited < rules.swing_seconds * CHECK_SLACK {
                return;
            }
            let at = crew.arms.stopped_at(broken).unwrap_or("nowhere").to_string();
            say(
                // **the place as well as the fact**: a task that has ended has no frames left to
                // ask, so an arm that says only "stopped" is an arm whose script the player has
                // to find the fault in by reading it (`inserters::where_it_broke`)
                if at.starts_with(inserters::SCRIPT_FILE) && !at.ends_with('?') { "ok  " } else { "FAIL" },
                &format!(
                    "an inserter whose script raises stops, and the game knows where: {at} (after {waited:.1} s)"
                ),
            );
            // and the game is still running: the *other* line's chest goes on filling
            let other = test.machines.last().cloned();
            let held = other
                .as_ref()
                .map(|m| grid.at(m.chest).map(|b| b.held.of(m.makes)).unwrap_or(0))
                .unwrap_or(0);
            let stopped = crew.arms.how_many_stopped();
            say(
                if stopped == 1 && held > 0 { "ok  " } else { "FAIL" },
                &format!(
                    "the rest of the factory is untouched: {stopped} of {} inserters stopped, and the {} line has {held} in its chest",
                    crew.how_many(),
                    other.map(|m| m.what).unwrap_or_default()
                ),
            );
            test.step = 22;
        }
        // ---- F3: and an arm that is taken away leaves nothing behind -------------------------
        22 => {
            let Some(gone) = test.broken else {
                test.step = 24;
                return;
            };
            test.before = crew.how_many();
            orders.write(build::Order { at: grid.tile_of(gone), what: None, dir: Dir::East });
            test.step = 23;
        }
        23 => {
            let Some(gone) = test.broken else {
                test.step = 24;
                return;
            };
            let ok = grid.at(gone).is_none()
                && crew.at(gone).is_none()
                && !crew.arms.has_stopped(gone)
                && crew.how_many() + 1 == test.before;
            say(
                if ok { "ok  " } else { "FAIL" },
                &format!(
                    "an inserter taken away leaves no script behind: {} scripts where there were {}, and none of them is waiting on an arm that is gone ({})",
                    crew.how_many(),
                    test.before,
                    crew.how_many_waiting()
                ),
            );
            test.step = 24;
        }
        // ---- F3: the panel, where there is one -----------------------------------------------
        24 => {
            // **A run with no window has no `Editor` at all**, so there is nothing here to
            // measure and saying `ok` would be claiming a check that never ran. It is known on
            // the first frame, so it says so at once rather than sitting out a wait.
            // **the last arm and not the first**: the first is the one step 20 gave a broken
            // script to and step 22 took away, and a check that drives a tile with nothing on it
            // is a check about nothing. (It passed in a window and failed in a page, which is
            // what a check that leans on one system running before another looks like.)
            let arm = test.arms.last().map(|&t| grid.index(t));
            let (Some(arm), true) = (arm, crew.panel.is_some()) else {
                say("--  ", "the editor was not driven (this run has no window)");
                test.step = 29;
                return;
            };
            // what the rest are running, to say afterwards that they still are
            test.others = crew
                .standing
                .iter()
                .filter(|(_, i)| i.tile != arm)
                .map(|(_, i)| crew.minds.text_for(i.tile).to_string())
                .collect();
            test.before = world.loaded_programs();
            test.broken = Some(arm);
            // **the panel is opened the way a player opens it**: an order that would build an
            // inserter on a tile that already has one is a click on that inserter
            // (`crate::window::follow_the_orders`), and nothing is rebuilt.
            orders.write(build::Order {
                at: grid.tile_of(arm),
                what: Some(What::Inserter),
                dir: Dir::East,
            });
            test.waited = 0;
            test.step = 25;
        }
        25 => {
            let Some(arm) = test.broken else {
                test.step = 29;
                return;
            };
            // **waited for and not assumed.** The order is read by the system that opens the
            // panel on the frame it is written or the one after — a Bevy message is readable for
            // two frames — and the panel is filled in that same frame, so two frames is the
            // answer and [`PANEL_WAIT_FRAMES`] is the bound with room.
            let opened = crew.panel.as_ref().and_then(|p| p.key) == Some(arm as u64);
            test.waited += 1;
            if !opened && test.waited < PANEL_WAIT_FRAMES {
                return;
            }
            say(
                if opened { "ok  " } else { "FAIL" },
                "clicking an inserter with an inserter in hand opens its script rather than building over it",
            );
            if let Some(panel) = crew.panel.as_mut() {
                panel.text = AN_IDLE_SCRIPT.to_string();
                panel.action = Some(rubevy_egui::EditorAction::Apply);
            }
            test.waited = 0;
            test.step = 26;
        }
        26 => {
            let Some(arm) = test.broken else {
                test.step = 29;
                return;
            };
            // the action is taken on the frame it is set, by a system after this one, and the
            // program is compiled there — **in a browser that is the page's own compiler, called
            // synchronously**, which is the half of this check only a page can fail
            let mine = crew.minds.text_for(arm) == AN_IDLE_SCRIPT;
            let others: Vec<String> = crew
                .standing
                .iter()
                .filter(|(_, i)| i.tile != arm)
                .map(|(_, i)| crew.minds.text_for(i.tile).to_string())
                .collect();
            let grew = world.loaded_programs();
            // **what is waited for is the whole of what is said**, and the dearest part of it is
            // the VM holding one more program, which is three frames after the button
            test.waited += 1;
            let ready = mine && others == test.others && grew > test.before;
            if !ready && test.waited < PANEL_WAIT_FRAMES {
                return;
            }
            test.waited = 0;
            say(
                if mine && crew.minds.is_its_own(arm) && others == test.others && grew > test.before {
                    "ok  "
                } else {
                    "FAIL"
                },
                &format!(
                    "the editor applied a script to one inserter and left the other {} alone ({} programs in the VM, was {})",
                    others.len(),
                    grew,
                    test.before
                ),
            );
            test.before = grew;
            if let Some(panel) = crew.panel.as_mut() {
                panel.action = Some(rubevy_egui::EditorAction::ApplyAll);
            }
            test.step = 27;
        }
        27 => {
            let all =
                crew.standing.iter().all(|(_, i)| crew.minds.text_for(i.tile) == AN_IDLE_SCRIPT);
            test.waited += 1;
            if !all && test.waited < PANEL_WAIT_FRAMES {
                return;
            }
            test.waited = 0;
            // **the same text is the same program**: applying it to every arm loads nothing new,
            // which is what "one program, one irep" means where it is spent (rubevy)
            let grew = world.loaded_programs();
            say(
                if all && grew == test.before { "ok  " } else { "FAIL" },
                &format!(
                    "Apply to every inserter reached all {} of them and loaded no new program ({} in the VM)",
                    crew.how_many(),
                    grew
                ),
            );
            if let Some(panel) = crew.panel.as_mut() {
                panel.action = Some(rubevy_egui::EditorAction::Revert);
            }
            test.step = 28;
        }
        28 => {
            let file = crew.minds.file().to_string();
            let back = crew.standing.iter().all(|(_, i)| crew.minds.text_for(i.tile) == file);
            test.waited += 1;
            if !back && test.waited < PANEL_WAIT_FRAMES {
                return;
            }
            test.waited = 0;
            say(
                if back { "ok  " } else { "FAIL" },
                &format!(
                    "Revert put all {} of them back on {}",
                    crew.how_many(),
                    inserters::SCRIPT_FILE
                ),
            );
            test.step = 29;
        }
        // ---- a data file that is wrong says which line it is wrong on ------------------------
        29 => {
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
            test.step = 30;
        }
        // ---- and the tables can be read back from Ruby ----------------------------------------
        30 => {
            let asked = read_the_tables_back(&mut world.vm, &data);
            say(
                if asked.is_some() { "ok  " } else { "FAIL" },
                &format!(
                    "a script reads the tables back: {}",
                    asked.unwrap_or_else(|| "it could not".into())
                ),
            );
            test.step = 31;
        }
        _ => {
            test.done = true;
            // **The checks have said their last word, which is not the same as the run being
            // over** (S11). F0 noticed that `--shot` and the checks could not be used together —
            // the checks ended the run before the picture's moment came — and F1 mended it here,
            // in this game, by asking whether a picture had been asked for. Both directions of
            // that are `games_shell::checks::end_the_run`'s now, in one place for three games,
            // and the `done` line with its reason is written there.
            errands.the_checks_are_done();
        }
    }
}

/// **F4's checks: the control stage** — the events reaching it, a goal being reached, the same
/// factory *not* reaching another one, a control.rb that will not run, and nothing dropped.
///
/// It is a system of its own because [`selftest`] takes the sixteen parameters a Bevy system may
/// take; the two share the `SelfTest` resource and hand the run to each other by its `step`
/// ([`CONTROL_CHECKS`]). Like [`selftest`] it runs **before the orders are carried out**, so that
/// a check that gives an order on one frame and looks at what it did on the next is on the same
/// side of the building in every frame.
#[allow(clippy::too_many_arguments)]
fn control_checks(
    mut test: ResMut<SelfTest>,
    time: Res<Time>,
    rules: Res<Rules>,
    grid: Res<Grid>,
    ore: Res<Ore>,
    control: Res<control::TheControl>,
    scripts: Res<ScriptWorld>,
    mut rewrite: MessageWriter<control::Rewrite>,
    mut orders: MessageWriter<build::Order>,
) {
    if !CONTROL_CHECKS.contains(&test.step) {
        return;
    }
    // **what the miner's line takes to put one more thing in its chest**, which is the bound of
    // every wait below: a dig and the three tiles of belt and the step into the chest, which is
    // the same sentence F1's own check is written with
    let a_delivery = (rules.mine_seconds + 4.0 / rules.belt_tiles_per_second) * CHECK_SLACK;
    let in_the_chest = |test: &SelfTest| {
        test.line
            .last()
            .and_then(|&t| grid.at(grid.index(t)))
            .map(|b| b.held.count())
            .unwrap_or(0)
    };
    match test.step {
        // ---- the events reached the script ----------------------------------------------------
        31 => {
            let seen = control.seen;
            let (built, crafted, delivered) = (seen[0], seen[2], seen[3]);
            say(
                if built > 0 && crafted > 0 && delivered > 0 { "ok  " } else { "FAIL" },
                &format!(
                    "the control stage heard what the factory did: {built} built, {crafted} crafted, {delivered} delivered"
                ),
            );
            // **and the miner's line is joined up again**: step 13 took its first belt away to
            // prove that taking things away works, and what the goal below counts is the ore that
            // line delivers. Everything else the checks built delivers once and stops.
            if let Some(&belt) = test.line.get(1) {
                orders.write(build::Order { at: belt, what: Some(What::Belt), dir: Dir::East });
            }
            test.step = 32;
        }
        // ---- a goal this factory reaches --------------------------------------------------
        32 => {
            rewrite.write(control::Rewrite(a_goal_of(1)));
            test.started = time.elapsed_secs();
            test.step = 33;
        }
        33 => {
            let waited = time.elapsed_secs() - test.started;
            // **waited for as a condition**: the belt was laid back a moment ago and a miner
            // takes `mine_seconds` over a dig, so a win is one delivery away
            if !control.won && waited < a_delivery {
                return;
            }
            say(
                if control.won { "ok  " } else { "FAIL" },
                &format!(
                    "the goal in control.rb is reached and the game is told: {} (after {waited:.1} s)",
                    control.saying.clone().unwrap_or_else(|| "it said nothing".into())
                ),
            );
            // and now **the same factory with a goal it cannot reach**: more ore than there is in
            // the ground, which is a number the world says rather than one anybody picked
            test.dropped_in_the_script = control.dropped;
            rewrite.write(control::Rewrite(a_goal_of(ore.total() + 1)));
            test.started = time.elapsed_secs();
            test.step = 34;
        }
        34 => {
            let heard = control.seen[3];
            let waited = time.elapsed_secs() - test.started;
            // what is waited for is the *new* script hearing a delivery: without that, "it did
            // not win" would be true of a script that never started
            if heard == 0 && waited < a_delivery {
                return;
            }
            say(
                if heard > 0 && !control.won { "ok  " } else { "FAIL" },
                &format!(
                    "the same factory does not win on a control.rb that asks for more than the ground holds: {heard} delivered, won {}",
                    control.won
                ),
            );
            // and now one that will not run at all
            rewrite.write(control::Rewrite(A_BROKEN_CONTROL.to_string()));
            test.waited = 0;
            test.step = 35;
        }
        // ---- a control.rb that will not run -------------------------------------------------
        35 => {
            // the program is compiled on the frame the message is read, the new `Script` lands at
            // the sync point after that and rubevy starts the task at the head of the frame after
            // *that* — the three frames [`PANEL_WAIT_FRAMES`] is twice
            test.waited += 1;
            if control.trouble.is_none() && test.waited < PANEL_WAIT_FRAMES {
                return;
            }
            test.before = in_the_chest(&test) as usize;
            test.started = time.elapsed_secs();
            test.step = 36;
        }
        36 => {
            let held = in_the_chest(&test) as usize;
            let waited = time.elapsed_secs() - test.started;
            if held <= test.before && waited < a_delivery {
                return;
            }
            let at = control.trouble.clone().unwrap_or_else(|| "nothing went wrong".into());
            say(
                // **the place as well as the fact**: a control.rb is written at the top level, so
                // the game puts it inside a method of one line to have its exceptions inside the
                // prelude's `rescue` (`crate::control::in_a_method`)
                if at.contains(&format!("{}:", control::SCRIPT_FILE)) && held > test.before {
                    "ok  "
                } else {
                    "FAIL"
                },
                &format!(
                    "a control.rb that will not run leaves the factory running: {at}, and the chest went from {} to {held}",
                    test.before
                ),
            );
            test.step = 37;
        }
        // ---- and nothing published to it was ever lost ---------------------------------------
        _ => {
            let vm = scripts.dropped();
            say(
                if vm == 0 && test.dropped_in_the_script == 0 { "ok  " } else { "FAIL" },
                &format!(
                    "nothing published to the control stage was dropped: {} in the script, {vm} in the VM",
                    test.dropped_in_the_script
                ),
            );
            test.step = CONTROL_CHECKS.end;
        }
    }
}

/// **A goal of so much ore**, which is the one thing the checks' own factory goes on delivering:
/// the miner's line digs it and its chest is nowhere near full.
fn a_goal_of(ore: u64) -> String {
    // **and an `on(:delivered)` that does nothing**: the counter the game reads off the script is
    // bumped where a handler runs (`ruby/control_prelude.rb`), so a script with no handler in it
    // hears everything and says it heard nothing. What the check needs to know is that *this*
    // script is being told about the same factory, which is what an empty handler proves.
    format!("goal deliver: {{ iron_ore: {ore} }}\non(:delivered) {{ |item, n, x, y| }}\n")
}

/// **A control.rb that will not run**, for the check that says one of those leaves a factory with
/// no goal rather than a game that stopped. It raises rather than failing to compile because a
/// raise is the harder half — and because it is the half a *line number* is hard to have for: it
/// happens at the top level of the file, where there is no block to be inside.
const A_BROKEN_CONTROL: &str = concat!(
    "goal deliver: { iron_ore: 1 }\n",
    "nothing.at.all\n",
);

/// **Somewhere to build the machine lines**: a rectangle with no ore under it and nothing built
/// on it yet, looked for **outwards from the middle of the map**.
///
/// It was `the middle of the map` until F4, with a comment saying `Ore::laid_out` keeps the middle
/// bare — which is true of an even number of patches and not of an odd one, because their middles
/// are at the middles of the cells (`worklog/2026-09-21-factory-F3a.md`, the third thing it
/// noticed). A check that knows where the ore is *because it has done the same arithmetic* stops
/// being true the moment the arithmetic changes, and `patches: [3, 3]` in `ruby/data.rb` is a line
/// anybody may write. So this asks the ore and the grid instead.
///
/// The middle is still where it starts, so nothing moves on the default map — and the picture
/// `--shot` takes is still the one it was, because the camera opens looking at the middle.
fn room_for_the_machine_lines(map: &Map, data: &Data, grid: &Grid, ore: &Ore) -> Option<UVec2> {
    let how_many = data.machines.len() as u32;
    let tall = data.machines.iter().map(|m| m.size.y).max().unwrap_or(1);
    let wide = data.machines.iter().map(|m| m.size.x).max().unwrap_or(1);
    // one row per machine, `tall + 1` apart so that a footprint never reaches the next row; and
    // `wide + 6` across, which is belt, belt, arm, machine, arm, belt, chest at its widest
    let block = UVec2::new(
        wide + 6,
        how_many.saturating_sub(1) * (tall + 1) + tall,
    );
    somewhere_clear(map, grid, ore, block)
}

/// **The bottom-left corner of a rectangle of this size with no ore in it and nothing built on
/// it**, looked for outwards from the middle of the map — or `None` where the map has no such
/// room anywhere.
///
/// It is what the checks ask instead of knowing where the ore is: how many patches there are and
/// how wide they are are `ruby/data.rb`'s, so **a check that works the bare ground out for itself
/// is a check that a line of Ruby can break** (F3a's third note). Both of the places that wanted
/// bare ground go through here — the tile a miner may not be built on, and the machine lines.
fn somewhere_clear(map: &Map, grid: &Grid, ore: &Ore, block: UVec2) -> Option<UVec2> {
    if block.x > map.tiles.x || block.y > map.tiles.y {
        return None;
    }
    let clear = |left: u32, bottom: u32| {
        (bottom..bottom + block.y).all(|y| {
            (left..left + block.x).all(|x| {
                let t = grid.index(UVec2::new(x, y));
                ore.left[t] == 0 && grid.at(t).is_none()
            })
        })
    };
    // **the middle first, and then outwards**: the block is centred on the middle of the map,
    // and the search walks away from there a row and a column at a time
    let middle = (map.tiles - block) / 2;
    for bottom in outwards(middle.y, map.tiles.y - block.y + 1) {
        for left in outwards(middle.x, map.tiles.x - block.x + 1) {
            if clear(left, bottom) {
                return Some(UVec2::new(left, bottom));
            }
        }
    }
    None
}

/// The whole numbers below `upto`, **nearest to `from` first** — the order a search for somewhere
/// to build walks. Ties go to the lower number, so a run is the same run twice.
fn outwards(from: u32, upto: u32) -> Vec<u32> {
    let mut all: Vec<u32> = (0..upto).collect();
    all.sort_by_key(|&i| (i as i64 - from as i64).abs());
    all
}

/// **F2's two little factories, built with clicks**: for each machine the data file declares, a
/// belt, the machine, a belt and a chest, in a row.
///
/// Where they go is [`room_for_the_machine_lines`]'s: somewhere with no ore in it and nothing on
/// it, so that a map whose patches land in the middle is a map the checks still work on. The
/// machine with the largest footprint decides how far apart the rows are.
fn lay_out_the_machine_lines(
    map: &Map,
    data: &Data,
    rules: &Rules,
    grid: &Grid,
    ore: &Ore,
    test: &mut SelfTest,
) -> bool {
    test.machines.clear();
    test.to_build.clear();
    test.arms_to_build.clear();
    let tall = data.machines.iter().map(|m| m.size.y).max().unwrap_or(1);
    let Some(corner) = room_for_the_machine_lines(map, data, grid, ore) else { return false };
    let (left, first_row) = (corner.x, corner.y);
    for (kind, machine) in data.machines.iter().enumerate() {
        let row = first_row + kind as u32 * (tall + 1);
        // the recipe it will run: the first one made in it, which is the one it will pick
        let Some(&recipe_id) = machine.recipes.first() else { continue };
        let recipe = &data.recipes[recipe_id as usize];
        let Some(&(makes, _)) = recipe.outputs.first() else { continue };
        // belt, belt, [arm], machine, [arm], belt, chest — with the two arms' tiles left empty
        // to begin with, which is where a player leaves them too
        let at = left + 3;
        let out = at + machine.size.x;
        for (x, what) in [
            (left, Some(What::Belt)),
            (left + 1, Some(What::Belt)),
            (at, Some(What::Machine(kind as data::MachineId))),
            (out + 1, Some(What::Belt)),
            (out + 2, Some(What::Chest)),
        ] {
            test.to_build.push((UVec2::new(x, row), what));
        }
        // and the two arms, built after the line has been watched stopping without them. They go
        // in the gaps, so nothing that was already on a belt is taken away with the tile it was
        // on — which is what building over one does, and would have made this check a check of
        // itself.
        for x in [left + 2, out] {
            test.arms_to_build.push((UVec2::new(x, row), Some(What::Inserter)));
        }
        let made_of: Vec<String> = recipe
            .inputs
            .iter()
            .map(|&(item, n)| format!("{n} {}", data.item_name(item)))
            .collect();
        // **what the game's own numbers say this takes**, from the moment the arms are there.
        // Four things, and the first of them is the one F3 added: a script waits a part of a
        // swing before its first look, so that a thousand of them do not wake on one frame
        // (`ruby/prelude.rb`), and the most that can be is a whole swing. Then one swing per
        // thing the recipe eats — an arm carries one at a time — then the craft at the machine's
        // own speed, then a swing out and a tile of belt into the chest.
        let eats: u32 = recipe.inputs.iter().map(|&(_, n)| n).sum();
        let needs = rules.swing_seconds
            + eats as f32 * rules.swing_seconds
            + recipe.time / machine.speed
            + rules.swing_seconds
            + 1.0 / rules.belt_tiles_per_second;
        test.machines.push(MachineCheck {
            what: machine.name.clone(),
            makes,
            recipe: recipe_id,
            made_of: made_of.join(" and "),
            at: (row * map.tiles.x + at) as usize,
            feed: (row * map.tiles.x + left + 1) as usize,
            arm_in: (row * map.tiles.x + left + 2) as usize,
            chest: (row * map.tiles.x + out + 2) as usize,
            needs,
            done_at: None,
        });
    }
    !test.machines.is_empty()
}

/// **What each machine eats, put on the belt that feeds it** — which is what a miner up the line
/// would have put there, and what F3's inserters will hand over.
///
/// **It is told where the lines are rather than working it out again** (F4). It did the same
/// arithmetic as [`lay_out_the_machine_lines`] until then, which was two copies of one thing and
/// stopped being possible the moment where they go became a search.
fn seed_the_machine_lines(data: &Data, rules: &Rules, lanes: &mut Lanes, lines: &[MachineCheck]) {
    for line in lines {
        let Some(recipe) = data.recipes.get(line.recipe as usize) else { continue };
        // the belt behind the one that runs up to the machine, which is where the line starts
        let start = line.feed - 1;
        let mut along = 0;
        for &(item, n) in &recipe.inputs {
            for _ in 0..n {
                lanes.put_on(start, OnBelt { along, item });
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
    // 1 item, 2 machine, 3 recipe, 4 belt, 5 miner, 6 chest, 7 ore, 8 inserter, 9 map
    let good = concat!(
        "item :rock, icon: 0\n",
        "machine :oven, size: [1, 1], sprite: [109], speed: 1.0\n",
        "recipe :rock, in: {}, out: { rock: 1 }, time: 1.0, made_in: :oven\n",
        "belt :line, tiles_per_second: 1.0, items_per_tile: 1\n",
        "miner :drill, seconds_per_item: 1.0\n",
        "chest :box, capacity: 1\n",
        "ore :rock, per_tile: 1, patch_radius: 1.0, patches: [1, 1]\n",
        "inserter :arm, seconds_per_item: 1.0\n",
        "map :world, size: [8, 8]\n",
    );
    vec![
        (good.replace("item :rock, icon: 0", "item :rock, icon: 0, colour: :grey"), 1, "an unknown field"),
        (good.replace("time: 1.0, made_in: :oven", "time: 0.0, made_in: :oven"), 3, "a time of zero"),
        (good.replace("out: { rock: 1 }", "out: { pebble: 1 }"), 3, "an item nothing declares"),
        (good.replace("size: [1, 1]", "size: [0, 1]"), 2, "a machine no tiles wide"),
        (good.replace("tiles_per_second: 1.0", "tiles_per_second: -1.0"), 4, "a belt that runs backwards"),
        // F2a's: a gap that is not a whole number of steps (`crate::data::fits_a_tile`)
        (good.replace("items_per_tile: 1", "items_per_tile: 3"), 4, "a gap that does not divide a tile"),
        // F3's: the ground named after an item nothing declares — the one reference the `ore`
        // word has, and the only check left that needs two declarations to be wrong
        (good.replace("ore :rock,", "ore :coal,"), 7, "ground nothing declares"),
        // **F3a's two, and they are the map's**: too small for its own ore, and too big for the
        // picture. Both are refused at the `map` line even though the first of them is decided by
        // the *ore*'s radius and count, because the line a player would change is this one — and
        // both of them are the numbers something really breaks at rather than a size anybody
        // preferred (`crate::draw::MOST_TILES_ACROSS`, `crate::grid::Ore::smallest_map`).
        (good.replace("size: [8, 8]", "size: [3, 8]"), 9, "a map too small for its ore"),
        (
            good.replace("size: [8, 8]", &format!("size: [{}, 8]", draw::MOST_TILES_ACROSS + 1)),
            9,
            "a map too big to draw",
        ),
        // **last, and on purpose**: a half-written line is reported where the parser gives up,
        // which is the *next* token — so `icon:` on line 1 of a file with six more lines is
        // reported at line 2. At the end of the file the next token is the end of the file, and
        // the line is the line. That is the compiler's reading and not something to work around.
        (format!("{good}item :half, icon:\n"), 10, "Ruby that will not parse"),
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
        // **a square and an oblong**, because the arithmetic that was right with one number for
        // both sides is the arithmetic F3a had to take apart
        for tiles in [UVec2::splat(32), UVec2::new(96, 16), UVec2::new(15, 41)] {
            let map = Map { tiles };
            let last = tiles - UVec2::ONE;
            for tile in [
                UVec2::ZERO,
                UVec2::new(last.x, 0),
                UVec2::new(0, last.y),
                last,
                tiles / 3,
            ] {
                assert_eq!(map.tile_at(map.tile_centre(tile)), Some(tile), "{tiles}: tile {tile}");
            }
        }
    }

    /// The corners belong to exactly one tile each: the bottom left of the map is tile (0, 0) and
    /// a hair below it is off the map, which is what keeps a click on the edge from placing
    /// something outside the world.
    #[test]
    fn the_edges_belong_to_one_tile_and_outside_is_outside() {
        for tiles in [UVec2::splat(32), UVec2::new(96, 16)] {
            let map = Map { tiles };
            let half = map.span() / 2.0;
            let last = tiles - UVec2::ONE;
            assert_eq!(map.tile_at(-half), Some(UVec2::ZERO));
            assert_eq!(map.tile_at(Vec2::new(-half.x - 0.01, -half.y)), None);
            assert_eq!(map.tile_at(Vec2::new(-half.x, -half.y - 0.01)), None);
            assert_eq!(map.tile_at(half - 0.01), Some(last));
            assert_eq!(
                map.tile_at(half),
                None,
                "{tiles}: the far edge is the next tile, which is not there"
            );
        }
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
        // **and the default world is the one it was before F3a**, which is the promise that stage
        // made: the size moved out of `factory.settings.txt` and into `ruby/data.rb` without
        // moving. 32 by 32 is still F0's provisional value and still nobody's measurement
        // (`docs/numbers.md` §9.6); what changed is who may change it.
        let rules = app.world().resource::<Rules>();
        assert_eq!(rules.map_tiles, UVec2::splat(32), "ruby/data.rb's map");
        assert_eq!(rules.ore_patches, UVec2::splat(2), "four patches, one to a quarter");
        assert_eq!(rules.ore_patch_radius, 3.0);
        // the four are where `Ore::laid_out` puts them and clear of the wall, which is the floor
        // the data stage refuses under
        assert!(rules.map_tiles.min_element() >= Ore::smallest_map(rules.ore_patch_radius, 2));
    }

    /// **A store written by an older build is told where its numbers went** (F3a), and a store
    /// that has neither of them says nothing at all — which is every store this build writes.
    #[test]
    fn a_setting_that_moved_into_the_data_file_says_so() {
        // a store read out of a string rather than off the disk: `Settings::load` takes the two
        // functions, which is what lets a page keep its store in `localStorage`
        fn an_old_file(_: &std::path::Path) -> Result<String, String> {
            Ok("map_tiles=64\ncamera_half_height=150\nore_patch_radius=2\n".into())
        }
        fn nothing(_: &std::path::Path) -> Result<String, String> {
            Err("no such store".into())
        }
        fn unwritable(_: &std::path::Path, _: &str) -> Result<(), String> {
            Err("the test does not write".into())
        }

        let old = games_shell::Settings::load("factory.settings.txt", "", an_old_file, unwritable);
        let said = moved_settings(&old);
        assert_eq!(said.len(), 2, "both of the keys that moved: {said:?}");
        assert!(said[0].contains("map_tiles=64") && said[0].contains("map :world"), "{}", said[0]);
        assert!(
            said[1].contains("ore_patch_radius=2") && said[1].contains("patch_radius"),
            "{}",
            said[1]
        );
        assert!(said.iter().all(|s| s.contains(DATA_FILE)), "and where to write it now");
        // the key that did not move is not mentioned, and a store with none of them is quiet
        assert!(said.iter().all(|s| !s.contains("camera_half_height")));
        let new = games_shell::Settings::load("factory.settings.txt", "", nothing, unwritable);
        assert!(moved_settings(&new).is_empty());
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
