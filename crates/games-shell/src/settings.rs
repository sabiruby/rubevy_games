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
use std::sync::Mutex;

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
    /// **What the store asked for and did not get** (S11), in the order it was asked for, each
    /// one once. See [`Settings::refused`] for why it is kept rather than said on the spot, and
    /// why a `Mutex` rather than a `&mut self` on every reader.
    refusals: Mutex<Vec<String>>,
}

impl Settings {
    /// Reads the store. A store that is not there yet is an empty one, not an error: the first
    /// run of a new build has nothing to remember, and a browser that keeps no local storage
    /// (a private window with site data blocked) is the same case.
    pub fn load(path: impl Into<PathBuf>, header: impl Into<String>, read: ReadFn, write: WriteFn) -> Settings {
        let path = path.into();
        let mut settings = Settings {
            path,
            read,
            write,
            pairs: Vec::new(),
            header: header.into(),
            refusals: Mutex::new(Vec::new()),
        };
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

    /// A stored number, if it is one — and **if a line of the file says something that is not a
    /// number at all, that is refused rather than passed over** (S11). A key nobody wrote is not
    /// a refusal; a key somebody wrote `fast` beside is.
    pub fn number(&self, key: &str) -> Option<f32> {
        let text = self.get(key)?;
        match text.parse::<f32>() {
            Ok(n) if n.is_finite() => Some(n),
            _ => {
                self.refuse(format!("{key} = {text} is not a number; the default stands"));
                None
            }
        }
    }

    /// **A stored number that has to be more than zero** (S11, out of Factory's F1), and the
    /// reason there is no floor inside the arithmetic that uses one.
    ///
    /// A belt speed of zero is not a slow belt, it is a division; a gap of zero is not a crowded
    /// tile, it is every item in the same place; a frame that buys no instructions at all is a
    /// check that waits four thousand million frames. There is no sensible number to clamp such a
    /// setting to — **the smallest belt speed that still means anything is not something anybody
    /// measured**, and a floor invented on the spot is a number with nowhere to have come from
    /// (`/home/kishima/book/CLAUDE.md`). So the store is told it is wrong and the default stands,
    /// which is a number that does have somewhere to have come from.
    ///
    /// Three games had grown the same shape: `.max(0.001)` and `.max(1.0)` and `.max(3.0)` inside
    /// the line that read the value, quietly turning a nought in a file into a number nobody
    /// chose. F1 wrote this out in Factory and said the other two would want it.
    pub fn positive(&self, key: &str, default: f32) -> f32 {
        match self.number(key) {
            Some(n) if n > 0.0 => n,
            Some(wrong) => {
                self.refuse(format!("{key} = {wrong} is not more than zero; using {default}"));
                default
            }
            None => default,
        }
    }

    /// **A stored number of things**: whole, and no fewer than `least` of them.
    ///
    /// `least` is the caller's because the reason is the caller's — a polygon of sky needs three
    /// sides, a shadow needs one cascade, and a run that may draw no trees at all takes a nought.
    /// A floor that cannot say why it is where it is does not belong in a store's reader any more
    /// than it belongs in the arithmetic.
    pub fn counted(&self, key: &str, least: u64, default: u64) -> u64 {
        let Some(asked) = self.number(key) else { return default };
        // **the floor first, because it is also what catches a negative**: `as u64` saturates, so
        // a minus one would otherwise arrive here as a nought and be called "not whole"
        if asked < least as f32 {
            self.refuse(format!("{key} = {asked} is not a count of {least} or more; using {default}"));
            return default;
        }
        let whole = asked as u64;
        if whole as f32 != asked {
            self.refuse(format!("{key} = {asked} is not a whole number of things; using {default}"));
            return default;
        }
        whole
    }

    /// **What the store asked for and did not get**, in the order it was asked for.
    ///
    /// It is kept rather than said on the spot because of where it is read: all three games read
    /// their settings in `main`, **before `App::run`**, and a `warn!` there goes nowhere at all —
    /// `LogPlugin` is inside `DefaultPlugins`, so until the app is built there is no subscriber
    /// and the line is dropped. F3a wrote a warning about a moved key in exactly that place and
    /// found out by running the game and seeing nothing
    /// (`docs/worklog/2026-09-21-factory-F3a.md`). So the refusals are collected and
    /// [`SettingsRefusalsPlugin`] says them in `Startup`, where there is a log.
    ///
    /// The list is behind a `Mutex` rather than the readers taking `&mut self` because of who
    /// reads: the panels are handed a plain `impl Fn(&str) -> Option<f32>` closure over a
    /// `Res<Settings>` (S5b-1), and every `read_from` in three games takes a `&Settings`. Making
    /// all of them mutable to record a refusal would be a wide change to say a narrow thing. The
    /// lock is taken at startup and never in a frame.
    pub fn refused(&self) -> Vec<String> {
        self.refusals.lock().map(|list| list.clone()).unwrap_or_default()
    }

    /// Keeps one refusal, once: a key read twice — `CheckPace::of` is asked for by two games'
    /// `main` and by a check — must not say the same sentence twice.
    fn refuse(&self, said: String) {
        if let Ok(mut list) = self.refusals.lock()
            && !list.contains(&said)
        {
            list.push(said);
        }
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

/// **What the store asked for and did not get, said where there is a log to say it in** (S11).
///
/// One system, in `Startup`, printing one `warn!` per refusal — the settings themselves are read
/// in `main`, which is before `LogPlugin` exists and therefore before anything said can be heard
/// ([`Settings::refused`]). A run with a store that is all right says nothing.
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use games_shell::settings::SettingsRefusalsPlugin;
/// # let mut app = App::new();
/// app.add_plugins(SettingsRefusalsPlugin);
/// ```
pub struct SettingsRefusalsPlugin;

impl Plugin for SettingsRefusalsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, say_what_was_refused);
    }
}

fn say_what_was_refused(settings: Option<Res<Settings>>) {
    let Some(settings) = settings else { return };
    for said in settings.refused() {
        warn!("setting refused: {said}");
    }
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

    /// **What a store says that the game cannot do** (S11): every shape of it, and that the
    /// default stands and the run is told. The three games used to write `.max(0.001)` into the
    /// line that read the value, which turned a nought in a file into a number nobody chose and
    /// said nothing about it.
    #[test]
    fn a_store_is_refused_rather_than_rounded_up() {
        let path = std::env::temp_dir().join("games-shell-refusals-test.txt");
        let _ = std::fs::remove_file(&path);
        let mut settings = Settings::load(&path, "a test", read, write);
        settings.set("belt_speed", "0");
        settings.set("gap", "-2.5");
        settings.set("speed", "fast");
        settings.set("sides", "2");
        settings.set("trees", "1.5");
        settings.set("rocks", "-1");
        settings.set("good_speed", "3.5");
        settings.set("good_count", "7");

        // more than zero, or the default
        assert_eq!(settings.positive("belt_speed", 8.0), 8.0);
        assert_eq!(settings.positive("gap", 8.0), 8.0);
        assert_eq!(settings.positive("speed", 8.0), 8.0);
        assert_eq!(settings.positive("good_speed", 8.0), 3.5);
        assert_eq!(settings.positive("nobody_wrote_this", 8.0), 8.0);

        // whole, and no fewer than the caller's `least` — which is the caller's because the
        // reason is: a dome wants three sides and a garden may have no trees at all
        assert_eq!(settings.counted("sides", 3, 24), 24);
        assert_eq!(settings.counted("trees", 0, 5), 5, "a tree and a half is not a tree");
        assert_eq!(settings.counted("rocks", 0, 5), 5, "and minus one rock is not none");
        assert_eq!(settings.counted("good_count", 1, 5), 7);
        assert_eq!(settings.counted("nobody_wrote_this", 1, 5), 5);
        // a nought where a nought is allowed is not a refusal
        settings.set("trees", "0");
        assert_eq!(settings.counted("trees", 0, 5), 0);

        // **and every one of them is remembered to be said where there is a log** — `main` has
        // none (F3a), which is the whole reason this list exists
        let said = settings.refused();
        assert_eq!(said.len(), 6, "one line each, and no line for what was accepted: {said:?}");
        assert!(said[0].starts_with("belt_speed = 0 is not more than zero; using 8"));
        assert!(said.iter().any(|s| s == "speed = fast is not a number; the default stands"));
        assert!(said.iter().any(|s| s == "sides = 2 is not a count of 3 or more; using 24"));
        assert!(said.iter().any(|s| s == "rocks = -1 is not a count of 0 or more; using 5"));
        assert!(said.iter().any(|s| s.contains("1.5 is not a whole number of things")));

        // a key read twice is one refusal, not two: `CheckPace` is asked for by `main` and by a
        // check, and a list that grew every time would say the same sentence over and over
        settings.positive("belt_speed", 8.0);
        assert_eq!(settings.refused().len(), 6);

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
