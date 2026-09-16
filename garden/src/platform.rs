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
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub use imp::*;

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;

    /// What the Save button says, since what it does differs.
    pub const SAVE_LABEL: &str = "Save to file (Ctrl+S)";

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

    /// Keys of `localStorage` are this followed by the file's path.
    const STORE: &str = "garden:";

    #[wasm_bindgen]
    extern "C" {
        /// `window.gardenCompile(source)`, defined by the page: the RITE bytes, or throws the
        /// compiler's message.
        #[wasm_bindgen(catch, js_name = gardenCompile)]
        fn js_compile(src: &str) -> Result<js_sys::Uint8Array, JsValue>;
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

    pub fn clock_seed() -> u64 {
        js_sys::Date::now() as u64
    }
}
