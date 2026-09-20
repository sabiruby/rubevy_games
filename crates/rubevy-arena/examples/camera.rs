//! **The camera, with nothing else in the window** (S3).
//!
//! A floor of plain coloured squares — no pictures, no assets, nothing to load — a camera to
//! drive over it, and one egui window, so that the three things that are hard to see in a test
//! can be seen by hand:
//!
//! * dragging, wheeling and walking the camera, and `Home` putting it back;
//! * **the panel takes the pointer**: a drag or a wheel that begins over the egui window does
//!   nothing to the camera, and a drag that begins on the floor keeps going when it crosses it;
//! * **the panel covers pixels** ([`ViewInsets`]): tick its box and the floor slides out from
//!   under it, which is the same arrangement sabibots' arena and its editor are in.
//!
//! A click writes a [`WorldClick`], and what this example does with it is what any game would do
//! first — turn the world point into the square it landed on, which this crate has no opinion
//! about.
//!
//! ```text
//! cargo run -p rubevy-arena --example camera        # until it is closed
//! cargo run -p rubevy-arena --example camera -- 5   # five seconds, for a machine with no hands
//! ```

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use rubevy_arena::camera::{CameraControls, CameraPlugin, CameraView, ViewInsets, WorldClick};

/// How wide one square of the floor is, in world units. The floor is a picture of the world's
/// scale and nothing else, so this is the example's own number: something a `TILE`-sized thing
/// would stand on.
const TILE: f32 = 4.0;
/// How many squares across and down. Twelve by twelve at four units is forty-eight units, which
/// is wider than the camera's home view below — the point of a camera you can drive is that the
/// world does not fit in the window.
const TILES: i32 = 12;
/// Half of how much world the window shows to begin with.
const HOME_VIEW: f32 = 16.0;

/// What the last click landed on, for the panel to say.
#[derive(Resource, Default)]
struct LastClick {
    at: Option<Vec2>,
    tile: Option<(i32, i32)>,
}

/// Whether the panel tells the camera how much of the window it is covering.
#[derive(Resource)]
struct PanelClaimsItsSpace(bool);

/// `cargo run ... -- 5`: close by itself after five seconds, so this example can be run where
/// nobody is watching (and on a machine where a window cannot be opened at all, one frame).
#[derive(Resource)]
struct StopAfter(f32);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "rubevy-arena: the camera".into(),
            resolution: (1280u32, 720u32).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(EguiPlugin::default())
    .add_plugins(
        CameraPlugin::showing(HOME_VIEW)
            // the floor has an edge, and the crate could not have guessed where: a game says so
            .with(CameraControls {
                bounds: Some(Rect::from_center_half_size(
                    Vec2::ZERO,
                    Vec2::splat(TILES as f32 * TILE * 0.5),
                )),
                ..default()
            }),
    )
    .init_resource::<LastClick>()
    .insert_resource(PanelClaimsItsSpace(true))
    .add_systems(Startup, spawn_floor)
    .add_systems(Update, take_clicks)
    .add_systems(EguiPrimaryContextPass, panel);

    if let Some(seconds) = std::env::args().nth(1).and_then(|s| s.parse::<f32>().ok()) {
        app.insert_resource(StopAfter(seconds)).add_systems(Update, stop_when_over);
    }
    app.run();
}

/// A checkerboard, dark and light, with one square marked so that "where am I" has an answer.
fn spawn_floor(mut commands: Commands) {
    let half = TILES / 2;
    for x in -half..half {
        for y in -half..half {
            let odd = (x + y).rem_euclid(2) == 0;
            let corner = x == -half && y == -half;
            let colour = if corner {
                Color::srgb(0.85, 0.55, 0.20)
            } else if odd {
                Color::srgb(0.16, 0.20, 0.26)
            } else {
                Color::srgb(0.22, 0.27, 0.34)
            };
            commands.spawn((
                Sprite {
                    color: colour,
                    // a hair's breadth of a gap, so the squares can be counted
                    custom_size: Some(Vec2::splat(TILE * 0.96)),
                    ..default()
                },
                Transform::from_xyz((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE, 0.0),
            ));
        }
    }
}

/// The crate says where in the world the click was; which square that is, is the game's own
/// arithmetic — here, one division.
fn take_clicks(mut clicks: MessageReader<WorldClick>, mut last: ResMut<LastClick>) {
    for click in clicks.read() {
        let tile = ((click.at.x / TILE).floor() as i32, (click.at.y / TILE).floor() as i32);
        info!("clicked {:.2}, {:.2} — square {:?}", click.at.x, click.at.y, tile);
        last.at = Some(click.at);
        last.tile = Some(tile);
    }
}

/// The one panel: what the camera is doing, what was clicked, and a box that decides whether the
/// floor is pushed out from under this window.
fn panel(
    mut contexts: EguiContexts,
    view: Res<CameraView>,
    last: Res<LastClick>,
    mut claims: ResMut<PanelClaimsItsSpace>,
    mut insets: ResMut<ViewInsets>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let content = ctx.content_rect();
    let shown = egui::Window::new("the camera")
        .default_pos([content.right() - 300.0, 8.0])
        .default_width(280.0)
        .show(ctx, |ui| {
            ui.label("drag with the right button, or Shift and the left one");
            ui.label("wheel to come nearer, WASD or the arrows to walk, Home to go back");
            ui.separator();
            ui.label(format!("looking at  {:.2}, {:.2}", view.focus.x, view.focus.y));
            ui.label(format!("half the window holds  {:.2} units", view.half_height));
            match (last.at, last.tile) {
                (Some(at), Some(tile)) => {
                    ui.label(format!("last click  {:.2}, {:.2}", at.x, at.y));
                    ui.label(format!("which is square  {}, {}", tile.0, tile.1));
                }
                _ => {
                    ui.label("nothing clicked yet");
                }
            }
            ui.separator();
            ui.checkbox(&mut claims.0, "this panel covers pixels (ViewInsets)");
            ui.label("with it off, the floor stays centred in the whole window");
        });

    // exactly what the editor does in this crate: say how wide the band it covers is, and let
    // whatever camera is behind it decide what to do about it
    let covered = shown.map(|w| w.response.rect);
    insets.right = match (claims.0, covered) {
        (true, Some(rect)) => (content.right() - rect.left()).max(0.0),
        _ => 0.0,
    };
}

fn stop_when_over(
    time: Res<Time>,
    stop: Res<StopAfter>,
    view: Res<CameraView>,
    insets: Res<ViewInsets>,
    mut exit: MessageWriter<AppExit>,
) {
    if time.elapsed_secs() >= stop.0 {
        // one line a run, so that a run nobody watched still says what it drew
        info!(
            "camera example: {:.2} s, looking at {:.2}, {:.2}, half-view {:.2}, covered right {:.0} px",
            time.elapsed_secs(),
            view.focus.x,
            view.focus.y,
            view.half_height,
            insets.right
        );
        exit.write(AppExit::Success);
    }
}
