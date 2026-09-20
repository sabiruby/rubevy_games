//! **The shell the games in this repository share.** Not the panels — those are `rubevy-egui`,
//! which this crate depends on — but everything a game here needs around them: what differs
//! between a PC build and a browser build, how a run is asked for its checks, the flags, the
//! in-game guide, the `key=value` store, the HUD, and two cameras.
//!
//! * [`ArenaPlugin`] — a 2D camera that shows a square arena whatever the window size is.
//! * [`camera`] — the other camera: one the player drags, wheels and walks about a world bigger
//!   than the window. Both of them read [`ViewInsets`] (`rubevy-egui`'s), the pixels a panel
//!   covers, to keep what matters out from under it.
//! * [`Hud`] — the line of text at the top, and the per-script panel below it.
//! * [`Guide`] — the in-game explanation (G6), in English or Japanese, that `H` opens. The frame
//!   and the Japanese font are here; the words are each game's, and G6b's `English | 日本語`
//!   buttons pick which of the two is drawn.
//! * [`Settings`] — the handful of `key=value` lines a game remembers between runs: the guide's
//!   language, the garden's night dial. Where they are kept is the game's `platform.rs`.
//! * [`platform`] — what differs between a PC build and a browser build: a file or a
//!   `localStorage` key, the compiler linked in or the page's, a clock for the dice.
//! * [`checks`] — how a run is asked for its `selftest`, and why a page does not exit when it is
//!   done.
//! * [`Args`] — the flags both games take (`--headless`, `--shot`, `--vm`, `--lang`), with every
//!   default left to the caller.
//!
//! Bevy's version is pinned once in the workspace; the version-dependent parts of a game live
//! here and in `rubevy-egui`, so bumping Bevy is two crates' problem rather than every game's.
//!
//! This crate and `rubevy-egui` were cut out of `rubevy-arena` (2026-09-20), whose name said
//! "the walled square SabiRuby Battle fights in" and whose contents had long since stopped being
//! that. The dependency goes one way only: the shell knows about the panels, and the panels know
//! nothing about the shell.

use bevy::prelude::*;

pub mod args;
pub mod camera;
pub mod checks;
pub mod guide;
pub mod hud;
pub mod platform;
pub mod settings;
pub use args::Args;
pub use camera::{
    CameraControls, CameraHome, CameraKeys, CameraPlugin, CameraSet, CameraView, Lens, PanCamera,
    WorldClick,
};
pub use guide::{Guide, GuideKey, GuideLang, GuideNote, GuidePlugin, GuideStyle};
pub use hud::{Hud, HudPlugin, HudStyle, ScriptPanel};
pub use settings::{remembered, PanelSettingsPlugin, Settings};
/// The pixels a panel covers, which both cameras here read. It is `rubevy-egui`'s, because the
/// panel that writes it is (`rubevy_egui::ViewInsets`); this is the same type under a second name.
pub use rubevy_egui::ViewInsets;

/// Half the width of the square the camera shows, in world units.
///
/// **There is no default** (S5b-1). How much world a window should hold is the one thing only the
/// game knows — the same reason [`CameraPlugin::showing`] has no default either — and the 32.0
/// that used to be here was SabiRuby Battle's arena sitting in a crate that has no arena in it.
/// It is now `sabibots`' own `ARENA_HALF_WIDTH`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ArenaSize(pub f32);

/// **The floor shown past the wall**, in world units: what stands at the very edge is not cut off
/// by the window.
///
/// *Reason only* — the sentence above is the whole of the record, and 3.0 itself has **no
/// recorded source** (`docs/numbers.md` §1.1). It was written into two expressions until S5b-1;
/// it is [`ArenaPlugin::floor_margin`] now.
pub const FLOOR_MARGIN: f32 = 3.0;

/// The shape a frame with no window stands in for, which is only ever read as a ratio.
///
/// **Cited**, as far as it goes: "the window is 16:9 and the arena square"
/// (`docs/sabiruby-battle.md`). It is not a setting because nothing is drawn on a frame with no
/// window: it keeps the arithmetic below from dividing by a zero-sized window, and what it
/// computes is thrown away.
const NO_WINDOW: Vec2 = Vec2::new(16.0, 9.0);

/// A 2D camera scaled so that the arena fills the window, and the arena floor behind it.
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use games_shell::ArenaPlugin;
/// # let mut app = App::new();
/// // half the width of the square, in world units: the game's number, not this crate's
/// app.add_plugins(ArenaPlugin::showing(32.0));
/// ```
pub struct ArenaPlugin {
    pub size: ArenaSize,
    pub floor: Color,
    /// Whether the camera zooms in as the arena shrinks. Off by default: with the view fixed, the
    /// walls are seen moving in, which is the point of shrinking them.
    pub follow_shrink: bool,
    /// [`FLOOR_MARGIN`]
    pub floor_margin: f32,
}

impl ArenaPlugin {
    /// The camera, framed on a square `half` world units from the middle to a wall.
    pub fn showing(half: f32) -> ArenaPlugin {
        ArenaPlugin {
            size: ArenaSize(half),
            // *source unknown*: the dark the arena has always been drawn on
            floor: Color::srgb(0.08, 0.08, 0.10),
            follow_shrink: false,
            floor_margin: FLOOR_MARGIN,
        }
    }

    /// The floor shown past the wall, changed ([`FLOOR_MARGIN`]).
    pub fn with_floor_margin(mut self, margin: f32) -> ArenaPlugin {
        self.floor_margin = margin;
        self
    }
}

/// How the camera treats the arena: the size the view was framed for, whether it follows, and how
/// much floor is shown past the wall.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ArenaView {
    pub framed: f32,
    pub follow_shrink: bool,
    /// [`FLOOR_MARGIN`]
    pub floor_margin: f32,
}

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.size)
            .insert_resource(ArenaView {
                framed: self.size.0,
                follow_shrink: self.follow_shrink,
                floor_margin: self.floor_margin,
            })
            .insert_resource(ClearColor(self.floor))
            .init_resource::<ViewInsets>()
            .add_systems(Startup, spawn_camera)
            .add_systems(PostUpdate, follow_arena);
    }
}

fn spawn_camera(mut commands: Commands, size: Res<ArenaSize>, view: Res<ArenaView>) {
    commands.spawn((
        Camera2d,
        // whatever the window's shape, the arena's height is in view; a wide window shows more
        // floor at the sides, which is where the panels sit
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: view_of(size.0, view.floor_margin),
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn view_of(half: f32, floor_margin: f32) -> bevy::camera::ScalingMode {
    // a little floor past the wall, so what stands at the edge is not cut off by the window
    let seen = half + floor_margin;
    bevy::camera::ScalingMode::AutoMin { min_width: seen * 2.0, min_height: seen * 2.0 }
}

/// How much world the window holds and what one pixel of it is worth, for an arena of this half
/// width. The margin is [`view_of`]'s: the floor shown past the wall.
fn seen_by(half: f32, window: Vec2, floor_margin: f32) -> (f32, f32) {
    let seen = half + floor_margin;
    let aspect = window.x / window.y.max(1.0);
    let visible_width = seen * 2.0 * aspect.max(1.0);
    (visible_width, visible_width / window.x.max(1.0))
}

/// **Everything [`follow_arena`] reads.** Where the camera ends up is a function of exactly these
/// three, so keeping the last one is what tells the system whether there is anything to do.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Placed {
    half: f32,
    window: Vec2,
    insets: ViewInsets,
}

/// A match that closes the arena in changes [`ArenaSize`]; the view follows it, so the fight
/// fills the window as the field gets smaller. Where a panel covers part of the window
/// ([`ViewInsets`]) the view slides, so the arena sits in the middle of what is left.
///
/// **It used to read `Res<Editor>` and slide by 16% of the window** — half of "the editor is
/// about a third of it on the right", which is a guess about a panel written into a camera. The
/// same sentence said twice: the panel now says how many pixels it covers and this takes half of
/// them, so a camera that has never heard of an editor puts the arena in the same place. On the
/// window both games open, 1600 by 900, the editor covers 528 pixels — 33% rather than the 32%
/// the old fraction stood for, which is the whole of the difference (`seen_by`'s test).
///
/// **When it recomputes** (S4a). It used to leave early unless [`ArenaSize`] or [`ViewInsets`]
/// had changed, and it followed a window being resized only by accident: `Editor` counts as
/// changed every frame it is drawn, so the old early return let nearly every frame through while
/// the panel was open. What is actually wanted is "recompute when what the answer depends on has
/// changed", and that is the three fields of [`Placed`] — the arena's size, the window, and the
/// insets — kept from the last placement and compared. A resize now moves the view whether or not
/// anything else did, and a still window with a still panel costs one comparison a frame.
///
/// (`Changed<Window>` would have been the obvious filter and is not the right one: winit writes
/// the cursor's position into the same `Window` component on every mouse move
/// (`bevy_winit::state`, `WindowEvent::CursorMoved`), so it is true whenever the pointer is
/// moving and false while a window is resized with the pointer outside it.)
fn follow_arena(
    size: Res<ArenaSize>,
    view: Res<ArenaView>,
    insets: Res<ViewInsets>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<Camera2d>>,
    mut placed: Local<Option<Placed>>,
) {
    // with following off, the view stays framed on the arena as it started
    let half = if view.follow_shrink { size.0 } else { view.framed };
    let window = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(NO_WINDOW);
    let now = Placed { half, window, insets: *insets };
    if *placed == Some(now) {
        return;
    }
    *placed = Some(now);
    let (_, world_per_px) = seen_by(half, window, view.floor_margin);
    let slide = insets.shift(world_per_px);
    for (mut projection, mut transform) in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = view_of(half, view.floor_margin);
        }
        transform.translation.x = slide.x;
        transform.translation.y = slide.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rubevy_egui::EditorLayout;

    /// **The half width the figures below were taken on**: SabiRuby Battle's arena, which is the
    /// game's own number since S5b-1 (`sabibots`' `ARENA_HALF_WIDTH`). The tests keep a copy
    /// because what they are about is the arithmetic, and it has to be done on *some* square.
    const HALF: f32 = 32.0;

    /// **The 16% said again, as pixels.** The old line slid the arena by `visible_width * 0.16`
    /// whenever the editor was open, which was half of "the editor is about a third of the
    /// window": a third covered, so the middle of the rest is a sixth of the window from the
    /// middle. The new line takes half of however many pixels the panel says it covers, so the
    /// first assertion is the old number and the new rule agreeing exactly on the window the old
    /// one was talking about.
    #[test]
    fn the_slide_is_half_of_whatever_is_covered() {
        let window = Vec2::new(1600.0, 900.0);
        let (visible_width, world_per_px) = seen_by(HALF, window, FLOOR_MARGIN);

        let a_third = ViewInsets { right: window.x / 3.0, ..ViewInsets::NONE };
        let slide = a_third.shift(world_per_px).x;
        assert!((slide - visible_width / 6.0).abs() < 1e-3, "{slide} vs {}", visible_width / 6.0);

        // and the editor as it actually stands, which is what sabibots will now see: 528 pixels
        // of a 1600-wide window is 33%, where 0.16 stood for 32%
        let editor = ViewInsets {
            right: EditorLayout::default().margin + EditorLayout::default().width,
            ..ViewInsets::NONE
        };
        let now = editor.shift(world_per_px).x;
        let before = visible_width * 0.16;
        assert!(now > before, "the real editor is wider than the guess: {now} vs {before}");
        assert!(
            (now - before) / visible_width < 0.006,
            "and by half a per cent of the window, not more: {now} vs {before}"
        );
        // in world units, on the window both games open
        assert!((now - 20.533).abs() < 1e-2, "{now}");
        assert!((before - 19.911).abs() < 1e-2, "{before}");
    }

    /// **The floor past the wall is a setting now** (S5b-1), and it is what decides how much
    /// world the window holds: it used to be a `3.0` written into two expressions.
    #[test]
    fn the_floor_past_the_wall_is_what_the_plugin_was_given() {
        let window = Vec2::new(1600.0, 900.0);
        let (default_width, _) = seen_by(HALF, window, FLOOR_MARGIN);
        // the square plus the margin on both sides, widened for a window wider than it is tall
        assert!((default_width - (HALF + 3.0) * 2.0 * (1600.0 / 900.0)).abs() < 1e-3);

        let (wider, _) = seen_by(HALF, window, 10.0);
        assert!(wider > default_width, "more floor shown is more world in the window");
        let (none, _) = seen_by(HALF, window, 0.0);
        assert!(none < default_width, "and none of it is the wall at the very edge");

        // the plugin carries it to both of the places that used to spell it out
        let plugin = ArenaPlugin::showing(HALF).with_floor_margin(10.0);
        assert_eq!(plugin.floor_margin, 10.0);
        assert_eq!(ArenaPlugin::showing(HALF).floor_margin, FLOOR_MARGIN);
    }

    /// Nothing covered, nothing moved — a closed editor leaves the arena in the middle.
    #[test]
    fn nothing_covered_leaves_the_arena_where_it_was() {
        let (_, world_per_px) = seen_by(HALF, Vec2::new(1600.0, 900.0), FLOOR_MARGIN);
        assert_eq!(ViewInsets::NONE.shift(world_per_px), Vec2::ZERO);
    }

    /// **A window that was resized asks for a new placement; a still one does not** (S4a).
    ///
    /// This is the condition `follow_arena` leaves early on, written out. Before S4a the window
    /// was not part of it at all: the system followed a resize only because `Editor` reported
    /// itself changed on every frame it was drawn, which let nearly every frame past the early
    /// return while the panel was open — and which meant that a resize with the panel *closed*
    /// was not followed.
    #[test]
    fn a_resize_asks_for_a_new_placement_and_a_still_window_does_not() {
        let insets = ViewInsets { right: 528.0, ..ViewInsets::NONE };
        let at = |window| Placed { half: 32.0, window, insets };
        let wide = at(Vec2::new(1600.0, 900.0));

        assert_eq!(wide, at(Vec2::new(1600.0, 900.0)), "nothing moved: nothing to do");
        assert_ne!(wide, at(Vec2::new(1200.0, 900.0)), "the window was narrowed");
        assert_ne!(wide, Placed { insets: ViewInsets::NONE, ..wide }, "the editor was closed");
        assert_ne!(wide, Placed { half: 24.0, ..wide }, "the walls closed in");
    }

    /// **And a resize really does move the view**, which is what the placement is recomputed for
    /// — with the one asymmetry worth knowing: it is the window's *height* that matters.
    ///
    /// [`view_of`] frames the height, so a pixel is worth `2 × (half + 3) / height` world units
    /// whatever the width is. A shorter window makes every pixel worth more world, and the 528
    /// the editor covers becomes more world to slide out from under; a wider window changes
    /// nothing the transform has to say, because `AutoMin` widens the view on its own.
    #[test]
    fn it_is_the_height_of_the_window_that_moves_the_view() {
        let insets = ViewInsets { right: 528.0, ..ViewInsets::NONE };
        let slide = |window: Vec2| {
            let (_, world_per_px) = seen_by(HALF, window, FLOOR_MARGIN);
            insets.shift(world_per_px).x
        };
        let tall = slide(Vec2::new(1600.0, 900.0));
        let short = slide(Vec2::new(1600.0, 600.0));
        assert!(short > tall * 1.4, "{short} vs {tall}");
        assert!((slide(Vec2::new(1200.0, 900.0)) - tall).abs() < 1e-4, "the width is not it");
    }
}
