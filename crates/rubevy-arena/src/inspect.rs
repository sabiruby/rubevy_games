//! **The VM panel: what a behaviour is waiting for, and — under "details" — the machine.**
//!
//! It is read through sabiruby's `Vm::snapshot` (`docs/design/inspect.md` there), which never
//! calls Ruby: a value is rendered in Rust, so looking at a robot cannot move it. Registers are
//! named from the LVAR section, which is why the panel can say `target` rather than `R3` — the
//! games compile their scripts with debug info for exactly this reason.
//!
//! Which of the VM's contexts a task stands in is `Vm::task_context(task)`, in sabiruby 0.5.0.
//! Until it was there the panel read the index out of the way the VM renders a task
//! (`#<Task 12 ctx=3>`), which is a debugging format and not an API; that parser is gone.
//!
//! **G9 turned it round.** Until then the panel opened with registers, a heap line, a context
//! count and the whole frame stack — `(no debug info)`, `prelude.rb:47`, `beetle.rb:118` — which
//! is a debugger's window and reads as one. What a person opening it wants first is the *one*
//! sentence: **this creature is waiting for that, on this line of its own file.** So the top of
//! the panel is now the name, the frames of the author's own file, why it is waiting, and three
//! numbers; everything else went under a "details" heading that starts closed
//! ([`Waiting`], [`draw_inspector`]). The panel also starts **closed** — the games open it on
//! `F2`, and a `--shot` only with `--vm`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use rubevy::{RubevySet, ScriptWorld};
use sabiruby::inspect::HeapView;
use sabiruby::value::ObjId;

/// How many of the innermost frames of a context carry their registers. A brain waiting for the
/// game stands about three frames deep in the DSL, so six reaches its own code as well.
pub const REGS_FRAMES: usize = 6;

/// One frame of the task, innermost first.
#[derive(Debug, Clone, Default)]
pub struct FrameRow {
    /// `robots/scout.rb:32`, in the script author's own line numbers where it is their file.
    pub at: String,
    /// The method this frame is in (`pop`, `radar`, `run`), or the block/top level.
    pub method: String,
    /// The class the method was found on.
    pub class: String,
    pub pc: usize,
    /// `Cci::Skip`: a native pushed this frame and is waiting for it.
    pub native_boundary: bool,
    /// Empty for frames outside [`REGS_FRAMES`].
    pub regs: Vec<RegRow>,
    /// This frame is in the script author's own file rather than the DSL or mrblib: the one a
    /// reader of the code wants to be looking at.
    pub own: bool,
}

impl FrameRow {
    /// A local of this frame by name, as it was rendered (`:Hunger`, `[:Plant]`). `None` where
    /// the frame keeps no registers in the snapshot or has no such local.
    fn local(&self, name: &str) -> Option<&str> {
        self.regs
            .iter()
            .find(|r| r.local && r.name.as_deref() == Some(name))
            .map(|r| r.text.as_str())
    }

    /// Whether this frame is a task parked on a queue: `Task::Queue#pop` and its two aliases.
    fn is_pop(&self) -> bool {
        matches!(self.method.as_str(), "pop" | "shift" | "deq")
    }
}

/// One register of a frame. `R0` is `self`; `R1..nlocals` are the locals, named where the
/// program carries debug info.
#[derive(Debug, Clone, Default)]
pub struct RegRow {
    pub index: usize,
    pub name: Option<String>,
    pub class: String,
    pub text: String,
    /// True for `R1..nlocals`: a named local rather than a temporary the compiler needed.
    pub local: bool,
}

/// **Why this behaviour is not running** — the line the panel puts at the top, worked out from
/// the frames alone (`why` below).
///
/// A task in this VM stops for one of two reasons, and the frames tell them apart:
///
/// * **It is on a queue.** `Task::Queue#pop` is Ruby, in mrblib, so a parked task carries a
///   frame whose method is `pop` — and the frame *behind* it says which kind of queue:
///   `Rubevy::Subscription` is an `on(:…)` handler's ([`Waiting::Event`]), `Rubevy::Proxy` or any
///   other method is a `Rubevy.ask` the game owes an answer to ([`Waiting::Ask`]).
///
///   There used to be a sixth reason here, and it was the commonest one in the Garden: a
///   component read, `me[:Hunger]`, whose `pop` sat under `Rubevy::Entity#get`. Since 2026-09-17
///   rubevy answers a read **inside the tick that asked it** — the tick runs the ready tasks,
///   answers the reads they parked on, and runs them again (rubevy `docs/host-api.md`, "A read
///   costs no frame") — so by the time this panel looks at the VM, no task is standing there.
///   The read is still a `pop` while it lasts; it just never lasts as long as a frame.
///
///   One case is left where it could last that long, and the panel does not cover it: a tick
///   that runs out of its budget of instructions or of its `frame_time` stops with the reads of
///   that round unanswered, and rubevy answers them at the top of the next tick. A task caught
///   there reads as [`Waiting::Ask`] on the method behind the `pop` (`get`), which is wrong in
///   that it names the *game* as the one who owes the answer. Neither game has come near its
///   budget — the Garden's ticks measure under 1 ms of the 8 ms it is given — so this has not
///   been seen; it is written down because "unreachable" would be too strong a word for it.
/// * **It is not.** `sleep` is a native and pushes no frame, so a sleeping task's innermost
///   frame is the line that called it ([`Waiting::Sleep`]).
///
/// The second of those is the one place the panel is guessing, and it says so: a task that had
/// run out of its timeslice — ready to go, not asleep — looks exactly the same from here. In
/// these two games nothing else parks a task, and the VM has no read-only way to ask a task for
/// its scheduler state (`Task#status` is Ruby, and the panel does not run Ruby).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Waiting {
    /// No frames at all: the task has ended, or has not started.
    #[default]
    Nothing,
    /// Parked on a subscription's queue: `on(:touched) { … }` waiting for its event. The name of
    /// the event is not on the stack — it is a local of the block that made the task — so the
    /// panel can say *that* it is an event and not *which*.
    Event,
    /// Parked on a `Rubevy.ask` queue: the game has been asked something and has not answered
    /// yet. The string is the method that asked (`nearest(:Plant)`, `radar`).
    Ask(String),
    /// Parked on a queue the frames do not place.
    Queue,
    /// Not on a queue: a `sleep` (see the caveat above).
    Sleep,
}

impl Waiting {
    /// The sentence the panel shows.
    pub fn text(&self) -> String {
        match self {
            Waiting::Nothing => "nothing to wait for: this task has no frames".into(),
            Waiting::Event => "waiting for an event — an `on(:…)` block, parked on its queue".into(),
            Waiting::Ask(what) if what.is_empty() => "waiting for the game to answer a question".into(),
            Waiting::Ask(what) => format!("waiting for the game to answer `{what}`"),
            Waiting::Queue => "waiting on a queue".into(),
            Waiting::Sleep => "sleeping — it asked for time, not for an answer".into(),
        }
    }

    /// How the panel knows, for the hover: the evidence, and where it is thin.
    pub fn how(&self) -> &'static str {
        match self {
            Waiting::Nothing => "the task has no context: it has run to its end, or has not had its first frame",
            Waiting::Event => "the innermost frames are `Task::Queue#pop` under `Rubevy::Subscription#pop` — a queue `Rubevy.subscribe` handed out, which is what `on(:…)` waits on. Which event it is, is a local of the block that started the task and is not on this stack",
            Waiting::Ask(_) => "the innermost frame is `Task::Queue#pop`, and the frame behind it is the method that called `Rubevy.ask`: the question has gone out with this frame's commands and the answer comes back on the next one",
            Waiting::Queue => "the innermost frame is `Task::Queue#pop`, and nothing behind it says which queue",
            Waiting::Sleep => "it is parked and *not* on a queue, and `sleep` is the only other thing that parks a task in this game's Ruby (`sleep` is a native and pushes no frame, so the line shown is the one that called it). A task that had used up its timeslice would look the same from here",
        }
    }
}

/// What the panel shows. A game fills it with [`VmInspector::fill`] on the frames it is open and
/// draws itself through [`VmInspectorPlugin`].
#[derive(Resource, Default)]
pub struct VmInspector {
    /// **Closed unless somebody asked** (G9): `F2` in both games, `--shot --vm` for a picture.
    pub open: bool,
    /// The world is stopped: the plugin's budget is 0, no task runs, and — from G9 — the game's
    /// own rules are not running either, so the snapshot stands still while it is read.
    pub paused: bool,
    /// Whose task this is (`3 blue/scout`, `Beetle 405v0`).
    pub title: String,
    /// The context it stands in, and what state that context is in. Under "details".
    pub status: String,
    pub frames: Vec<FrameRow>,
    /// Why it is not running, in the panel's own words.
    pub waiting: Waiting,
    /// The innermost frame of the script's own file: the line a reader is looking for.
    pub waiting_at: Option<String>,
    /// Which frame's registers are shown (an index into `frames`). Under "details".
    pub selected: usize,
    /// While true, the panel picks the innermost frame in the script's own file every time it is
    /// filled — the brain usually stands several frames deep in the DSL, and the DSL's locals are
    /// not what a reader of the robot came for. Clicking a row turns it off.
    pub follow: bool,
    pub heap: Option<HeapView>,
    /// Instructions this task has run since it started, and what it spent on the last frame.
    pub instructions: u64,
    pub spent: u64,
    /// Instructions per frame over this script's whole life, where the game keeps the figure
    /// (the garden's HUD does). `None` leaves the line to `watched_per_frame`, which the panel
    /// works out for itself.
    pub per_frame: Option<u64>,
    /// What the panel has seen this task spend, per frame, since it was opened on it: the
    /// instruction count is read every frame anyway, and the difference between two frames is
    /// what that frame cost. Smoothed, because a parked task spends nothing on most frames and a
    /// number that reads 0, 0, 340, 0, 0 is not a number anybody can read. It is here so that a
    /// game with no counter of its own (SabiRuby Battle keeps none) still has the figure.
    watched_per_frame: f32,
    /// the count this task stood at when the panel last looked, and whose task that was
    watched_at: u64,
    watching: String,
    /// Contexts the VM holds, and how many of them have not ended. One per task — a behaviour,
    /// each of its handlers, the match — so it is also the task count. A task left parked is one
    /// of these for as long as the VM lives, which is how a leak shows.
    pub contexts_live: usize,
    pub contexts: usize,
    /// Why there is nothing to show, when there is nothing.
    pub note: String,
}

impl VmInspector {
    /// Reads one script's task out of the VM. `prelude_lines` is what the game put in front of
    /// the author's file, so the panel can report the author's own line numbers.
    pub fn fill(&mut self, world: &ScriptWorld, task: ObjId, title: String, prelude_lines: u32) {
        self.title = title;
        self.note.clear();
        let vm = &world.vm;
        let snapshot = vm.snapshot(REGS_FRAMES);
        self.heap = Some(snapshot.heap.clone());
        self.contexts = snapshot.contexts.len();
        self.contexts_live = snapshot
            .contexts
            .iter()
            .filter(|c| !matches!(c.status, sabiruby::vm::FiberState::Terminated))
            .count();
        self.instructions = vm.task_instructions(task);
        // insn/frame, worked out from the frames the panel has been open: see `watched_per_frame`
        if self.watching != self.title {
            self.watching = self.title.clone();
            self.watched_per_frame = 0.0;
            self.watched_at = self.instructions;
        }
        let this_frame = self.instructions.saturating_sub(self.watched_at) as f32;
        self.watched_at = self.instructions;
        self.watched_per_frame = self.watched_per_frame * 0.95 + this_frame * 0.05;

        let Some(ctx) = vm.task_context(task) else {
            self.frames.clear();
            self.status.clear();
            self.waiting = Waiting::Nothing;
            self.waiting_at = None;
            self.note = "this script has no context: it has run to its end".into();
            return;
        };
        let Some(context) = snapshot.contexts.iter().find(|c| c.index == ctx) else {
            self.frames.clear();
            self.status.clear();
            self.waiting = Waiting::Nothing;
            self.waiting_at = None;
            self.note = format!("context {ctx} is not in the snapshot");
            return;
        };
        self.status = format!(
            "context {ctx}  {:?}{}",
            context.status,
            if context.is_current { "  (running)" } else { "" }
        );

        // `Vm::task_frames` is the only public way to a frame's *file*: a `FrameView` carries the
        // irep and the line but not the name of the source. Both skip the same frames — those
        // whose irep has no debug info, which is all of mrblib — and both run innermost first, so
        // the files line up with the frames that have a line.
        let files = vm.task_frames(task);
        let mut file = files.iter();
        self.frames = context
            .frames
            .iter()
            .rev()
            .map(|f| {
                let mut own = false;
                let at = match f.line {
                    Some(line) => {
                        let name = file.next().map(|(n, _)| n.as_str()).unwrap_or("?");
                        // the prelude sits in front of the author's file in the same program
                        if line > prelude_lines {
                            own = true;
                            format!("{name}:{}", line - prelude_lines)
                        } else {
                            format!("prelude.rb:{line}")
                        }
                    }
                    None => "(no debug info)".to_string(),
                };
                FrameRow {
                    at,
                    own,
                    method: f.mid.clone().unwrap_or_else(|| "(block or top level)".into()),
                    class: f.target_class.clone(),
                    pc: f.pc,
                    native_boundary: f.native_boundary,
                    regs: f
                        .regs
                        .iter()
                        .map(|r| RegRow {
                            index: r.index,
                            name: r.name.clone(),
                            class: r.value.class.clone(),
                            text: r.value.text.clone(),
                            local: r.index >= 1 && r.index < f.nlocals,
                        })
                        .collect(),
                }
            })
            .collect();
        self.waiting = why(&self.frames);
        self.waiting_at = self.frames.iter().find(|f| f.own).map(|f| f.at.clone());
        // the innermost frame of the robot's own file, which is where its author is reading. A
        // brain waiting for a scan stands in `pop`, in `incoming`, in `Kernel#loop` — three
        // frames of somebody else's code before its own.
        if self.follow {
            self.selected = self.frames.iter().position(|f| f.own && !f.regs.is_empty()).unwrap_or(0);
        }
        if self.selected >= self.frames.len() {
            self.selected = 0;
        }
    }

    /// What this script spends a frame: the game's own figure where it keeps one, and otherwise
    /// what the panel has watched it spend (`watched_per_frame`).
    pub fn insn_per_frame(&self) -> u64 {
        match self.per_frame {
            Some(n) => n,
            None => self.watched_per_frame.round() as u64,
        }
    }

    /// The frames of the script author's own file — the only ones the top half shows.
    pub fn own_frames(&self) -> impl Iterator<Item = &FrameRow> {
        self.frames.iter().filter(|f| f.own)
    }

    /// Following the script's own frame, and closed: what a game inserts as the resource. `open`
    /// is the game's to set — `F2`, or `--shot --vm`.
    pub fn following() -> VmInspector {
        VmInspector { open: false, follow: true, ..VmInspector::default() }
    }

    /// Open from the start after all: `--shot --vm` asking for a picture with the panel in it.
    pub fn opened(mut self, yes: bool) -> VmInspector {
        self.open = yes;
        self
    }

    pub fn clear(&mut self, note: impl Into<String>) {
        self.frames.clear();
        self.status.clear();
        self.heap = None;
        self.waiting = Waiting::Nothing;
        self.waiting_at = None;
        self.note = note.into();
    }

    /// The whole of it in a few lines, for a log where there is no window (`--headless`). The
    /// first line is the panel's top half: who, why, where, and what it spends.
    pub fn log_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!(
            "vm: {} — {} — {} — {} insn/frame, {} in all — {} tasks",
            self.title,
            self.waiting.text(),
            self.waiting_at.clone().unwrap_or_else(|| "(nothing of its own on the stack)".into()),
            self.insn_per_frame(),
            self.instructions,
            self.contexts_live,
        ));
        if !self.note.is_empty() {
            out.push(format!("vm: {}", self.note));
        }
        let heap = match &self.heap {
            Some(h) => format!(
                "heap live {} of {} ({} free, {} since gc of {}), gc {} times, {} live after the last",
                h.live, h.len, h.free, h.allocated_since_gc, h.alloc_threshold, h.gc_count, h.live_after_gc
            ),
            None => "heap ?".to_string(),
        };
        out.push(format!(
            "vm:   details: {} — contexts {} live of {} — {heap}",
            self.status, self.contexts_live, self.contexts
        ));
        for (i, f) in self.frames.iter().enumerate().take(4) {
            let locals: Vec<String> = f
                .regs
                .iter()
                .filter(|r| r.local && r.name.is_some())
                .map(|r| format!("{}={}", r.name.clone().unwrap_or_default(), r.text))
                .collect();
            out.push(format!(
                "vm:   #{i} {:<22} {:<16} pc {:<5} {}",
                f.at,
                f.method,
                f.pc,
                if locals.is_empty() { String::from("(no locals)") } else { locals.join("  ") }
            ));
        }
        out
    }
}

/// **Why a task is not running, from its frames** (see [`Waiting`]).
///
/// Innermost first. A `pop` frame means a queue, and then the first frame behind it that is not
/// another `pop` says which queue it is: the plumbing each kind of wait goes through is a
/// different class, and a class is a fact the VM reports rather than a string to be parsed.
fn why(frames: &[FrameRow]) -> Waiting {
    let Some(inner) = frames.first() else { return Waiting::Nothing };
    if !inner.is_pop() {
        return Waiting::Sleep;
    }
    for f in frames.iter().skip(1) {
        // `Rubevy.subscribe` extends the queue it hands out with this module, so an event's
        // `pop` goes through it and an answer's does not (rubevy `src/prelude.rb`)
        if f.class.contains("Rubevy::Subscription") {
            return Waiting::Event;
        }
        // `garden.nearest(:Plant)` → `Rubevy::Proxy#method_missing` → `Rubevy.ask(…).pop`
        if f.class.contains("Rubevy::Proxy") {
            // `name` is the Symbol `method_missing` was called with, and it renders as `:nearest`;
            // what the author wrote is `nearest`
            let name = f
                .local("name")
                .map(clean)
                .map(|n| n.trim_start_matches(':').to_string())
                .unwrap_or_default();
            let args = f.local("args").map(clean).unwrap_or_default();
            return Waiting::Ask(match (name.is_empty(), args.is_empty() || args == "[]") {
                (true, _) => String::new(),
                (false, true) => name,
                (false, false) => format!("{name}({})", args.trim_start_matches('[').trim_end_matches(']')),
            });
        }
        if f.is_pop() {
            continue;
        }
        // anything else that called a `pop`: the DSL method that asked (`radar`, `status`), or a
        // line of the author's own file that spelled `Rubevy.ask(…).pop` out
        return Waiting::Ask(f.method.clone());
    }
    Waiting::Queue
}

/// A rendered value as a person would write it: `:Hunger` rather than `:Hunger`'s quotes, and
/// nothing longer than a column.
fn clean(text: &str) -> String {
    let text = text.trim_matches('"');
    if text.chars().count() <= 40 {
        return text.to_string();
    }
    let kept: String = text.chars().take(39).collect();
    format!("{kept}…")
}

/// **How long the VM had this frame, against what it is allowed.**
///
/// The span is measured round `RubevySet::Tick` — `tick_scripts`, then `drain_commands` and
/// `apply_component_writes`, which are the frame's scripts and everything they asked for. It is
/// wall time, not CPU: it is the number the budget is *about* (`ScriptWorld::frame_time`, 8 ms),
/// so it is the number to show beside it.
///
/// It was the garden's (`garden/src/window.rs`, G4). G9 moved it here because the VM panel shows
/// it and both games have a VM panel; [`VmInspectorPlugin`] measures it, and a game with no
/// window (the garden's `--headless`, where the figure is printed) adds the two systems itself.
#[derive(Resource, Default)]
pub struct VmClock {
    started: Option<bevy::platform::time::Instant>,
    /// milliseconds the last frame's scripts took
    pub spent_ms: f32,
    /// smoothed, because a single frame's figure flickers faster than it can be read
    pub mean_ms: f32,
    /// what `ScriptWorld::frame_time` allows, in milliseconds
    pub budget_ms: f32,
}

pub fn vm_clock_start(mut clock: ResMut<VmClock>) {
    clock.started = Some(bevy::platform::time::Instant::now());
}

pub fn vm_clock_end(mut clock: ResMut<VmClock>, world: Res<ScriptWorld>) {
    let Some(started) = clock.started.take() else { return };
    let spent = started.elapsed().as_secs_f32() * 1000.0;
    clock.spent_ms = spent;
    // a fifth of the new reading: about a sixth of a second of memory at 60 Hz
    clock.mean_ms = clock.mean_ms * 0.8 + spent * 0.2;
    clock.budget_ms = world.frame_time.map(|d| d.as_secs_f32() * 1000.0).unwrap_or(0.0);
}

pub struct VmInspectorPlugin;

impl Plugin for VmInspectorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<VmInspector>()
            .init_resource::<VmClock>()
            .add_systems(Update, vm_clock_start.before(RubevySet::Tick))
            .add_systems(Update, vm_clock_end.after(RubevySet::Tick).before(RubevySet::Answer))
            .add_systems(EguiPrimaryContextPass, draw_inspector);
    }
}

const FONT: f32 = 12.0;
/// How tall the window stands by default, and how much of it the frames and registers get.
const PANEL_HEIGHT: f32 = 440.0;
const FRAMES_HEIGHT: f32 = 148.0;
const REGS_HEIGHT: f32 = 150.0;

/// A rendered value, short enough for a column. The VM renders an object of a robot's own class
/// as `#<#<Class:0x…>:0x… ivars=3>`, which says nothing worth this much room.
fn short(text: &str) -> String {
    const LIMIT: usize = 52;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let kept: String = text.chars().take(LIMIT - 1).collect();
    format!("{kept}…")
}

/// **The panel, read top down.** The name, what it is waiting for and where; the lines of its own
/// file it is standing on; three numbers. Then "details", closed, with the machine in it.
fn draw_inspector(mut contexts: EguiContexts, mut panel: ResMut<VmInspector>, clock: Option<Res<VmClock>>) {
    if !panel.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let amber = egui::Color32::from_rgb(240, 190, 90);
    let pale = egui::Color32::from_rgb(200, 206, 216);
    let title = panel.title.clone();
    let status = panel.status.clone();
    let note = panel.note.clone();
    let heap = panel.heap.clone();
    let frames = panel.frames.clone();
    let waiting = panel.waiting.clone();
    let waiting_at = panel.waiting_at.clone();
    let (paused, instructions, spent, per_frame) =
        (panel.paused, panel.instructions, panel.spent, panel.insn_per_frame());
    let (live, total) = (panel.contexts_live, panel.contexts);
    let vm_ms = clock.as_ref().map(|c| (c.mean_ms, c.budget_ms));

    // the bottom left: the scoreboard has the top left and the editor the right side, and all
    // three can be dragged anywhere
    let bottom = ctx.content_rect().bottom();
    let width = 640.0_f32.min(ctx.content_rect().width() - 16.0);
    egui::Window::new("VM")
        .collapsible(true)
        .resizable(true)
        .default_width(width)
        .default_pos([8.0, (bottom - 8.0 - PANEL_HEIGHT).max(8.0)])
        .show(ctx, |ui| {
            // --- who, and whether the world is moving ------------------------------------
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&title).strong().size(15.0));
                if paused {
                    ui.label(egui::RichText::new("the world is paused (P)").color(amber).strong())
                        .on_hover_text("no script runs and no rule of the game runs: nothing in this panel can change while it is read");
                } else {
                    ui.label(egui::RichText::new("the world is running — P to stop it").weak());
                }
            });

            // --- why it is waiting, which is what the panel is for ------------------------
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(waiting.text())
                    .color(egui::Color32::from_rgb(255, 236, 150))
                    .size(14.0),
            )
            .on_hover_text(waiting.how());
            if let Some(at) = &waiting_at {
                ui.label(
                    egui::RichText::new(format!("on {at}"))
                        .font(egui::FontId::monospace(FONT + 1.0))
                        .color(pale),
                )
                .on_hover_text("the innermost line of this script's own file — which a VM that parks tasks can say and a callback cannot");
            }
            if !note.is_empty() {
                ui.label(egui::RichText::new(&note).color(amber));
            }

            // --- its own frames, and nothing else -----------------------------------------
            ui.add_space(4.0);
            let own: Vec<&FrameRow> = frames.iter().filter(|f| f.own).collect();
            if own.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "nothing of its own is on the stack: every frame is the DSL's or the VM's (see the details)",
                    )
                    .weak(),
                );
            } else {
                for f in &own {
                    ui.label(
                        egui::RichText::new(format!("{:<22} {}", f.at, f.method))
                            .font(egui::FontId::monospace(FONT))
                            .color(pale),
                    )
                    .on_hover_text(format!("in {}", f.class));
                }
            }

            // --- the three numbers ---------------------------------------------------------
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{per_frame} insn/frame · {live} tasks in the VM{}",
                    match vm_ms {
                        Some((ms, budget)) if budget > 0.0 => format!(" · VM {ms:.2} / {budget:.1} ms"),
                        _ => String::new(),
                    }
                ))
                .weak(),
            )
            .on_hover_text("what this script spends a frame; how many tasks the VM is running (a behaviour, each of its handlers, the match — one context each); and the wall time this frame's scripts took against what the scheduler allows them");

            // --- and the machine, folded away ----------------------------------------------
            ui.add_space(2.0);
            egui::CollapsingHeader::new("details — registers, the heap, every frame")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{status}   {instructions} insn total, {spent} on the last frame   contexts {live} live of {total}"
                        ))
                        .weak(),
                    );
                    if let Some(h) = &heap {
                        ui.label(
                            egui::RichText::new(format!(
                                "heap: {} live of {} ({} free) · {} allocated since a collection, threshold {} · {} collections, {} live after the last{}{}",
                                h.live, h.len, h.free, h.allocated_since_gc, h.alloc_threshold, h.gc_count, h.live_after_gc,
                                if h.stress { " · stress" } else { "" },
                                if h.disabled { " · gc off" } else { "" },
                            ))
                            .weak(),
                        )
                        .on_hover_text("the collector's own counters: rubevy collects at the scheduler's idle points, so a script that allocates never pauses a frame for it");
                    }
                    ui.separator();
                    let mut follow = panel.follow;
                    if ui
                        .checkbox(&mut follow, "follow the script's own frame")
                        .on_hover_text("a behaviour waiting for a scan stands three frames deep in the DSL; this keeps the registers on the line of its own file")
                        .changed()
                    {
                        panel.follow = follow;
                    }

                    // two lists, each with a fixed height and its own scrollbar: a deep stack must
                    // not push the registers — or the window — off the bottom of the screen
                    ui.label(egui::RichText::new("every frame, innermost first").weak());
                    egui::ScrollArea::vertical().id_salt("frames").auto_shrink([false, false]).max_height(FRAMES_HEIGHT).show(ui, |ui| {
                        for (i, f) in frames.iter().enumerate() {
                            let mark = if panel.selected == i { "▸" } else { " " };
                            let text = egui::RichText::new(format!(
                                "{mark} {:<24} {:<18} pc {:<5}{}",
                                f.at,
                                f.method,
                                f.pc,
                                if f.native_boundary { "  (a native is waiting for this frame)" } else { "" }
                            ))
                            .font(egui::FontId::monospace(FONT))
                            .color(if panel.selected == i {
                                egui::Color32::from_rgb(255, 236, 150)
                            } else if f.own {
                                pale
                            } else {
                                egui::Color32::from_gray(140)
                            });
                            if ui
                                .add(egui::Label::new(text).sense(egui::Sense::click()).wrap_mode(egui::TextWrapMode::Extend))
                                .on_hover_text(format!("in {}", f.class))
                                .clicked()
                            {
                                panel.selected = i;
                                panel.follow = false;
                            }
                        }
                        if frames.is_empty() {
                            ui.label(egui::RichText::new("no frames").weak());
                        }
                    });
                    ui.separator();
                    let Some(frame) = frames.get(panel.selected) else { return };
                    if frame.regs.is_empty() {
                        ui.label(
                            egui::RichText::new(format!(
                                "this frame keeps no registers in the snapshot: only the innermost {REGS_FRAMES} do"
                            ))
                            .weak(),
                        );
                        return;
                    }
                    ui.label(
                        egui::RichText::new(format!("registers of #{} {} — locals are named from the debug info", panel.selected, frame.at))
                            .weak(),
                    );
                    egui::ScrollArea::vertical().id_salt("regs").auto_shrink([false, false]).max_height(REGS_HEIGHT).show(ui, |ui| {
                        egui::Grid::new("regs").num_columns(4).spacing([10.0, 2.0]).striped(true).show(ui, |ui| {
                            for r in &frame.regs {
                                let name = match (&r.name, r.index) {
                                    (Some(n), _) => n.clone(),
                                    (None, 0) => "self".into(),
                                    (None, _) => String::new(),
                                };
                                let color = if r.local || r.index == 0 {
                                    egui::Color32::from_rgb(210, 214, 222)
                                } else {
                                    egui::Color32::from_gray(130)
                                };
                                ui.label(egui::RichText::new(format!("R{}", r.index)).font(egui::FontId::monospace(FONT)).color(egui::Color32::from_gray(120)));
                                ui.label(egui::RichText::new(name).font(egui::FontId::monospace(FONT)).color(color));
                                ui.label(egui::RichText::new(short(&r.class)).font(egui::FontId::monospace(FONT)).color(egui::Color32::from_gray(140)));
                                ui.label(egui::RichText::new(short(&r.text)).font(egui::FontId::monospace(FONT)).color(color))
                                    .on_hover_text(&r.text);
                                ui.end_row();
                            }
                        });
                    });
                });
        });
}

// -------------------------------------------------------------------------------------------
// What the frames of each kind of wait look like, and what `why` makes of them.
//
// The rows below are copied from a running garden — `Vm::snapshot` over every context of a
// headless run, printed frame by frame (`docs/worklog/2026-09-17-garden-G9.md`). Two of the four
// kinds are hard to catch in a live run and easy to be wrong about, so they are written down
// here: an event (the panel follows a behaviour's task, and only a handler's task waits on a
// subscription) and a question asked through `Rubevy::Proxy` (one frame in the life of a pass).
#[cfg(test)]
mod tests {
    use super::*;

    fn frame(method: &str, class: &str, at: &str, own: bool, locals: &[(&str, &str)]) -> FrameRow {
        FrameRow {
            at: at.into(),
            method: method.into(),
            class: class.into(),
            own,
            regs: locals
                .iter()
                .enumerate()
                .map(|(i, (name, text))| RegRow {
                    index: i + 1,
                    name: Some((*name).into()),
                    class: String::new(),
                    text: (*text).into(),
                    local: true,
                })
                .collect(),
            ..FrameRow::default()
        }
    }

    /// A creature parked on `sleep 0.2` at the foot of its own `run` loop: no `pop` anywhere.
    #[test]
    fn a_sleep_is_the_line_that_called_it() {
        let frames = vec![
            frame("run", "#<Class:0x11ac0>", "beetle.rb:125", true, &[("plant", "nil")]),
            frame("loop", "Kernel", "(no debug info)", false, &[]),
            frame("run", "#<Class:0x11ac0>", "beetle.rb:96", true, &[]),
        ];
        assert_eq!(why(&frames), Waiting::Sleep);
    }

    /// `garden.nearest(:Plant)` — the proxy turns the name into a question, and the name and the
    /// arguments are its own locals.
    #[test]
    fn a_question_through_the_proxy_names_itself() {
        let frames = vec![
            frame("pop", "Task::Queue", "(no debug info)", false, &[]),
            frame(
                "method_missing",
                "Rubevy::Proxy",
                "prelude.rb:47",
                false,
                &[("name", ":nearest"), ("args", "[:Plant]"), ("blk", "nil")],
            ),
            frame("run", "#<Class:0x11ac0>", "beetle.rb:118", true, &[]),
        ];
        assert_eq!(why(&frames), Waiting::Ask("nearest(:Plant)".into()));
    }

    /// SabiRuby Battle's DSL spells the question out — `Rubevy.ask("radar", range).pop` — so the
    /// frame behind the `pop` is the method a reader of the robot knows by name.
    #[test]
    fn a_question_from_the_dsl_is_named_by_its_method() {
        let frames = vec![
            frame("pop", "Task::Queue", "(no debug info)", false, &[]),
            frame("radar", "Robot", "prelude.rb:67", false, &[("range", "45.0")]),
            frame("run", "#<Class:0x2a40>", "scout.rb:36", true, &[]),
        ];
        assert_eq!(why(&frames), Waiting::Ask("radar".into()));
    }

    /// A handler's task: `Rubevy.subscribe` extends the queue it hands out, so the `pop` goes
    /// through `Rubevy::Subscription#pop` on its way to `Task::Queue#pop`.
    #[test]
    fn an_event_is_told_by_the_subscription() {
        let frames = vec![
            frame("pop", "Task::Queue", "(no debug info)", false, &[]),
            frame("pop", "Rubevy::Subscription", "(no debug info)", false, &[]),
            frame("(block or top level)", "Object", "prelude.rb:448", false, &[]),
            frame("loop", "Kernel", "(no debug info)", false, &[]),
        ];
        assert_eq!(why(&frames), Waiting::Event);
    }

    #[test]
    fn a_task_with_no_frames_has_nothing_to_say() {
        assert_eq!(why(&[]), Waiting::Nothing);
    }
}
