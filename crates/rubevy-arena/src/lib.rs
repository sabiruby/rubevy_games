//! The 2D floor the games here stand on. Three things, none of them game-specific:
//!
//! * [`ArenaPlugin`] — a 2D camera that shows a square arena whatever the window size is.
//! * [`Hud`] — the line of text at the top, and the per-script panel below it.
//! * [`CodePanel`] — the script a game is showing, with the line it stands on marked.
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
pub mod hud;
pub use code::{CodePanel, CodePanelPlugin};
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
            .add_systems(Startup, spawn_camera);
    }
}

fn spawn_camera(mut commands: Commands, size: Res<ArenaSize>) {
    commands.spawn((
        Camera2d,
        // the arena is square: whatever the window is, its short side shows exactly the arena
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: size.0 * 2.0,
                min_height: size.0 * 2.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
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
