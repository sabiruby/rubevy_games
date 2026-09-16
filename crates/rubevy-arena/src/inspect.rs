//! The VM inspector: the frames a script's task is standing in, the values in its registers, and
//! what the collector is doing — the same numbers the browser playground draws, in the game's own
//! window ([`VmInspector`]).
//!
//! It is all read through sabiruby's `Vm::snapshot` (`docs/design/inspect.md` there), which never
//! calls Ruby: a value is rendered in Rust, so looking at a robot cannot move it. Registers are
//! named from the LVAR section, which is why the panel can say `target` rather than `R3` — the
//! game compiles its scripts with debug info for exactly this reason.
//!
//! Which of the VM's contexts a task stands in is `Vm::task_context(task)`, in sabiruby 0.5.0.
//! Until it was there the panel read the index out of the way the VM renders a task
//! (`#<Task 12 ctx=3>`), which is a debugging format and not an API; that parser is gone.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use rubevy::ScriptWorld;
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

/// What the panel shows. A game fills it with [`VmInspector::fill`] on the frames it is open and
/// draws itself through [`VmInspectorPlugin`].
#[derive(Resource, Default)]
pub struct VmInspector {
    pub open: bool,
    /// The scripts are stopped: the plugin's budget is 0, so no task runs and the snapshot
    /// stands still while it is read.
    pub paused: bool,
    /// Whose task this is (`3 blue/scout`).
    pub title: String,
    /// The context it stands in, and what state that context is in.
    pub status: String,
    pub frames: Vec<FrameRow>,
    /// Which frame's registers are shown.
    pub selected: usize,
    /// While true, the panel picks the innermost frame in the script's own file every time it is
    /// filled — the brain usually stands several frames deep in the DSL, and the DSL's locals are
    /// not what a reader of the robot came for. Clicking a row turns it off.
    pub follow: bool,
    pub heap: Option<HeapView>,
    /// Instructions this task has run since it started, and what it spent on the last frame.
    pub instructions: u64,
    pub spent: u64,
    /// Contexts the VM holds, and how many of them have not ended. A task left parked is one of
    /// these for as long as the VM lives, which is how a leak shows.
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

        let Some(ctx) = vm.task_context(task) else {
            self.frames.clear();
            self.status.clear();
            self.note = "this script has no context: it has run to its end".into();
            return;
        };
        let Some(context) = snapshot.contexts.iter().find(|c| c.index == ctx) else {
            self.frames.clear();
            self.status.clear();
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

    /// Open, following the script's own frame: what a game inserts as the resource.
    pub fn following() -> VmInspector {
        VmInspector { open: true, follow: true, ..VmInspector::default() }
    }

    pub fn clear(&mut self, note: impl Into<String>) {
        self.frames.clear();
        self.status.clear();
        self.heap = None;
        self.note = note.into();
    }

    /// The whole of it in a few lines, for a log where there is no window (`--headless`).
    pub fn log_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let heap = match &self.heap {
            Some(h) => format!(
                "heap live {} of {} ({} free, {} since gc of {}), gc {} times, {} live after the last",
                h.live, h.len, h.free, h.allocated_since_gc, h.alloc_threshold, h.gc_count, h.live_after_gc
            ),
            None => "heap ?".to_string(),
        };
        out.push(format!(
            "vm: {} — {} — {} insn — contexts {} live of {} — {heap}",
            self.title, self.status, self.instructions, self.contexts_live, self.contexts
        ));
        if !self.note.is_empty() {
            out.push(format!("vm: {}", self.note));
        }
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

pub struct VmInspectorPlugin;

impl Plugin for VmInspectorPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        app.init_resource::<VmInspector>()
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

fn draw_inspector(mut contexts: EguiContexts, mut panel: ResMut<VmInspector>) {
    if !panel.open {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let amber = egui::Color32::from_rgb(240, 190, 90);
    let title = panel.title.clone();
    let status = panel.status.clone();
    let note = panel.note.clone();
    let heap = panel.heap.clone();
    let frames = panel.frames.clone();
    let (paused, instructions, spent) = (panel.paused, panel.instructions, panel.spent);
    let (live, total) = (panel.contexts_live, panel.contexts);

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
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&title).strong());
                if paused {
                    ui.label(egui::RichText::new("paused (P)").color(amber).strong())
                        .on_hover_text("the scripts get no instructions this frame; the game keeps drawing");
                } else {
                    ui.label(egui::RichText::new("running — P to pause").weak());
                }
                let mut follow = panel.follow;
                if ui
                    .checkbox(&mut follow, "follow the robot's own frame")
                    .on_hover_text("a behaviour waiting for a scan stands three frames deep in the DSL; this keeps the panel on the line of its own file")
                    .changed()
                {
                    panel.follow = follow;
                }
            });
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
            if !note.is_empty() {
                ui.label(egui::RichText::new(&note).color(amber));
            }
            ui.separator();

            // two lists, each with a fixed height and its own scrollbar: a deep stack must not
            // push the registers — or the window — off the bottom of the screen
            ui.label(egui::RichText::new("frames, innermost first").weak());
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
                    .color(if panel.selected == i { egui::Color32::from_rgb(255, 236, 150) } else { egui::Color32::from_rgb(200, 206, 216) });
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
}
