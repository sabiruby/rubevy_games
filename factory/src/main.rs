//! **Factory** — a small top-down factory, the third sample of [rubevy].
//!
//! The plan is `docs/plans/factory-plan.md`; this file is **F0**, which is the skeleton and one
//! question: *can Bevy's own `TilemapChunk` draw in a browser?* Everything the plan is actually
//! about — the grid, the conveyors, the data stage, the inserters' Ruby, the control stage —
//! comes later, and none of it is here yet.
//!
//! What is here:
//!
//! * a floor laid with one [`TilemapChunk`] out of Kenney's Tiny Factory sheet, loaded as an
//!   **array texture** (the tileset is one image and the chunk wants one layer per tile, so the
//!   cut is asked for in code — `AssetMetaCheck::Never` means a `.meta` file beside it would be a
//!   404 in a page). The answer to the question is **yes, with one condition**, and the condition
//!   is on [`TILESET_LAYERS`];
//! * a camera the player drives, which is `games_shell::CameraPlugin` and nothing of this game's;
//! * a click turned into the tile it landed on, which is the one piece of arithmetic F1 will
//!   build everything else on;
//! * `--headless N` for a run with no window, and the checks (`FACTORY_SELFTEST`, `?selftest`).
//!
//! **Where the numbers are.** None of the numbers below is a constant a player cannot reach:
//! every one of them is a line in `factory.settings.txt`, and the `const`s are only the defaults'
//! names. The two that are `const` *because they cannot move* are the pack's own 16 px and how
//! many tiles the sheet has — change either and the picture is cut wrong, or not drawn at all,
//! which is not a setting. `docs/numbers.md` has the table, with where each default came from.
//!
//! [rubevy]: https://github.com/sabiruby/rubevy

mod belt_sample;
mod platform;

use bevy::asset::AssetMetaCheck;
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::prelude::*;
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkTileData};
use belt_sample::Belts;
use games_shell::camera::{CameraControls, CameraPlugin, WorldClick};
use rubevy::RubevyPlugin;

// ---------------------------------------------------------------------------------------------
// The two numbers that are not settings
// ---------------------------------------------------------------------------------------------

/// **How big one tile of the art is.** Kenney's Tiny Factory is 16 px × 16 px
/// (`factory/art/Tilesheet-tiny-factory.txt`: "Tile size • 16px × 16px"). It is not a setting:
/// another number here does not draw the same picture differently, it cuts the sheet in the
/// wrong places.
pub const TILE_PX: u32 = 16;

/// **How many tiles the sheet has.** The sheet is a vertical strip of 16 px squares built by
/// `tools/factory-tileset.py`, and this is the number of them: Kenney's 132 in the pack's own
/// order, then the twelve `tools/factory-belts.py` draws (F0a's two candidate corners), then one
/// blank on the end.
///
/// **The blank is not decoration.** wgpu's OpenGL backend guesses a texture's bind target from
/// its shape, and a `D2` texture with square layers and a count that is a multiple of six is
/// guessed to be a *cube map array* (`wgpu-hal-29.0.4/src/gles/mod.rs:458`). Kenney's 132 square
/// tiles are 6 × 22, so a browser bound the tileset as `TEXTURE_CUBE_MAP_ARRAY`, which WebGL2
/// does not have, and drew a black page with no page error at all — while the same code drew the
/// floor correctly on a PC, where the backend is Vulkan. The script keeps the count off every
/// multiple of six and says why; this is the number it arrived at, and the check below reads it
/// back out of the loaded image so that a sheet rebuilt to a different length cannot go unnoticed.
const TILESET_LAYERS: u32 = 145;

/// The tiles of that sheet this stage uses, read off the sheet itself on 2026-09-21 (the numbered
/// blow-up is in `docs/factory.md`). Kenney numbers row-major from 0.
///
/// F1 replaces the whole of this floor, so these three are here to make a picture that can be
/// *seen to be tiles* — three different plates, laid in diagonal stripes — rather than to be a
/// floor anybody plays on.
const FLOOR_PLATES: [u16; 3] = [0, 1, 2];
/// The plain dark ground, for a one-tile border that shows where the map ends.
const GROUND: u16 = 3;

// ---------------------------------------------------------------------------------------------
// The defaults, every one of which `factory.settings.txt` can move
// ---------------------------------------------------------------------------------------------

/// **How big the map is, in tiles each way.** *F0's provisional value* (plan §4: "a provisional
/// map, enough that the floor can be seen"), derived from the one below: the home view holds 16
/// tiles from top to bottom, and the map is **twice that**, so that the world does not fit in the
/// window and a camera you can drive is a camera that has somewhere to go.
/// `docs/plans/factory-plan.md` §3.7 decides the real one at F1, from how many machines have to
/// fit on it.
const MAP_TILES: f32 = 32.0;

/// **Half of how much world the window holds, top to bottom, in world units** — and one world
/// unit is one pixel of the art, so this is 8 tiles above the middle and 8 below. *F0's
/// provisional value*: 16 tiles is enough that a tile is drawn about 56 screen pixels tall in the
/// default 900 px window (900 / (16 × 16) ≈ 3.5 screen pixels per pixel of the art), which is
/// "the floor can be seen". Decided properly at F1 with the map.
const CAMERA_HALF_HEIGHT: f32 = 8.0 * TILE_PX as f32;

/// The window the game opens. **The same as the other two games here open**, and the number
/// itself has no recorded source in either of them (`sabibots/src/main.rs`: "Source unknown").
/// It is a setting, so nobody has to accept it.
const WINDOW: [f32; 2] = [1600.0, 900.0];

/// `--headless` with no number, and `--shot` with no file or seconds.
///
/// **The headless default is an upper bound and not a wait.** Nothing in this stage takes time:
/// the checks wait for conditions and end the run the moment they are all answered, and on the
/// machine this was written on that was **0.3 s** for the whole run (2026-09-21). Ten seconds is
/// thirty times that, which is the room a slower machine gets before a run is called stuck.
const HEADLESS_SECONDS: f32 = 10.0;
const SHOT_FILE: &str = "shot.png";
/// `--shot` with no seconds. The picture wants the floor on it, and the floor is there on the
/// frame after the tileset finishes loading; two seconds is the measured worst case (0.4 s in the
/// container) with room over it.
const SHOT_SECONDS: f32 = 2.0;

/// **How long the checks wait for the tileset to arrive, in frames.** They wait for the thing
/// itself rather than for a number of seconds (S7's rule), but a wait with no end is a check that
/// can never fail, and in a page there is no run ending underneath it to stop it. The bound is in
/// frames because what is being waited for is the asset server getting a turn, which is once a
/// frame. Measured: the tileset was in `Assets<Image>` on **frame 3** in the container and
/// **frame 4** in the browser, so this is a hundred times the worst of the two.
const TILESET_WAIT_FRAMES: u32 = 400;

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
    fn tile_centre(&self, tile: UVec2) -> Vec2 {
        let half = self.span() / 2.0;
        Vec2::new(
            tile.x as f32 * TILE_PX as f32 + TILE_PX as f32 / 2.0 - half,
            tile.y as f32 * TILE_PX as f32 + TILE_PX as f32 / 2.0 - half,
        )
    }

    /// Which tile of the sheet goes at a place on the floor. **F0's floor**, and F1 throws it
    /// away: a one-tile border of plain ground so the edge of the map is visible, and the pack's
    /// three floor plates in diagonal stripes inside it, so that a screenshot of this shows
    /// *tiles* and not one flat colour.
    fn floor_tile(&self, tile: UVec2) -> u16 {
        let last = self.tiles - 1;
        if tile.x == 0 || tile.y == 0 || tile.x == last || tile.y == last {
            GROUND
        } else {
            FLOOR_PLATES[((tile.x + tile.y) % FLOOR_PLATES.len() as u32) as usize]
        }
    }
}

/// **The floor, as the game holds it**: one tile of the sheet per square, row 0 at the bottom.
///
/// This is deliberately *not* the [`TilemapChunkTileData`] the renderer reads. A headless run has
/// no renderer at all — `TilemapChunk`'s insert hook wants a mesh cache and a material, and both
/// belong to `SpriteRenderPlugin` — so the grid the game reasons about has to be the game's own
/// and the chunk has to be a **picture of it**. F1 wants exactly that split anyway: the conveyors
/// and the machines are a Rust grid, and what is drawn is a view of it.
#[derive(Resource, Debug)]
struct Floor {
    tiles: Vec<u16>,
}

/// The last click, as the tile it landed on — the whole of what F0 does with the mouse.
#[derive(Resource, Debug, Default)]
struct LastClick {
    at: Option<Vec2>,
    tile: Option<UVec2>,
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

    // **F0a only** (`src/belt_sample.rs`, which F1 deletes): `--belts i` or `--belts ii` lays a
    // fixed arrangement of conveyors and machines instead of a plain floor, so that the author
    // can look at the two ways of drawing a belt side by side. It moves the map and the view to
    // its own picture's size, which is what "large enough to see a chevron" means here.
    let belts = args.value("--belts").and_then(|word| Belts::from_word(&word));
    if args.value("--belts").is_some() && belts.is_none() {
        warn!("--belts takes `i` or `ii`");
    }
    let map = Map {
        tiles: match belts {
            Some(_) => belt_sample::TILES,
            None => settings.number("map_tiles").unwrap_or(MAP_TILES).max(3.0) as u32,
        },
    };
    let half_height = match belts {
        Some(_) => belt_sample::half_height(),
        None => settings.number("camera_half_height").unwrap_or(CAMERA_HALF_HEIGHT),
    };
    let window = [
        settings.number("window_width").unwrap_or(WINDOW[0]),
        settings.number("window_height").unwrap_or(WINDOW[1]),
    ];
    let headless = args.headless(settings.number("headless_seconds").unwrap_or(HEADLESS_SECONDS));
    let shot = args.shot(
        settings.get("shot_file").unwrap_or(SHOT_FILE),
        settings.number("shot_seconds").unwrap_or(SHOT_SECONDS),
    );

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            // No renderer, the same startup. There is no image loader without `ImagePlugin`, so
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
            // one and measure the arithmetic where there is no mouse.
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
                    // 16 px tile blown up eight times into a smear.
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
            ));
        }
    }
    if let Some(belts) = belts {
        app.insert_resource(belts);
    }
    app.insert_resource(map)
        .insert_resource(settings)
        .init_resource::<LastClick>()
        // The grid in both modes; the picture of it only where there is something to draw with.
        .add_systems(Startup, lay_floor)
        .add_systems(Update, take_clicks);
    if headless.is_none() {
        app.add_systems(Startup, draw_floor.after(lay_floor));
    }

    if platform::selftest_asked() {
        app.init_resource::<SelfTest>().add_systems(Update, selftest);
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    // F2 onwards reads this; F0 only says where it is, so that a run in the wrong directory says
    // so now rather than in two stages' time.
    if !ruby.is_dir() && platform::RUBY_FILES.is_empty() {
        warn!("no Ruby at {ruby:?}: nothing to run there yet, but F2 will want it");
    }
    app.run();
}

/// **The grid.** Runs in both modes, and nothing in it knows what a renderer is.
fn lay_floor(mut commands: Commands, map: Res<Map>) {
    let tiles: Vec<u16> = (0..map.tiles * map.tiles)
        .map(|i| map.floor_tile(UVec2::new(i % map.tiles, i / map.tiles)))
        .collect();
    info!(
        "floor: {} x {} tiles of {} px, {} world units across",
        map.tiles,
        map.tiles,
        TILE_PX,
        map.span()
    );
    commands.insert_resource(Floor { tiles });
}

/// **The picture of it: one chunk, one draw call.**
///
/// The tileset is one 192 × 176 image and the chunk wants an array texture with one layer per
/// tile, so the cut is asked for through the loader's settings. It cannot be asked for in a
/// `.meta` file beside the image: `AssetMetaCheck::Never` is what keeps a page from asking for a
/// `.meta` that is not there and being handed a 404, and it is set for both builds so that the
/// PC build and the page load the same way (plan §5).
///
/// Only where there is a renderer. `TilemapChunk`'s insert hook reaches for
/// `TilemapChunkMeshCache` and `Assets<TilemapChunkMaterial>`, which are `SpriteRenderPlugin`'s,
/// so a headless run that spawned one would panic before its first frame.
fn draw_floor(
    mut commands: Commands,
    assets: Res<AssetServer>,
    map: Res<Map>,
    floor: Res<Floor>,
    belts: Option<Res<Belts>>,
) {
    let mut tiles: Vec<Option<TileData>> =
        floor.tiles.iter().map(|&i| Some(TileData::from_tileset_index(i))).collect();
    // F0a's mock-up, over the floor. F1 deletes this and the module it calls.
    if let Some(belts) = belts {
        for (at, data) in belt_sample::over_the_floor(&map, *belts) {
            tiles[(at.y * map.tiles + at.x) as usize] = Some(data);
        }
    }
    commands.spawn((
        TilemapChunk {
            chunk_size: UVec2::splat(map.tiles),
            // one world unit is one pixel of the art
            tile_display_size: UVec2::splat(TILE_PX),
            tileset: assets
                .load_builder()
                .with_settings(|settings: &mut ImageLoaderSettings| {
                    // a vertical stack of 16 px squares: one layer per tile, in the order
                    // `tools/factory-tileset.py` wrote them
                    settings.array_layout =
                        Some(ImageArrayLayout::RowHeight { pixels: TILE_PX });
                })
                .load("tiles/factory-tiles.png"),
            ..default()
        },
        TilemapChunkTileData(tiles),
    ));
}

/// The crate says where in the world the click was; **which tile that is, is this game's own
/// arithmetic** — and F1 builds placing and removing on it.
fn take_clicks(
    mut clicks: MessageReader<WorldClick>,
    map: Res<Map>,
    mut last: ResMut<LastClick>,
) {
    for click in clicks.read() {
        let tile = map.tile_at(click.at);
        match tile {
            Some(tile) => info!("clicked {:.1}, {:.1} — tile {}, {}", click.at.x, click.at.y, tile.x, tile.y),
            None => info!("clicked {:.1}, {:.1} — off the map", click.at.x, click.at.y),
        }
        last.at = Some(click.at);
        last.tile = tile;
    }
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
#[derive(Resource, Default)]
struct SelfTest {
    step: u8,
    frames: u32,
    /// the tile the click was aimed at, so the answer can be compared with the question
    aimed_at: Option<UVec2>,
    done: bool,
}

/// One line, in the shape the other two games print and `tools/fixedlines.sh` matches: the
/// verdict padded to four columns, one space, the sentence. `ok  `, `FAIL`, `--  `.
fn say(verdict: &str, what: &str) {
    info!("selftest: {verdict} {what}");
}

/// **Why this is a few steps and not one:** two of the four things are about something arriving —
/// the tileset, and a click coming back as a tile — and each is waited for by asking whether it
/// is there yet, not by sleeping. The bound is [`TILESET_WAIT_FRAMES`] frames, because what is
/// being waited on happens once a frame.
fn selftest(
    mut test: ResMut<SelfTest>,
    map: Res<Map>,
    floor: Option<Res<Floor>>,
    chunks: Query<(&TilemapChunk, &TilemapChunkTileData)>,
    images: Res<Assets<Image>>,
    last: Res<LastClick>,
    mut clicks: MessageWriter<WorldClick>,
    mut exit: MessageWriter<AppExit>,
) {
    if test.done {
        return;
    }
    test.frames += 1;
    match test.step {
        // ---- the floor was laid ------------------------------------------------------------
        0 => {
            let Some(floor) = floor else { return };
            let wanted = map.tiles as usize * map.tiles as usize;
            let laid = floor.tiles.len();
            let on_the_sheet = floor
                .tiles
                .iter()
                .all(|&i| i < TILESET_LAYERS as u16);
            say(
                if laid == wanted && on_the_sheet { "ok  " } else { "FAIL" },
                &format!(
                    "the floor is {} by {} tiles, all {} of them laid with a tile of the sheet",
                    map.tiles, map.tiles, laid
                ),
            );
            test.step = 1;
        }
        // ---- the arithmetic F1 is built on -------------------------------------------------
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
            let Ok((chunk, _)) = chunks.single() else {
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
        // ---- a click is read as a tile -----------------------------------------------------
        3 => {
            // The click is forged rather than driven with a mouse: what is being checked is the
            // game's arithmetic on the message the camera writes, and a run with no window has no
            // camera to write one. The tile is an off-centre one so that a wrong sign or a
            // forgotten half-tile cannot pass.
            let aim = UVec2::new(map.tiles / 4, map.tiles / 3);
            test.aimed_at = Some(aim);
            clicks.write(WorldClick {
                at: map.tile_centre(aim),
                button: MouseButton::Left,
                cursor: Vec2::ZERO,
            });
            test.step = 4;
        }
        4 => {
            let aim = test.aimed_at;
            let ok = last.tile.is_some() && last.tile == aim;
            say(
                if ok { "ok  " } else { "FAIL" },
                &format!(
                    "a click in the middle of a tile is read as that tile (asked {:?}, read {:?})",
                    aim.map(|t| (t.x, t.y)),
                    last.tile.map(|t| (t.x, t.y))
                ),
            );
            test.step = 5;
        }
        _ => {
            test.done = true;
            if platform::CHECKS_EXIT_WHEN_DONE {
                exit.write(AppExit::Success);
            } else {
                info!("selftest: done — the factory keeps running (a page has nothing to exit to)");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one piece of arithmetic F1 builds on. It is checked here as well as in the run because
    /// a run needs a window for three of its four checks and this needs nothing.
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

    /// The floor F0 lays: a border of plain ground, and inside it the pack's three plates. The
    /// point of the check is that every tile has *some* picture — a hole in the floor is what a
    /// wrong index looks like.
    #[test]
    fn every_tile_of_the_floor_has_a_picture() {
        let map = Map { tiles: 32 };
        for x in 0..map.tiles {
            for y in 0..map.tiles {
                let tile = map.floor_tile(UVec2::new(x, y));
                assert!(tile < TILESET_LAYERS as u16, "tile {x},{y} is off the sheet");
            }
        }
        assert_eq!(map.floor_tile(UVec2::ZERO), GROUND, "the border is the plain ground");
        assert!(FLOOR_PLATES.contains(&map.floor_tile(UVec2::splat(5))), "inside it is a plate");
    }
}
