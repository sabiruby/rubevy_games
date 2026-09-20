//! **How a run is asked for its checks, and what happens when they are done.**
//!
//! Every game here has a `selftest` that prints one line per thing it has proved, and both games
//! are asked for it the same way: an environment variable on a PC (`GARDEN_SELFTEST=1`,
//! `SABIBOTS_SELFTEST=1`) and `?selftest` in the page's address in a browser, because **a page has
//! no environment** and the checks are the only way to see from outside that the world is alive.
//! The variable's name is the only part of that a game owns, so it is the one argument here.

/// Whether this run was asked for the checks: `<NAME>=anything` on a PC, `?selftest` in the
/// page's address in a browser.
#[cfg(not(target_arch = "wasm32"))]
pub fn selftest_asked(env_name: &str) -> bool {
    std::env::var(env_name).is_ok()
}

#[cfg(target_arch = "wasm32")]
pub fn selftest_asked(_env_name: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .is_some_and(|q| q.contains("selftest"))
}

/// **A number the checks were handed in the environment**, such as the garden's
/// `GARDEN_RELOAD_AT=20` — the checks' only way to press F9 with no keyboard. There is no
/// environment in a page and nothing to read, so the answer there is `None`: a browser's checks
/// press the key for real (the page takes it back for them: the game's `KEYS` in `web/games.sh`).
#[cfg(not(target_arch = "wasm32"))]
pub fn asked_number(env_name: &str) -> Option<f32> {
    std::env::var(env_name).ok().and_then(|s| s.parse::<f32>().ok())
}

#[cfg(target_arch = "wasm32")]
pub fn asked_number(_env_name: &str) -> Option<f32> {
    None
}

/// Whether the checks end the run when they are done. On a PC they do: they were asked for on a
/// command line and the shell wants its prompt back.
#[cfg(not(target_arch = "wasm32"))]
pub const CHECKS_EXIT_WHEN_DONE: bool = true;

/// **A page has nothing to exit to**, and `AppExit` in a browser is not "the run ended", it is
/// "this canvas stops". winit's wasm event loop stops being pumped, every system stops running,
/// and the last frame drawn stays on the screen looking like a garden — so the page goes on
/// *looking* alive while no key, no click and no creature does anything ever again. That is the
/// garden's G5 finding (`docs/web.md`, "the page stops answering the mouse and the keyboard"):
/// not egui holding the keyboard and not the DOM, but the checks ending the app under the
/// player's feet. So the checks do not exit here; they say they are done and the game goes on.
#[cfg(target_arch = "wasm32")]
pub const CHECKS_EXIT_WHEN_DONE: bool = false;
