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
//!
//! **And how a run ends** ([`Errands`], S11). That is here because the checks are one of the two
//! things a run is asked for on top of being a game, and because ending the run is the thing they
//! did that no other check does.

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

/// **What this run was asked for on top of being a game, and which of those is still unfinished**
/// (S11) — the one place that answers "may this run end now, and if not, why not".
///
/// There are two such errands and they are asked for in the same breath on the same command line:
/// the checks (`GARDEN_SELFTEST=1`) and a picture (`--shot FILE SECONDS`, which opens a window,
/// waits, takes one picture and leaves). Each of them used to end the run by itself the moment it
/// was finished, and **each of them was therefore able to cut the other one short**:
///
/// * the checks end on a quiet PC well before a picture's moment, so
///   `GARDEN_SELFTEST=1 … --shot g.png 30` shut the window at about nine seconds and wrote no
///   picture at all (`docs/worklog/2026-09-21-factory-F0.md` §10, item 4);
/// * and Factory's checks take thirteen seconds of the game's own time while its picture is at
///   eight, so the picture ended the run with four of the check's lines unsaid
///   (`docs/worklog/2026-09-21-factory-F2.md`).
///
/// S9 mended the first direction for two games with a function that asked the command line
/// whether a picture had been asked for, and F2 mended the second direction inside Factory with a
/// `SelfTest` the picture's system could read. Two halves of one question in two places, which is
/// how the third game got a `done` line that gave a browser's reason for a PC's run. So the
/// question is asked here, of a resource rather than of `argv`, for three reasons:
///
/// 1. **half of it is not on the command line at all.** "Has the picture been taken yet" is a
///    fact about the run, not about the words it was started with; `argv` can only ever answer
///    the half S9 answered.
/// 2. **a page has no command line**, so the `argv` form could never give a browser more than
///    [`CHECKS_EXIT_WHEN_DONE`], and a page is where two of these games' faults live.
/// 3. it is read by a system that runs every frame, and `Args::from_env` collects the whole
///    command line into a `Vec<String>` each time it is asked.
///
/// It is a [`Plugin`] as well as a [`Resource`] so that a game says all of it once:
///
/// ```no_run
/// # use bevy::prelude::*;
/// # use games_shell::checks::Errands;
/// # let mut app = App::new();
/// # let (checks_asked, a_picture_asked) = (true, false);
/// app.add_plugins(Errands::of("the garden", checks_asked, a_picture_asked));
/// ```
///
/// and then marks its own errand finished — [`the_checks_are_done`](Errands::the_checks_are_done)
/// where the last check has spoken, [`the_picture_is_taken`](Errands::the_picture_is_taken) where
/// the file has been written. Neither of them writes `AppExit`: [`end_the_run`] does, once
/// nothing is left, and it is also the one place the `done` line is worded.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Errands {
    /// what the `done` line calls this game — "the garden", "the match", "the factory"
    keeps_running: &'static str,
    checks: Errand,
    picture: Errand,
}

/// One thing a run was told to do, and how far it has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Errand {
    /// nobody asked for it in this run
    #[default]
    NotAsked,
    Unfinished,
    Done,
}

impl Errands {
    /// What this run was asked for: the name the `done` line calls the game by, whether the
    /// checks are running, and whether a picture was asked for.
    pub fn of(keeps_running: &'static str, checks: bool, a_picture: bool) -> Errands {
        let asked = |yes: bool| if yes { Errand::Unfinished } else { Errand::NotAsked };
        Errands { keeps_running, checks: asked(checks), picture: asked(a_picture) }
    }

    /// The last check has spoken.
    pub fn the_checks_are_done(&mut self) {
        if self.checks == Errand::Unfinished {
            self.checks = Errand::Done;
        }
    }

    /// The picture is on disk — which is a frame or two after the shutter, because the file is
    /// written by an observer and not by the system that asked for it.
    pub fn the_picture_is_taken(&mut self) {
        if self.picture == Errand::Unfinished {
            self.picture = Errand::Done;
        }
    }

    /// Whether the checks have said their last word.
    pub fn the_checks_have_finished(&self) -> bool {
        self.checks == Errand::Done
    }

    /// **Why this run is still going, or `None` if nothing is left of it.**
    ///
    /// The sentence is the parenthesis of the `done` line, and the order the three are asked in
    /// is the order of what a reader wants told: what this run is still working on first, and the
    /// standing fact about the platform last. Until S11 a PC run with a picture to take said
    /// *a page has nothing to exit to*, which is the browser's reason and not true of it.
    pub fn left(&self) -> Option<&'static str> {
        if self.checks == Errand::Unfinished {
            return Some("the checks are not finished");
        }
        if self.picture == Errand::Unfinished {
            return Some("the picture is still to be taken");
        }
        if !CHECKS_EXIT_WHEN_DONE {
            return Some("a page has nothing to exit to");
        }
        None
    }

    /// Whether this run may end now — which needs something to have been asked for in the first
    /// place. **An ordinary game was asked for nothing and is never over**: a run with no checks
    /// and no picture has an empty list of errands, and an empty list must not read as "finished".
    pub fn may_end(&self) -> bool {
        let asked = self.checks != Errand::NotAsked || self.picture != Errand::NotAsked;
        asked && self.left().is_none()
    }
}

impl Plugin for Errands {
    fn build(&self, app: &mut App) {
        app.insert_resource(*self).add_systems(Last, end_the_run);
    }
}

/// **The end of the run, in one place**: it ends when nothing this run was asked for is left, and
/// it says why it is going on when something is.
///
/// It runs in `Last` so that whatever marked an errand finished this frame is taken account of in
/// the same frame the old code would have exited in.
///
/// The `done` line is printed **once, when the checks have finished and the run is not over** —
/// which in a browser is every time (there is nothing to exit to) and on a PC is when a picture
/// is still to come. A run whose picture was already taken by the time the checks finished simply
/// ends, and says nothing, exactly as a run with no picture at all does.
fn end_the_run(
    errands: Res<Errands>,
    mut exit: MessageWriter<AppExit>,
    mut said: Local<bool>,
    mut ended: Local<bool>,
) {
    match errands.left() {
        None if errands.may_end() && !*ended => {
            *ended = true;
            exit.write(AppExit::Success);
        }
        Some(why) if errands.the_checks_have_finished() && !*said => {
            *said = true;
            info!("selftest: done — {} keeps running ({why})", errands.keeps_running);
        }
        _ => {}
    }
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
    /// A frame that buys **nothing** is not a frame, and it is worth saying why: this number
    /// divides a budget, so a nought in a store would make the quotient infinite and `as u32`
    /// would saturate — a check that waits 4,294,967,295 frames is a check that never says
    /// anything at all, which is the one failure mode a bound exists to prevent.
    ///
    /// Until S11 a nought was quietly read as a one, which is a rate nobody measured and nobody
    /// asked for. It is refused now and the measured default stands
    /// ([`Settings::positive`](crate::Settings::positive)).
    pub fn read_from(&mut self, settings: &crate::Settings) {
        self.instructions_a_frame =
            settings.positive("checks_instructions_a_frame", self.instructions_a_frame);
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
    use super::{query_name, CheckPace, Errands, INSTRUCTIONS_A_FRAME_BUYS};
    use bevy::prelude::*;

    /// **Neither of the two errands may cut the other short** (S11) — both directions of it, which
    /// until now were mended in two places and in two different ways (S9 for one, Factory's F2 for
    /// the other).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn neither_errand_ends_a_run_the_other_is_still_working_on() {
        // the checks finish first: the run goes on, and the reason it gives is the picture
        let mut both = Errands::of("the garden", true, true);
        both.the_checks_are_done();
        assert!(!both.may_end());
        assert_eq!(both.left(), Some("the picture is still to be taken"));
        both.the_picture_is_taken();
        assert!(both.may_end());

        // the picture is taken first: the run goes on, and nothing is said, because the `done`
        // line is the checks' line and they have not finished
        let mut both = Errands::of("the garden", true, true);
        both.the_picture_is_taken();
        assert!(!both.may_end());
        assert!(!both.the_checks_have_finished());
        assert_eq!(both.left(), Some("the checks are not finished"));
        both.the_checks_are_done();
        assert!(both.may_end());

        // one errand on its own is the run's whole list
        let mut checks = Errands::of("the match", true, false);
        assert!(!checks.may_end());
        checks.the_checks_are_done();
        assert!(checks.may_end());

        // **and a game that was asked for nothing is never over** — an empty list of errands is
        // not a finished one, which is the one way this could have ended a player's run
        let mut game = Errands::of("the factory", false, false);
        assert!(!game.may_end());
        game.the_checks_are_done();
        game.the_picture_is_taken();
        assert!(!game.may_end(), "nothing was asked for, so nothing can be finished");
        assert_eq!(game.left(), None);
    }

    /// The `done` line and the exit, driven by the systems rather than by the rule — a run of an
    /// app with nothing in it but the plugin and one check that finishes on the second frame.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_run_ends_on_the_frame_the_last_errand_is_finished() {
        fn finish(mut errands: ResMut<Errands>) {
            errands.the_checks_are_done();
        }
        let mut app = App::new();
        app.add_plugins(Errands::of("the garden", true, false)).add_message::<AppExit>();
        app.update();
        assert!(app.should_exit().is_none(), "the checks have not finished");
        app.add_systems(Update, finish);
        app.update();
        assert_eq!(
            app.should_exit(),
            Some(AppExit::Success),
            "the frame the checks finished in is the frame the run ends in"
        );
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

        // **and a frame that buys nothing would be a check that never gives up, so it is
        // refused** (S11): until then a nought was quietly read as a one, which is a rate nobody
        // measured, and the run said nothing about it
        settings.set("checks_instructions_a_frame", "0");
        assert_eq!(CheckPace::of(&settings).instructions_a_frame, INSTRUCTIONS_A_FRAME_BUYS);
        assert!(
            settings.refused().iter().any(|said| said.contains("is not more than zero")),
            "and the run is told: {:?}",
            settings.refused()
        );

        let _ = std::fs::remove_file(&path);
    }
}
