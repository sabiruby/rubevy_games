//! **How a run is asked for its checks, and what happens when they are done.**
//!
//! Every game here has a `selftest` that prints one line per thing it has proved, and both games
//! are asked for it the same way: an environment variable on a PC (`GARDEN_SELFTEST=1`,
//! `SABIBOTS_SELFTEST=1`) and `?selftest` in the page's address in a browser, because **a page has
//! no environment** and the checks are the only way to see from outside that the world is alive.
//! The variable's name is the only part of that a game owns, so it is the one argument here.
//!
//! **And the knobs the checks themselves take** ([`asked_number`]), which since S5b-5 a page can
//! be given too: `<GAME>_<NAME>=<value>` in a shell is `?selftest&<name>=<value>` in an address.
//! A browser is where two of this game's flakes have shown themselves, and a knob that could only
//! be turned on a PC meant the probe and the flake could not be in the same run.

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

/// **A number the checks were handed**, such as the garden's `GARDEN_RELOAD_AT=20` — the checks'
/// only way to press F9 with no keyboard.
///
/// On a PC it is the environment variable of that name. **In a page it is the address**
/// (S5b-5): `?selftest&reload_at=20`, the same name in the same order with the game's own prefix
/// taken off and in lower case, since the prefix is there to keep two games' variables apart in
/// one shell and a page has no such shell. The rule is [`query_name`], and it is the one place
/// the two spellings are tied together.
///
/// Until S5b-5 a page answered `None` to every knob, and the cost of that was not theoretical:
/// S8 had a flake that showed itself **only in a browser** and a probe that could only be turned
/// on with an environment variable, so the probe and the flake could not be in the same run
/// (`docs/worklog/2026-09-20-writes-landing-in-a-pause.md`). A measurement that cannot be taken
/// where the thing happens is not a measurement.
#[cfg(not(target_arch = "wasm32"))]
pub fn asked_number(env_name: &str) -> Option<f32> {
    std::env::var(env_name).ok().and_then(|s| s.parse::<f32>().ok())
}

#[cfg(target_arch = "wasm32")]
pub fn asked_number(env_name: &str) -> Option<f32> {
    let want = query_name(env_name);
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| key.eq_ignore_ascii_case(&want))
        .and_then(|(_, value)| value.parse::<f32>().ok())
}

/// The name a page spells a knob with: the environment variable's, with the game's prefix taken
/// off and in lower case — `GARDEN_RELOAD_AT` is `reload_at`.
///
/// The prefix is everything up to the first underscore, because that is what the shell side
/// builds it from: `docker/run.sh` makes it out of the game's name in capitals, and a game whose
/// name has a `-` in it becomes a `_` there. **A game whose own name has two words would need to
/// say so here**, and none does.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn query_name(env_name: &str) -> String {
    env_name.split_once('_').map(|(_, rest)| rest).unwrap_or(env_name).to_ascii_lowercase()
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

#[cfg(test)]
mod tests {
    use super::query_name;

    /// The two spellings of one knob are tied together by one rule, so the test is of the rule.
    #[test]
    fn a_page_spells_a_knob_without_the_games_name() {
        assert_eq!(query_name("GARDEN_RELOAD_AT"), "reload_at");
        assert_eq!(query_name("SABIBOTS_SELFTEST"), "selftest");
        // a name with no prefix at all is its own lower case, rather than nothing
        assert_eq!(query_name("SELFTEST"), "selftest");
    }
}
