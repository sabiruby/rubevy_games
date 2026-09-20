//! **egui panels for a game whose Ruby is edited while it runs.** Written for
//! [rubevy](https://github.com/sabiruby/rubevy) — Bevy plus the SabiRuby VM — and for
//! [`bevy_egui`], and for nothing about any particular game: there are no robots, no creatures
//! and no arenas in here.
//!
//! * [`Editor`] — the script a game is showing, editable over egui: change a brain without
//!   leaving the game, in colour where the game hands over a [`Highlighter`]. It does no file
//!   I/O; the buttons become an [`EditorAction`] the game carries out.
//! * [`VmInspector`] — the frames, registers and heap of the selected script, read out of the
//!   VM, and [`VmClock`], which times the tick the scripts run in.
//! * [`CodePanel`] — the same listing without egui, for a game drawing its own UI.
//! * [`Watch`] — a directory of `.rb` files, watched, so that saving one restarts the script it
//!   belongs to. There is no directory in a browser, and there it answers `None`.
//! * [`ViewInsets`] — **how many pixels of each edge of the window a panel covers.** A panel
//!   writes it and a camera reads it; that is the whole of the arrangement, and it is what lets
//!   a camera keep what matters out from under an editor without knowing what an editor is.
//!
//! This crate was cut out of `rubevy-arena` (2026-09-20), which had grown into two things: these
//! panels, and the shell the games in this repository share. The shell is `games-shell`, which
//! depends on this and not the other way round.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

pub mod code;
pub mod editor;
pub mod inspect;
pub use code::{CodePanel, CodePanelPlugin};
pub use editor::{Editor, EditorAction, EditorChoice, EditorPlugin, Highlighter};
pub use inspect::{VmClock, VmInspector, VmInspectorPlugin, Waiting};

/// **How many pixels of each edge of the window something is drawn over**, so that a camera can
/// put what matters in the part nobody is covering.
///
/// A panel writes it — [`EditorPlugin`] writes the band its window covers — and a camera reads
/// it. That is the whole of the arrangement, and it is why neither has to know the other: the
/// arena camera in `games-shell` used to read `Res<Editor>` and slide by a fraction of the window
/// that stood for "the editor is about a third of it", which was a guess about a panel written
/// into a camera.
///
/// The unit is logical pixels, the same ones [`Window::width`] and egui's points are in.
///
/// It lives here, with the panel that writes it, rather than with the cameras that read it: a
/// camera is a reader and there may be several, while what covers the window is the panel's own
/// business and nobody else's.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl ViewInsets {
    /// Nothing covered.
    pub const NONE: ViewInsets = ViewInsets { left: 0.0, right: 0.0, top: 0.0, bottom: 0.0 };

    /// **How far the camera has to move so that what it is aimed at sits in the middle of what is
    /// still visible**, in world units, given how many world units a pixel is worth.
    ///
    /// Half of what is covered, towards the covered side: cover 528 pixels on the right and the
    /// middle of the rest is 264 pixels to the left of the window's middle, so the camera looks
    /// 264 pixels' worth further right. The `y` is the screen's top and bottom turned the world's
    /// way up.
    pub fn shift(&self, world_per_px: f32) -> Vec2 {
        Vec2::new((self.right - self.left) * 0.5, (self.top - self.bottom) * 0.5) * world_per_px
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
