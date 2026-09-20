//! **What the player chose, kept between runs (G6b).**
//!
//! Two of the author's browser findings end in the same place: a number or a word the player
//! picked with the mouse has to be there again the next time the page is opened. The guide's
//! language is one (`English | 日本語`), the night's brightness dial is the other — and the dial
//! is not really a preference at all, it is a **question being asked of the author**: turn it
//! until the night reads, tell us the number, and it is baked into the constants. A dial that
//! forgets its number between reloads cannot ask that question.
//!
//! There is no settings library here and there does not need to be one. The store is a handful
//! of `key=value` lines:
//!
//! ```text
//! # garden — what the panel remembers. Delete a line to go back to the default.
//! night=1.30
//! lang=ja
//! ```
//!
//! and **where those lines live is the game's `platform.rs`, not this file**. Both games already
//! have a `read`/`write` pair that means "a file beside the game" on a PC and "a key in this
//! browser's `localStorage`" in a page — it is what the save file and the edited scripts go
//! through — so the store is handed the two functions and asks no questions about them. That is
//! the whole reason this is `fn` pointers rather than a trait: `platform::read` and
//! `platform::write` are plain functions in both games and in both builds.
//!
//! Text rather than JSON because sabibots has no JSON parser and should not grow one to remember
//! a two-letter language tag; the garden's `serde_json` is there for the save file, which is a
//! document, and this is two lines.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

/// A game's `platform::read`: the text at a path, or why not.
pub type ReadFn = fn(&Path) -> Result<String, String>;
/// A game's `platform::write`.
pub type WriteFn = fn(&Path, &str) -> Result<(), String>;

/// The `key=value` store, read once at startup and written whenever a value changes.
///
/// Insert one before the first frame; the panels take it as an `Option`, so a build without one
/// (the headless run, which has no panels to choose anything with) simply keeps its defaults.
#[derive(Resource)]
pub struct Settings {
    path: PathBuf,
    read: ReadFn,
    write: WriteFn,
    /// in the order they were read or set, so the file does not shuffle itself between runs
    pairs: Vec<(String, String)>,
    /// the line at the head of the file, so whoever opens it knows what it is
    header: String,
}

impl Settings {
    /// Reads the store. A store that is not there yet is an empty one, not an error: the first
    /// run of a new build has nothing to remember, and a browser that keeps no local storage
    /// (a private window with site data blocked) is the same case.
    pub fn load(path: impl Into<PathBuf>, header: impl Into<String>, read: ReadFn, write: WriteFn) -> Settings {
        let path = path.into();
        let mut settings =
            Settings { path, read, write, pairs: Vec::new(), header: header.into() };
        let text = match (settings.read)(&settings.path) {
            Ok(text) => text,
            Err(_) => return settings,
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                settings.pairs.push((key.trim().to_owned(), value.trim().to_owned()));
            }
        }
        settings
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    /// A stored number, if it is one.
    pub fn number(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(|v| v.parse::<f32>().ok())
    }

    /// Remembers a value and writes the store out.
    ///
    /// **The value goes to the log as well**, and that is not debugging output: the dial exists so
    /// that the author can say "1.4" and have it become the constant, and in a browser the log is
    /// the console — the one place a number chosen with the mouse can be read off. It is quiet
    /// when nothing changed, so holding a slider still does not fill the console.
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if self.get(key) == Some(value.as_str()) {
            return;
        }
        match self.pairs.iter_mut().find(|(k, _)| k == key) {
            Some(slot) => slot.1 = value.clone(),
            None => self.pairs.push((key.to_owned(), value.clone())),
        }
        info!("setting: {key} = {value}  (remembered in {})", self.path.display());
        let mut text = String::new();
        if !self.header.is_empty() {
            text.push_str("# ");
            text.push_str(&self.header);
            text.push('\n');
        }
        for (k, v) in &self.pairs {
            text.push_str(k);
            text.push('=');
            text.push_str(v);
            text.push('\n');
        }
        if let Err(e) = (self.write)(&self.path, &text) {
            warn!("could not remember {key}: {e}");
        }
    }
}

/// **What the player chose last time, read before the first frame** — the store and the language
/// in it, which is the ten lines both games open a window with.
///
/// It is read here rather than in a system because the language is wanted *before the first
/// frame*: the guide opens by itself at startup and would otherwise show one language and then
/// jump to the other. `lang_asked` is `--lang` from the command line, which wins over the store
/// and is not written back to it — a picture asked for in Japanese should not change what the
/// next run shows a person ([`GuideLang::pick`](crate::guide::GuideLang::pick)).
///
/// The `"lang"` key is this crate's: the guide's buttons write it (`guide.rs`), and it is the
/// name it has in every `*.settings.txt` already out there.
pub fn remembered(
    path: impl Into<PathBuf>,
    header: impl Into<String>,
    read: ReadFn,
    write: WriteFn,
    lang_asked: Option<&str>,
) -> (Settings, crate::guide::GuideLang) {
    let settings = Settings::load(path, header, read, write);
    let lang = crate::guide::GuideLang::pick(lang_asked, settings.get("lang"));
    (settings, lang)
}

/// **The panels, as the player left them** (S5b-1).
///
/// Every panel in `rubevy-egui` and in this crate keeps its numbers in a resource of its own now,
/// and the ones a person can sensibly be asked about can come out of the store: how big the
/// editor is, how big its letters are, how big the VM panel is and how deep it looks, the HUD's
/// text, the guide's window. Each of those resources knows its own keys; this plugin is only the
/// wiring, and it is a plugin rather than ten lines in each game because the ten lines would be
/// the same ten lines twice and wrong in the third game.
///
/// It reads once, in `PreStartup`, which is before anything has been drawn with the defaults —
/// the same place [`CameraPlugin`](crate::CameraPlugin) reads its own `camera_*` keys, and it
/// leaves those to it. A game without a store, or a panel a game does not use, is simply skipped:
/// every one of them is an `Option`.
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use games_shell::settings::PanelSettingsPlugin;
/// # let mut app = App::new();
/// app.add_plugins(PanelSettingsPlugin);
/// ```
pub struct PanelSettingsPlugin;

impl Plugin for PanelSettingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, read_panels);
    }
}

fn read_panels(
    settings: Option<Res<Settings>>,
    editor: Option<ResMut<rubevy_egui::EditorLayout>>,
    inspector: Option<ResMut<rubevy_egui::VmInspector>>,
    clock: Option<ResMut<rubevy_egui::VmClock>>,
    code: Option<ResMut<rubevy_egui::CodeStyle>>,
    hud: Option<ResMut<crate::hud::HudStyle>>,
    guide: Option<ResMut<crate::guide::GuideStyle>>,
) {
    let Some(settings) = settings else { return };
    let number = |key: &str| settings.number(key);
    if let Some(mut editor) = editor {
        editor.read_from(number);
    }
    if let Some(mut inspector) = inspector {
        inspector.style.read_from(number);
    }
    if let Some(mut clock) = clock
        && let Some(value) = settings.number("vm_clock_smoothing")
    {
        clock.smoothing = value;
    }
    if let Some(mut code) = code {
        code.read_from(number);
    }
    if let Some(mut hud) = hud {
        hud.read_from(&settings);
    }
    if let Some(mut guide) = guide {
        guide.read_from(&settings);
    }
}

/// The language the machine is set to, as a tag like `ja_JP.UTF-8` or `ja-JP`, if it says.
///
/// Two environments, one question. On a PC it is `LC_ALL` then `LANG`, which is where a Unix
/// keeps it; in a page it is `navigator.language`, which is what the browser was asked for. Only
/// the front of the tag is ever looked at ([`crate::guide::GuideLang::of_environment`]), so the
/// difference in shape between the two does not matter.
#[cfg(not(target_arch = "wasm32"))]
pub fn environment_language() -> Option<String> {
    std::env::var("LC_ALL").ok().filter(|s| !s.is_empty()).or_else(|| std::env::var("LANG").ok())
}

#[cfg(target_arch = "wasm32")]
pub fn environment_language() -> Option<String> {
    web_sys::window().and_then(|w| w.navigator().language())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }
    fn write(path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// The whole of the store's job, on a PC: a file that is not there yet, a value set, the same
    /// file read back by the next run. The browser's half of it is the same two functions with a
    /// `localStorage` key behind them, which only a browser can be asked about — G6b drove that
    /// one in headless Chromium instead (`docs/worklog/2026-09-17-garden-G6b.md`).
    #[test]
    fn remembers_across_a_run() {
        let path = std::env::temp_dir().join("games-shell-settings-test.txt");
        let _ = std::fs::remove_file(&path);

        let mut first = Settings::load(&path, "a test", read, write);
        assert_eq!(first.get("lang"), None, "a store that is not there yet is an empty one");
        assert_eq!(first.number("night"), None);
        first.set("lang", "ja");
        first.set("night", "1.30");
        // setting what is already there writes nothing and says nothing
        first.set("lang", "ja");

        let second = Settings::load(&path, "a test", read, write);
        assert_eq!(second.get("lang"), Some("ja"));
        assert_eq!(second.number("night"), Some(1.30));
        assert!(read(&path).unwrap().starts_with("# a test\n"), "the file says what it is");

        let _ = std::fs::remove_file(&path);
    }

    /// **The wiring, end to end** (S5b-1): a store with one line about the editor and one about
    /// the VM panel, and the two resources those panels are drawn from carrying it before the
    /// first frame. What each key means is the panel's own business and tested there; what is
    /// tested here is that a line in a file reaches it at all, and that the panels a game does
    /// not have are not a problem.
    #[test]
    fn the_store_reaches_the_panels_before_the_first_frame() {
        let path = std::env::temp_dir().join("games-shell-panels-test.txt");
        let _ = std::fs::remove_file(&path);
        let mut settings = Settings::load(&path, "a test", read, write);
        settings.set("editor_width", "700");
        settings.set("vm_regs_frames", "3");

        let mut app = App::new();
        app.add_plugins(PanelSettingsPlugin)
            .insert_resource(settings)
            // the two panels this app has; there is no HUD, no guide and no code panel, and
            // their absence is what the `Option`s are for
            .init_resource::<rubevy_egui::EditorLayout>()
            .init_resource::<rubevy_egui::VmInspector>();
        app.update();

        assert_eq!(app.world().resource::<rubevy_egui::EditorLayout>().width, 700.0);
        assert_eq!(app.world().resource::<rubevy_egui::VmInspector>().style.regs_frames, 3);
        // and what nobody wrote about is what it always was
        assert_eq!(
            app.world().resource::<rubevy_egui::EditorLayout>().height,
            rubevy_egui::editor::HEIGHT
        );

        let _ = std::fs::remove_file(&path);
    }
}
