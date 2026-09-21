//! What differs between the PC build and the browser build, and nothing else: where the Ruby and
//! the models come from, how Ruby is compiled, where a saved script goes, and a clock for the
//! dice. The rest of the game is the same code in both.
//!
//! **Since S1 the work is `games_shell::platform`'s and what is left here is the garden's own
//! names.** This file used to be sabibots' with the crate's own name in it, and said so at the
//! top: making it shareable means passing four things in, "which is a change to sabibots for no
//! gain while there are two games; when there are three it is worth doing". The third game
//! (`factory`) is being written, so it is done — the four are the `localStorage` prefix, the
//! `ruby/` files built into the browser's binary, the names of the page's two bridges, and this
//! crate's own directory. Everything below is one of those, a name only the garden knows, or a
//! two-line wrapper so that `platform::read` still means what it meant in every caller.
//!
//! G3's save file needed nothing here beyond a name. `read` and `write` already take a path and
//! already mean "a file" on a PC and "a `localStorage` key" in a browser, and a garden written as
//! JSON is a string like a script is a string — so `save_world` writes with the same two
//! functions the editor saves a creature's file with, and the browser build of it is the PC build.
#![allow(dead_code)]

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
/// published page's saves and edited scripts are kept under it, in the browsers of the people who
/// have played it, and a different prefix is an empty garden and a lost script for every one of
/// them.
const STORE: &str = "garden:";

/// The page's two bridges, by the names the page defines them under — `window.gardenCompile`
/// and `window.gardenHighlight`, which are this game's word out of `web/games.sh` filled into
/// `web/page.html.in`. They are looked up by name rather than bound at compile time, which is
/// what lets one shared binding serve every game.
const COMPILE_BRIDGE: &str = "gardenCompile";
const HIGHLIGHT_BRIDGE: &str = "gardenHighlight";

/// What the Save button says, since what it does differs; and a clock for the dice.
pub use games_shell::platform::{clock_seed, SAVE_LABEL};
/// **What this run was asked for besides being a garden**, and the end of the run (S11): the
/// checks and a `--shot`, each of which used to be able to cut the other short. A page has
/// nothing to exit to (`docs/web.md`), which is one of the reasons this run may go on.
pub use games_shell::checks::Errands;

/// Where a saved garden goes (G3): a file in the directory the game was started from, which is
/// the workspace root when it is `cargo run`; `--save PATH` overrides it. In a browser there are
/// no files and this name is the `localStorage` key `garden:garden.save.json`, which survives the
/// page being closed, in that browser and nowhere else.
///
/// That is also why the save got a version number in G5: a file a player can see and delete is
/// one thing, and a string that sits in a browser across every future build of this page is
/// another. The first garden a new build meets there is usually one the old build wrote.
pub const SAVE_FILE: &str = "garden.save.json";

/// What the HUD's two buttons say when the pointer rests on them (G5). The same two buttons mean
/// two different things, and this is the sentence that says which.
#[cfg(not(target_arch = "wasm32"))]
pub const SAVE_WHERE: &str = "the whole garden, as JSON, in garden.save.json beside the game";
#[cfg(target_arch = "wasm32")]
pub const SAVE_WHERE: &str =
    "the whole garden, as JSON, in this browser's local storage (key garden:garden.save.json)";

/// **Where the panel's choices are kept (G6b)**: a small text file beside the save, in the
/// directory the game was started from — the `localStorage` key `garden:garden.settings.txt` in a
/// browser. It holds the guide's language and the night's dial — two lines of `key=value` — and
/// it is written only when one of them is changed with the mouse. Deleting it puts everything
/// back to its default, which is what a file the player can see is for.
pub const SETTINGS_FILE: &str = "garden.settings.txt";

/// Where the Ruby lives: next to the crate, from the workspace root or from the crate — and in a
/// browser the bare name the built-in table's paths start with.
pub fn ruby_dir() -> PathBuf {
    games_shell::crate_dir!("ruby", "garden/ruby")
}

/// Where the models live. Bevy looks next to the executable by default, which is not where a
/// workspace puts them. G0 has none — the meshes are Bevy's own primitives — but rubevy reads a
/// `require` from here, so it still has to point somewhere real.
pub fn assets_dir() -> String {
    games_shell::crate_dir!("assets", "garden/assets").to_string_lossy().into_owned()
}

pub fn read(path: &Path) -> Result<String, String> {
    platform::read(STORE, RUBY_FILES, path)
}

pub fn write(path: &Path, text: &str) -> Result<(), String> {
    platform::write(STORE, path, text)
}

/// Ruby source to RITE bytecode, with line numbers: the compiler linked in on a PC, the page's
/// `window.gardenCompile` in a browser.
pub fn compile(src: &str, name: &str) -> Result<Vec<u8>, String> {
    platform::compile(COMPILE_BRIDGE, src, name)
}

/// **What kind each byte of the source is** (0..=8), for the editor's colours.
pub fn highlight(src: &str) -> Vec<u8> {
    platform::highlight(HIGHLIGHT_BRIDGE, src)
}

/// Whether the run was asked for the checks (G0's `GARDEN_SELFTEST`, `?selftest` in a page).
pub fn selftest_asked() -> bool {
    games_shell::checks::selftest_asked("GARDEN_SELFTEST")
}

/// `GARDEN_RELOAD_AT=SECONDS`, the checks' only way to press F9 without a keyboard.
///
/// A headless run has no `ButtonInput` at all, so the one path a key drives — "read a file into a
/// garden that is already running" — had no check on it, and the one thing that went wrong there
/// (G5's finding 1) was found in a browser rather than here. This says when to open the `--load`
/// file, instead of opening it before the first frame. It is read only when `GARDEN_SELFTEST` is
/// set, so it is not a switch a player can trip — and a page has no environment and no `--load`,
/// where the checks press F9 for real (the page takes the key back for them — the game's `KEYS`
/// in `web/games.sh`) and there is nothing to defer.
pub fn reload_asked_at() -> Option<f32> {
    games_shell::checks::asked_number("GARDEN_RELOAD_AT")
}

/// `GARDEN_EGUI_FRAMES=N`, and `?selftest&egui_frames=N` in a page (S9): how many frames the
/// window's checks wait for bevy_egui to learn where the pointer is (`window::EGUI_FRAMES`).
///
/// It is a knob for the same reason the one above is: the wait it bounds is the one that has
/// twice been measured *in a browser*, where the frames are four times longer than a PC's and
/// an environment variable cannot be set. Read only when the checks were asked for.
pub fn egui_frames_asked() -> Option<f32> {
    games_shell::checks::asked_number("GARDEN_EGUI_FRAMES")
}

/// **Where the window's checks press Save** (S9) — and it is never where the player's garden
/// goes.
///
/// The checks had no line about F5 at all until S9, and the reason was that they had nowhere
/// safe to press it: the save's own path is the directory the game was started from, which for
/// `cargo run` is the repository, and a check that wrote a garden into the repository every run
/// would be a check that changes the thing it is checking. So `save_file` was the one setting in
/// the whole inventory that nothing had ever exercised (S5b-3's note).
///
/// The answer is that the checks move the target for the length of one press and put it back.
/// On a PC that is a file in the temporary directory, which is where the version check's fixture
/// already goes ([`another_version_file`]); in a page it is **another `localStorage` key** —
/// `garden:garden.checks.save.json` beside the player's `garden:garden.save.json` — because
/// `read` and `write` treat the two the same and the check needs no second road for the browser.
///
/// What is left behind afterwards: nothing on a PC (the file is removed), and in a page an empty
/// string under that key, in the browser of somebody who put `?selftest` in the address himself.
pub fn check_save_file() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::temp_dir().join("garden-checks.save.json").to_string_lossy().into_owned()
    }
    #[cfg(target_arch = "wasm32")]
    {
        "garden.checks.save.json".to_string()
    }
}

/// And how the checks put it back: the file goes on a PC, the key is emptied in a page (S9).
/// Neither can touch the player's garden, because neither is ever handed its path.
pub fn forget(path: &Path) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = std::fs::remove_file(path);
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = write(path, "");
    }
}

/// Where the tenth check's file goes (G5). A temporary directory on a PC, because the file is
/// about `--load` and not about where a file lives.
#[cfg(not(target_arch = "wasm32"))]
pub fn another_version_file() -> String {
    std::env::temp_dir().join("garden-from-another-version.json").to_string_lossy().into_owned()
}

/// Where the tenth check's file goes (G5). There is no temporary directory here: it is a
/// `localStorage` key beside the save's own, `garden:garden.from-another-version.json`, which
/// clears with the rest of the site's storage. A page asked for the checks is asked for this.
#[cfg(target_arch = "wasm32")]
pub fn another_version_file() -> String {
    "garden.from-another-version.json".into()
}
