//! What differs between the PC build and the browser build — and, since S1, nothing of *how* it
//! differs: both halves are `games_shell::platform` and `games_shell::checks`, and what is left
//! here is **this game's own names**.
//!
//! * PC (`cargo run -p factory`): the files under `ruby/`, read and written in place; the
//!   reference compiler linked in; `factory.settings.txt` beside the game.
//! * Browser (`web/build.sh factory`): the `ruby/` files built into the binary by `build.rs`, a
//!   save goes to `localStorage` under the prefix below, and Ruby is compiled by the playground's
//!   compiler module that the page loads beside the game.
//!
//! The four names here are the ones a published page would carry for ever, so they are chosen
//! once: the `localStorage` prefix, the settings file, the two bridges the page hangs on
//! `window`, and the name the checks are asked for by.

use std::path::{Path, PathBuf};

use games_shell::platform;

// **The `ruby/` files, built into the browser's binary** by `build.rs`: a page has no directory
// to read from.
#[cfg(target_arch = "wasm32")]
include!(concat!(env!("OUT_DIR"), "/ruby_files.rs"));

/// The PC build reads `ruby/` itself, so there is nothing built in.
#[cfg(not(target_arch = "wasm32"))]
pub static RUBY_FILES: platform::RubyFiles = &[];

/// **Keys of `localStorage` are this followed by the file's path.** It is not to be changed once
/// the page is published: a player's edited scripts are kept under it, in their own browser, and
/// a different prefix loses every one of them.
const STORE: &str = "factory:";

/// The page's two bridges, by the names the page defines them under — `window.factoryCompile` and
/// `window.factoryHighlight`. They are this game's word out of `web/games.sh` filled into
/// `web/page.html.in`, and they are looked up by name rather than bound at compile time, which is
/// what lets one shared binding serve every game.
const COMPILE_BRIDGE: &str = "factoryCompile";
const HIGHLIGHT_BRIDGE: &str = "factoryHighlight";

// `games_shell::platform` also has what the Save button says (`SAVE_LABEL`) and a clock for a
// seed that works in a page as well (`clock_seed`, which is why no `std::time` goes near the
// browser's road). Neither is re-exported here yet: F5 is the first stage with a Save button,
// and a `pub use` nothing calls is a warning.

/// Whether the checks end the run when they are done — `true` on a PC, `false` in a page, which
/// has nothing to exit to.
pub use games_shell::checks::CHECKS_EXIT_WHEN_DONE;

/// **Where the settings live**: a small `key=value` text file in the directory the game was
/// started from, and in a browser the `localStorage` key `factory:factory.settings.txt`.
/// Deleting it puts every number back to its default.
pub const SETTINGS_FILE: &str = "factory.settings.txt";

/// Where the Ruby lives: next to the crate, from the workspace root or from the crate — and in a
/// browser the bare name the built-in table's paths start with.
pub fn ruby_dir() -> PathBuf {
    games_shell::crate_dir!("ruby", "factory/ruby")
}

/// Where the tiles live. Bevy looks next to the executable by default, which is not where a
/// workspace puts them; in a browser it is where `web/build.sh` copies them, beside the page.
pub fn assets_dir() -> String {
    games_shell::crate_dir!("assets", "factory/assets").to_string_lossy().into_owned()
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn read(path: &Path) -> Result<String, String> {
    platform::read(STORE, RUBY_FILES, path)
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn write(path: &Path, text: &str) -> Result<(), String> {
    platform::write(STORE, path, text)
}

/// Ruby source to RITE bytecode, with line numbers: the compiler linked in on a PC, the page's
/// `window.factoryCompile` in a browser. **Not called yet** — F2's data stage is the first thing
/// here with a script to compile — but the bridge's name has to be the same word the page uses,
/// and that word is settled here.
#[allow(dead_code)]
pub fn compile(src: &str, name: &str) -> Result<Vec<u8>, String> {
    platform::compile(COMPILE_BRIDGE, src, name)
}

/// **What kind each byte of the source is** (0..=8), for the editor's colours. Not called until
/// F5 opens an editor, for the same reason as `compile`.
#[allow(dead_code)]
pub fn highlight(src: &str) -> Vec<u8> {
    platform::highlight(HIGHLIGHT_BRIDGE, src)
}

/// Whether the run was asked for the checks: `FACTORY_SELFTEST` on a PC, `factory/?selftest` in a
/// page. A knob the checks take is `FACTORY_<NAME>` in a shell and `?selftest&<name>=…` in an
/// address (`games_shell::checks`).
pub fn selftest_asked() -> bool {
    games_shell::checks::selftest_asked("FACTORY_SELFTEST")
}
