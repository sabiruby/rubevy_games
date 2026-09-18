//! What differs between the PC build and the browser build, and nothing else: where the Ruby and
//! the models come from, how Ruby is compiled, where a saved script goes, and a clock for the
//! dice. The rest of the game is the same code in both.
//!
//! This is sabibots' `src/platform.rs` with the crate's own name in it. It is **not** shared from
//! `rubevy-arena`, and cannot be as it stands: `ruby_dir` and `assets_dir` are
//! `env!("CARGO_MANIFEST_DIR")`, which in a shared crate would answer `crates/rubevy-arena`, the
//! fallback paths and the `localStorage` prefix name the game, and the browser build's compiler is
//! a `#[wasm_bindgen]` binding to a function the page defines under the game's name
//! (`window.sabibotsCompile`). Making it shareable means passing all four in, which is a change to
//! sabibots for no gain while there are two games; when there are three it is worth doing.
//!
//! G0 uses `assets_dir` and `clock_seed`. The rest is here because G1 loads Ruby from `ruby/` the
//! way sabibots does, and a file that is half a platform is worse than one that is whole.
//!
//! G3's save file needed nothing new here beyond a name. `read` and `write` already take a path
//! and already mean "a file" on a PC and "a `localStorage` key" in a browser, and a garden written
//! as JSON is a string like a script is a string — so `save_world` writes with the same two
//! functions the editor saves a creature's file with, and the browser build of it is the PC build.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// What the Save button says, since what it does differs.
    pub const SAVE_LABEL: &str = "Save to file (Ctrl+S)";

    /// Where a saved garden goes (G3): a file in the directory the game was started from, which
    /// is the workspace root when it is `cargo run`. `--save PATH` overrides it.
    pub const SAVE_FILE: &str = "garden.save.json";

    /// What the HUD's two buttons say when the pointer rests on them (G5). The same two buttons
    /// mean two different things, and this is the sentence that says which.
    pub const SAVE_WHERE: &str = "the whole garden, as JSON, in garden.save.json beside the game";

    /// **Where the panel's choices are kept (G6b)**: a small text file beside the save, in the
    /// directory the game was started from. It holds the guide's language and the night's dial —
    /// two lines of `key=value` — and it is written only when one of them is changed with the
    /// mouse. Deleting it puts everything back to its default, which is what a file the player
    /// can see is for.
    pub const SETTINGS_FILE: &str = "garden.settings.txt";

    /// Where the Ruby lives: next to the crate, from the workspace root or from the crate.
    pub fn ruby_dir() -> PathBuf {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("ruby");
        if here.is_dir() { here } else { PathBuf::from("garden/ruby") }
    }

    /// Where the models live. Bevy looks next to the executable by default, which is not where a
    /// workspace puts them. G0 has none — the meshes are Bevy's own primitives — but rubevy reads
    /// a `require` from here, so it still has to point somewhere real.
    pub fn assets_dir() -> String {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let dir = if here.is_dir() { here } else { PathBuf::from("garden/assets") };
        dir.to_string_lossy().into_owned()
    }

    pub fn read(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn write(path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// Ruby source to RITE bytecode, with line numbers.
    pub fn compile(src: &str, name: &str) -> Result<Vec<u8>, String> {
        let opts = sabiruby_compiler::Options { filename: name.to_string(), debug_info: true, ..Default::default() };
        sabiruby_compiler::compile(src.as_bytes(), &opts).map_err(|e| format!("{name}: {e}"))
    }

    /// **What kind each byte of the source is** (0..=8), for the editor's colours: Prism's lexer
    /// and a pass over the tree, in the compiler that is already linked in here. It never fails
    /// — a half-typed file classifies as far as it got — so there is no `Result`.
    pub fn highlight(src: &str) -> Vec<u8> {
        sabiruby_compiler::highlight(src.as_bytes())
    }

    /// Whether the run was asked for the checks (G0's `GARDEN_SELFTEST`).
    ///
    /// An environment variable here, and the page's query string in a browser: the checks are the
    /// only way to see from outside that the world is alive, and a browser has no environment.
    pub fn selftest_asked() -> bool {
        std::env::var("GARDEN_SELFTEST").is_ok()
    }

    /// Whether the window's checks end the run when they are done. On a PC they do: the checks
    /// are asked for on a command line and the shell wants its prompt back.
    pub const CHECKS_EXIT_WHEN_DONE: bool = true;

    /// `GARDEN_RELOAD_AT=SECONDS`, the checks' only way to press F9 without a keyboard.
    ///
    /// A headless run has no `ButtonInput` at all, so the one path a key drives — "read a file
    /// into a garden that is already running" — had no check on it, and the one thing that went
    /// wrong there (G5's finding 1) was found in a browser rather than here. This says when to
    /// open the `--load` file, instead of opening it before the first frame. It is read only when
    /// `GARDEN_SELFTEST` is set, so it is not a switch a player can trip.
    pub fn reload_asked_at() -> Option<f32> {
        std::env::var("GARDEN_RELOAD_AT").ok().and_then(|s| s.parse::<f32>().ok())
    }

    /// Where the tenth check's file goes (G5). A temporary directory on a PC, because the file is
    /// about `--load` and not about where a file lives.
    pub fn another_version_file() -> String {
        std::env::temp_dir().join("garden-from-another-version.json").to_string_lossy().into_owned()
    }

    /// A seed for a world that did not name one.
    pub fn clock_seed() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use wasm_bindgen::prelude::*;

    include!(concat!(env!("OUT_DIR"), "/ruby_files.rs"));

    pub const SAVE_LABEL: &str = "Save in browser (Ctrl+S)";

    /// Where a saved garden goes (G3). There are no files here: `read` and `write` below turn a
    /// path into a `localStorage` key, so this name is the key `garden:garden.save.json` and the
    /// save survives the page being closed, in that browser and nowhere else.
    ///
    /// That is also why the save got a version number in G5: a file a player can see and delete
    /// is one thing, and a string that sits in a browser across every future build of this page is
    /// another. The first garden a new build meets here is usually one the old build wrote.
    pub const SAVE_FILE: &str = "garden.save.json";

    pub const SAVE_WHERE: &str =
        "the whole garden, as JSON, in this browser's local storage (key garden:garden.save.json)";

    /// Where the panel's choices are kept (G6b). There are no files here either: `read` and
    /// `write` below make this the `localStorage` key `garden:garden.settings.txt`, beside the
    /// save's own, so the language the player clicked and the night they dialled in survive the
    /// page being closed — in that browser and nowhere else.
    pub const SETTINGS_FILE: &str = "garden.settings.txt";

    /// Keys of `localStorage` are this followed by the file's path.
    const STORE: &str = "garden:";

    #[wasm_bindgen]
    extern "C" {
        /// `window.gardenCompile(source)`, defined by the page: the RITE bytes, or throws the
        /// compiler's message.
        #[wasm_bindgen(catch, js_name = gardenCompile)]
        fn js_compile(src: &str) -> Result<js_sys::Uint8Array, JsValue>;

        /// `window.gardenHighlight(source)`, defined by the page beside the compiler: one kind
        /// per source byte. `catch` because a page built before H2 does not define it at all,
        /// and a missing global is a `ReferenceError` at the call.
        #[wasm_bindgen(catch, js_name = gardenHighlight)]
        fn js_highlight(src: &str) -> Result<js_sys::Uint8Array, JsValue>;
    }

    /// The files have no directory; their paths start with this, as the built-in table's do.
    pub fn ruby_dir() -> PathBuf {
        PathBuf::from("ruby")
    }

    /// Relative to the page, where `web/build.sh` copies them.
    pub fn assets_dir() -> String {
        "assets".into()
    }

    fn key(path: &Path) -> String {
        format!("{STORE}{}", path.to_string_lossy())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    /// A script saved in this browser, or else the file as it was built in.
    pub fn read(path: &Path) -> Result<String, String> {
        if let Some(saved) = storage().and_then(|s| s.get_item(&key(path)).ok().flatten()) {
            return Ok(saved);
        }
        let want = path.to_string_lossy();
        RUBY_FILES
            .iter()
            .find(|(p, _)| *p == want)
            .map(|(_, text)| text.to_string())
            .ok_or_else(|| format!("{want}: not in this build"))
    }

    pub fn write(path: &Path, text: &str) -> Result<(), String> {
        let storage = storage().ok_or("this browser keeps no local storage")?;
        storage.set_item(&key(path), text).map_err(|e| format!("{e:?}"))
    }

    pub fn compile(src: &str, name: &str) -> Result<Vec<u8>, String> {
        js_compile(src)
            .map(|bytes| bytes.to_vec())
            .map_err(|e| format!("{name}: {}", e.as_string().unwrap_or_else(|| format!("{e:?}"))))
    }

    /// **The editor's colours, from the page's bridge** — and the default kind for every byte if
    /// there is no bridge to ask.
    ///
    /// A game can be opened from a page that is older than this function (the site publishes one
    /// copy of `sabi.js` per game and a browser holds pages in its cache), and a table of the
    /// wrong length is not a table about *this* text. Neither is worth a black screen for: both
    /// answer zeroes, which is the kind the listing painted everything before there were any
    /// colours (`docs/worklog/2026-09-18-web-black-screen.md` is what a panic here costs).
    pub fn highlight(src: &str) -> Vec<u8> {
        match js_highlight(src) {
            Ok(kinds) if kinds.length() as usize == src.len() => kinds.to_vec(),
            _ => vec![0; src.len()],
        }
    }

    /// Whether the page was asked for the checks: `garden/?selftest`.
    ///
    /// A page has no environment to put `GARDEN_SELFTEST` in, and until G5 that meant the browser
    /// build could be watched but not *checked* — the lines that say a creature ate, that the
    /// probe beetle reached its plant, that the sleepers were asleep, are all written by the
    /// checks. The query string is the environment a page has. It is read once, at startup, by the
    /// same `main` that reads the variable on a PC; everything after it is the same code.
    pub fn selftest_asked() -> bool {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|q| q.contains("selftest"))
    }

    /// **A page has nothing to exit to**, and `AppExit` in a browser is not "the run ended", it is
    /// "this canvas stops". winit's wasm event loop stops being pumped, every system stops running,
    /// and the last frame drawn stays on the screen looking like a garden — so the page goes on
    /// *looking* alive while no key, no click and no creature does anything ever again. That is
    /// G5's second finding (`docs/web.md`, "the page stops answering the mouse and the keyboard"):
    /// not egui holding the keyboard and not the DOM, but the checks ending the app under the
    /// player's feet. So the checks do not exit here; they say they are done and the garden goes on.
    pub const CHECKS_EXIT_WHEN_DONE: bool = false;

    /// A page has no environment and no `--load`: the browser's checks press F9 for real
    /// (`web/garden.html` sends the key), so there is nothing to defer here.
    pub fn reload_asked_at() -> Option<f32> {
        None
    }

    /// Where the tenth check's file goes (G5). There is no temporary directory here: it is a
    /// `localStorage` key beside the save's own, `garden:garden.from-another-version.json`, which
    /// clears with the rest of the site's storage. A page asked for the checks is asked for this.
    pub fn another_version_file() -> String {
        "garden.from-another-version.json".into()
    }

    pub fn clock_seed() -> u64 {
        js_sys::Date::now() as u64
    }
}
