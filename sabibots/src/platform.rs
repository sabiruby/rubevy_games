//! What differs between the PC build and the browser build, and nothing else: where the Ruby and
//! the sprites come from, how Ruby is compiled, where a saved brain goes, and a clock for the
//! dice. The rest of the game is the same code in both.
//!
//! * PC (`cargo run -p sabibots`): the files under `ruby/`, read and written in place and
//!   watched for edits; the reference compiler linked in.
//! * Browser (`web/build.sh`): the `ruby/` files built into the binary, a save goes to the
//!   browser's `localStorage` and wins over the built-in file from then on; Ruby is compiled by
//!   the playground's compiler module, which the page loads beside the game (`web/index.html`).
//!
//! And **how the checks are asked for** (2026-09-18): `SABIBOTS_SELFTEST` on a PC, `?selftest` in
//! the page's address. That pair was the garden's from G5 and is this game's now, with the whole
//! difference between the two builds in the two functions at the foot of each half.

use std::path::{Path, PathBuf};

pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// What the Save button says, since what it does differs.
    pub const SAVE_LABEL: &str = "Save to file (Ctrl+S)";

    /// **Where the panel's choices are kept (G6b)**: a small text file in the directory the game
    /// was started from, holding the one thing the Battle remembers — which language the guide
    /// opens in. Deleting it puts that back to the machine's own language.
    pub const SETTINGS_FILE: &str = "sabibots.settings.txt";

    /// Where the Ruby lives: next to the crate, from the workspace root or from the crate.
    pub fn ruby_dir() -> PathBuf {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("ruby");
        if here.is_dir() { here } else { PathBuf::from("sabibots/ruby") }
    }

    /// Where the sprites live. Bevy looks next to the executable by default, which is not where
    /// a workspace puts them.
    pub fn assets_dir() -> String {
        let here = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let dir = if here.is_dir() { here } else { PathBuf::from("sabibots/assets") };
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

    /// Whether the run was asked for the checks.
    ///
    /// An environment variable here, and the page's query string in a browser (2026-09-18): the
    /// checks are the only way to see from outside that the editor's buttons still do what they
    /// say, and a browser has no environment. It is the garden's `selftest_asked` with this
    /// game's name in it, for the reason the head of this file gives.
    pub fn selftest_asked() -> bool {
        std::env::var("SABIBOTS_SELFTEST").is_ok()
    }

    /// Whether the checks end the run when they are done. On a PC they do: they were asked for on
    /// a command line and the shell wants its prompt back.
    pub const CHECKS_EXIT_WHEN_DONE: bool = true;

    /// A seed for a match that did not name one.
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

    /// Where the guide's language is remembered (G6b): the `localStorage` key
    /// `sabibots:sabibots.settings.txt`, beside the scripts this page has saved.
    pub const SETTINGS_FILE: &str = "sabibots.settings.txt";

    /// Keys of `localStorage` are this followed by the file's path.
    const STORE: &str = "sabibots:";

    #[wasm_bindgen]
    extern "C" {
        /// `window.sabibotsCompile(source)`, defined by the page: the RITE bytes, or throws the
        /// compiler's message.
        #[wasm_bindgen(catch, js_name = sabibotsCompile)]
        fn js_compile(src: &str) -> Result<js_sys::Uint8Array, JsValue>;

        /// `window.sabibotsHighlight(source)`, defined by the page beside the compiler: one kind
        /// per source byte. `catch` because a page built before H2 does not define it at all,
        /// and a missing global is a `ReferenceError` at the call.
        #[wasm_bindgen(catch, js_name = sabibotsHighlight)]
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

    /// A brain saved in this browser, or else the file as it was built in.
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

    /// Whether the page was asked for the checks: `sabibots/?selftest` (2026-09-18).
    ///
    /// The garden grew this in G5 and the Battle did not, so until today the browser build of
    /// this game could be watched but not *checked* — the lines that say Apply reached one robot
    /// and not its twin, that `P` stopped the match's clock as well as the VM, that a `def` in
    /// the listing is painted in the keyword colour, are all written by the checks and there was
    /// no way to ask for them here. The query string is the environment a page has. It is read
    /// once, at startup, by the same `main` that reads the variable on a PC; everything after it
    /// is the same code.
    pub fn selftest_asked() -> bool {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|q| q.contains("selftest"))
    }

    /// **A page has nothing to exit to** (the garden's G5 finding, `docs/web.md`): `AppExit` in a
    /// browser is not "the run ended", it is "this canvas stops" — winit's wasm event loop stops
    /// being pumped, every system stops running, and the last frame drawn stays on the screen
    /// looking like an arena while no key, no click and no robot does anything ever again. So the
    /// checks do not exit here; they say they are done and the match goes on.
    pub const CHECKS_EXIT_WHEN_DONE: bool = false;

    pub fn clock_seed() -> u64 {
        js_sys::Date::now() as u64
    }
}
