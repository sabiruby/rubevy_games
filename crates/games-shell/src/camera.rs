//! **A 2D camera a player can drive** (S3).
//!
//! [`ArenaPlugin`](crate::ArenaPlugin) shows a square that always fits the window and never
//! moves. That is right for a match in a walled arena and wrong for a world bigger than the
//! window, which is what the third game needs, so this is the other camera: drag it, wheel it,
//! walk it with the keys, `Home` to put it back.
//!
//! **Nothing here is new thinking.** All of it is what the garden's 3D camera learned in G6 and
//! on 2026-09-18, written once more in two dimensions:
//!
//! * A wheel message carries a *unit*. A mouse on a PC sends `Line` with `y = ±1` per notch and a
//!   browser sends `Pixel` with a hundred of them, so the message is turned into **notches**
//!   ([`notches_of`]) before anything is done with it, and a notch is a **ratio** and not a
//!   subtraction ([`zoom_by`]) — the same felt step close in as far out.
//! * **egui asks first.** A wheel turned over a panel is the panel's scroll bar's, and the
//!   question is `wants_pointer_input() || is_pointer_over_area()`: egui's own
//!   `wants_pointer_input` is false for a pointer resting on a panel *with a button held*, which
//!   is exactly when a drag is under way.
//! * **A drag is decided when the button goes down.** One that began on the world stays the
//!   camera's wherever the pointer travels, and one that began on a panel never becomes the
//!   camera's however far it is dragged out of it.
//! * **A press that let go where it went down was a click**, and anything further was a drag
//!   ([`is_click`]). The click leaves as a [`WorldClick`] message carrying the world point that
//!   was under the cursor; turning that into a tile, a robot or a creature is the game's.
//!
//! **Where the numbers are.** Every one of them is a field of [`CameraControls`], which a game
//! can hand the plugin, write into at any time, or let a player change through
//! [`Settings`](crate::Settings) (`camera_*` keys, [`CameraControls::read_from`]). The `const`s
//! here are the names of the defaults and nothing else; each one says in a line where its value
//! came from, and where that is "the garden had it and nobody wrote down why", it says so.

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
// **What covers the window** is `rubevy-egui`'s, because the panel that writes it is
// (`rubevy_egui::EditorPlugin`); a camera is only ever a reader of it (S4a). Re-exported so that
// a game reaching for the camera's vocabulary finds it here, where it used to be defined.
pub use rubevy_egui::ViewInsets;

// ---------------------------------------------------------------------------------------------
// The numbers
// ---------------------------------------------------------------------------------------------

/// How much nearer one notch of the wheel brings the view.
///
/// The garden's, and **derived there rather than picked**: a ratio of 1.10 puts about thirty
/// notches between the two ends of the zoom range, which is ten flicks of a finger
/// (`garden/src/main.rs`, G6, and its unit test — the same range is checked below).
pub const ZOOM_PER_NOTCH: f32 = 1.10;

/// One notch of a wheel, in the pixels a browser measures one in.
///
/// An outside fact rather than a taste: Chromium's `wheel` event carries `deltaY = 100` for one
/// notch of a real mouse, and Firefox sends three lines, which winit turns back into `Line`
/// (`garden/src/main.rs`, G6). It is a field all the same, because a trackpad is not a mouse.
pub const PIXELS_PER_NOTCH: f32 = 100.0;

/// How far in the wheel may go, as a fraction of the home view.
///
/// The garden's `ZOOM_MIN` over its default distance (6 of 42). Written as the division it is so
/// that it says where it came from; the garden's own two numbers have no recorded source
/// (`docs/numbers.md`), only a sentence about what they look like, so **this is inherited and not
/// justified**. What *is* justified is the ratio between the two: 110/6 is the span the notch
/// size above was derived from.
pub const ZOOM_IN_LIMIT: f32 = 6.0 / 42.0;

/// How far out the wheel may go, as a fraction of the home view (the garden's `ZOOM_MAX` over its
/// default distance). See [`ZOOM_IN_LIMIT`]: inherited, and the span is what matters.
pub const ZOOM_OUT_LIMIT: f32 = 110.0 / 42.0;

/// How far the world moves per pixel of drag, as a multiple of "keeps up with the cursor".
///
/// **This one is derived and not inherited.** The garden's `PAN_PER_PIXEL` is 0.0016 world units
/// per pixel per unit of camera distance, a number with no recorded source, and in three
/// dimensions there is no exact answer to inherit: a point on the ground is at a different
/// distance in the top of the window than in the bottom. Flat on, there is one — a pixel is worth
/// exactly `2 × half_height / window_height` world units — so the default is 1.0, meaning the
/// point of the world that was under the cursor when the button went down is still under it when
/// it comes up. (Exactly so where the window's scale factor is 1: Bevy reports mouse motion in
/// the device's own pixels.)
pub const DRAG_PER_PIXEL: f32 = 1.0;

/// How fast the keys walk the camera, in half-views per second.
///
/// The garden's `PAN_PER_SECOND`, 0.9 world units per second per unit of camera distance, with
/// the half-view standing where the distance stood — both are the number that says how much world
/// the window holds. **The garden's value has no recorded source** (`docs/numbers.md`: "不明"),
/// so this is "the garden's speed, transcribed", not a measurement.
pub const KEYS_PER_SECOND: f32 = 0.9;

/// How far the mouse may travel between press and release and still be a click rather than a drag,
/// in pixels. The garden's `CLICK_SLOP` (`garden/src/window.rs`), whose value has no recorded
/// source beyond "a few pixels".
pub const CLICK_SLOP: f32 = 6.0;

/// **Which keys and buttons drive the camera.** Every one of them is a list, so a game can add to
/// them or empty one out to take a gesture away.
#[derive(Debug, Clone)]
pub struct CameraKeys {
    pub left: Vec<KeyCode>,
    pub right: Vec<KeyCode>,
    pub up: Vec<KeyCode>,
    pub down: Vec<KeyCode>,
    /// Puts the camera back where it started ([`CameraHome`]).
    pub home: Vec<KeyCode>,
    /// Held with [`CameraKeys::click_button`], this drags too — for a trackpad, and for a browser
    /// that keeps the right button for its own menu.
    pub drag_modifier: Vec<KeyCode>,
    /// Buttons that drag on their own.
    pub drag_buttons: Vec<MouseButton>,
    /// The button that picks something out of the world ([`WorldClick`]).
    pub click_button: MouseButton,
}

impl Default for CameraKeys {
    fn default() -> Self {
        // the garden's set, which is the one the two published games have taught anybody who has
        // played them: WASD or the arrows, right-drag or Shift-drag, Home to go back
        CameraKeys {
            left: vec![KeyCode::KeyA, KeyCode::ArrowLeft],
            right: vec![KeyCode::KeyD, KeyCode::ArrowRight],
            up: vec![KeyCode::KeyW, KeyCode::ArrowUp],
            down: vec![KeyCode::KeyS, KeyCode::ArrowDown],
            home: vec![KeyCode::Home],
            drag_modifier: vec![KeyCode::ShiftLeft, KeyCode::ShiftRight],
            drag_buttons: vec![MouseButton::Right],
            click_button: MouseButton::Left,
        }
    }
}

/// **Every number the camera has**, in one resource a game can hand the plugin or write into
/// while it runs. The defaults are the `const`s above, each of which says where its value came
/// from.
#[derive(Resource, Debug, Clone)]
pub struct CameraControls {
    /// [`ZOOM_PER_NOTCH`]
    pub zoom_per_notch: f32,
    /// [`PIXELS_PER_NOTCH`]
    pub pixels_per_notch: f32,
    /// [`ZOOM_IN_LIMIT`]
    pub zoom_in_limit: f32,
    /// [`ZOOM_OUT_LIMIT`]
    pub zoom_out_limit: f32,
    /// [`DRAG_PER_PIXEL`]
    pub drag_per_pixel: f32,
    /// [`KEYS_PER_SECOND`]
    pub keys_per_second: f32,
    /// [`CLICK_SLOP`]
    pub click_slop: f32,
    /// How far the camera may be walked, in world units. `None` — the default — is a world with
    /// no edge: a game that has one (the garden stops the eye a little past its wall) says where
    /// it is, because this crate cannot know.
    pub bounds: Option<Rect>,
    pub keys: CameraKeys,
}

impl Default for CameraControls {
    fn default() -> Self {
        CameraControls {
            zoom_per_notch: ZOOM_PER_NOTCH,
            pixels_per_notch: PIXELS_PER_NOTCH,
            zoom_in_limit: ZOOM_IN_LIMIT,
            zoom_out_limit: ZOOM_OUT_LIMIT,
            drag_per_pixel: DRAG_PER_PIXEL,
            keys_per_second: KEYS_PER_SECOND,
            click_slop: CLICK_SLOP,
            bounds: None,
            keys: CameraKeys::default(),
        }
    }
}

impl CameraControls {
    /// **What a player left in the `key=value` store** ([`Settings`](crate::Settings)), for the
    /// numbers a person can sensibly be asked about. Keys, buttons and the world's edge are not
    /// among them: a key name in a text file would need a parser, and the edge is the game's.
    ///
    /// | key | field |
    /// |---|---|
    /// | `camera_zoom_per_notch` | [`CameraControls::zoom_per_notch`] |
    /// | `camera_pixels_per_notch` | [`CameraControls::pixels_per_notch`] |
    /// | `camera_zoom_in_limit` | [`CameraControls::zoom_in_limit`] |
    /// | `camera_zoom_out_limit` | [`CameraControls::zoom_out_limit`] |
    /// | `camera_drag_per_pixel` | [`CameraControls::drag_per_pixel`] |
    /// | `camera_keys_per_second` | [`CameraControls::keys_per_second`] |
    /// | `camera_click_slop` | [`CameraControls::click_slop`] |
    ///
    /// A key that is not there leaves the field alone, which is what makes a store written by an
    /// older build safe to read.
    pub fn read_from(&mut self, settings: &crate::Settings) {
        let take = |key: &str, slot: &mut f32| {
            if let Some(value) = settings.number(key) {
                *slot = value;
            }
        };
        take("camera_zoom_per_notch", &mut self.zoom_per_notch);
        take("camera_pixels_per_notch", &mut self.pixels_per_notch);
        take("camera_zoom_in_limit", &mut self.zoom_in_limit);
        take("camera_zoom_out_limit", &mut self.zoom_out_limit);
        take("camera_drag_per_pixel", &mut self.drag_per_pixel);
        take("camera_keys_per_second", &mut self.keys_per_second);
        take("camera_click_slop", &mut self.click_slop);
    }
}

// ---------------------------------------------------------------------------------------------
// Where the camera is
// ---------------------------------------------------------------------------------------------

/// **Where the camera starts, and where `Home` puts it back.** It is also what the zoom limits
/// are a fraction of, so a game that shows a world sixty units tall gets a range around that
/// rather than around some number this crate made up.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraHome {
    pub focus: Vec2,
    /// Half of how much world the window shows from top to bottom.
    pub half_height: f32,
}

/// **Where the camera is now.** A game may write into it (to follow something, or to answer a
/// script that asks to be shown a place); the systems here read it every frame and put the camera
/// where it says.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraView {
    /// The world point the camera is aimed at — which is the middle of *what can be seen*, not
    /// the middle of the window, where a panel covers part of it ([`ViewInsets`]).
    pub focus: Vec2,
    /// Half of how much world the window shows from top to bottom. Smaller is nearer.
    pub half_height: f32,
}

impl CameraView {
    /// The numbers that turn a point on the screen into a point in the world and back.
    pub fn lens(&self, window: Vec2, insets: &ViewInsets) -> Lens {
        let world_per_px = (self.half_height * 2.0) / window.y.max(1.0);
        Lens { centre: self.focus + insets.shift(world_per_px), world_per_px, window }
    }
}

/// **What the camera is showing, as the arithmetic between a screen point and a world point.**
///
/// It is a plain value rather than a system's business so that anything can ask: the click below
/// asks it, a game's own picking can ask it, and it is where a `camera.world_at(x, y)` from Ruby
/// would be answered from when rubevy's script-facing layer (R9) arrives — one `Res<CameraView>`,
/// one `Res<ViewInsets>` and the window is the whole of what an answer needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    /// The world point at the middle of the window (not the middle of what is visible).
    pub centre: Vec2,
    /// World units per pixel.
    pub world_per_px: f32,
    /// The window, in pixels.
    pub window: Vec2,
}

impl Lens {
    /// The world point under a cursor position (Bevy's: pixels from the window's top left, `y`
    /// downwards).
    pub fn world_at(&self, cursor: Vec2) -> Vec2 {
        let from_middle = cursor - self.window * 0.5;
        self.centre + Vec2::new(from_middle.x, -from_middle.y) * self.world_per_px
    }

    /// Where a world point is drawn, in the same cursor pixels.
    pub fn screen_at(&self, world: Vec2) -> Vec2 {
        let from_centre = (world - self.centre) / self.world_per_px;
        self.window * 0.5 + Vec2::new(from_centre.x, -from_centre.y)
    }
}

// ---------------------------------------------------------------------------------------------
// The arithmetic, apart from the systems that call it
// ---------------------------------------------------------------------------------------------

/// One wheel message as a number of notches, whatever unit it arrived in.
pub fn notches_of(unit: MouseScrollUnit, y: f32, pixels_per_notch: f32) -> f32 {
    match unit {
        MouseScrollUnit::Line => y,
        MouseScrollUnit::Pixel => y / pixels_per_notch,
    }
}

/// `notches` notches of wheel from `half_height`, as a ratio, kept inside the range the home view
/// and the two limits describe. Positive notches (the wheel pushed forward) come nearer.
pub fn zoom_by(half_height: f32, notches: f32, home: f32, controls: &CameraControls) -> f32 {
    (half_height * controls.zoom_per_notch.powf(-notches))
        .clamp(home * controls.zoom_in_limit, home * controls.zoom_out_limit)
}

/// Whether a press that went down at `down` and came up at `up` was a click rather than a drag.
pub fn is_click(down: Vec2, up: Vec2, slop: f32) -> bool {
    down.distance(up) <= slop
}

// ---------------------------------------------------------------------------------------------
// The plugin
// ---------------------------------------------------------------------------------------------

/// The camera this plugin drives. A game that wants to look through it for itself — a ray, a
/// viewport, a second camera for a minimap — asks for this rather than for every `Camera2d`.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PanCamera;

/// **Somebody clicked a place in the world.** What is there is the game's question: this crate
/// has no tiles, no robots and no creatures in it.
///
/// It is written on the *release*, and only for a press that stayed within
/// [`CameraControls::click_slop`] of where it went down and did not begin over a panel.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct WorldClick {
    /// Where in the world the cursor was.
    pub at: Vec2,
    /// Which button it was, so a game can give the other ones meanings of its own.
    pub button: MouseButton,
    /// Where on the screen it was, for a game that wants to open something next to the cursor.
    pub cursor: Vec2,
}

/// **A 2D camera the player drives**: drag, wheel, walk, `Home`.
///
/// It spawns its own [`Camera2d`], so it is an alternative to
/// [`ArenaPlugin`](crate::ArenaPlugin) rather than an addition to it.
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use games_shell::camera::{CameraControls, CameraPlugin};
/// # let mut app = App::new();
/// app.add_plugins(
///     // half the world the window shows from top to bottom, in world units
///     CameraPlugin::showing(20.0)
///         .centred_on(Vec2::new(8.0, 0.0))
///         // the world has an edge, and this crate could not have guessed where
///         .with(CameraControls { bounds: Some(Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(64.0))), ..default() }),
/// );
/// ```
pub struct CameraPlugin {
    pub home: CameraHome,
    pub controls: CameraControls,
    /// Whether to read `camera_*` out of [`Settings`](crate::Settings) at startup, where the game
    /// has a store. On by default; a game whose numbers are not a player's business turns it off.
    pub from_settings: bool,
}

impl CameraPlugin {
    /// The camera, showing `half_height` world units above and below the middle. There is no
    /// default for it: how much world a window should hold is the one thing only the game knows.
    pub fn showing(half_height: f32) -> CameraPlugin {
        CameraPlugin {
            home: CameraHome { focus: Vec2::ZERO, half_height },
            controls: CameraControls::default(),
            from_settings: true,
        }
    }

    /// Where it looks at first, and where `Home` puts it back.
    pub fn centred_on(mut self, focus: Vec2) -> CameraPlugin {
        self.home.focus = focus;
        self
    }

    /// The numbers, changed.
    pub fn with(mut self, controls: CameraControls) -> CameraPlugin {
        self.controls = controls;
        self
    }
}

/// Where the camera's work sits in a frame, for a game that wants to run before or after it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraSet {
    /// Reads the mouse and the keys and writes [`CameraView`] (in `Update`).
    Drive,
    /// Writes the camera's `Transform` and projection from [`CameraView`] and [`ViewInsets`]
    /// (in `PostUpdate`).
    Place,
}

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.home)
            .insert_resource(CameraView { focus: self.home.focus, half_height: self.home.half_height })
            .insert_resource(self.controls.clone())
            .init_resource::<ViewInsets>()
            .add_message::<WorldClick>()
            .add_systems(Startup, spawn_camera);
        if self.from_settings {
            app.add_systems(PreStartup, settings_override);
        }
        app.add_systems(Update, (drive_camera, report_clicks).in_set(CameraSet::Drive))
            .add_systems(PostUpdate, place_camera.in_set(CameraSet::Place));
    }
}

/// Whatever the player left in the store, before the first frame draws anything with it.
fn settings_override(settings: Option<Res<crate::Settings>>, mut controls: ResMut<CameraControls>) {
    if let Some(settings) = settings {
        controls.read_from(&settings);
    }
}

fn spawn_camera(mut commands: Commands, view: Res<CameraView>) {
    commands.spawn((
        Camera2d,
        PanCamera,
        // the height is what is held constant: a wider window shows more world at the sides,
        // which is where the panels sit
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: view.half_height * 2.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// Drag to slide the world, wheel to come nearer, the keys to walk, `Home` to put it back.
///
/// The three rules the garden paid for are all in here: a drag is decided when the button goes
/// down, the wheel is read whether or not it is used (so that a turn over a panel is not still
/// sitting in the reader when the pointer comes off it), and egui is asked with both of its
/// questions.
#[allow(clippy::too_many_arguments)]
fn drive_camera(
    time: Res<Time>,
    controls: Res<CameraControls>,
    home: Res<CameraHome>,
    mut view: ResMut<CameraView>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    pointer: Option<Res<bevy_egui::input::EguiWantsInput>>,
    windows: Query<&Window>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    // whether the drag under way is the camera's: decided on the press, held until the buttons
    // are all up again
    mut grabbed: Local<bool>,
) {
    let (egui_pointer, mine_keys) = match pointer {
        Some(p) => (p.wants_pointer_input() || p.is_pointer_over_area(), !p.wants_keyboard_input()),
        None => (false, true),
    };
    let height = windows.iter().next().map(|w| w.height()).unwrap_or(1.0).max(1.0);
    let world_per_px = (view.half_height * 2.0) / height;

    let mut dragging: Vec<MouseButton> = controls.keys.drag_buttons.clone();
    let modifier = controls.keys.drag_modifier.iter().any(|k| keys.pressed(*k));
    if modifier {
        dragging.push(controls.keys.click_button);
    }
    // the button that picks things is a drag button too, so a press on it decides the grab: a
    // click that turns out to have travelled is a drag of the camera and not a lost click
    let watched: Vec<MouseButton> =
        dragging.iter().copied().chain(std::iter::once(controls.keys.click_button)).collect();
    if !watched.iter().any(|b| buttons.pressed(*b)) {
        *grabbed = false;
    } else if watched.iter().any(|b| buttons.just_pressed(*b)) {
        *grabbed = !egui_pointer;
    }
    let sliding = *grabbed && dragging.iter().any(|b| buttons.pressed(*b));

    let mut focus = view.focus;
    for m in motion.read() {
        if sliding {
            // the ground is grabbed and pulled: the cursor goes right, the world goes right, so
            // the point the camera is aimed at goes left
            let step = world_per_px * controls.drag_per_pixel;
            focus += Vec2::new(-m.delta.x, m.delta.y) * step;
        }
    }
    for w in wheel.read() {
        if egui_pointer {
            continue;
        }
        let notches = notches_of(w.unit, w.y, controls.pixels_per_notch);
        view.half_height = zoom_by(view.half_height, notches, home.half_height, &controls);
    }
    if mine_keys {
        let mut d = Vec2::ZERO;
        if controls.keys.up.iter().any(|k| keys.pressed(*k)) {
            d.y += 1.0;
        }
        if controls.keys.down.iter().any(|k| keys.pressed(*k)) {
            d.y -= 1.0;
        }
        if controls.keys.right.iter().any(|k| keys.pressed(*k)) {
            d.x += 1.0;
        }
        if controls.keys.left.iter().any(|k| keys.pressed(*k)) {
            d.x -= 1.0;
        }
        if d != Vec2::ZERO {
            // the half-view is how much world the window holds, so walking at a fixed share of it
            // per second feels the same close in as far out — which is the reason the garden made
            // its own speed proportional to the camera's distance
            let step = controls.keys_per_second * view.half_height * time.delta_secs();
            focus += d.normalize_or_zero() * step;
        }
        if controls.keys.home.iter().any(|k| keys.just_pressed(*k)) {
            focus = home.focus;
            view.half_height = home.half_height;
        }
    }
    view.focus = match controls.bounds {
        Some(edge) => focus.clamp(edge.min, edge.max),
        None => focus,
    };
}

/// A press that let go where it went down, as a point in the world.
fn report_clicks(
    controls: Res<CameraControls>,
    view: Res<CameraView>,
    insets: Res<ViewInsets>,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Option<Res<bevy_egui::input::EguiWantsInput>>,
    windows: Query<&Window>,
    mut clicked: MessageWriter<WorldClick>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    let egui_pointer = pointer
        .is_some_and(|p| p.wants_pointer_input() || p.is_pointer_over_area());
    let Some(window) = windows.iter().next() else { return };
    let cursor = window.cursor_position();
    let button = controls.keys.click_button;
    if buttons.just_pressed(button) {
        *pressed_at = if egui_pointer { None } else { cursor };
    }
    if buttons.just_released(button)
        && let (Some(down), Some(up)) = (pressed_at.take(), cursor)
        && is_click(down, up, controls.click_slop)
    {
        let lens = view.lens(Vec2::new(window.width(), window.height()), &insets);
        clicked.write(WorldClick { at: lens.world_at(up), button, cursor: up });
    }
}

/// [`CameraView`] and [`ViewInsets`], as a `Transform` and a projection.
fn place_camera(
    view: Res<CameraView>,
    insets: Res<ViewInsets>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<PanCamera>>,
) {
    let height = windows.iter().next().map(|w| w.height()).unwrap_or(1.0).max(1.0);
    let world_per_px = (view.half_height * 2.0) / height;
    let centre = view.focus + insets.shift(world_per_px);
    for (mut projection, mut transform) in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode =
                bevy::camera::ScalingMode::FixedVertical { viewport_height: view.half_height * 2.0 };
        }
        transform.translation.x = centre.x;
        transform.translation.y = centre.y;
    }
}

// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The garden's `Orbit::default().distance`. The wheel tests below are the garden's own, with
    /// this standing where its camera distance stood, so that the numbers being compared are the
    /// ones that were checked in three dimensions (`garden/src/main.rs`).
    const HOME: f32 = 42.0;

    fn controls() -> CameraControls {
        CameraControls::default()
    }

    #[test]
    fn a_notch_is_a_notch_in_either_unit() {
        let c = controls();
        // a PC mouse through winit: one notch, one line
        assert_eq!(notches_of(MouseScrollUnit::Line, 1.0, c.pixels_per_notch), 1.0);
        // Chromium: one notch, a hundred pixels of would-be scrolling
        assert_eq!(notches_of(MouseScrollUnit::Pixel, 100.0, c.pixels_per_notch), 1.0);
        assert_eq!(notches_of(MouseScrollUnit::Pixel, -300.0, c.pixels_per_notch), -3.0);
    }

    #[test]
    fn ten_notches_are_ten_steps_and_not_the_end_of_the_range() {
        let c = controls();
        // what the author's browser did: ten notches from the home view, in pixels, one at a time
        let mut h = HOME;
        let mut seen = vec![h];
        for _ in 0..10 {
            h = zoom_by(h, notches_of(MouseScrollUnit::Pixel, 100.0, c.pixels_per_notch), HOME, &c);
            seen.push(h);
        }
        for pair in seen.windows(2) {
            assert!((pair[0] / pair[1] - c.zoom_per_notch).abs() < 1e-4, "{pair:?}");
        }
        assert!(seen.last().unwrap() > &(HOME * c.zoom_in_limit), "{seen:?}");
        // and the same ten in lines land in the same place: this is the whole of the browser fix
        let mut line = HOME;
        for _ in 0..10 {
            line = zoom_by(line, notches_of(MouseScrollUnit::Line, 1.0, c.pixels_per_notch), HOME, &c);
        }
        assert!((line - seen[10]).abs() < 1e-3, "{line} vs {}", seen[10]);
    }

    #[test]
    fn the_range_is_reached_but_not_passed() {
        let c = controls();
        let (near, far) = (HOME * c.zoom_in_limit, HOME * c.zoom_out_limit);
        // the garden's two ends, since the limits are its numbers over its distance
        assert!((near - 6.0).abs() < 1e-4, "{near}");
        assert!((far - 110.0).abs() < 1e-4, "{far}");
        assert_eq!(zoom_by(near, 5.0, HOME, &c), near);
        assert_eq!(zoom_by(far, -5.0, HOME, &c), far);
        // from one end to the other is about thirty notches, not one
        let notches = (c.zoom_out_limit / c.zoom_in_limit).ln() / c.zoom_per_notch.ln();
        assert!((29.0..32.0).contains(&notches), "{notches}");
    }

    #[test]
    fn a_press_that_travelled_is_a_drag_and_not_a_click() {
        let slop = controls().click_slop;
        let down = Vec2::new(400.0, 300.0);
        assert!(is_click(down, down, slop), "a press that did not move at all");
        assert!(is_click(down, down + Vec2::new(3.0, 4.0), slop), "five pixels is still a click");
        assert!(is_click(down, down + Vec2::new(6.0, 0.0), slop), "the slop itself is a click");
        assert!(!is_click(down, down + Vec2::new(0.0, 6.1), slop), "past it is a drag");
        assert!(!is_click(down, down + Vec2::new(60.0, 60.0), slop));
    }

    #[test]
    fn a_covered_edge_moves_the_middle_and_nothing_else() {
        // sabibots' window, with the editor open over the right of it
        let window = Vec2::new(1600.0, 900.0);
        let view = CameraView { focus: Vec2::ZERO, half_height: 35.0 };
        let open = ViewInsets { right: 528.0, ..ViewInsets::NONE };

        let shut = view.lens(window, &ViewInsets::NONE);
        assert_eq!(shut.centre, Vec2::ZERO, "nothing covered, nothing moved");
        let lens = view.lens(window, &open);
        // half of what is covered, in world units
        assert!((lens.centre.x - 264.0 * lens.world_per_px).abs() < 1e-4, "{}", lens.centre.x);
        assert_eq!(lens.centre.y, 0.0, "the top and the bottom are clear");
        assert_eq!(lens.world_per_px, shut.world_per_px, "covering an edge is not a zoom");

        // and the point of it: what the camera is aimed at is drawn in the middle of what is left
        let middle_of_the_rest = Vec2::new((window.x - 528.0) / 2.0, window.y / 2.0);
        let drawn = lens.screen_at(view.focus);
        assert!((drawn - middle_of_the_rest).length() < 1e-3, "{drawn} vs {middle_of_the_rest}");
        // a cursor is still turned into the world point it is over
        assert!((lens.world_at(drawn) - view.focus).length() < 1e-3);
    }

    #[test]
    fn a_covered_top_and_bottom_move_it_the_other_way() {
        let window = Vec2::new(1600.0, 900.0);
        let view = CameraView { focus: Vec2::new(10.0, -4.0), half_height: 20.0 };
        let hud = ViewInsets { top: 100.0, bottom: 20.0, ..ViewInsets::NONE };
        let lens = view.lens(window, &hud);
        // a band across the top pushes what is looked at up the world, not down
        assert!(lens.centre.y > view.focus.y, "{}", lens.centre.y);
        let middle_of_the_rest = Vec2::new(window.x / 2.0, (100.0 + window.y - 20.0) / 2.0);
        let drawn = lens.screen_at(view.focus);
        assert!((drawn - middle_of_the_rest).length() < 1e-3, "{drawn} vs {middle_of_the_rest}");
    }

    #[test]
    fn a_cursor_and_a_world_point_are_the_same_point_read_two_ways() {
        let window = Vec2::new(1280.0, 720.0);
        let view = CameraView { focus: Vec2::new(-3.0, 7.5), half_height: 12.0 };
        let lens = view.lens(window, &ViewInsets { left: 64.0, bottom: 32.0, ..ViewInsets::NONE });
        for cursor in [Vec2::ZERO, Vec2::new(640.0, 360.0), Vec2::new(1279.0, 12.0)] {
            let back = lens.screen_at(lens.world_at(cursor));
            assert!((back - cursor).length() < 1e-3, "{back} vs {cursor}");
        }
    }

    #[test]
    fn the_store_changes_a_number_and_leaves_the_rest() {
        let path = std::env::temp_dir().join("games-shell-camera-settings-test.txt");
        let _ = std::fs::remove_file(&path);
        fn read(path: &std::path::Path) -> Result<String, String> {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        fn write(path: &std::path::Path, text: &str) -> Result<(), String> {
            std::fs::write(path, text).map_err(|e| e.to_string())
        }
        let mut settings = crate::Settings::load(&path, "a test", read, write);
        settings.set("camera_keys_per_second", "2.5");

        let mut c = CameraControls::default();
        c.read_from(&settings);
        assert_eq!(c.keys_per_second, 2.5);
        assert_eq!(c.click_slop, CLICK_SLOP, "a key nobody wrote leaves the default alone");
        let _ = std::fs::remove_file(&path);
    }
}
