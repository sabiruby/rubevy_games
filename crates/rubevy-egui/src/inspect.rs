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

// ---------------------------------------------------------------------------------------------
// The numbers (S5b-1). Every one of them is a field of `InspectStyle`, which sits in
// `VmInspector` — the resource whose method `fill` is, and which a headless game builds by hand
// without any of this crate's plugins being in the app. The `const`s are the names of the
// defaults and nothing else; each says where its value came from, and most of them say "unknown".
// ---------------------------------------------------------------------------------------------

/// How many of the innermost frames of a context carry their registers. A brain waiting for the
/// game stands about three frames deep in the DSL, so six reaches its own code as well.
///
/// *Reason only*: the sentence above is the record. Nobody counted the DSL's frames.
pub const REGS_FRAMES: usize = 6;

/// The size of the letters in the panel's monospace lines. **Source unknown**.
pub const FONT: f32 = 12.0;

/// How tall the window stands by default, and how much of it the frames and the registers get.
/// **Source unknown** — all three arrived with the panel.
pub const PANEL_HEIGHT: f32 = 440.0;
pub const FRAMES_HEIGHT: f32 = 148.0;
pub const REGS_HEIGHT: f32 = 150.0;

/// How wide it stands, and how much of the window is left clear when the window is narrower than
/// that. **Source unknown** for both.
pub const WIDTH: f32 = 640.0;
pub const WIDTH_MARGIN: f32 = 16.0;

/// How long a rendered value may be before it is cut, in characters. **Source unknown**; what is
/// written down is only that a class rendered as `#<#<Class:0x…>:0x… ivars=3>` is not worth the
/// room.
pub const VALUE_CHARS: usize = 52;

/// How long a name from the frames may be before it is cut ( `nearest(:Plant)`, `:Hunger`).
/// **Source unknown**.
pub const NAME_CHARS: usize = 40;

/// How many frames a headless log prints of a stack. **Source unknown**.
pub const LOG_FRAMES: usize = 4;

/// How much of a new reading goes into the instructions-per-frame figure the panel watches for
/// itself. **Source unknown**; what is written down is why it is smoothed at all — a parked task
/// spends nothing on most frames, and 0, 0, 340, 0 is not a number anybody can read.
pub const INSN_SMOOTHING: f32 = 0.05;

// ---------------------------------------------------------------------------------------------
// The colours (S5b-2). They were written into `draw_inspector` in nine places until 2026-09-21,
// when the S5b-1 inventory turned out to have caught the editor's colours next door and missed
// these (`docs/numbers.md` §1.4). **One `const` per distinct colour, and one field per `const`**:
// where the same value was written twice it is one setting now, and the rustdoc says which places
// it paints. None of them has a recorded source.
// ---------------------------------------------------------------------------------------------

/// **The world is paused**, and the panel's own note under the waiting line.
///
/// **Source unknown**. It is the same value as [`crate::editor::AMBER`] and is **not read from
/// it**: nothing written down says whether the two panels are meant to be one colour, and
/// inventing that intent here would make a game that changed the editor's `* edited` mark quietly
/// repaint the VM panel too (`docs/numbers.md` §7).
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(240, 190, 90);

/// The script's own lines: the line it is waiting on, and its own frames above the numbers.
/// **Source unknown**.
pub const PALE: egui::Color32 = egui::Color32::from_rgb(200, 206, 216);

/// **What to read first**: the sentence saying why the script is waiting, and — in the details —
/// the frame whose registers are shown. One colour because it is one job; it was written twice.
/// **Source unknown**.
pub const LIT: egui::Color32 = egui::Color32::from_rgb(255, 236, 150);

/// The grey of what is there but not the point: a frame that is not the script's own, and the
/// class of a register's value. **Source unknown**; it was written twice.
pub const DIM: egui::Color32 = egui::Color32::from_gray(140);

/// A named local of the selected frame — the row a reader is there for. **Source unknown**; it is
/// the same value as the editor's body colour ([`crate::editor::KINDS`]`[0]`) and, for the reason
/// [`AMBER`] gives, not read from it.
pub const REG_NAME: egui::Color32 = egui::Color32::from_rgb(210, 214, 222);

/// A register the compiler needed rather than one the author named. **Source unknown**.
pub const REG_TEMP: egui::Color32 = egui::Color32::from_gray(130);

/// The `R3` column down the left of the registers. **Source unknown**.
pub const REG_INDEX: egui::Color32 = egui::Color32::from_gray(120);

/// **Every number the VM panel has.** It is a field of [`VmInspector`] rather than a resource of
/// its own because [`VmInspector::fill`] is a method — a game with no window builds a panel by
/// hand to print a log with (SabiRuby Battle's `--headless`), and none of this crate's plugins is
/// in that app to have inserted a second resource.
///
/// A game hands its own in at startup, writes into it while it runs, or lets a player change the
/// sizes through the `key=value` store ([`InspectStyle::read_from`]).
#[derive(Debug, Clone, PartialEq)]
pub struct InspectStyle {
    /// [`REGS_FRAMES`] — and the one number here that costs something: `Vm::snapshot` carries
    /// this many frames' registers, every frame the panel is open.
    pub regs_frames: usize,
    /// [`FONT`]
    pub font: f32,
    /// [`PANEL_HEIGHT`]
    pub panel_height: f32,
    /// [`FRAMES_HEIGHT`]
    pub frames_height: f32,
    /// [`REGS_HEIGHT`]
    pub regs_height: f32,
    /// [`WIDTH`]
    pub width: f32,
    /// [`WIDTH_MARGIN`]
    pub width_margin: f32,
    /// [`VALUE_CHARS`]
    pub value_chars: usize,
    /// [`NAME_CHARS`]
    pub name_chars: usize,
    /// [`LOG_FRAMES`]
    pub log_frames: usize,
    /// [`INSN_SMOOTHING`]
    pub insn_smoothing: f32,
    /// [`AMBER`]
    pub amber: egui::Color32,
    /// [`PALE`]
    pub pale: egui::Color32,
    /// [`LIT`]
    pub lit: egui::Color32,
    /// [`DIM`]
    pub dim: egui::Color32,
    /// [`REG_NAME`]
    pub reg_name: egui::Color32,
    /// [`REG_TEMP`]
    pub reg_temp: egui::Color32,
    /// [`REG_INDEX`]
    pub reg_index: egui::Color32,
}

impl Default for InspectStyle {
    fn default() -> Self {
        InspectStyle {
            regs_frames: REGS_FRAMES,
            font: FONT,
            panel_height: PANEL_HEIGHT,
            frames_height: FRAMES_HEIGHT,
            regs_height: REGS_HEIGHT,
            width: WIDTH,
            width_margin: WIDTH_MARGIN,
            value_chars: VALUE_CHARS,
            name_chars: NAME_CHARS,
            log_frames: LOG_FRAMES,
            insn_smoothing: INSN_SMOOTHING,
            amber: AMBER,
            pale: PALE,
            lit: LIT,
            dim: DIM,
            reg_name: REG_NAME,
            reg_temp: REG_TEMP,
            reg_index: REG_INDEX,
        }
    }
}

impl InspectStyle {
    /// **What a player left in a `key=value` store**, for the numbers a person can sensibly be
    /// asked about: how big the panel is, and how deep into the VM it looks. The store is the
    /// game's (`games_shell::Settings`), which this crate does not depend on, so what comes in is
    /// a function that answers a key.
    ///
    /// | key | field |
    /// |---|---|
    /// | `vm_font` | [`InspectStyle::font`] |
    /// | `vm_panel_height` | [`InspectStyle::panel_height`] |
    /// | `vm_frames_height` | [`InspectStyle::frames_height`] |
    /// | `vm_regs_height` | [`InspectStyle::regs_height`] |
    /// | `vm_width` | [`InspectStyle::width`] |
    /// | `vm_regs_frames` | [`InspectStyle::regs_frames`] |
    /// | `vm_value_chars` | [`InspectStyle::value_chars`] |
    ///
    /// A key that is not there leaves the field alone. A count that is written as a fraction is
    /// truncated, and one written as a negative number is read as 0 — which for `vm_regs_frames`
    /// means a snapshot with no registers in it, not a panic.
    ///
    /// **The seven colours are not in the store** (S5b-2), for the reason the editor's nine are
    /// not: a colour in a text file needs a parser. A game may still write them.
    pub fn read_from(&mut self, number: impl Fn(&str) -> Option<f32>) {
        let take = |key: &str, slot: &mut f32| {
            if let Some(value) = number(key) {
                *slot = value;
            }
        };
        let count = |key: &str, slot: &mut usize| {
            if let Some(value) = number(key) {
                *slot = value.max(0.0) as usize;
            }
        };
        take("vm_font", &mut self.font);
        take("vm_panel_height", &mut self.panel_height);
        take("vm_frames_height", &mut self.frames_height);
        take("vm_regs_height", &mut self.regs_height);
        take("vm_width", &mut self.width);
        count("vm_regs_frames", &mut self.regs_frames);
        count("vm_value_chars", &mut self.value_chars);
    }
}

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
///   `Rubevy::Entity` is a component read, and finding one is now news ([`Waiting::Read`]). Until
///   2026-09-17 it was the commonest reason in the Garden — every `me[:Hunger]` parked here for a
///   frame. Now rubevy answers a read **inside the tick that asked it**: the tick runs the ready
///   tasks, answers the reads they parked on, and runs them again (rubevy `docs/host-api.md`, "A
///   read costs no frame"), so a read never outlives the tick it was made in and this panel —
///   which looks from outside the tick — should never find one. The exception is the whole of
///   what it means now: a tick that spends its instruction budget or its `frame_time` stops with
///   that round's reads unanswered, and the next tick answers them before it runs anything else.
///   So a task standing here is a task whose VM ran out of frame, which is worth a sentence of
///   its own rather than being filed under "waiting for the game".
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
    /// Parked on a component read that the tick ran out of frame before answering — the one way
    /// a read can still be seen from outside a tick (see above). The string is the component as
    /// the script wrote it (`:Hunger`).
    ///
    /// It is not an error and nothing is lost: the next tick answers these before it runs
    /// anything, so the task is a frame late rather than stuck. What it says is that the VM has
    /// more to do in a frame than its budget allows.
    Read(String),
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
            Waiting::Read(name) if name.is_empty() => {
                "waiting for a component read the tick ran out of budget before answering".into()
            }
            // `name` is the symbol as the script wrote it (`:Hunger`), so the brackets round it
            // are the line the reader will find in their own file: `me[:Hunger]`
            Waiting::Read(name) => {
                format!("waiting for a component read the tick ran out of budget before answering — `[{name}]`")
            }
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
            Waiting::Read(_) => "the innermost frame is `Task::Queue#pop` under `Rubevy::Entity#get` — a component read, which rubevy answers itself and normally answers *inside* the tick that asked it. Seeing one from out here means that tick stopped first, on its instruction budget or its `frame_time`, with this round's reads still out; the next tick answers them before it runs anything else. The game never sees the question either way",
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
    /// **What the prelude in front of the author's file is called**, for the frames that fall
    /// inside it. `None` is `prelude.rb`, which is what a creature and a robot have; the garden
    /// sets it to `world_prelude.rb` when the panel is pointed at the world's VM, whose program
    /// is a different file in front of a different file (2026-09-18).
    pub prelude_file: Option<String>,
    /// **How big the panel is, how deep into the VM it looks, and where it cuts a long line**
    /// ([`InspectStyle`]). It travels with the panel rather than in a resource of its own
    /// because [`VmInspector::fill`] needs it and a headless game calls that on a `VmInspector`
    /// it made by hand.
    pub style: InspectStyle,
}

impl VmInspector {
    /// Reads one script's task out of the VM. `prelude_lines` is what the game put in front of
    /// the author's file, so the panel can report the author's own line numbers.
    ///
    /// **Whose VM it is, is the caller's** (2026-09-18). rubevy's `ScriptWorld<M>` carries a
    /// marker so that an app can hold two of them — the garden holds the creatures' and the
    /// world's (`ScriptWorld<World>`, `docs/host-api.md`, "Two VMs in one app") — and this used
    /// to name the first by its default, which is why the garden's `F2` could show a beetle and
    /// not the rules it lives under.
    ///
    /// The parameter is on the **method** and not on the panel. A `VmInspector<M>` would be a
    /// resource per VM, a `VmInspectorPlugin<M>` per VM, and two egui windows both called "VM"
    /// (egui takes a window's title as its id), for a panel that shows one thing at a time by
    /// construction: `F2` opens *the* panel, and what it is looking at is a question the game
    /// answers every frame. Nothing in what is read here is about the marker — a snapshot, a
    /// task's frames, a heap — so the method is where the marker belongs. A game with one VM
    /// writes `fill(&world, …)` exactly as before and never spells it.
    pub fn fill<M: Send + Sync + 'static>(
        &mut self,
        world: &ScriptWorld<M>,
        task: ObjId,
        title: String,
        prelude_lines: u32,
    ) {
        self.title = title;
        self.note.clear();
        let vm = &world.vm;
        let snapshot = vm.snapshot(self.style.regs_frames);
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
        let keep = 1.0 - self.style.insn_smoothing;
        self.watched_per_frame = self.watched_per_frame * keep + this_frame * self.style.insn_smoothing;

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
        let prelude_file = self.prelude_file.clone().unwrap_or_else(|| "prelude.rb".into());
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
                            format!("{prelude_file}:{line}")
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
        self.waiting = why(&self.frames, self.style.name_chars);
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
        for (i, f) in self.frames.iter().enumerate().take(self.style.log_frames) {
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
fn why(frames: &[FrameRow], name_chars: usize) -> Waiting {
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
        // `e[:Hunger]` → `Rubevy::Entity#[]` → `#get` → `Rubevy.ask("component.get", …).pop`.
        // Answered inside the tick, so a task is only found here when the tick ran out of frame.
        if f.class == "Rubevy::Entity" {
            return Waiting::Read(f.local("name").map(|t| clean(t, name_chars)).unwrap_or_default());
        }
        // `garden.nearest(:Plant)` → `Rubevy::Proxy#method_missing` → `Rubevy.ask(…).pop`
        if f.class.contains("Rubevy::Proxy") {
            // `name` is the Symbol `method_missing` was called with, and it renders as `:nearest`;
            // what the author wrote is `nearest`
            let name = f
                .local("name")
                .map(|t| clean(t, name_chars))
                .map(|n| n.trim_start_matches(':').to_string())
                .unwrap_or_default();
            let args = f.local("args").map(|t| clean(t, name_chars)).unwrap_or_default();
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
/// nothing longer than a column ([`InspectStyle::name_chars`]).
fn clean(text: &str, chars: usize) -> String {
    let text = text.trim_matches('"');
    if text.chars().count() <= chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(chars.saturating_sub(1)).collect();
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
#[derive(Resource)]
pub struct VmClock {
    started: Option<bevy::platform::time::Instant>,
    /// milliseconds the last frame's scripts took
    pub spent_ms: f32,
    /// smoothed, because a single frame's figure flickers faster than it can be read
    pub mean_ms: f32,
    /// what `ScriptWorld::frame_time` allows, in milliseconds
    pub budget_ms: f32,
    /// **How much of a new reading goes into [`VmClock::mean_ms`]** ([`CLOCK_SMOOTHING`]). A game
    /// that wants the figure to settle faster or slower writes it.
    pub smoothing: f32,
}

/// A fifth of each new reading: about a sixth of a second of memory at 60 Hz. *Reason only* —
/// the sentence is the record, and the fifth itself has **no recorded source**.
pub const CLOCK_SMOOTHING: f32 = 0.2;

impl Default for VmClock {
    fn default() -> Self {
        VmClock {
            started: None,
            spent_ms: 0.0,
            mean_ms: 0.0,
            budget_ms: 0.0,
            smoothing: CLOCK_SMOOTHING,
        }
    }
}

/// **Where the clock's two systems stand**, so that a game with a *second* VM can keep the two
/// ticks apart.
///
/// [`VmClock`] measures `ScriptWorld` — the app's first VM — by starting before
/// `RubevySet::Tick` and stopping after it. With one VM that is the whole of the frame's Ruby and
/// nothing can land in between. With two, the other VM's tick is ordered against `RubevySet::Tick`
/// and **not** against these two systems, so Bevy is free to put it inside the window and the
/// number quietly becomes the sum of both ticks measured against one VM's `frame_time` — which is
/// what the garden saw when it grew a world VM (rubevy_games,
/// `docs/worklog/2026-09-17-garden-world.md`): 0.9 ms became 2.1.
///
/// Naming the pair is what lets that game say `RubevySet::<Other>::tick().before(VmClockSet)`. A
/// game with one VM need never write it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmClockSet;

pub fn vm_clock_start(mut clock: ResMut<VmClock>) {
    clock.started = Some(bevy::platform::time::Instant::now());
}

pub fn vm_clock_end(mut clock: ResMut<VmClock>, world: Res<ScriptWorld>) {
    let Some(started) = clock.started.take() else { return };
    let spent = started.elapsed().as_secs_f32() * 1000.0;
    clock.spent_ms = spent;
    // a fifth of the new reading: about a sixth of a second of memory at 60 Hz
    clock.mean_ms = clock.mean_ms * (1.0 - clock.smoothing) + spent * clock.smoothing;
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
            .add_systems(Update, vm_clock_start.before(RubevySet::Tick).in_set(VmClockSet))
            .add_systems(
                Update,
                vm_clock_end.after(RubevySet::Tick).before(RubevySet::Answer).in_set(VmClockSet),
            )
            .add_systems(EguiPrimaryContextPass, draw_inspector);
    }
}

/// A rendered value, short enough for a column. The VM renders an object of a robot's own class
/// as `#<#<Class:0x…>:0x… ivars=3>`, which says nothing worth this much room.
fn short(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(chars.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// **The panel, read top down.** The name, what it is waiting for and where; the lines of its own
/// file it is standing on; three numbers. Then "details", closed, with the machine in it.
fn draw_inspector(mut contexts: EguiContexts, mut panel: ResMut<VmInspector>, clock: Option<Res<VmClock>>) {
    if !panel.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    // S5b-2: the panel's colours are `InspectStyle`'s, as its sizes have been since S5b-1
    let style = panel.style.clone();
    let (amber, pale) = (style.amber, style.pale);
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
    let font = style.font;
    let width = style.width.min(ctx.content_rect().width() - style.width_margin);
    egui::Window::new("VM")
        .collapsible(true)
        .resizable(true)
        .default_width(width)
        .default_pos([8.0, (bottom - 8.0 - style.panel_height).max(8.0)])
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
                    .color(style.lit)
                    .size(14.0),
            )
            .on_hover_text(waiting.how());
            if let Some(at) = &waiting_at {
                ui.label(
                    egui::RichText::new(format!("on {at}"))
                        .font(egui::FontId::monospace(font + 1.0))
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
                            .font(egui::FontId::monospace(font))
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
                    egui::ScrollArea::vertical().id_salt("frames").auto_shrink([false, false]).max_height(style.frames_height).show(ui, |ui| {
                        for (i, f) in frames.iter().enumerate() {
                            let mark = if panel.selected == i { "▸" } else { " " };
                            let text = egui::RichText::new(format!(
                                "{mark} {:<24} {:<18} pc {:<5}{}",
                                f.at,
                                f.method,
                                f.pc,
                                if f.native_boundary { "  (a native is waiting for this frame)" } else { "" }
                            ))
                            .font(egui::FontId::monospace(font))
                            .color(if panel.selected == i {
                                style.lit
                            } else if f.own {
                                pale
                            } else {
                                style.dim
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
                                "this frame keeps no registers in the snapshot: only the innermost {} do",
                                style.regs_frames
                            ))
                            .weak(),
                        );
                        return;
                    }
                    ui.label(
                        egui::RichText::new(format!("registers of #{} {} — locals are named from the debug info", panel.selected, frame.at))
                            .weak(),
                    );
                    egui::ScrollArea::vertical().id_salt("regs").auto_shrink([false, false]).max_height(style.regs_height).show(ui, |ui| {
                        egui::Grid::new("regs").num_columns(4).spacing([10.0, 2.0]).striped(true).show(ui, |ui| {
                            for r in &frame.regs {
                                let name = match (&r.name, r.index) {
                                    (Some(n), _) => n.clone(),
                                    (None, 0) => "self".into(),
                                    (None, _) => String::new(),
                                };
                                let color = if r.local || r.index == 0 {
                                    style.reg_name
                                } else {
                                    style.reg_temp
                                };
                                ui.label(egui::RichText::new(format!("R{}", r.index)).font(egui::FontId::monospace(font)).color(style.reg_index));
                                ui.label(egui::RichText::new(name).font(egui::FontId::monospace(font)).color(color));
                                ui.label(egui::RichText::new(short(&r.class, style.value_chars)).font(egui::FontId::monospace(font)).color(style.dim));
                                ui.label(egui::RichText::new(short(&r.text, style.value_chars)).font(egui::FontId::monospace(font)).color(color))
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
        assert_eq!(why(&frames, NAME_CHARS), Waiting::Sleep);
    }

    /// `me[:Hunger]` — `Rubevy::Entity#[]`, `#get`, `Rubevy.ask("component.get", …).pop`.
    ///
    /// Since 2026-09-17 a read is answered inside the tick that asked it, so this stack is only
    /// seen when a tick ran out of its budget with the read still out (`Waiting::Read`). The
    /// shape is the one it always was, and it is held here so that the panel keeps a sentence for
    /// it rather than filing it under "waiting for the game".
    #[test]
    fn a_component_read_left_over_names_the_component() {
        let frames = vec![
            frame("pop", "Task::Queue", "(no debug info)", false, &[]),
            frame("get", "Rubevy::Entity", "(no debug info)", false, &[("name", ":Hunger")]),
            frame("[]", "Rubevy::Entity", "(no debug info)", false, &[]),
            frame("hunger", "Creature", "prelude.rb:138", false, &[]),
        ];
        assert_eq!(why(&frames, NAME_CHARS), Waiting::Read(":Hunger".into()));
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
        assert_eq!(why(&frames, NAME_CHARS), Waiting::Ask("nearest(:Plant)".into()));
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
        assert_eq!(why(&frames, NAME_CHARS), Waiting::Ask("radar".into()));
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
        assert_eq!(why(&frames, NAME_CHARS), Waiting::Event);
    }

    #[test]
    fn a_task_with_no_frames_has_nothing_to_say() {
        assert_eq!(why(&[], NAME_CHARS), Waiting::Nothing);
    }

    /// **A number in the store changes the panel and leaves the rest alone** (S5b-1), and a
    /// count written as a fraction or a negative is read as a count all the same.
    #[test]
    fn the_store_changes_a_number_and_leaves_the_rest() {
        let mut style = InspectStyle::default();
        style.read_from(|key| match key {
            "vm_width" => Some(900.0),
            "vm_regs_frames" => Some(2.7),
            "vm_value_chars" => Some(-4.0),
            _ => None,
        });
        assert_eq!(style.width, 900.0);
        assert_eq!(style.regs_frames, 2, "a fraction of a frame is the frames below it");
        assert_eq!(style.value_chars, 0, "and below zero is nothing, not a panic");
        assert_eq!(style.font, FONT, "a key nobody wrote leaves the default alone");

        let mut untouched = InspectStyle::default();
        untouched.read_from(|_| None);
        assert_eq!(untouched, InspectStyle::default());
    }

    /// **The panel's seven colours are settings, and none of them is in the store** (S5b-2).
    ///
    /// They were written into `draw_inspector` in nine places — two values twice — until the
    /// S5b-1 inventory was found to have caught the editor's colours next door and missed these.
    /// What this holds is that a game can write one, that reading a store does not touch any of
    /// them, and that where the same value was written twice there is now one setting rather than
    /// two: painting `dim` green must reach both the foreign frames and the class column.
    #[test]
    fn the_colours_are_settings_and_not_in_the_store() {
        let mut style = InspectStyle::default();
        style.read_from(|_| Some(3.0));
        assert_eq!(style.amber, AMBER, "a store cannot repaint the panel");
        assert_eq!(style.lit, LIT);

        let green = egui::Color32::from_rgb(0, 255, 0);
        let mine = InspectStyle { dim: green, ..InspectStyle::default() };
        assert_eq!(mine.dim, green);
        assert_ne!(mine, InspectStyle::default());

        // the two that were written twice are one setting each
        assert_eq!(LIT, egui::Color32::from_rgb(255, 236, 150), "why it waits, and the shown frame");
        assert_eq!(DIM, egui::Color32::from_gray(140), "a foreign frame, and a value's class");
        // and the one that is a copy of the editor's is deliberately *not* read from it
        assert_eq!(AMBER, crate::editor::AMBER, "the same value today…");
        assert_eq!(REG_NAME, crate::editor::KINDS[0], "…and so is this one");
    }

    /// The two lengths a long line is cut at are the panel's own settings now: what a name is cut
    /// to is read by [`why`], and what a rendered value is cut to by the register list.
    #[test]
    fn a_long_name_is_cut_where_the_setting_says() {
        let long = ":".to_string() + &"a".repeat(80);
        let frames = vec![
            frame("pop", "Task::Queue", "(no debug info)", false, &[]),
            frame("get", "Rubevy::Entity", "(no debug info)", false, &[("name", &long)]),
        ];
        // the default: forty characters, the last of them an ellipsis
        let Waiting::Read(cut) = why(&frames, NAME_CHARS) else { panic!("not a read") };
        assert_eq!(cut.chars().count(), NAME_CHARS);
        assert!(cut.ends_with('…'));
        // and a game that asks for ten gets ten
        let Waiting::Read(shorter) = why(&frames, 10) else { panic!("not a read") };
        assert_eq!(shorter.chars().count(), 10);
        // what is short enough is not touched at either length
        assert_eq!(short("scout", VALUE_CHARS), "scout");
        assert_eq!(short("scout", 3).chars().count(), 3);
    }
}
