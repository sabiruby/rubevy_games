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
//! the page's address.
//!
//! **Since S1 both halves of all of that are `games_shell::platform` and `games_shell::checks`,
//! and what is left here is this game's own names**: the `localStorage` prefix, the `ruby/` files
//! the browser's binary carries, the names the page gives its two bridges, and wrappers thin
//! enough that `platform::read` still means what it meant in every caller.

use std::path::{Path, PathBuf};

use games_shell::platform;

// **The `ruby/` files, built into the browser's binary** by `build.rs`: a page has no directory
// to read from.
#[cfg(target_arch = "wasm32")]
include!(concat!(env!("OUT_DIR"), "/ruby_files.rs"));

/// The PC build reads `ruby/` itself, so there is nothing built in — which is what it was before
/// S1, when the `include!` above was inside the browser's half of this file and the PC's half had
/// no table at all.
#[cfg(not(target_arch = "wasm32"))]
pub static RUBY_FILES: platform::RubyFiles = &[];

/// **Keys of `localStorage` are this followed by the file's path.** It is not to be changed: a
/// published page's edited brains are kept under it, in the browsers of the people who have
/// played it, and a different prefix loses every one of them.
const STORE: &str = "sabibots:";

/// The page's two bridges, by the names the page defines them under — `window.sabibotsCompile`
/// and `window.sabibotsHighlight`, which are this game's word out of `web/games.sh` filled into
/// `web/page.html.in`. They are looked up by name rather than bound at compile time, which is
/// what lets one shared binding serve every game.
const COMPILE_BRIDGE: &str = "sabibotsCompile";
const HIGHLIGHT_BRIDGE: &str = "sabibotsHighlight";

/// What the Save button says, since what it does differs; and a clock for the dice.
pub use games_shell::platform::{clock_seed, SAVE_LABEL};
/// Whether the checks end the run when they are done — `true` on a PC, `false` in a page, which
/// has nothing to exit to (`docs/web.md`, and the note on the constant).
pub use games_shell::checks::CHECKS_EXIT_WHEN_DONE;

/// **Where the panel's choices are kept (G6b)**: a small text file in the directory the game was
/// started from, holding the one thing the Battle remembers — which language the guide opens in.
/// Deleting it puts that back to the machine's own language. In a browser it is the
/// `localStorage` key `sabibots:sabibots.settings.txt`, beside the scripts this page has saved.
pub const SETTINGS_FILE: &str = "sabibots.settings.txt";

/// Where the Ruby lives: next to the crate, from the workspace root or from the crate — and in a
/// browser the bare name the built-in table's paths start with.
pub fn ruby_dir() -> PathBuf {
    games_shell::crate_dir!("ruby", "sabibots/ruby")
}

/// Where the sprites live. Bevy looks next to the executable by default, which is not where a
/// workspace puts them; in a browser it is where `web/build.sh` copies them, beside the page.
pub fn assets_dir() -> String {
    games_shell::crate_dir!("assets", "sabibots/assets").to_string_lossy().into_owned()
}

pub fn read(path: &Path) -> Result<String, String> {
    platform::read(STORE, RUBY_FILES, path)
}

pub fn write(path: &Path, text: &str) -> Result<(), String> {
    platform::write(STORE, path, text)
}

/// Ruby source to RITE bytecode, with line numbers: the compiler linked in on a PC, the page's
/// `window.sabibotsCompile` in a browser.
pub fn compile(src: &str, name: &str) -> Result<Vec<u8>, String> {
    platform::compile(COMPILE_BRIDGE, src, name)
}

/// **What kind each byte of the source is** (0..=8), for the editor's colours.
pub fn highlight(src: &str) -> Vec<u8> {
    platform::highlight(HIGHLIGHT_BRIDGE, src)
}

/// Whether the run was asked for the checks: `SABIBOTS_SELFTEST` on a PC, `sabibots/?selftest` in
/// a page (2026-09-18).
///
/// The garden grew this in G5 and the Battle did not, so until that day the browser build of this
/// game could be watched but not *checked* — the lines that say Apply reached one robot and not
/// its twin, that `P` stopped the match's clock as well as the VM, that a `def` in the listing is
/// painted in the keyword colour, are all written by the checks and there was no way to ask for
/// them there. The query string is the environment a page has.
pub fn selftest_asked() -> bool {
    games_shell::checks::selftest_asked("SABIBOTS_SELFTEST")
}
