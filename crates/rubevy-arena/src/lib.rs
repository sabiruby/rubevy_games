//! The 2D floor the games here stand on. Three things, none of them game-specific:
//!
//! * [`ArenaPlugin`] — a 2D camera that shows a square arena whatever the window size is.
//! * [`Hud`] — the line of text at the top, and the per-script panel below it.
//! * [`CodePanel`] — the script a game is showing, with the line it stands on marked.
//! * [`Editor`] — the same, editable, over egui: change a robot's brain without leaving the game.
//! * [`Guide`] — the in-game explanation (G6), in English or Japanese, that `H` opens. The frame
//!   and the Japanese font are here; the words are each game's, and G6b's `English | 日本語`
//!   buttons pick which of the two is drawn.
//! * [`Settings`] — the handful of `key=value` lines a game remembers between runs: the guide's
//!   language, the garden's night dial. Where they are kept is the game's `platform.rs`.
//! * [`VmInspector`] — the frames, registers and heap of the selected script, read out of the VM.
//! * [`Watch`] — the Ruby directory, watched: saving a file tells the game to start that script
//!   again. Editing a robot's brain and seeing it change without restarting is the point.
//!
//! Bevy's version is pinned once in the workspace; the version-dependent parts of a game live
//! here, so bumping Bevy is one crate's problem rather than every game's.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

pub mod code;
pub mod editor;
pub mod guide;
pub mod hud;
pub mod inspect;
pub mod settings;
pub use code::{CodePanel, CodePanelPlugin};
pub use editor::{Editor, EditorAction, EditorChoice, EditorPlugin};
pub use guide::{Guide, GuideKey, GuideLang, GuideNote, GuidePlugin};
pub use hud::{Hud, HudPlugin, ScriptPanel};
pub use inspect::{VmClock, VmInspector, VmInspectorPlugin, Waiting};
pub use settings::Settings;

/// Half the width of the square the camera shows, in world units.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ArenaSize(pub f32);

impl Default for ArenaSize {
    fn default() -> Self {
        ArenaSize(32.0)
    }
}

/// A 2D camera scaled so that the arena fills the window, and the arena floor behind it.
pub struct ArenaPlugin {
    pub size: ArenaSize,
    pub floor: Color,
    /// Whether the camera zooms in as the arena shrinks. Off by default: with the view fixed, the
    /// walls are seen moving in, which is the point of shrinking them.
    pub follow_shrink: bool,
}

impl Default for ArenaPlugin {
    fn default() -> Self {
        ArenaPlugin { size: ArenaSize::default(), floor: Color::srgb(0.08, 0.08, 0.10), follow_shrink: false }
    }
}

/// How the camera treats the arena: the size the view was framed for, and whether it follows.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ArenaView {
    pub framed: f32,
    pub follow_shrink: bool,
}

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.size)
            .insert_resource(ArenaView { framed: self.size.0, follow_shrink: self.follow_shrink })
            .insert_resource(ClearColor(self.floor))
            .add_systems(Startup, spawn_camera)
            .add_systems(PostUpdate, follow_arena);
    }
}

fn spawn_camera(mut commands: Commands, size: Res<ArenaSize>) {
    commands.spawn((
        Camera2d,
        // whatever the window's shape, the arena's height is in view; a wide window shows more
        // floor at the sides, which is where the panels sit
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: view_of(size.0),
            ..OrthographicProjection::default_2d()
        }),
    ));
}

fn view_of(half: f32) -> bevy::camera::ScalingMode {
    // a little floor past the wall, so what stands at the edge is not cut off by the window
    let seen = half + 3.0;
    bevy::camera::ScalingMode::AutoMin { min_width: seen * 2.0, min_height: seen * 2.0 }
}

/// A match that closes the arena in changes [`ArenaSize`]; the view follows it, so the fight
/// fills the window as the field gets smaller. While the editor is open the view slides so the
/// arena sits in the part of the window the editor does not cover.
fn follow_arena(
    size: Res<ArenaSize>,
    view: Res<ArenaView>,
    editor: Option<Res<Editor>>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<Camera2d>>,
) {
    let editor_open = editor.as_ref().is_some_and(|e| e.open);
    let editor_changed = editor.as_ref().is_some_and(|e| e.is_changed());
    if !(size.is_changed() || editor_changed) {
        return;
    }
    // with following off, the view stays framed on the arena as it started
    let half = if view.follow_shrink { size.0 } else { view.framed };
    let aspect = windows.iter().next().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let seen = half + 3.0;
    let visible_width = seen * 2.0 * aspect.max(1.0);
    // the editor is about a third of the window on the right: centre the arena in the rest
    let slide = if editor_open { visible_width * 0.16 } else { 0.0 };
    for (mut projection, mut transform) in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = view_of(half);
        }
        transform.translation.x = slide;
    }
}

/// A directory of `.rb` files, watched. In the browser build there is no directory: `new`
/// answers `None` there, as it does anywhere the platform has no watcher. `changed()` answers the files written since the last
/// call, so a game can restart exactly the scripts that changed.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
pub struct Watch {
    pub dir: PathBuf,
    rx: std::sync::Mutex<std::sync::mpsc::Receiver<PathBuf>>,
    _watcher: Box<dyn notify::Watcher + Send + Sync>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Watch {
    /// Watches `dir` and everything under it. Answers `None` where the platform has no watcher
    /// (the game then simply does not reload).
    pub fn new(dir: impl AsRef<Path>) -> Option<Watch> {
        use notify::{RecursiveMode, Watcher};
        let dir = dir.as_ref().to_path_buf();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(event) = res else { return };
            if !matches!(event.kind, notify::EventKind::Modify(_) | notify::EventKind::Create(_)) {
                return;
            }
            for path in event.paths {
                if path.extension().is_some_and(|e| e == "rb") {
                    let _ = tx.send(path);
                }
            }
        })
        .ok()?;
        watcher.watch(&dir, RecursiveMode::Recursive).ok()?;
        Some(Watch { dir, rx: std::sync::Mutex::new(rx), _watcher: Box::new(watcher) })
    }

    /// The `.rb` files written since the last call, without repeats.
    pub fn changed(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        let Ok(rx) = self.rx.lock() else { return out };
        while let Ok(p) = rx.try_recv() {
            if !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Resource)]
pub struct Watch {
    pub dir: PathBuf,
}

#[cfg(target_arch = "wasm32")]
impl Watch {
    pub fn new(_dir: impl AsRef<Path>) -> Option<Watch> {
        None
    }

    pub fn changed(&self) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// Reads a Ruby file and every file it `require`s from the same directory, in one string, so a
/// game can hand the VM one program per script. (`require` itself works through rubevy's host,
/// but a game that reloads a single file wants the whole of it in hand.)
pub fn read_script(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}
