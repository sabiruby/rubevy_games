//! The 2D floor the games here stand on. Three things, none of them game-specific:
//!
//! * [`ArenaPlugin`] — a 2D camera that shows a square arena whatever the window size is.
//! * [`Hud`] — the line of text at the top, and the per-script panel below it.
//! * [`CodePanel`] — the script a game is showing, with the line it stands on marked.
//! * [`Editor`] — the same, editable, over egui: change a robot's brain without leaving the game.
//! * [`Watch`] — the Ruby directory, watched: saving a file tells the game to start that script
//!   again. Editing a robot's brain and seeing it change without restarting is the point.
//!
//! Bevy's version is pinned once in the workspace; the version-dependent parts of a game live
//! here, so bumping Bevy is one crate's problem rather than every game's.

use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Mutex;

use bevy::prelude::*;

pub mod code;
pub mod editor;
pub mod hud;
pub use code::{CodePanel, CodePanelPlugin};
pub use editor::{Editor, EditorPlugin};
pub use hud::{Hud, HudPlugin, ScriptPanel};

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
}

impl Default for ArenaPlugin {
    fn default() -> Self {
        ArenaPlugin { size: ArenaSize::default(), floor: Color::srgb(0.08, 0.08, 0.10) }
    }
}

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.size)
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
    editor: Option<Res<Editor>>,
    windows: Query<&Window>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<Camera2d>>,
) {
    let editor_open = editor.as_ref().is_some_and(|e| e.open);
    let editor_changed = editor.as_ref().is_some_and(|e| e.is_changed());
    if !(size.is_changed() || editor_changed) {
        return;
    }
    let aspect = windows.iter().next().map(|w| w.width() / w.height().max(1.0)).unwrap_or(16.0 / 9.0);
    let seen = size.0 + 3.0;
    let visible_width = seen * 2.0 * aspect.max(1.0);
    // the editor is about a third of the window on the right: centre the arena in the rest
    let slide = if editor_open { visible_width * 0.16 } else { 0.0 };
    for (mut projection, mut transform) in &mut cameras {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = view_of(size.0);
        }
        transform.translation.x = slide;
    }
}

/// A directory of `.rb` files, watched. `changed()` answers the files written since the last
/// call, so a game can restart exactly the scripts that changed.
#[derive(Resource)]
pub struct Watch {
    pub dir: PathBuf,
    rx: Mutex<std::sync::mpsc::Receiver<PathBuf>>,
    _watcher: Box<dyn notify::Watcher + Send + Sync>,
}

impl Watch {
    /// Watches `dir` and everything under it. Answers `None` where the platform has no watcher
    /// (the game then simply does not reload).
    pub fn new(dir: impl AsRef<Path>) -> Option<Watch> {
        use notify::{RecursiveMode, Watcher};
        let dir = dir.as_ref().to_path_buf();
        let (tx, rx) = channel();
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
        Some(Watch { dir, rx: Mutex::new(rx), _watcher: Box::new(watcher) })
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

/// Reads a Ruby file and every file it `require`s from the same directory, in one string, so a
/// game can hand the VM one program per script. (`require` itself works through rubevy's host,
/// but a game that reloads a single file wants the whole of it in hand.)
pub fn read_script(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}
