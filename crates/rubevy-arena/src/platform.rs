//! **What differs between a PC build and a browser build, once rather than once per game.**
//!
//! Both games here had a `src/platform.rs` holding the same six things twice — where a file is
//! read and written, how Ruby is compiled and coloured, and a clock for the dice — and the
//! garden's copy said so at the top: *"Making it shareable means passing all four in, which is a
//! change to sabibots for no gain while there are two games; when there are three it is worth
//! doing."* (`docs/plans/shared-crate-plan.md`, stage S1; the third game is `factory`.)
//!
//! Four things are passed in, and they are the whole of what a game keeps:
//!
//! * **the `localStorage` prefix** (`garden:`, `sabibots:`) — the browser's half of `read` and
//!   `write`. It is a published page's key: **changing it throws away every save and every
//!   script the people playing it have in their browsers**, so a game's is a constant it does not
//!   edit.
//! * **the built-in `ruby/` table**, which `build.rs` writes into `OUT_DIR` for the browser build
//!   (a page has no directory to read from).
//! * **the names of the page's two bridges**, `window.<game>Compile` and `window.<game>Highlight`.
//!   `#[wasm_bindgen(js_name = …)]` takes a literal, so a shared binding cannot be given a name at
//!   run time; [`js_sys::Reflect::get`] can, and looks the same function up on `window` by the
//!   same name the page defines. No `unsafe` is involved either way.
//! * **the crate's own directory**, for `ruby/` and `assets/` on a PC: `env!("CARGO_MANIFEST_DIR")`
//!   in a shared crate would answer `crates/rubevy-arena`, so it is passed in by the
//!   [`crate_dir!`](crate::crate_dir) macro, which expands where the game is compiled.
//!
//! Everything a game says about *itself* — the name of its save file, the sentence its Save
//! button's tooltip says, the file its checks write — is still in the game's own `platform.rs`.

use std::path::{Path, PathBuf};

pub use imp::*;

/// The `ruby/` files built into the binary: `("ruby/robots/scout.rb", "…")`, as each game's
/// `build.rs` writes them. The browser build reads scripts out of this when the player's
/// `localStorage` has nothing newer; a PC build reads the directory and passes an empty one.
pub type RubyFiles = &'static [(&'static str, &'static str)];

/// **Where the game's own `ruby/` or `assets/` is.** It takes `env!("CARGO_MANIFEST_DIR")` rather
/// than reading it, because the value wanted is the *game's* manifest directory and a shared
/// crate's `env!` would answer its own. [`crate_dir!`](crate::crate_dir) is the one line that
/// passes it, so a game writes `rubevy_arena::crate_dir!("ruby", "garden/ruby")`.
///
/// `from_root` is where to look when the crate's own directory is not there — the workspace root,
/// which is where `cargo run -p garden` starts. In a browser there are no directories at all and
/// the answer is the bare name, which is what the built-in table's paths and `web/build.sh`'s
/// copy of `assets/` both use.
#[macro_export]
macro_rules! crate_dir {
    ($sub:literal, $from_root:literal) => {
        $crate::platform::pick_dir(env!("CARGO_MANIFEST_DIR"), $sub, $from_root)
    };
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// What the Save button says, since what it does differs. Both games say this.
    pub const SAVE_LABEL: &str = "Save to file (Ctrl+S)";

    /// See [`crate_dir!`](crate::crate_dir).
    pub fn pick_dir(manifest_dir: &str, sub: &str, from_root: &str) -> PathBuf {
        let here = Path::new(manifest_dir).join(sub);
        if here.is_dir() { here } else { PathBuf::from(from_root) }
    }

    /// A file. The prefix and the built-in table are the browser's business.
    pub fn read(_store: &str, _built_in: RubyFiles, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn write(_store: &str, path: &Path, text: &str) -> Result<(), String> {
        std::fs::write(path, text).map_err(|e| e.to_string())
    }

    /// Ruby source to RITE bytecode, with line numbers, by the compiler linked in here. The
    /// bridge's name is the browser's business.
    pub fn compile(_bridge: &str, src: &str, name: &str) -> Result<Vec<u8>, String> {
        let opts = sabiruby_compiler::Options {
            filename: name.to_string(),
            debug_info: true,
            ..Default::default()
        };
        sabiruby_compiler::compile(src.as_bytes(), &opts).map_err(|e| format!("{name}: {e}"))
    }

    /// **What kind each byte of the source is** (0..=8), for the editor's colours: Prism's lexer
    /// and a pass over the tree, in the compiler that is already linked in here. It never fails
    /// — a half-typed file classifies as far as it got — so there is no `Result`.
    pub fn highlight(_bridge: &str, src: &str) -> Vec<u8> {
        sabiruby_compiler::highlight(src.as_bytes())
    }

    /// A seed for a world, or a match, that did not name one.
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
    use wasm_bindgen::{JsCast, JsValue};

    pub const SAVE_LABEL: &str = "Save in browser (Ctrl+S)";

    /// There are no directories in a page: the answer is the bare name, which is what the paths
    /// in the built-in table start with and where `web/build.sh` puts `assets/`.
    pub fn pick_dir(_manifest_dir: &str, sub: &str, _from_root: &str) -> PathBuf {
        PathBuf::from(sub)
    }

    fn key(store: &str, path: &Path) -> String {
        format!("{store}{}", path.to_string_lossy())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    /// A script saved in this browser, or else the file as it was built in.
    pub fn read(store: &str, built_in: RubyFiles, path: &Path) -> Result<String, String> {
        if let Some(saved) = storage().and_then(|s| s.get_item(&key(store, path)).ok().flatten()) {
            return Ok(saved);
        }
        let want = path.to_string_lossy();
        built_in
            .iter()
            .find(|(p, _)| *p == want)
            .map(|(_, text)| text.to_string())
            .ok_or_else(|| format!("{want}: not in this build"))
    }

    pub fn write(store: &str, path: &Path, text: &str) -> Result<(), String> {
        let storage = storage().ok_or("this browser keeps no local storage")?;
        storage.set_item(&key(store, path), text).map_err(|e| format!("{e:?}"))
    }

    /// **`window.<name>(source)`, by name.** The page defines `gardenCompile`, `sabibotsCompile`
    /// and their `Highlight` pair (`web/*.html`); a `#[wasm_bindgen(js_name = …)]` binding names
    /// one of them at compile time and so cannot be shared, and `Reflect::get` asks the same
    /// `window` for the same property with the name as a `&str`. It is ordinary JS reflection —
    /// nothing here is `unsafe`.
    ///
    /// A page older than the function it is asked for simply does not define it. The binding this
    /// replaced needed `catch` for that (a missing global is a `ReferenceError` *at the call*);
    /// here the lookup answers `undefined`, which is not a function, and the sentence below is
    /// what the caller gets instead of the browser's.
    fn bridge(name: &str, src: &str) -> Result<js_sys::Uint8Array, String> {
        let window = web_sys::window().ok_or_else(|| "there is no window here".to_string())?;
        let found = js_sys::Reflect::get(&window, &JsValue::from_str(name)).map_err(|e| said(&e))?;
        let f = found
            .dyn_ref::<js_sys::Function>()
            .ok_or_else(|| format!("this page defines no window.{name}"))?;
        let answer = f.call1(&window, &JsValue::from_str(src)).map_err(|e| said(&e))?;
        answer
            .dyn_into::<js_sys::Uint8Array>()
            .map_err(|_| format!("window.{name} answered something that is not bytes"))
    }

    /// What a thrown JS value says: the compiler's message when the page threw a string, and the
    /// value's own description otherwise. This is what the `catch` binding reported.
    fn said(e: &JsValue) -> String {
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    pub fn compile(bridge_name: &str, src: &str, name: &str) -> Result<Vec<u8>, String> {
        bridge(bridge_name, src).map(|bytes| bytes.to_vec()).map_err(|e| format!("{name}: {e}"))
    }

    /// **The editor's colours, from the page's bridge** — and the default kind for every byte if
    /// there is no bridge to ask.
    ///
    /// A game can be opened from a page that is older than this function (the site publishes one
    /// copy of `sabi.js` per game and a browser holds pages in its cache), and a table of the
    /// wrong length is not a table about *this* text. Neither is worth a black screen for: both
    /// answer zeroes, which is the kind the listing painted everything before there were any
    /// colours (`docs/worklog/2026-09-18-web-black-screen.md` is what a panic here costs).
    pub fn highlight(bridge_name: &str, src: &str) -> Vec<u8> {
        match bridge(bridge_name, src) {
            Ok(kinds) if kinds.length() as usize == src.len() => kinds.to_vec(),
            _ => vec![0; src.len()],
        }
    }

    pub fn clock_seed() -> u64 {
        js_sys::Date::now() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The PC half of `read`/`write`, and the one thing `pick_dir` decides there: a directory
    /// that is there is used, and one that is not falls back to the path from the workspace root.
    /// The browser's half is a `localStorage` key, which only a browser can be asked about —
    /// `?selftest` in a page is what checks it.
    #[test]
    fn a_file_is_read_back_and_a_missing_directory_falls_back() {
        let dir = std::env::temp_dir();
        let path = dir.join("rubevy-arena-platform-test.txt");
        let _ = std::fs::remove_file(&path);

        assert!(read("game:", &[], &path).is_err(), "a file that is not there says so");
        write("game:", &path, "two lines\n").unwrap();
        assert_eq!(read("game:", &[], &path).unwrap(), "two lines\n");

        let here = pick_dir(&dir.to_string_lossy(), ".", "from/the/root");
        assert_eq!(here, dir.join("."), "a directory that is there is the answer");
        let gone = pick_dir(&dir.to_string_lossy(), "no-such-directory-here", "from/the/root");
        assert_eq!(gone, PathBuf::from("from/the/root"));

        let _ = std::fs::remove_file(&path);
    }
}
