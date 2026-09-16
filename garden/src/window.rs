//! **The window (G4).** The editor, the VM panel, the HUD, and the keys that drive them.
//!
//! None of it is new code in the sense of being written here: the editor and the inspector are
//! `rubevy-arena`'s, the same two SabiRuby Battle uses, and what is in this file is the game's
//! half — which creature is being looked at, what a species' file is, what happens when Apply is
//! pressed, and what the HUD shows. The whole of it is off in the headless build, which has no
//! window and no egui; the numbers the HUD draws are printed there instead (`stop_when_over`).
//!
//! Two things are different from sabibots, and both come from the same fact:
//!
//! * **A creature's brain is its species', not its own.** Every beetle in the garden runs
//!   `beetle.rb`, so the editor's unit is the file, Apply restarts every beetle, and a beetle
//!   born afterwards is born running the applied text ([`crate::Brains`]). sabibots applies to
//!   one robot because a robot *has* a file. That is why the editor grew
//!   `Editor::apply_all_label`: in this game there is no second Apply to draw.
//! * **`F5` is taken.** It saves the garden (G3), so the editor's apply key here is
//!   `Ctrl+Enter` and the button (`Editor::apply_key`).

use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use rubevy::{MrbAsset, Script, ScriptTask, ScriptWorld};
use rubevy_arena::{Editor, EditorAction, VmInspector, Watch};

use crate::{
    compile, compile_source, platform, Brains, Creature, Hunger, Mind, Plant, RubyDir, Sky,
    Species,
};

/// Which creature the editor and the VM panel are about.
#[derive(Resource, Default)]
pub struct Watched {
    pub entity: Option<Entity>,
}

/// The budget the scripts get when they are not paused, kept while they are (`P`).
#[derive(Resource, Default)]
pub struct Paused {
    was: Option<u64>,
}

/// How long the VM had this frame, against what it is allowed.
///
/// The span is measured round `RubevySet::Tick` — `tick_scripts`, then `drain_commands` and
/// `apply_component_writes`, which are the frame's scripts and everything they asked for. It is
/// wall time, not CPU: it is the number the budget is *about* (`ScriptWorld::frame_time`, 8 ms),
/// so it is the number to show beside it.
#[derive(Resource, Default)]
pub struct VmClock {
    started: Option<Instant>,
    /// milliseconds the last frame's scripts took
    pub spent_ms: f32,
    /// smoothed, because a single frame's figure flickers faster than it can be read
    pub mean_ms: f32,
    /// what `ScriptWorld::frame_time` allows, in milliseconds
    pub budget_ms: f32,
}

pub fn vm_clock_start(mut clock: ResMut<VmClock>) {
    clock.started = Some(Instant::now());
}

pub fn vm_clock_end(mut clock: ResMut<VmClock>, world: Res<ScriptWorld>) {
    let Some(started) = clock.started.take() else { return };
    let spent = started.elapsed().as_secs_f32() * 1000.0;
    clock.spent_ms = spent;
    // a fifth of the new reading: about a sixth of a second of memory at 60 Hz
    clock.mean_ms = clock.mean_ms * 0.8 + spent * 0.2;
    clock.budget_ms = world.frame_time.map(|d| d.as_secs_f32() * 1000.0).unwrap_or(0.0);
}

// ---------------------------------------------------------------------------------------------
// Picking a creature
// ---------------------------------------------------------------------------------------------

/// How near the ground under the cursor a creature has to be to be the one that was clicked.
const CLICK_REACH: f32 = 1.6;
/// How far the mouse may travel between press and release and still be a click rather than a drag
/// of the camera.
const CLICK_SLOP: f32 = 6.0;

/// Click one to look at it, `Tab` for the next, `F1` hides the editor.
///
/// The click has to share the left button with the camera's orbit, so it is decided on the
/// *release*: a press that let go within a few pixels of where it went down was a click, and
/// anything further was a drag. The ray goes from the camera through the cursor to the ground
/// plane, and the creature nearest where it lands — within about a body's width — is the one
/// meant. There is no picking in Bevy to lean on here and no collider library in this game; a
/// plane and a distance is the whole of it.
#[allow(clippy::too_many_arguments)]
pub fn choose_watched(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    creatures: Query<(Entity, &Creature, &Transform)>,
    mut watched: ResMut<Watched>,
    mut editor: ResMut<Editor>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    let mut all: Vec<(Entity, Species)> = creatures.iter().map(|(e, c, _)| (e, c.species)).collect();
    all.sort_by_key(|(e, _)| e.to_bits());
    if all.is_empty() {
        watched.entity = None;
        return;
    }
    // the creature being looked at died: move on to one that has not
    if !watched.entity.is_some_and(|e| all.iter().any(|(o, _)| *o == e)) {
        watched.entity = Some(all[0].0);
        editor.open = true;
    }
    // a file button clicked along the top of the editor: show a creature that runs it
    if let Some(picked) = editor.picked.take()
        && let Some((e, _)) = all.iter().find(|(_, s)| s.index() as u64 == picked)
    {
        watched.entity = Some(*e);
    }

    let egui_keyboard = typing.as_ref().is_some_and(|t| t.wants_keyboard_input());
    let egui_pointer = typing.as_ref().is_some_and(|t| t.wants_pointer_input());

    // the click, on the release, and only where egui did not want the mouse
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if buttons.just_pressed(MouseButton::Left) {
        *pressed_at = if egui_pointer { None } else { cursor };
    }
    if buttons.just_released(MouseButton::Left)
        && let (Some(down), Some(up)) = (pressed_at.take(), cursor)
        && down.distance(up) <= CLICK_SLOP
        && let Some(entity) = creature_under(up, &cameras, &creatures)
    {
        watched.entity = Some(entity);
        editor.open = true;
    }

    if egui_keyboard {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let at = all.iter().position(|(e, _)| Some(*e) == watched.entity).unwrap_or(0);
        watched.entity = Some(all[(at + 1) % all.len()].0);
        editor.open = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        editor.open = !editor.open;
    }
}

/// The creature nearest to where a ray through `cursor` meets the ground.
fn creature_under(
    cursor: Vec2,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    creatures: &Query<(Entity, &Creature, &Transform)>,
) -> Option<Entity> {
    let (camera, at) = cameras.iter().next()?;
    let ray = camera.viewport_to_world(at, cursor).ok()?;
    let far = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))?;
    let ground = ray.get_point(far);
    let here = Vec2::new(ground.x, ground.z);
    let mut best: Option<(Entity, f32)> = None;
    for (entity, _, place) in creatures {
        let span = here.distance(Vec2::new(place.translation.x, place.translation.z));
        if span <= CLICK_REACH && best.is_none_or(|(_, b)| span < b) {
            best = Some((entity, span));
        }
    }
    best.map(|(e, _)| e)
}

// ---------------------------------------------------------------------------------------------
// The editor
// ---------------------------------------------------------------------------------------------

const BEETLE_COLOR: (u8, u8, u8) = (226, 176, 92);
const RABBIT_COLOR: (u8, u8, u8) = (196, 204, 226);

pub fn species_color(species: Species) -> (u8, u8, u8) {
    match species {
        Species::Beetle => BEETLE_COLOR,
        Species::Rabbit => RABBIT_COLOR,
    }
}

/// The editor follows the creature being looked at — or rather its **file**, which is what the
/// two buttons along the top are: `beetle.rb` and `rabbit.rb`, and nothing else the game has.
pub fn show_code(
    watched: Res<Watched>,
    brains: Res<Brains>,
    ruby: Res<RubyDir>,
    minds: Query<&Mind>,
    creatures: Query<&Creature>,
    mut editor: ResMut<Editor>,
) {
    editor.choices = Species::ALL
        .iter()
        .map(|species| rubevy_arena::EditorChoice {
            id: species.index() as u64,
            label: format!(
                "{}{}",
                species.file(),
                if brains.text(*species).is_some() { "*" } else { "" }
            ),
            color: species_color(*species),
            dim: !creatures.iter().any(|c| c.species == *species),
        })
        .collect();
    // `F5` writes the garden down in this game, so it cannot also mean Apply; and there is one
    // Apply, because a species is everything that runs its file. Set before the early returns
    // below, so a frame with nothing selected does not leave `F5` meaning two things.
    editor.apply_key = None;
    editor.apply_all_label = None;
    editor.noun = "creature".into();
    editor.save_label = Some(platform::SAVE_LABEL.into());

    let Some(entity) = watched.entity else { return };
    let Ok(mind) = minds.get(entity) else { return };
    let species = mind.species;
    editor.selected = Some(species.index() as u64);
    let running = brains.text(species).cloned();
    let path = brains.path(&ruby.0, species);
    editor.show(species.index() as u64, || {
        running.unwrap_or_else(|| platform::read(&path).unwrap_or_default())
    });
    editor.file = species.file().into();
    editor.label = mind.name.clone();
    editor.in_memory = brains.text(species).is_some();
    editor.apply_label = format!("▶ Apply to every {} (Ctrl+Enter)", species.name());
    editor.current = mind.own_line;
    editor.heat = mind.heat.clone();
    editor.elsewhere = None;
}

/// What the editor's buttons asked for. Nothing here writes a file except Save.
#[allow(clippy::too_many_arguments)]
pub fn do_editor_actions(
    mut commands: Commands,
    mut editor: ResMut<Editor>,
    watched: Res<Watched>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut minds: Query<(Entity, &mut Mind)>,
) {
    let Some(action) = editor.action.take() else { return };
    let Some(shown) = watched.entity else { return };
    let Ok((_, mind)) = minds.get(shown) else { return };
    let species = mind.species;
    let text = editor.text.clone();
    let path = brains.path(&ruby.0, species);

    match action {
        // there is one of these in this game (`apply_all_label` is None), and it is the species
        EditorAction::Apply | EditorAction::ApplyAll => {
            let Some((handle, lines)) = compile_source(&ruby.0, species.file(), &text, &mut mrb)
            else {
                editor.message = format!("not applied: {} would not compile (see the log)", species.file());
                return;
            };
            brains.set(species, Some(text));
            let n = restart_species(&mut commands, &mut minds, species, handle, lines, true);
            editor.applied(format!(
                "{n} {}s restarted on it — in memory, and so is anything born into it. Save to keep it.",
                species.name()
            ));
        }
        EditorAction::Save => {
            if let Err(e) = platform::write(&path, &text) {
                editor.message = format!("could not save {}: {e}", species.file());
                return;
            }
            // the file says this now, so nothing is running a text of its own any more
            brains.set(species, None);
            let Some((handle, lines)) = compile(&ruby.0, &path, &mut mrb) else {
                editor.message = format!("saved {}, but it would not compile", species.file());
                return;
            };
            restart_species(&mut commands, &mut minds, species, handle, lines, false);
            editor.applied(format!("saved to {}", species.file()));
        }
        EditorAction::Revert => {
            let Ok(source) = platform::read(&path) else {
                editor.message = format!("could not read {}", species.file());
                return;
            };
            let Some((handle, lines)) = compile(&ruby.0, &path, &mut mrb) else {
                editor.message = format!("{} does not compile", species.file());
                return;
            };
            brains.set(species, None);
            restart_species(&mut commands, &mut minds, species, handle, lines, false);
            editor.reset_to(source, format!("back to {}", species.file()));
        }
    }
}

/// Every creature of one species, started over on a freshly compiled program — all of them, in
/// this frame.
///
/// Dropping `ScriptTask` is what stops the old script: rubevy terminates the task, closes the
/// queues it had subscribed to and ends the reflex tasks waiting on them (`stop_removed_task`).
/// The new `Script` becomes a task on the next frame and subscribes again. **What does not come
/// back is what the creature remembered**: `@memory` is a Hash on the object the old script made,
/// and the new script makes a new one. That is the same in sabibots and it is the honest
/// behaviour — a brain that has been rewritten is not the brain that learnt those things.
///
/// **This used to be a queue.** Until sabiruby 0.5.1 the game handed the creatures over one at a
/// time, one every 0.4 s (`Restarting`, `RESTARTS_PER_FRAME`, `RESTART_GAP`), because replacing
/// ten of them at once stopped the VM's scheduler for good: `Vm::task_pending()` stayed true,
/// `task_run_limits` ran nothing, and every task in the VM froze, the rabbits included
/// (`docs/worklog/2026-09-17-garden-G4.md` §5a). The cause was not the contexts being slow to go,
/// as it looked from here: ending a `ScriptTask` wakes each of the creature's six reflex tasks
/// with `Rubevy::Unsubscribed`, their empty `rescue` makes the block's value `nil`, and
/// `Vm::task_run_limited` read that `nil` as "nothing left to run" and ended the host's whole
/// frame — one frame per task, so sixty frames for ten beetles
/// (sabiruby `docs/worklog/2026-09-17-task-end-nil.md`). 0.5.1 tells the two apart, and the queue
/// went with it: every creature of the species is handed over here, in the frame Apply was pressed.
fn restart_species(
    commands: &mut Commands,
    minds: &mut Query<(Entity, &mut Mind)>,
    species: Species,
    handle: Handle<MrbAsset>,
    prelude_lines: u32,
    in_memory: bool,
) -> usize {
    let mut n = 0;
    for (entity, mut mind) in minds.iter_mut() {
        if mind.species != species {
            continue;
        }
        mind.in_memory = in_memory;
        mind.prelude_lines = prelude_lines;
        // everything the HUD says about this creature is about the script it is running, and
        // that is about to be a different one: the heat, the line, what it has spent, and what a
        // round trip has cost it all start again with the new brain
        mind.heat.clear();
        mind.own_line = None;
        mind.at.clear();
        mind.last_instructions = 0;
        mind.spent = 0;
        mind.frames = 0;
        mind.restart();
        commands
            .entity(entity)
            .remove::<ScriptTask>()
            .remove::<rubevy::ScriptDone>()
            .insert(Script::new(handle.clone()).with_name(&mind.name).with_priority(100));
        n += 1;
    }
    n
}

/// A creature file saved from outside the game restarts that species, exactly as Save does.
pub fn reload_changed(
    mut commands: Commands,
    watch: Option<Res<Watch>>,
    ruby: Res<RubyDir>,
    brains: Res<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut minds: Query<(Entity, &mut Mind)>,
    mut editor: ResMut<Editor>,
) {
    let Some(watch) = watch else { return };
    for path in watch.changed() {
        for species in Species::ALL {
            let mine = brains.path(&ruby.0, species);
            if path != mine && !path.ends_with("prelude.rb") {
                continue;
            }
            // a species running a text applied in the editor keeps it until Save or Revert
            if brains.text(species).is_some() {
                continue;
            }
            let Some((handle, lines)) = compile(&ruby.0, &mine, &mut mrb) else {
                editor.message = format!("{}: compile error (see the log)", species.file());
                continue;
            };
            if editor.key == Some(species.index() as u64)
                && !editor.changed()
                && let Ok(source) = platform::read(&mine)
            {
                editor.reset_to(source, "the file changed");
            }
            let n = restart_species(&mut commands, &mut minds, species, handle, lines, false);
            info!("{} changed: {n} restarted", species.file());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The VM panel
// ---------------------------------------------------------------------------------------------

/// `F2` shows and hides the VM panel; `P` pauses the scripts.
///
/// Pausing is `ScriptWorld::budget = 0`: the scheduler returns before handing any task the CPU,
/// so nothing in the VM moves and the panel's numbers stand still while they are read. The garden
/// keeps drawing and the creatures keep walking on the velocity their scripts last wrote — it is
/// the Ruby that is stopped, not the world. rubevy `fa37eaa` stops the scheduler's clock with it,
/// so a creature half way through a `sleep 0.2` is still half way through it when the budget comes
/// back, and there is nothing here to call for that.
pub fn inspect_keys(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut panel: ResMut<VmInspector>,
    mut world: ResMut<ScriptWorld>,
    mut paused: ResMut<Paused>,
) {
    if typing.is_some_and(|t| t.wants_keyboard_input()) {
        return;
    }
    if keys.just_pressed(KeyCode::F2) {
        panel.open = !panel.open;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        match paused.was.take() {
            Some(budget) => {
                world.budget = budget;
                panel.paused = false;
            }
            None => {
                paused.was = Some(world.budget);
                world.budget = 0;
                panel.paused = true;
                panel.open = true;
            }
        }
    }
}

/// The VM panel follows the creature being looked at, as the editor follows its file.
pub fn show_vm(
    watched: Res<Watched>,
    world: Res<ScriptWorld>,
    mut panel: ResMut<VmInspector>,
    minds: Query<(&Mind, Option<&ScriptTask>)>,
) {
    if !panel.open {
        return;
    }
    let Some(entity) = watched.entity else { return };
    let Ok((mind, script)) = minds.get(entity) else {
        panel.clear("nothing is selected");
        return;
    };
    let Some(script) = script else {
        panel.title = mind.name.clone();
        panel.clear("this creature has no task: it has just been given a new brain, or it starved");
        return;
    };
    panel.spent = mind.spent;
    panel.fill(&world, script.task(), mind.name.clone(), mind.prelude_lines);
}

// ---------------------------------------------------------------------------------------------
// The HUD
// ---------------------------------------------------------------------------------------------

/// One creature's line, the same fields the headless run prints (`crate::stop_when_over`).
pub struct HudRow {
    pub entity: Entity,
    pub name: String,
    pub species: Species,
    pub hunger: f32,
    pub insn_per_frame: u64,
    pub per_decision: Option<f32>,
    pub at: String,
    pub in_memory: bool,
}

/// The rows, ready to draw or to print.
///
/// Sorted by species and then by **age**, so the list does not jump about as creatures are born
/// and starve, and a newborn appears at the bottom of its species rather than the top. Age is the
/// entity's index, which is `Entity::to_bits` **reversed**: bevy stores the row bit-inverted for
/// the niche, so the raw bits of a fresh entity are smaller than an old one's.
pub fn hud_rows(creatures: &Query<(Entity, &Creature, &Hunger, &Mind)>) -> Vec<HudRow> {
    let mut rows: Vec<HudRow> = creatures
        .iter()
        .map(|(entity, creature, hunger, mind)| HudRow {
            entity,
            name: mind.name.clone(),
            species: creature.species,
            hunger: hunger.0,
            insn_per_frame: mind.last_instructions / mind.frames.max(1),
            per_decision: mind.frames_per_decision(),
            at: mind.at.clone(),
            in_memory: mind.in_memory,
        })
        .collect();
    rows.sort_by_key(|r| (r.species.index(), std::cmp::Reverse(r.entity.to_bits())));
    rows
}

/// The panel at the top left: the whole garden in one line, and one line per creature.
#[allow(clippy::too_many_arguments)]
pub fn draw_hud(
    mut contexts: EguiContexts,
    clock: Res<VmClock>,
    note: Res<crate::SaveNote>,
    mut asked: ResMut<crate::Asked>,
    sky: Res<Sky>,
    world: Res<ScriptWorld>,
    plants: Query<&Plant>,
    creatures: Query<(Entity, &Creature, &Hunger, &Mind)>,
    mut watched: ResMut<Watched>,
    mut editor: ResMut<Editor>,
    mut dial: ResMut<crate::NightDial>,
    mut settings: Option<ResMut<rubevy_arena::Settings>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let rows = hud_rows(&creatures);
    let plants = plants.iter().count();
    let paused = world.budget == 0;
    let amber = egui::Color32::from_rgb(240, 190, 90);

    egui::Window::new("Garden")
        .collapsible(true)
        .resizable(true)
        .default_width(560.0)
        .default_pos([8.0, 8.0])
        .show(ctx, |ui| {
            ui.style_mut().interaction.selectable_labels = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{} creatures · {plants} plants · {} {:.2}",
                        rows.len(),
                        if sky.night { "night" } else { "day" },
                        sky.phase
                    ))
                    .strong()
                    .size(15.0),
                );
                // the VM's share of the frame against what it is allowed (`frame_time`)
                let over = clock.mean_ms > clock.budget_ms && clock.budget_ms > 0.0;
                let text = egui::RichText::new(format!(
                    "VM {:.2} / {:.1} ms",
                    clock.mean_ms, clock.budget_ms
                ))
                .monospace();
                ui.label(if over { text.color(amber).strong() } else { text })
                    .on_hover_text(
                        "the wall time this frame's scripts took — `tick_scripts`, the commands they left and the component writes — against `ScriptWorld::frame_time`, which is what the scheduler cuts a timeslice short at",
                    );
                if paused {
                    ui.label(egui::RichText::new("paused (P)").color(amber).strong());
                }
            });
            night_dial(ui, &mut dial, &mut settings);
            // The garden's own save, as two buttons (G5), **above** the list of creatures rather
            // than below it. The first version had them at the foot, beside the key hint, where
            // they read better — and in a browser at 1280×800 with fourteen creatures they were
            // off the bottom of the panel and under the VM window, which is the one place a
            // player cannot get at them. What is above the list cannot be pushed anywhere by the
            // list. The keys do the same thing and are what a PC player uses; the buttons are for
            // the browser, where F5 is the browser's Reload until the page takes it back and
            // where nobody has been told which key saves. They set a flag rather than saving
            // here: see `crate::Asked`.
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Save the garden (F5)")
                    .on_hover_text(platform::SAVE_WHERE)
                    .clicked()
                {
                    asked.save = true;
                }
                if ui.button("Load it back (F9)").on_hover_text(platform::SAVE_WHERE).clicked() {
                    asked.load = true;
                }
                if !note.text.is_empty() {
                    let text = egui::RichText::new(format!("{} · at {:.0} s", note.text, note.at))
                        .monospace();
                    ui.label(if note.bad {
                        text.color(amber).strong()
                    } else {
                        text.color(egui::Color32::from_gray(170))
                    });
                }
            });
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "click a creature, or Tab · hunger · insn/frame · frames/decision · the line it is waiting on",
                )
                .weak(),
            );
            // shrink to the rows there are: an empty half-panel pushes the VM panel off the
            // bottom of a 900-pixel window, and a garden of forty scrolls
            egui::ScrollArea::vertical().auto_shrink([false, true]).max_height(300.0).show(ui, |ui| {
                egui::Grid::new("creatures").num_columns(5).spacing([12.0, 2.0]).striped(true).show(ui, |ui| {
                    for row in &rows {
                        let selected = watched.entity == Some(row.entity);
                        let (r, g, b) = species_color(row.species);
                        let color = if selected {
                            egui::Color32::from_rgb(255, 236, 150)
                        } else {
                            egui::Color32::from_rgb(r, g, b)
                        };
                        let name = format!(
                            "{}{}{}",
                            if selected { "▸ " } else { "  " },
                            row.name,
                            if row.in_memory { "*" } else { "" }
                        );
                        if ui
                            .add(
                                egui::Label::new(
                                    egui::RichText::new(name).monospace().color(color),
                                )
                                .sense(egui::Sense::click())
                                .wrap_mode(egui::TextWrapMode::Extend),
                            )
                            .clicked()
                        {
                            watched.entity = Some(row.entity);
                            editor.open = true;
                        }
                        hunger_bar(ui, row.hunger);
                        ui.label(
                            egui::RichText::new(format!("{:>6} insn/f", row.insn_per_frame))
                                .monospace()
                                .color(egui::Color32::from_gray(170)),
                        );
                        ui.label(
                            egui::RichText::new(match row.per_decision {
                                Some(n) => format!("{n:>4.1} f/dec"),
                                None => "   – f/dec".into(),
                            })
                            .monospace()
                            .color(egui::Color32::from_gray(170)),
                        )
                        .on_hover_text(
                            "frames between this creature asking the world something — a component read or one of the game's two questions — and its task running again with the answer",
                        );
                        ui.label(
                            egui::RichText::new(&row.at)
                                .monospace()
                                .color(egui::Color32::from_rgb(200, 206, 216)),
                        )
                        .on_hover_text("the line of its own file it is parked on, which a VM that parks tasks can say and a callback cannot");
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.label(
                egui::RichText::new(
                    "Tab next · F1 editor · F2 VM · P pause · F5 save · F9 load · drag to turn, right-drag or WASD to slide, wheel to zoom, Home to reset",
                )
                .weak(),
            );
            // G6: the one line that says the rest of it is explained inside the game. It is drawn
            // in the colour the keys are, not the weak grey the line above it is, because a hint
            // nobody notices is the same as no hint — which is what the author's play found.
            ui.label(
                egui::RichText::new(rubevy_arena::Guide::HINT)
                    .color(egui::Color32::from_rgb(255, 226, 150))
                    .strong(),
            );
        });
}

/// **The night dial (G6b).** One slider, multiplying the moon, the ambient light and the sky
/// together (`crate::NightDial`).
///
/// It is in this panel rather than in the guide because it is not an explanation, and it is a
/// slider rather than a key because the question it asks — *how bright does the night have to be
/// on your screen?* — is answered by moving something and looking, not by pressing a key three
/// times and counting. The number is shown to two decimal places for the same reason it is
/// written to the log and remembered: it is meant to be **read off and reported**, and then baked
/// into `MOON_LUX` and the two beside it so that the dial goes back to being 1.0 for everybody.
///
/// It writes to the store when the drag *stops*, not while it moves. The garden brightens under
/// the pointer as it is dragged — that is the whole point of a slider — but a `localStorage`
/// write and a line of log on every one of sixty frames a second would be neither.
fn night_dial(
    ui: &mut egui::Ui,
    dial: &mut crate::NightDial,
    settings: &mut Option<bevy::prelude::ResMut<rubevy_arena::Settings>>,
) {
    let mut value = dial.0;
    let slider = ui
        .add(
            egui::Slider::new(&mut value, crate::NIGHT_DIAL_MIN..=crate::NIGHT_DIAL_MAX)
                .fixed_decimals(2)
                .text("night"),
        )
        .on_hover_text(
            "how bright the night is: the moonlight, the light that fills the shadows and the sky, all multiplied by this. 1.00 is what the game is built with — turn it until the night reads on your screen, and the number in the log is the one to tell us",
        );
    if slider.changed() {
        dial.0 = value;
    }
    // the click and the arrow keys change it without a drag; the drag reports itself when it ends
    if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
        if let Some(settings) = settings.as_mut() {
            settings.set("night", format!("{:.2}", dial.0));
        }
    }
}

/// 0 is dead, 100 is stuffed. Green while it is comfortable, amber under the line a beetle's own
/// script goes looking for grass at, red near the end.
fn hunger_bar(ui: &mut egui::Ui, hunger: f32) {
    let share = (hunger / crate::HUNGER_MAX).clamp(0.0, 1.0);
    let color = if hunger < 20.0 {
        egui::Color32::from_rgb(220, 90, 80)
    } else if hunger < 55.0 {
        egui::Color32::from_rgb(230, 180, 80)
    } else {
        egui::Color32::from_rgb(120, 200, 120)
    };
    // one cell of the grid: the bar and the number beside it
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 11.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, egui::Color32::from_gray(48));
        let mut filled = rect;
        filled.set_width(rect.width() * share);
        painter.rect_filled(filled, 2.0, color);
        ui.label(
            egui::RichText::new(format!("{hunger:>5.1}"))
                .monospace()
                .color(egui::Color32::from_gray(190)),
        );
    });
}

// ---------------------------------------------------------------------------------------------
// The window's own checks (`GARDEN_SELFTEST=1` with a window)
// ---------------------------------------------------------------------------------------------

/// What the ten headless checks cannot reach: the editor's buttons and the two keys.
///
/// It is sabibots' arrangement (`SABIBOTS_SELFTEST=1 docker/run.sh`) — drive the editor the way a
/// click would, by setting `Editor::action`, and look at what happened to the creatures and to the
/// file. Save is left out on purpose, since it writes to the repository.
#[derive(Resource, Default)]
pub struct WindowTest {
    step: usize,
    at: f32,
    /// what `beetle.rb` says on disk, to prove Apply did not touch it
    original: String,
    /// instructions every creature had run together, for the pause check
    insn: u64,
    /// ticks until the earliest sleeper is due, sampled on the first frame of the pause
    wake: Option<u32>,
}

impl WindowTest {
    /// Starts once the garden has been running for `at` seconds — long enough for every creature
    /// to have a task and for the editor to be showing one.
    pub fn after(at: f32) -> WindowTest {
        WindowTest { at, ..WindowTest::default() }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn window_selftest(
    time: Res<Time>,
    mut test: ResMut<WindowTest>,
    mut editor: ResMut<Editor>,
    mut watched: ResMut<Watched>,
    ruby: Res<RubyDir>,
    brains: Res<Brains>,
    panel: Res<VmInspector>,
    world: Res<ScriptWorld>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    minds: Query<(Entity, &Mind, Option<&ScriptTask>)>,
    tasks: Query<&ScriptTask>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < test.at {
        return;
    }
    let ok = |cond: bool, what: &str| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
    let spent = || tasks.iter().map(|t| world.vm.task_instructions(t.task())).sum::<u64>();
    let a_beetle = || minds.iter().find(|(_, m, _)| m.species == Species::Beetle);
    let beetles = || minds.iter().filter(|(_, m, _)| m.species == Species::Beetle);
    let rabbits = || minds.iter().filter(|(_, m, _)| m.species == Species::Rabbit);

    match test.step {
        // --- the keys first, while nothing has been restarted -------------
        0 => {
            let Some((entity, mind, _)) = a_beetle() else { return };
            watched.entity = Some(entity);
            test.original = platform::read(&brains.path(&ruby.0, mind.species)).unwrap_or_default();
            keys.press(KeyCode::KeyP);
            test.step = 1;
            test.at = now + 0.2;
        }
        1 => {
            // once the pause has taken hold, and not on the frame it was asked for: the scripts
            // of *that* frame had already run when the key was read
            test.insn = spent();
            test.wake = world.vm.task_next_wakeup_ticks();
            test.step = 2;
            test.at = now + 0.5;
        }
        2 => {
            ok(panel.paused && world.budget == 0, "P pauses: the scripts' budget is 0");
            ok(spent() == test.insn, "nothing ran while it was paused");
            ok(panel.open && !panel.frames.is_empty(), "the VM panel has the creature's frames");
            ok(panel.heap.as_ref().is_some_and(|h| h.live > 0), "the panel has the heap counters");
            let what = "nothing that was sleeping woke on the resume frame";
            match test.wake {
                Some(was) if was > 0 => {
                    let left = world.vm.task_next_wakeup_ticks();
                    let same = left == Some(was);
                    info!(
                        "selftest: {} {what}: the next one is due in {was} ticks, as it was half a second ago{}",
                        if same { "ok  " } else { "FAIL" },
                        if same { String::new() } else { format!(" (now {left:?})") },
                    );
                }
                other => info!("selftest: --   {what}: the next wakeup was {other:?} ticks off when the pause began"),
            }
            test.insn = spent();
            keys.release(KeyCode::KeyP);
            keys.press(KeyCode::KeyP);
            test.step = 3;
            test.at = now + 0.5;
        }
        3 => {
            ok(!panel.paused && world.budget > 0, "P again gives the budget back");
            ok(spent() > test.insn, "the creatures are thinking again");
            keys.release(KeyCode::KeyP);
            keys.press(KeyCode::F2);
            test.step = 4;
            test.at = now + 0.2;
        }
        4 => {
            ok(!panel.open, "F2 hides the VM panel");
            keys.release(KeyCode::F2);
            keys.press(KeyCode::F2);
            test.step = 5;
            test.at = now + 0.2;
        }
        5 => {
            ok(panel.open, "F2 shows it again");
            keys.release(KeyCode::F2);
            test.step = 6;
            test.at = now + 0.2;
        }
        // --- and then the editor, which restarts the scripts ---------------
        6 => {
            ok(
                editor.file == "beetle.rb" && editor.text.contains("creature \"Beetle\""),
                "the editor shows the file of the creature that was clicked",
            );
            editor.text = editor.text.replace("sleep 0.2", "sleep 0.9");
            ok(editor.changed(), "typing marks the text edited");
            test.insn = spent();
            editor.action = Some(EditorAction::Apply);
            // Every beetle is handed over in the frame `do_editor_actions` runs — there is no
            // queue any more (see `restart_species`) — so this is a breath for the new tasks to
            // be made and to run their first instructions, not a wait for a queue to drain.
            test.step = 7;
            test.at = now + 0.6;
        }
        7 => {
            let on_disk = platform::read(&brains.path(&ruby.0, Species::Beetle)).unwrap_or_default();
            let every = beetles().count();
            let applied = beetles().filter(|(_, m, _)| m.in_memory).count();
            ok(every > 0 && applied == every, "Apply restarts every beetle on the edited text");
            ok(
                brains.text(Species::Beetle).is_some_and(|t| t.contains("sleep 0.9")),
                "a beetle born from now on is born running it",
            );
            ok(rabbits().all(|(_, m, _)| !m.in_memory), "the rabbits are left alone");
            ok(on_disk == test.original, "Apply does not touch the file");
            ok(!editor.changed(), "after Apply the text is what the beetles run");
            // the restarted scripts are running: a new task that never ran would be a brain that
            // was replaced by nothing
            ok(
                beetles().all(|(_, m, _)| m.last_instructions > 0),
                "every restarted beetle's new task has run",
            );
            test.insn = spent();
            test.step = 8;
            test.at = now + 9.0;
        }
        8 => {
            // the VM did not stop: every task, restarted or not, is still being given the CPU
            ok(spent() > test.insn, "the whole VM is still running afterwards");
            editor.action = Some(EditorAction::Revert);
            test.step = 9;
            test.at = now + 0.6;
        }
        9 => {
            ok(
                beetles().count() > 0 && beetles().all(|(_, m, _)| !m.in_memory),
                "Revert puts every beetle back on the file",
            );
            ok(editor.text == test.original, "Revert shows the file again");
            ok(
                platform::read(&brains.path(&ruby.0, Species::Beetle)).unwrap_or_default() == test.original,
                "nothing was written",
            );
            // what the HUD's frames/decision says, with a window and whatever frame rate this
            // machine gives: the number the doc quotes is the headless one, at a steady 60 Hz
            for (_, mind, _) in minds.iter().take(3) {
                info!(
                    "selftest: {} — {} ask round trips, {} component reads, {} frames/decision",
                    mind.name,
                    mind.ask_trips,
                    mind.read_trips,
                    match mind.frames_per_decision() {
                        Some(n) => format!("{n:.2}"),
                        None => "–".into(),
                    }
                );
            }
            // A PC run was asked for the checks on a command line and should give the prompt
            // back. A page was asked for them in its address, by somebody who is looking at the
            // garden — and `AppExit` there does not end a run, it stops the canvas for good
            // (`platform::CHECKS_EXIT_WHEN_DONE`).
            if platform::CHECKS_EXIT_WHEN_DONE {
                exit.write(AppExit::Success);
            } else {
                info!("selftest: done — the garden keeps running (a page has nothing to exit to)");
            }
            test.step = 10;
        }
        _ => {}
    }
}
