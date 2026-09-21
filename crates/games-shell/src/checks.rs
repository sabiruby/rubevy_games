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
//!
//! **And the one number a check needs that is about the machine rather than about a game**
//! ([`CheckPace`], S10): how many instructions a frame of the VM buys here, which is what turns a
//! script budget into "how many frames this check may wait".

use bevy::prelude::*;

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

/// Whether this **platform** ends a run when the checks are done. On a PC it does: they were
/// asked for on a command line and the shell wants its prompt back.
///
/// It says what the platform can do, not what this run should do — [`checks_end_the_run`] is the
/// one to ask, and it is this and one thing more.
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

/// **Whether the checks are the last thing this run was asked for** (S9) — which is
/// [`CHECKS_EXIT_WHEN_DONE`] unless the same command line also asked for a picture.
///
/// `--shot FILE SECONDS` opens a window, waits, takes one picture and leaves. The checks end the
/// run the moment they are done, which on a quiet PC is well before the picture's moment, so
/// `GARDEN_SELFTEST=1 … --shot g.png 30` shut the window at about nine seconds and wrote no
/// picture at all (`docs/worklog/2026-09-21-factory-F0.md` §10, item 4; all three games have the
/// same shape). The way round it was to take the picture in a second run with the checks off,
/// which is a picture of a *different* run — and the runs one wants a picture of are exactly the
/// ones a check has something to say about.
///
/// So the question the checks ask before exiting is not "can this platform exit" but "is there
/// anything else this run was told to do". There is one such thing, it is on the command line,
/// and [`crate::args`] is already where the command line is read — so no game has to hold a flag
/// for the shell, and the answer is the same for all three of them.
///
/// The picture's own system ends the run when it has the file (`take_shot` in each game), so
/// nothing is left running: the two ends of the run hand over rather than race.
#[cfg(not(target_arch = "wasm32"))]
pub fn checks_end_the_run() -> bool {
    checks_end(&crate::args::Args::from_env())
}

/// The same question of a line that is handed over rather than read out of the process, so that
/// the rule can be tested. `Args::from_env` is the only thing the public one adds.
#[cfg(not(target_arch = "wasm32"))]
fn checks_end(args: &crate::args::Args) -> bool {
    CHECKS_EXIT_WHEN_DONE && !args.has("--shot")
}

/// A page has no command line, so there is no picture to wait for and nothing to exit to either.
#[cfg(target_arch = "wasm32")]
pub fn checks_end_the_run() -> bool {
    CHECKS_EXIT_WHEN_DONE
}

/// **How fast this machine's VM is, as far as a check needs to know** — and the sum that turns a
/// script budget into a number of frames a check may wait (S10).
///
/// Both games here wait on a VM and both have to give up eventually, and both worked the giving-up
/// point out of the same two things: the frames the errand costs whatever the VM is allowed, and
/// one whole frame's allowance of the VM's turns. The second half is a budget divided by
/// [`INSTRUCTIONS_A_FRAME_BUYS`], and **that number was written out twice** — in
/// `garden/src/window.rs` and in `sabibots/src/main.rs`, under the same name, with the same value
/// and the same paragraph of provenance beside each (S9's first finding). It is not a coincidence
/// of two games agreeing on a number; it is **one fact about one machine**, and neither copy could
/// so much as cite the other, because they are in two binaries.
///
/// It is a setting and not a `const` for the reason every number here is
/// (`/home/kishima/book/CLAUDE.md`): the default is a measurement of *this* machine, and the next
/// machine is a different measurement. The key is `checks_instructions_a_frame` and it is spelt
/// the same in every game's store, because what it describes is the same in every game.
///
/// ```
/// # use games_shell::checks::CheckPace;
/// let pace = CheckPace::default();
/// // two structural frames, and one frame's worth of a 41,000-instruction budget
/// assert_eq!(pace.frames_to_wait(2, 41_000), 3);
/// ```
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct CheckPace {
    /// [`INSTRUCTIONS_A_FRAME_BUYS`], or what `checks_instructions_a_frame` said instead.
    pub instructions_a_frame: f32,
}

impl Default for CheckPace {
    fn default() -> Self {
        CheckPace { instructions_a_frame: INSTRUCTIONS_A_FRAME_BUYS }
    }
}

impl CheckPace {
    /// The pace this run is really going at: the default with whatever the store says written
    /// over it.
    pub fn of(settings: &crate::Settings) -> CheckPace {
        let mut pace = CheckPace::default();
        pace.read_from(settings);
        pace
    }

    /// | key | field |
    /// |---|---|
    /// | `checks_instructions_a_frame` | [`CheckPace::instructions_a_frame`] |
    ///
    /// A frame that buys **nothing** is not a frame, and it is worth saying why the floor is here
    /// rather than trusting the file: this number divides a budget, so a nought in a store would
    /// make the quotient infinite and `as u32` would saturate — a check that waits 4,294,967,295
    /// frames is a check that never says anything at all, which is the one failure mode a bound
    /// exists to prevent.
    pub fn read_from(&mut self, settings: &crate::Settings) {
        if let Some(value) = settings.number("checks_instructions_a_frame") {
            self.instructions_a_frame = value.max(1.0);
        }
    }

    /// **How many frames a check may wait for a VM it has asked something of**: the frames the
    /// errand costs whatever the VM is allowed, plus one whole frame's allowance of the VM's
    /// turns.
    ///
    /// `structural` is **an argument and not a number in here**, and that is the point of the
    /// signature. Both games pass a two, and the two twos are not the same two:
    ///
    /// * the garden's (`window::STRUCTURAL_FRAMES`) is a **restart** — the `Script` lands at the
    ///   end of the frame that asked for it and rubevy makes a task of it and runs it on the next.
    /// * Battle's (`HANDLER_STRUCTURAL_FRAMES`) is a **published message** — the publish is made
    ///   after this frame's tick, so the next frame's tick is the earliest that can give the woken
    ///   handler its turn, and the check is not ordered against the chain that answers the
    ///   handler's `act`, so the frame it is *certain* to see the swerve in is the one after that.
    ///
    /// Two errands, two derivations, the same answer today. Folding them into one number here
    /// would be S5b-5's mistake in a new place: the garden's `script_budget` moved, the egui wait
    /// that had been riding on the same sum quietly got shorter, and three runs in eighty-eight
    /// gave up (`docs/worklog/2026-09-21-checks-and-leftovers.md` §14-1). **A bound is an argument
    /// about what is being waited for**, so a game that changes what its errand costs must be able
    /// to move its own half and nobody else's.
    ///
    /// What *is* shared is the division, because the divisor is a fact about the machine and the
    /// VM rather than about either errand.
    pub fn frames_to_wait(&self, structural: u32, budget: u64) -> u32 {
        structural + (budget as f32 / self.instructions_a_frame).ceil() as u32
    }
}

/// **What one frame of this machine's VM buys, in instructions** — the default of
/// [`CheckPace::instructions_a_frame`], and the one place this measurement is written down.
///
/// It is a number about *this machine's* wall clock, which is why it is the checks' and not a
/// number a game plays by: a check is allowed to know how fast the machine it is running on is.
///
/// **There are two measurements of it, and this is the slower one** (S5b-5):
///
/// | | how it was measured | rate | 8 ms buys |
/// |---|---|---|---|
/// | S6, 2026-09-20 | the garden's `frame_time` cut to **300 µs**, the VM's slowest frame in that run: 1,708 instructions (`docs/worklog/2026-09-20-window-check-flakes.md` §4.4, `s6/ft300.log`, f59 — two beetles' first pass of 854 each) | 5.7 insn/µs | **45,600** |
/// | S5b-3, 2026-09-21 | the capped garden at its **own 8 ms**, three runs of a minute, 10,749 frames (`docs/worklog/2026-09-21-numbers-garden-settings.md` §4) | 6.85 insn/µs | 54,800 |
///
/// The difference is the condition, not the machine: a frame cut to 300 µs pays the cost of
/// starting and stopping the tick over a twenty-seventh of the work, so the rate it measures is
/// the rate of a *short* frame. A garden's own frames are the second row, and they are quicker.
///
/// **The slower rate is the one to keep**, and the reason is which way this number's error hurts.
/// It divides a budget to say how many frames a check may wait, so a number that is too **big**
/// makes the wait too short and produces a FAIL for a VM that was merely being slow — a false
/// FAIL, the thing S6 and S7 were called in to remove. A number that is too small only makes a
/// check wait longer before it says what it was going to say. So the rate that buys *less* per
/// frame is the safe side, and 45,600 is it.
///
/// **At the budgets the games ship with, the two agree**: `ceil(41,000 / 45,600)` and
/// `ceil(41,000 / 54,800)` are both 1, so the garden's sum is 3 either way, and since S10b took
/// Battle's budget to 114,000 both of its divisors give 3 as well, so its sum is 5 either way.
/// (They did not agree while Battle ran on the inherited 200,000: 5 against 4.) The choice shows
/// above 45,600 of budget, which is `script_budget` in somebody's `*.settings.txt` — and the
/// machine it was measured on is not the machine the next person runs this on, which is what
/// `checks_instructions_a_frame` is for.
pub const INSTRUCTIONS_A_FRAME_BUYS: f32 = 45_600.0;

#[cfg(test)]
mod tests {
    use super::{query_name, CheckPace, INSTRUCTIONS_A_FRAME_BUYS};

    /// **A run that was also asked for a picture is not over when the checks are** (S9). The
    /// three games all press this one button, and the thing it turns on is a word on the
    /// command line rather than anything a game holds.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_shot_keeps_the_run_alive_after_the_checks() {
        use crate::args::Args;
        assert!(super::checks_end(&Args::of(["garden"])));
        assert!(super::checks_end(&Args::of(["garden", "--headless", "90"])));
        assert!(!super::checks_end(&Args::of(["garden", "--shot"])));
        assert!(!super::checks_end(&Args::of(["garden", "--shot", "g.png", "30"])));
        // and the flag is the whole word, not a prefix of one
        assert!(super::checks_end(&Args::of(["garden", "--shots"])));
    }

    /// The two spellings of one knob are tied together by one rule, so the test is of the rule.
    #[test]
    fn a_page_spells_a_knob_without_the_games_name() {
        assert_eq!(query_name("GARDEN_RELOAD_AT"), "reload_at");
        assert_eq!(query_name("SABIBOTS_SELFTEST"), "selftest");
        // a name with no prefix at all is its own lower case, rather than nothing
        assert_eq!(query_name("SELFTEST"), "selftest");
    }

    /// **The shape of the sum** (S10), which until now was written out in two games. The two
    /// halves are tested apart because they come from different places: the structural frames are
    /// the caller's and are added whatever the budget is, and the quotient is this crate's.
    #[test]
    fn a_bigger_budget_buys_a_check_more_frames() {
        let pace = CheckPace::default();
        // the garden's shipped 41,000, Battle's 114,000 since S10b, and the 200,000 both of them
        // ran on before they were measured — the sum is tested at all three because what is being
        // tested is the sum and not what anybody's default happens to be today
        assert_eq!(pace.frames_to_wait(2, 41_000), 3);
        assert_eq!(pace.frames_to_wait(2, 114_000), 5);
        assert_eq!(pace.frames_to_wait(2, 200_000), 7);
        // ten times the budget is not the same wait (S5b-3's lesson, which is why this is worked
        // out rather than written down): `ceil(410,000 / 45,600)` is nine
        assert_eq!(pace.frames_to_wait(2, 410_000), 2 + 9);
        // the structural frames survive a budget too small to buy a whole frame, and they are the
        // caller's number: two games pass a two for two different reasons, and a third may not
        assert_eq!(pace.frames_to_wait(2, 1), 3);
        assert_eq!(pace.frames_to_wait(0, 1), 1);
        // a budget of nothing is a VM that runs nothing, and the errand still costs what it costs
        assert_eq!(pace.frames_to_wait(2, 0), 2);
    }

    /// **A store reaches it** (S10) — "it is a setting" and "the setting is doing anything" are
    /// two claims, and this is the first of them; the second is a run of the game with a line in
    /// its file (`docs/worklog/2026-09-21-s10.md`).
    #[test]
    fn a_store_can_say_how_fast_the_machine_is() {
        use crate::Settings;
        use std::path::Path;
        fn read(path: &Path) -> Result<String, String> {
            std::fs::read_to_string(path).map_err(|e| e.to_string())
        }
        fn write(path: &Path, text: &str) -> Result<(), String> {
            std::fs::write(path, text).map_err(|e| e.to_string())
        }
        let path = std::env::temp_dir().join("games-shell-checkpace-test.txt");
        let _ = std::fs::remove_file(&path);

        let settings = Settings::load(&path, "a test", read, write);
        assert_eq!(CheckPace::of(&settings).instructions_a_frame, INSTRUCTIONS_A_FRAME_BUYS);

        let mut settings = Settings::load(&path, "a test", read, write);
        // half the machine: a frame buys half as much, so a budget costs twice the frames
        settings.set("checks_instructions_a_frame", "22800");
        let pace = CheckPace::of(&settings);
        assert_eq!(pace.instructions_a_frame, 22_800.0);
        assert_eq!(pace.frames_to_wait(2, 200_000), 2 + 9);

        // and a frame that buys nothing would be a check that never gives up, so it cannot
        settings.set("checks_instructions_a_frame", "0");
        assert_eq!(CheckPace::of(&settings).instructions_a_frame, 1.0);

        let _ = std::fs::remove_file(&path);
    }
}
