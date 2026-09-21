//! **Everything a window has** — the editor, the VM panel, the HUD, the guide, and `P`.
//!
//! F3 had one panel in here and a line of `bevy_ui` over the map. F5 is the rest of it, and the
//! shape is the two other games': `rubevy_egui` for the editor and the VM panel, `games_shell`
//! for the guide and for reading every panel's size out of `factory.settings.txt`.
//!
//! # Three files, one editor
//!
//! The editor's buttons are the three Ruby files this game is made of, and they are three
//! different kinds of thing:
//!
//! | | what Apply does | what Save writes |
//! |---|---|---|
//! | **the inserter** | hands this one arm, or every arm, a new script (`replace_script`) | `ruby/inserter.rb` |
//! | **`control.rb`** | swaps the control stage's script; the goal starts again from zero (F4) | `ruby/control.rb` |
//! | **`data.rb`** | **reads the declarations again and may rebuild the world** — see below | `ruby/data.rb` |
//!
//! **Nothing here writes a file except Save** ([`EditorAction::Save`]), which is F3's rule kept:
//! Apply is for trying something, and the file on disk is untouched until Ctrl+S. In a browser
//! "the file" is a `localStorage` key (`crate::platform`), so a page keeps what a player wrote.
//!
//! # `data.rb` while the game runs
//!
//! This is the point of the stage, and it is what makes the size of the map a thing a player can
//! change in a page while playing (F3a made it a declaration; this makes it a thing you can turn).
//! Three outcomes, and the first two leave the factory exactly as it was:
//!
//! 1. **It will not read.** The line it is wrong on goes in the panel and in the log, and the
//!    world is not touched at all — the tables in use are still the ones that built it.
//! 2. **It reads, and the world still fits it.** The tables and the numbers are swapped and
//!    nothing is rebuilt: a belt that was carrying goes on carrying, at the new speed.
//! 3. **It reads, and the world does not fit it** — the map is a different size, or the ore, or
//!    the items and machines are not the same list (every tile and every item in the world is a
//!    number into those). The world has to be laid out again, **which loses what was built**, so
//!    the panel says what would change and asks for the Apply again. A player does not lose a
//!    factory by pressing a button once.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use rubevy::ScriptWorld;
use rubevy_egui::{Editor, EditorAction, EditorChoice, VmInspector};

use crate::belts::{Lanes, Rules};
use crate::data::Data;
use crate::grid::{Grid, What};
use crate::inserters::{Crew, Prelude, SCRIPT_FILE};
use crate::platform;

// ---------------------------------------------------------------------------------------------
// Which of the three the editor is on
// ---------------------------------------------------------------------------------------------

/// **What the editor is showing.** The three choices along the top of the panel, and the one the
/// game is following.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub enum Watched {
    /// One arm, by its tile. There may be a thousand of them, so what chooses one is the map and
    /// not a button.
    Arm(usize),
    /// `ruby/data.rb`
    Data,
    /// `ruby/control.rb`
    Control,
    /// Nothing yet: no arm has been clicked and no file button pressed.
    #[default]
    Nothing,
}

/// The ids of the three buttons, handed back in `Editor::picked`. An arm's own key is its tile
/// with [`ARM`] added, so that tile 1 and `control.rb` are not the same key to the editor's
/// drafts (`Editor::show`).
const DATA: u64 = 0;
const CONTROL: u64 = 1;
const ARM: u64 = 2;

impl Watched {
    fn of_key(key: u64) -> Watched {
        match key {
            DATA => Watched::Data,
            CONTROL => Watched::Control,
            other => Watched::Arm((other - ARM) as usize),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// What opens the panel
// ---------------------------------------------------------------------------------------------

/// **A click on a tile that already has an inserter opens it**, and so does building a new one.
///
/// It reads the same [`crate::build::Order`] the building reads, and it is ordered **before** the
/// building so that "was there an inserter here already?" is answered about the grid as the
/// player saw it. An order that builds one is an order that opens the one it built; an order that
/// builds anything else, or takes something away, closes nothing — a player laying belts past an
/// open panel is not asking for it to shut.
pub fn follow_the_orders(
    mut orders: MessageReader<crate::build::Order>,
    grid: Res<Grid>,
    mut watched: ResMut<Watched>,
    mut editor: ResMut<Editor>,
) {
    // this one may take the panel as a parameter of its own: nothing else in it does
    for order in orders.read() {
        if order.what != Some(What::Inserter) {
            continue;
        }
        if !grid.holds(order.at) {
            continue;
        }
        *watched = Watched::Arm(grid.index(order.at));
        editor.open = true;
    }
}

/// `F1` opens and shuts the editor, `F2` the VM panel, and `P` stops the world — the two other
/// games' keys, unchanged, because a player who has seen one of them has learned these.
///
/// **None of them is read while the caret is in a text box.** `P` is a letter and belongs to
/// whatever is being typed into; `F1` and `F2` are not, but egui's own focus does not go away
/// when the pointer leaves the panel, so they are guarded together rather than by kind.
pub fn panel_keys(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut editor: ResMut<Editor>,
    mut panel: ResMut<VmInspector>,
    mut paused: ResMut<Paused>,
    mut scripts: ResMut<ScriptWorld>,
) {
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::F1) {
        editor.open = !editor.open;
    }
    if keys.just_pressed(KeyCode::F2) {
        panel.open = !panel.open;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        paused.turn(&mut scripts, &mut panel);
    }
}

/// **`P`: the whole factory stands still**, and this is the half of it that is not a run
/// condition.
///
/// Two things stop, and both of them have to: the **scripts**, by setting the VM's budget to zero
/// (rubevy stops the scheduler's clock with it, so an arm half way through a `sleep` is still
/// half way through it when the budget comes back), and the **world**, because
/// [`crate::is_running`] is on every system of the factory's own step. Stopping only the first
/// was the garden's G4 and the author's third play is why it is not enough: everything went on
/// moving around scripts that had stopped thinking.
///
/// **Building and editing go on while it is paused**, and that is a decision: a pause here is
/// what a factory game is for — it is how you look at a jammed line and work out what is wrong —
/// and a pause that would not let you lay the belt you had just worked out you needed would be a
/// pause nobody used. It is also the honest reading of what `P` stops: the belts and the arms.
/// A belt laid while paused carries nothing until `P` is pressed again, which is what the player
/// asked for.
#[derive(Resource, Debug, Default)]
pub struct Paused {
    /// **Whether the factory's own step runs.** It is the run condition on
    /// [`crate::FactorySet::Step`] and it is read by nothing else.
    world: bool,
    /// What the VM's budget was before, so that `P` again gives back exactly what was taken.
    /// `None` is "the scripts are running".
    was: Option<u64>,
}

impl Paused {
    /// Whether the belts and the arms are standing still.
    pub fn on(&self) -> bool {
        self.world
    }

    /// Whether the scripts are stopped as well, which `P` does and the checks do not.
    pub fn scripts_too(&self) -> bool {
        self.was.is_some()
    }

    /// **`P`: both halves.** The world by the flag above, the scripts by their budget.
    fn turn(&mut self, scripts: &mut ScriptWorld, panel: &mut VmInspector) {
        self.world = !self.world;
        match self.was.take() {
            Some(budget) => {
                scripts.budget = budget;
                panel.paused = false;
            }
            None => {
                self.was = Some(scripts.budget);
                scripts.budget = 0;
                panel.paused = true;
            }
        }
    }

    /// **The world alone, held where it stands**, which is what the save's round-trip check needs
    /// and `P` is not: a factory written down, read back and written down again is the same text
    /// only if nothing moved in between, and the reading back *needs the VM running* — a loaded
    /// arm has nowhere to put what it remembered until its script has reached `run`. So the two
    /// halves of a pause are two things here, and only `P` does both.
    pub fn hold_the_world(&mut self, yes: bool) {
        self.world = yes;
    }
}

/// The run condition on the factory's own step.
pub fn the_world_is_running(paused: Option<Res<Paused>>) -> bool {
    paused.is_none_or(|p| !p.on())
}

// ---------------------------------------------------------------------------------------------
// What the panel is showing
// ---------------------------------------------------------------------------------------------

/// The panel follows whichever of the three is being watched: the three buttons, the text, the
/// name, and whether what is running came out of the file or out of the panel.
pub fn show_code(
    mut watched: ResMut<Watched>,
    grid: Res<Grid>,
    control: Res<crate::control::TheControl>,
    data_file: Res<DataFile>,
    mut crew: Crew,
) {
    // the panel is reached through `Crew` and not as a parameter of its own: two `ResMut` of one
    // resource in one system is a panic, and `Crew` already holds it (`crate::inserters::Crew`)
    let Some(editor) = crew.panel.as_mut() else { return };
    if let Some(picked) = editor.picked.take() {
        *watched = Watched::of_key(picked);
    }
    // **the arm is not a button**: there may be a thousand of them and what chooses one is the
    // map, so the third choice is whichever one was clicked last and is dim until there is one
    let arm = match *watched {
        Watched::Arm(tile) => Some(tile),
        _ => None,
    };
    editor.choices = vec![
        EditorChoice { id: DATA, label: crate::DATA_FILE.into(), color: (150, 190, 150), dim: false },
        EditorChoice {
            id: CONTROL,
            label: crate::control::SCRIPT_FILE.into(),
            color: (190, 170, 130),
            dim: false,
        },
        EditorChoice {
            id: ARM + arm.unwrap_or(0) as u64,
            label: match arm {
                Some(_) => SCRIPT_FILE.into(),
                None => format!("{SCRIPT_FILE} (click an arm)"),
            },
            color: (150, 170, 200),
            dim: arm.is_none(),
        },
    ];
    editor.apply_key = None;
    editor.save_label = Some(games_shell::platform::SAVE_LABEL.into());

    match *watched {
        Watched::Nothing => {
            editor.selected = None;
            editor.noun = "file".into();
            editor.apply_label = "▶ Apply (Ctrl+Enter)".into();
            editor.apply_all_label = None;
        }
        Watched::Data => {
            editor.selected = Some(DATA);
            editor.noun = "data stage".into();
            editor.apply_label = "▶ Read it again (Ctrl+Enter)".into();
            editor.apply_all_label = None;
            let text = data_file.text.clone();
            editor.show(DATA, || text);
            editor.file = crate::DATA_FILE.into();
            editor.label = "what the world is made of".into();
            editor.in_memory = data_file.in_memory;
            editor.elsewhere = data_file.trouble.clone();
            editor.current = None;
            editor.heat.clear();
        }
        Watched::Control => {
            editor.selected = Some(CONTROL);
            editor.noun = "control stage".into();
            editor.apply_label = "▶ Apply (Ctrl+Enter)".into();
            editor.apply_all_label = None;
            let text = control.text().to_string();
            editor.show(CONTROL, || text);
            editor.file = crate::control::SCRIPT_FILE.into();
            editor.label = "what the factory is for".into();
            editor.in_memory = control.in_memory;
            editor.elsewhere = control.trouble.clone();
            editor.current = None;
            editor.heat.clear();
        }
        Watched::Arm(tile) => {
            editor.selected = Some(ARM + tile as u64);
            editor.noun = "inserter".into();
            editor.apply_label = "▶ Apply to this one (Ctrl+Enter)".into();
            editor.apply_all_label = Some("Apply to every inserter".into());
            // the arm was taken away while its script was on the screen
            if grid.at(tile).map(|b| b.what) != Some(What::Inserter) {
                return;
            }
            let text = crew.minds.text_for(tile).to_string();
            editor.show(ARM + tile as u64, || text);
            let at = grid.tile_of(tile);
            editor.file = SCRIPT_FILE.into();
            editor.label = format!("the inserter at {}, {}", at.x, at.y);
            editor.in_memory = crew.minds.in_memory(tile);
            // **the mark a stopped arm wears, said in words as well.** The picture puts an orange
            // mark over it; the panel says where it stopped, which is the half a player can act on.
            editor.elsewhere = crew.arms.stopped_at(tile).map(|at| format!("stopped at {at}"));
            editor.current = None;
            editor.heat.clear();
        }
    }
}

/// The VM panel follows the same three, so that `F2` and the editor are never about different
/// scripts. `data.rb` has no task at all — it ran once in `Startup` and was done — so the panel
/// says so rather than showing an empty one.
pub fn show_vm(
    watched: Res<Watched>,
    control: Res<crate::control::TheControl>,
    control_task: Query<&rubevy::ScriptTask, With<crate::control::ControlScript>>,
    crew: Crew,
    mut panel: ResMut<VmInspector>,
    scripts: Res<ScriptWorld>,
    tasks: Query<&rubevy::ScriptTask>,
) {
    if !panel.open {
        return;
    }
    match *watched {
        Watched::Arm(tile) => {
            let Some(entity) = crew.at(tile) else {
                return panel.clear("this inserter has no script running");
            };
            let Ok(task) = tasks.get(entity) else {
                return panel.clear("this inserter has no script running");
            };
            panel.fill(
                &scripts,
                task.task(),
                format!("inserter {tile}"),
                crew.minds.prelude_lines_for(tile).unwrap_or(0),
            );
        }
        Watched::Control => match control_task.single() {
            Ok(task) => panel.fill(
                &scripts,
                task.task(),
                crate::control::SCRIPT_FILE.into(),
                control.prelude_lines(),
            ),
            Err(_) => panel.clear("the control stage is not running"),
        },
        // the data stage is not a script that runs: it was compiled, run and finished before the
        // first frame, and a panel about its frames would be a panel about nothing
        Watched::Data | Watched::Nothing => {
            panel.clear("the data stage ran once, in Startup, and has no task")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The buttons
// ---------------------------------------------------------------------------------------------

/// **`ruby/data.rb` as the game is holding it**: the text, whether it is the file's, and what was
/// wrong with it the last time it was read.
#[derive(Resource, Debug, Default)]
pub struct DataFile {
    pub text: String,
    /// applied in the panel and not written to disk
    pub in_memory: bool,
    pub trouble: Option<String>,
    /// **A text that would rebuild the world, waiting for the Apply to be pressed again.** The
    /// one click of confirmation, and it is kept here rather than in the panel because it is a
    /// fact about the game and not about the window.
    pub confirming: Option<String>,
}

/// The game's own message for "read this `data.rb` instead" — the same shape as
/// `control::Rewrite`, and for the same reason: what is *meant* is written down and one system
/// carries it out, so the checks can drive it without a window.
#[derive(Message, Debug, Clone)]
pub struct Reread(pub String);

/// What the buttons asked for. **Nothing here writes a file except Save.**
#[allow(clippy::too_many_arguments)]
pub fn do_editor_actions(
    watched: Res<Watched>,
    prelude: Res<Prelude>,
    data: Res<Data>,
    rules: Res<Rules>,
    stagger: Res<crate::inserters::Stagger>,
    ruby: Res<crate::RubyDir>,
    mut data_file: ResMut<DataFile>,
    mut rewrites: MessageWriter<crate::control::Rewrite>,
    mut rereads: MessageWriter<Reread>,
    mut crew: Crew,
    mut mrb: ResMut<Assets<rubevy::MrbAsset>>,
) {
    let Some(action) = crew.panel.as_mut().and_then(|p| p.action.take()) else { return };
    let text = crew.panel.as_ref().map(|p| p.text.clone()).unwrap_or_default();
    // **A text that will not compile is not applied at all**, and whatever was running goes on
    // running — the same answer a `data.rb` that will not read gets, and the opposite of what a
    // panic would say about a file a player is invited to edit.
    let compiles = |crew: &mut Crew, mrb: &mut Assets<rubevy::MrbAsset>, text: &str| {
        crate::inserters::would_compile(&mut crew.minds, &prelude.0, &data, &rules, *stagger, text, mrb)
    };
    let said: Result<(String, Option<String>), String> = match (*watched, action) {
        // ---- Save: the one thing that touches the disk -------------------------------------
        (where_, EditorAction::Save) => {
            let name = match where_ {
                Watched::Data => crate::DATA_FILE,
                Watched::Control => crate::control::SCRIPT_FILE,
                _ => SCRIPT_FILE,
            };
            match platform::write(&ruby.0.join(name), &text) {
                Ok(()) => {
                    info!("{name} written");
                    // **the file on disk is what it now says**, so Revert goes back to this and
                    // the panel stops saying "in memory"
                    match where_ {
                        Watched::Data => {
                            data_file.text = text.clone();
                            data_file.in_memory = false;
                        }
                        Watched::Arm(_) | Watched::Nothing => crew.minds.take_as_the_file(&text),
                        Watched::Control => {}
                    }
                    Ok((format!("written to {name}"), None))
                }
                Err(why) => Err(format!("{name}: {why}")),
            }
        }
        // ---- the data stage ----------------------------------------------------------------
        (Watched::Data, EditorAction::Apply | EditorAction::ApplyAll) => {
            rereads.write(Reread(text.clone()));
            // what it did is said by `reread_the_data_stage`, which is where the answer is known
            Ok((String::new(), None))
        }
        (Watched::Data, EditorAction::Revert) => {
            let file = platform::read(&ruby.0.join(crate::DATA_FILE)).unwrap_or_default();
            rereads.write(Reread(file.clone()));
            Ok((String::new(), Some(file)))
        }
        // ---- the control stage ---------------------------------------------------------------
        (Watched::Control, EditorAction::Apply | EditorAction::ApplyAll) => {
            rewrites.write(crate::control::Rewrite(text.clone()));
            Ok((
                "the control stage is running it — and its goal counts from now".into(),
                None,
            ))
        }
        (Watched::Control, EditorAction::Revert) => {
            let file = platform::read(&ruby.0.join(crate::control::SCRIPT_FILE)).unwrap_or_default();
            rewrites.write(crate::control::Rewrite(file.clone()));
            Ok((format!("back to {}", crate::control::SCRIPT_FILE), Some(file)))
        }
        // ---- the arms --------------------------------------------------------------------------
        (Watched::Arm(tile), EditorAction::Apply) => compiles(&mut crew, &mut mrb, &text).map(|()| {
            crew.minds.give_to_one(tile, text.clone());
            ("this inserter is running it — in memory, and no other one has changed".into(), None)
        }),
        (Watched::Arm(_) | Watched::Nothing, EditorAction::ApplyAll) => {
            compiles(&mut crew, &mut mrb, &text).map(|()| {
                let n = crew.how_many();
                crew.minds.give_to_all(text.clone());
                (format!("all {n} inserters are running it — in memory, and any that had a script of their own have lost it"), None)
            })
        }
        (Watched::Arm(_) | Watched::Nothing, EditorAction::Revert) => {
            let file = crew.minds.file().to_string();
            crew.minds.back_to_the_file();
            Ok((format!("every inserter back to {SCRIPT_FILE}"), Some(file)))
        }
        (Watched::Nothing, EditorAction::Apply) => {
            Err("click an inserter, or pick a file above".into())
        }
    };
    let Some(panel) = crew.panel.as_mut() else { return };
    match said {
        Ok((message, _)) if message.is_empty() => {}
        Ok((message, None)) => panel.applied(message),
        Ok((message, Some(source))) => panel.reset_to(source, message),
        Err(why) => {
            // a text that would not compile leaves every arm running what it was running, and
            // the first line of the complaint — the one with the place in it — is what the panel
            // has room for; the log has the whole thing
            if why.contains(':') {
                error!("{why}");
            }
            panel.message = format!("not applied — {}", why.lines().next().unwrap_or(&why));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// What the editor is spending
// ---------------------------------------------------------------------------------------------

/// **What the editor is spending**, said whenever either number moves.
///
/// Every distinct text a script is started from is an irep the VM keeps for the life of the
/// process — SabiRuby has no way to drop one — so an editor is a thing that spends memory as it
/// is used. `loaded_programs` is rubevy's count of distinct programs and `swaps` is how many
/// times an arm has been handed one; the two apart say whether a hundred arms cost a hundred
/// programs (they do not: one text is one program, however many arms run it).
pub fn watch_the_programs(scripts: Res<ScriptWorld>, crew: Crew, mut said: Local<(u64, usize)>) {
    let now = (crew.minds.swaps, scripts.loaded_programs());
    if *said == now {
        return;
    }
    *said = now;
    info!(
        "scripts: {} handed over, {} programs in the VM, {} that would not load",
        now.0,
        now.1,
        scripts.broken_programs()
    );
}

// ---------------------------------------------------------------------------------------------
// The HUD
// ---------------------------------------------------------------------------------------------

/// **The one panel that says what the factory is doing**, and it replaces F4's line of
/// `bevy_ui` — one drawing system rather than two, and room for the numbers a player watching a
/// factory of three thousand arms wants.
///
/// Four rows, and each of them is a thing that was measured rather than a thing that looked nice
/// in a panel:
///
/// * **the factory**: how many buildings, how many items are on the belts, how many arms. The
///   item count is a field rather than a walk since F4 (`Lanes::count`), which is what makes it
///   free to show on a map of two thousand tiles a side.
/// * **the scripts**: what one frame of them cost against the budget, how many are parked on a
///   `move`, and what the VM's tick took against `frame_time`. The budget was chosen by
///   measuring three thousand arms (`docs/numbers.md` §9.5), so this is the number it was chosen
///   against.
/// * **the control stage**: its own line (`@saying`), and what was dropped — **the number that
///   says the design is wrong if it is not zero** (§3.4: a subscription holds sixty four
///   messages a frame).
/// * **the editor**: how many programs the VM is holding and how many times an arm has been
///   handed one, because every distinct text is an irep SabiRuby never drops.
#[allow(clippy::too_many_arguments)]
pub fn draw_hud(
    mut contexts: EguiContexts,
    clock: Res<rubevy_egui::VmClock>,
    scripts: Res<ScriptWorld>,
    grid: Res<Grid>,
    lanes: Res<Lanes>,
    control: Res<crate::control::TheControl>,
    note: Res<crate::save::SaveNote>,
    paused: Res<Paused>,
    mut asked: ResMut<crate::save::Asked>,
    crew: Crew,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let amber = egui::Color32::from_rgb(240, 190, 90);
    egui::Window::new("Factory")
        .collapsible(true)
        .resizable(true)
        .default_width(560.0)
        .default_pos([8.0, 8.0])
        .show(ctx, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} built · {} on the belts · {} arms",
                        grid.built().len(),
                        lanes.count(),
                        crew.how_many()
                    ))
                    .strong()
                    .size(15.0),
                );
                if paused.on() {
                    ui.label(egui::RichText::new("paused (P)").color(amber).strong());
                }
            });
            ui.horizontal_wrapped(|ui| {
                let budget = scripts.budget;
                let spent = scripts.last_frame().instructions;
                let over = budget > 0 && spent >= budget;
                let text = egui::RichText::new(format!(
                    "scripts {spent} / {budget} insn · {:.2} / {:.1} ms · {} waiting · {} carried over",
                    clock.mean_ms,
                    clock.budget_ms,
                    crew.how_many_waiting(),
                    scripts.last_frame().carried_reflect + scripts.last_frame().carried_in_tick,
                ))
                .monospace();
                ui.label(if over { text.color(amber).strong() } else { text }).on_hover_text(
                    "what one frame of the arms' scripts spent against `ScriptWorld::budget`, the wall time the tick took against `frame_time`, how many arms are parked on a `move` (rubevy's `Held`), and how many tasks were left for the next frame because the budget ran out",
                );
            });
            ui.horizontal_wrapped(|ui| {
                let dropped = control.dropped + scripts.dropped();
                // **the same sentence a headless run would have in its log**, so that there is
                // one wording of what the control stage has to say (`control::what_it_says`)
                let text = egui::RichText::new(
                    crate::control::what_it_says(&control, scripts.dropped()).replace('\n', " · "),
                );
                ui.label(if control.won { text.color(amber).strong() } else { text });
                if dropped > 0 {
                    ui.label(
                        egui::RichText::new(format!("· {dropped} messages dropped"))
                            .color(amber)
                            .strong(),
                    )
                    .on_hover_text(
                        "a subscription holds sixty four messages between two looks; anything above that is a design that publishes too finely (`docs/plans/factory-plan.md` §3.4). It should be zero",
                    );
                }
            });
            if let Some(why) = &control.trouble {
                ui.label(egui::RichText::new(why).color(amber));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} programs in the VM · {} handed over · {} machines",
                        scripts.loaded_programs(),
                        crew.minds.swaps,
                        control.seen[EVENT_CRAFTED],
                    ))
                    .monospace()
                    .weak(),
                )
                .on_hover_text(
                    "every distinct text a script is started from is an irep SabiRuby keeps for the life of the process, so an editor used for an hour spends this",
                );
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save (F5)").clicked() {
                    asked.save = true;
                }
                if ui.button("Load (F9)").clicked() {
                    asked.load = true;
                }
                if !note.text.is_empty() {
                    let text = egui::RichText::new(&note.text);
                    ui.label(if note.bad { text.color(amber).strong() } else { text });
                }
                ui.label(egui::RichText::new(games_shell::Guide::HINT).weak());
            });
        });
}

/// Which of `control::EVENTS` is `crafted`, for the one number the HUD takes out of that array.
const EVENT_CRAFTED: usize = 2;

// ---------------------------------------------------------------------------------------------
// Setting up
// ---------------------------------------------------------------------------------------------

/// The window's own setup: the panel, at whatever size the store says.
pub fn the_editor(settings: &games_shell::Settings) -> rubevy_egui::EditorPlugin {
    let mut layout = rubevy_egui::EditorLayout::default();
    layout.read_from(|key| settings.number(key));
    rubevy_egui::EditorPlugin::with_highlighter(platform::highlight).sized(layout)
}
