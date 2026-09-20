//! **The window (G4).** The editor, the VM panel, the HUD, and the keys that drive them.
//!
//! None of it is new code in the sense of being written here: the editor and the inspector are
//! `rubevy-egui`'s, the same two SabiRuby Battle uses, and what is in this file is the game's
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

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use rubevy::{replace_script, MrbAsset, Script, ScriptTask, ScriptWorld};
use rubevy_egui::{Editor, EditorAction, VmInspector, Watch};

use crate::{
    compile, compile_source, platform, Brains, Creature, Hunger, Mind, Plant, RubyDir, Sky,
    Species,
};

/// Which creature the editor and the VM panel are about.
#[derive(Resource, Default)]
pub struct Watched {
    pub entity: Option<Entity>,
    /// **W3: the editor is on the rules rather than on the creature's file** (`F3`).
    ///
    /// It is a flag beside the creature and not a third value of it, because the creature being
    /// looked at is still the creature being looked at: the VM panel goes on showing its task and
    /// the HUD goes on marking its row while `world.rb` is in the editor, and `Tab` or a click
    /// brings the editor back to it. The two panels are about two different things and only one
    /// of them has a second file to show.
    pub world: bool,
}

/// **`P`: the world is stopped.** The budget the scripts get when they are not paused, kept
/// while they are — and the flag every rule of the garden is gated on ([`crate::is_still`]).
///
/// G4 stopped the Ruby and nothing else: the scheduler got a budget of 0, and the garden went on
/// growing, eating, breeding and starving round a set of creatures that slid along the velocity
/// their last thought had written. The author's third play said what that is worth — *"the VM
/// panel cannot be followed unless the world stops"* — so from G9 `P` is the world: the rules,
/// the clock, the breeding, the starving and the walking all stop together, and the drawing, the
/// camera, the panels, the editor and the save do not (G9).
#[derive(Resource, Default)]
pub struct Paused {
    was: Option<u64>,
    /// the same, for the world's VM (W1). The rules stop because `is_still` is the run condition
    /// on `RubevySet::<World>::tick()`, exactly as it was on the rule chain they replaced; this is
    /// the other half, so that the VM panel and a script that looks at its own budget see the two
    /// VMs saying the same thing about whether the world is stopped.
    was_world: Option<u64>,
}

impl Paused {
    /// Whether the world is stopped. It is the same fact as `ScriptWorld::budget == 0`, said by
    /// the thing that decided it: a run condition has no business reading the VM's budget.
    pub fn on(&self) -> bool {
        self.was.is_some()
    }
}

/// **The VM's share of the frame lives in `rubevy-egui` now** (G9). It was written here in G4,
/// and the VM panel — which both games have — is where it is shown, so it moved to the panel:
/// `VmInspectorPlugin` measures it in the windowed build and the headless one adds the two
/// systems itself (`main`). The names are re-exported because the callers here, the HUD and
/// `crate::stop_when_over`, are the garden's own.
pub use rubevy_egui::inspect::{vm_clock_end, vm_clock_start, VmClock, VmClockSet};

// ---------------------------------------------------------------------------------------------
// Picking a creature
// ---------------------------------------------------------------------------------------------

/// How near the ground under the cursor a creature has to be to be the one that was clicked.
///
/// *Reason only*: "within about a body's width". The sentence and the number do not quite agree —
/// a creature's radius is 0.40 to 0.50, so a body is 0.8 to 1.0 wide and this is nearer two of
/// them — and neither S5a nor S5b-3 found a record of which was meant, so **the number is the
/// default and the sentence is left as it was written** (`docs/numbers.md` §7-5).
pub const CLICK_REACH: f32 = 1.6;
/// How far the mouse may travel between press and release and still be a click rather than a drag
/// of the camera. *Reason only*: "a few pixels"; 6.0 itself is **unknown**.
pub const CLICK_SLOP: f32 = 6.0;

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
    eye: Res<crate::Eye>,
    mut pressed_at: Local<Option<Vec2>>,
) {
    let mut all: Vec<(Entity, Species)> = creatures.iter().map(|(e, c, _)| (e, c.species)).collect();
    all.sort_by_key(|(e, _)| e.to_bits());

    let egui_keyboard = typing.as_ref().is_some_and(|t| t.wants_keyboard_input());
    let egui_pointer = typing.as_ref().is_some_and(|t| t.wants_pointer_input());

    // **`F3`: the rules** (W3). It is read before the early return below, because a garden with
    // no creatures left standing in it is exactly the moment somebody wants to read the rules
    // that emptied it. There is no second press to go back: `Tab`, a click, or either species'
    // button is what puts the editor on a creature again, which is the same set of gestures that
    // chooses a creature in the first place.
    if !egui_keyboard && keys.just_pressed(KeyCode::F3) {
        watched.world = true;
        editor.open = true;
    }

    // a file button clicked along the top of the editor: the rules, or a creature that runs it
    let picked = editor.picked.take();
    if picked == Some(crate::Brains::WORLD as u64) {
        watched.world = true;
    } else if picked.is_some() {
        watched.world = false;
    }

    if all.is_empty() {
        watched.entity = None;
        return;
    }
    // the creature being looked at died: move on to one that has not. It does not take the editor
    // off the rules — a creature starving is not a request to be shown a different panel — but it
    // does open the editor, as it did before, for the case where it is showing a creature.
    if !watched.entity.is_some_and(|e| all.iter().any(|(o, _)| *o == e)) {
        watched.entity = Some(all[0].0);
        editor.open = true;
    }
    if let Some(picked) = picked
        && let Some((e, _)) = all.iter().find(|(_, s)| s.index() as u64 == picked)
    {
        watched.entity = Some(*e);
    }

    // the click, on the release, and only where egui did not want the mouse
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    if buttons.just_pressed(MouseButton::Left) {
        *pressed_at = if egui_pointer { None } else { cursor };
    }
    if buttons.just_released(MouseButton::Left)
        && let (Some(down), Some(up)) = (pressed_at.take(), cursor)
        && down.distance(up) <= eye.click_slop
        && let Some(entity) = creature_under(up, eye.click_reach, &cameras, &creatures)
    {
        watched.entity = Some(entity);
        watched.world = false;
        editor.open = true;
    }

    if egui_keyboard {
        return;
    }
    if keys.just_pressed(KeyCode::Tab) {
        let at = all.iter().position(|(e, _)| Some(*e) == watched.entity).unwrap_or(0);
        watched.entity = Some(all[(at + 1) % all.len()].0);
        watched.world = false;
        editor.open = true;
    }
    if keys.just_pressed(KeyCode::F1) {
        editor.open = !editor.open;
    }
}

/// The creature nearest to where a ray through `cursor` meets the ground.
fn creature_under(
    cursor: Vec2,
    reach: f32,
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
        if span <= reach && best.is_none_or(|(_, b)| span < b) {
            best = Some((entity, span));
        }
    }
    best.map(|(e, _)| e)
}

// ---------------------------------------------------------------------------------------------
// The editor
// ---------------------------------------------------------------------------------------------

/// The colour a species' name is written in — in the HUD's list, on the editor's two tabs, and on
/// the VM panel's heading.
///
/// **G8: it is the model's own colour now.** It used to be two colours picked to be legible on a
/// panel and nothing else, amber and pale blue, and the models were two browns; the author, who
/// could not tell a rabbit from a beetle on the grass, had nothing in the picture to check the
/// list against. `crate::species_tint` is one number per species, `tint_species` washes the model
/// in it and this writes the name in it, so the row that says `Beetle 405v0` is the colour of the
/// thing that is standing in the grass. Which also means the two are legible for the same reason:
/// a colour that reads against the field reads against a dark panel.
pub fn species_color(species: Species) -> (u8, u8, u8) {
    let (r, g, b) = crate::species_tint(species);
    let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
    (byte(r), byte(g), byte(b))
}

/// The colour of the third button, the one that is not a creature.
///
/// The two species' buttons are the colour of the model standing in the grass (G8, `species_tint`)
/// — a row in the HUD can be checked against the thing it is about. The rules are not a thing in
/// the grass and have no model to borrow a colour from, so `world.rb` takes the editor's own
/// foreground, the grey the listing writes an ordinary line in (`rubevy_egui::editor::listing`).
/// It reads as "the panel", which is what it is.
const WORLD_COLOR: (u8, u8, u8) = (210, 214, 222);

/// The editor follows the creature being looked at — or rather its **file**, which is what the
/// buttons along the top are: `beetle.rb`, `rabbit.rb`, and — since W3 — `world.rb`, the rules
/// themselves. That is everything in the game that is Ruby.
pub fn show_code(
    watched: Res<Watched>,
    brains: Res<Brains>,
    ruby: Res<RubyDir>,
    trouble: Res<crate::WorldTrouble>,
    minds: Query<&Mind>,
    creatures: Query<&Creature>,
    mut editor: ResMut<Editor>,
) {
    editor.choices = Species::ALL
        .iter()
        .map(|species| rubevy_egui::EditorChoice {
            id: species.index() as u64,
            label: format!(
                "{}{}",
                species.file(),
                if brains.text(*species).is_some() { "*" } else { "" }
            ),
            color: species_color(*species),
            dim: !creatures.iter().any(|c| c.species == *species),
        })
        // W3: and the rules. `dim` says the same thing about it that it says about a species with
        // nothing alive running it — the file is there and nothing is running it — which for the
        // rules means `world.rb` would not compile (`WorldTrouble`).
        .chain(std::iter::once(rubevy_egui::EditorChoice {
            id: crate::Brains::WORLD as u64,
            label: format!("{}{}", crate::WORLD_FILE, if brains.world().is_some() { "*" } else { "" }),
            color: WORLD_COLOR,
            dim: trouble.0.is_some(),
        }))
        .collect();
    // `F5` writes the garden down in this game, so it cannot also mean Apply; and there is one
    // Apply, because a species is everything that runs its file. Set before the early returns
    // below, so a frame with nothing selected does not leave `F5` meaning two things.
    editor.apply_key = None;
    editor.apply_all_label = None;
    editor.noun = "creature".into();
    editor.save_label = Some(platform::SAVE_LABEL.into());

    // **W3: the rules, which are one file and no creature.** Everything below this is about a
    // species; this is the whole of the other case, and it takes the same four buttons.
    if watched.world {
        editor.selected = Some(crate::Brains::WORLD as u64);
        let running = brains.world().cloned();
        let path = brains.world_path(&ruby.0);
        editor.show(crate::Brains::WORLD as u64, || {
            running.unwrap_or_else(|| platform::read(&path).unwrap_or_default())
        });
        editor.file = crate::WORLD_FILE.into();
        editor.label = "the rules".into();
        editor.in_memory = brains.world().is_some();
        editor.apply_label = "▶ Apply the rules (Ctrl+Enter)".into();
        editor.noun = "world".into();
        // No band and no shading. Those two come from `watch_minds`, which reads the line a
        // *creature's* task stands on and how long it has been standing there, and the world's
        // script has no `Mind` to keep either in (`give_the_world_its_rules`). The rules run one
        // pass a frame from top to bottom, so the line they are on is whichever line the frame
        // was sampled in the middle of, which is not a thing worth drawing.
        editor.current = None;
        editor.heat.clear();
        editor.elsewhere = None;
        return;
    }

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

/// **What the editor says when a text will not compile** (2026-09-18).
///
/// It used to be `not applied: beetle.rb would not compile (see the log)`, and the log had a line
/// number that was the prelude's length out — 600 where the author's file has 118. The number is
/// right now (rubevy's `in_the_authors_lines`), so the status can carry it: the first error, whole,
/// in the panel the typing was done in. The rest of them, if there were several, are still in the
/// log, and the caller logs the whole thing either way.
fn first_trouble(why: &str) -> &str {
    why.lines().next().unwrap_or(why)
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
    mut trouble: ResMut<crate::WorldTrouble>,
    rules: Query<Entity, With<crate::WorldScript>>,
) {
    let Some(action) = editor.action.take() else { return };
    if watched.world {
        do_world_actions(&mut commands, &mut editor, &ruby, &mut brains, &mut mrb, &mut trouble, &rules, action);
        return;
    }
    let Some(shown) = watched.entity else { return };
    let Ok((_, mind)) = minds.get(shown) else { return };
    let species = mind.species;
    let text = editor.text.clone();
    let path = brains.path(&ruby.0, species);

    match action {
        // there is one of these in this game (`apply_all_label` is None), and it is the species
        EditorAction::Apply | EditorAction::ApplyAll => {
            let (handle, lines) = match compile_source(&ruby.0, species.file(), &text, &mut mrb) {
                Ok(it) => it,
                Err(why) => {
                    error!("{why}");
                    editor.message = format!("not applied — {}", first_trouble(&why));
                    return;
                }
            };
            brains.set(species, Some(text));
            brains.hand_over(species, handle, lines, true);
            let n = restart_species(&mut commands, &mut minds, species, &brains);
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
            let (handle, lines) = match compile(&ruby.0, &path, &mut mrb) {
                Ok(it) => it,
                Err(why) => {
                    error!("{why}");
                    editor.message =
                        format!("saved {}, but it would not compile — {}", species.file(), first_trouble(&why));
                    return;
                }
            };
            brains.hand_over(species, handle, lines, false);
            restart_species(&mut commands, &mut minds, species, &brains);
            editor.applied(format!("saved to {}", species.file()));
        }
        EditorAction::Revert => {
            let Ok(source) = platform::read(&path) else {
                editor.message = format!("could not read {}", species.file());
                return;
            };
            let (handle, lines) = match compile(&ruby.0, &path, &mut mrb) {
                Ok(it) => it,
                Err(why) => {
                    error!("{why}");
                    editor.message = format!("{} does not compile — {}", species.file(), first_trouble(&why));
                    return;
                }
            };
            brains.set(species, None);
            brains.hand_over(species, handle, lines, false);
            restart_species(&mut commands, &mut minds, species, &brains);
            editor.reset_to(source, format!("back to {}", species.file()));
        }
    }
}

/// **The same four buttons, about `ruby/world.rb`** (W3).
///
/// Line for line it is the species branch above with the species taken out, and that is the
/// finding rather than a coincidence: applying a text to a running script, keeping it in memory
/// until Save, writing the file and going back to it are facts about *a script the game is
/// showing*, and nothing in them was ever about creatures. What differs is the count — a species
/// restarts a dozen creatures and the rules restart one entity — and the sentence each button
/// leaves behind, because "12 Beetles restarted on it" and "the garden is running these rules
/// now" are different news.
///
/// A text that will not compile leaves the rules that are running alone: the editor says so and
/// the garden goes on under the old ones, which is the same answer `give_the_world_its_rules`
/// gives a `world.rb` that will not compile at startup, and the opposite of what a `panic` would
/// say about a file the player is invited to edit.
#[allow(clippy::too_many_arguments)]
fn do_world_actions(
    commands: &mut Commands,
    editor: &mut Editor,
    ruby: &RubyDir,
    brains: &mut Brains,
    mrb: &mut Assets<MrbAsset>,
    trouble: &mut crate::WorldTrouble,
    rules: &Query<Entity, With<crate::WorldScript>>,
    action: EditorAction,
) {
    let Ok(entity) = rules.single() else { return };
    let text = editor.text.clone();
    let path = brains.world_path(&ruby.0);
    // the one entity's script is replaced by the road W1's twelfth check drives
    // (`crate::wear_the_rules`), and a garden that had no rules at all has them from this moment
    let mut wear = |handle, prelude_lines| {
        crate::wear_the_rules(commands, entity, handle, prelude_lines);
        trouble.0 = None;
    };

    match action {
        EditorAction::Apply | EditorAction::ApplyAll => {
            match crate::compile_world_source(&ruby.0, &text, mrb) {
                Ok((handle, lines)) => {
                    brains.set_world(Some(text));
                    wear(handle, lines);
                    editor.applied(
                        "the garden is running these rules now — in memory. Save to keep them."
                            .to_string(),
                    );
                }
                Err(why) => {
                    error!("{why}");
                    editor.message = format!("not applied — {}", first_trouble(&why));
                }
            }
        }
        EditorAction::Save => {
            if let Err(e) = platform::write(&path, &text) {
                editor.message = format!("could not save {}: {e}", crate::WORLD_FILE);
                return;
            }
            // the file says this now, so nothing is running a text of its own any more
            brains.set_world(None);
            match crate::compile_world(&ruby.0, mrb) {
                Ok((handle, lines)) => {
                    wear(handle, lines);
                    editor.applied(format!("saved to {}", crate::WORLD_FILE));
                }
                Err(why) => {
                    error!("{why}");
                    editor.message = format!(
                        "saved {}, but it would not compile — {}",
                        crate::WORLD_FILE,
                        first_trouble(&why)
                    );
                }
            }
        }
        EditorAction::Revert => {
            let Ok(source) = platform::read(&path) else {
                editor.message = format!("could not read {}", crate::WORLD_FILE);
                return;
            };
            match crate::compile_world(&ruby.0, mrb) {
                Ok((handle, lines)) => {
                    brains.set_world(None);
                    wear(handle, lines);
                    editor.reset_to(source, format!("back to {}", crate::WORLD_FILE));
                }
                Err(why) => {
                    error!("{why}");
                    editor.message =
                        format!("{} does not compile — {}", crate::WORLD_FILE, first_trouble(&why));
                }
            }
        }
    }
}

/// Every creature of one species, started over on a freshly compiled program — all of them, in
/// this frame.
///
/// Dropping `ScriptTask` is what stops the old script: rubevy terminates the task, closes the
/// queues it had subscribed to and ends the handler tasks waiting on them (`stop_removed_task`).
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
/// as it looked from here: ending a `ScriptTask` wakes each of the creature's six handler tasks
/// with `Rubevy::Unsubscribed`, their empty `rescue` makes the block's value `nil`, and
/// `Vm::task_run_limited` read that `nil` as "nothing left to run" and ended the host's whole
/// frame — one frame per task, so sixty frames for ten beetles
/// (sabiruby `docs/worklog/2026-09-17-task-end-nil.md`). 0.5.1 tells the two apart, and the queue
/// went with it: every creature of the species is handed over here, in the frame Apply was pressed.
///
/// **Every creature it can see** (S7). What it cannot see is a creature whose `Mind` was put in
/// the `Commands` queue earlier in this same frame — a creature born in it — and that one used to
/// be missed for good. It is not missed any more: the hand-over is numbered
/// ([`crate::Wearing`]) and [`catch_up_minds`] gives the program to whoever is behind, on this
/// frame or on the next one.
fn restart_species(
    commands: &mut Commands,
    minds: &mut Query<(Entity, &mut Mind)>,
    species: Species,
    brains: &Brains,
) -> usize {
    let Some(wearing) = brains.wearing(species) else { return 0 };
    let mut n = 0;
    for (entity, mut mind) in minds.iter_mut() {
        if mind.species != species {
            continue;
        }
        wear_mind(commands, entity, &mut mind, wearing);
        n += 1;
    }
    n
}

/// **One creature, handed the program its species is wearing.**
///
/// The body of what [`restart_species`] used to do inline, so that [`catch_up_minds`] does
/// exactly the same thing to a creature that arrives after the hand-over has gone by.
fn wear_mind(commands: &mut Commands, entity: Entity, mind: &mut Mind, wearing: &crate::Wearing) {
    mind.in_memory = wearing.in_memory;
    mind.prelude_lines = wearing.prelude_lines;
    mind.generation = wearing.generation;
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
    // S2: the three lines this was — `ScriptTask` off, `ScriptDone` off, the new `Script` on
    // — are rubevy's `replace_script` (R6). Forgetting the `ScriptDone` is the invisible
    // half: a creature whose script had run to its end could never be given another one.
    replace_script(commands, entity, Script::new(wearing.handle.clone()).with_name(&mind.name).with_priority(100));
}

/// **Whoever is behind their species' program, given it** (S7).
///
/// The hand-over in [`restart_species`] reaches every creature that is in the `Query` when the
/// editor's button is taken. A creature born in that same frame is not: `children_arrive` builds
/// its body and queues its `Mind` through `Commands`, and there is no ordering edge between that
/// system and `do_editor_actions`, so Bevy puts no sync point between them and the `Mind` is
/// still in the queue when the hand-over goes round. Before S7 that creature kept the program it
/// was born with **for the rest of the run** — the editor said twelve beetles had been restarted
/// and one of them was quietly running the other text
/// (`docs/worklog/2026-09-20-window-check-flakes.md` §3).
///
/// Asking "is this creature behind?" every frame is the same question asked where it can be
/// answered. It costs one comparison per creature per frame and it hands nothing over in a frame
/// where nothing was applied, since every number matches. **It adds no number**: a generation is
/// a count of hand-overs, not a threshold.
///
/// What it costs the creature that was missed is one frame: it runs the old program for the
/// frame it was born in and is handed the new one on the next. That is the whole change in
/// behaviour, and it is against never being handed it at all.
pub fn catch_up_minds(mut commands: Commands, brains: Res<Brains>, mut minds: Query<(Entity, &mut Mind)>) {
    for (entity, mut mind) in minds.iter_mut() {
        let Some(wearing) = brains.wearing(mind.species) else { continue };
        if mind.generation == wearing.generation {
            continue;
        }
        info!(
            "{} was born in the frame its species was handed a new program — handing it over now",
            mind.name
        );
        wear_mind(&mut commands, entity, &mut mind, wearing);
    }
}

/// A creature file saved from outside the game restarts that species, exactly as Save does — and
/// `world.rb` saved from outside restarts the rules, exactly as their Save does (W3).
#[allow(clippy::too_many_arguments)]
pub fn reload_changed(
    mut commands: Commands,
    watch: Option<Res<Watch>>,
    ruby: Res<RubyDir>,
    mut brains: ResMut<Brains>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut minds: Query<(Entity, &mut Mind)>,
    mut editor: ResMut<Editor>,
    mut trouble: ResMut<crate::WorldTrouble>,
    rules: Query<Entity, With<crate::WorldScript>>,
) {
    let Some(watch) = watch else { return };
    for path in watch.changed() {
        // **W3: the rules' own two files.** `world_prelude.rb` is `world.rb`'s prelude the way
        // `prelude.rb` is a creature's, and until now it matched the `ends_with("prelude.rb")`
        // below — so saving it restarted every beetle and every rabbit and left the rules it
        // actually belongs to running the old text. Both tests name the file exactly now.
        let is_world = path.file_name().is_some_and(|f| {
            f == std::ffi::OsStr::new(crate::WORLD_FILE)
                || f == std::ffi::OsStr::new(crate::WORLD_PRELUDE_FILE)
        });
        if is_world {
            // rules applied in the editor keep their text until Save or Revert, as a species does
            if brains.world().is_none()
                && let Ok(entity) = rules.single()
            {
                match crate::compile_world(&ruby.0, &mut mrb) {
                    Ok((handle, lines)) => {
                        crate::wear_the_rules(&mut commands, entity, handle, lines);
                        trouble.0 = None;
                        if editor.key == Some(crate::Brains::WORLD as u64)
                            && !editor.changed()
                            && let Ok(source) = platform::read(&brains.world_path(&ruby.0))
                        {
                            editor.reset_to(source, "the file changed");
                        }
                        info!("{} changed: the garden is running it", crate::WORLD_FILE);
                    }
                    Err(why) => {
                        error!("{why}");
                        editor.message = first_trouble(&why).to_string();
                    }
                }
            }
            continue;
        }
        for species in Species::ALL {
            let mine = brains.path(&ruby.0, species);
            if path != mine && path.file_name() != Some(std::ffi::OsStr::new(crate::PRELUDE_FILE)) {
                continue;
            }
            // a species running a text applied in the editor keeps it until Save or Revert
            if brains.text(species).is_some() {
                continue;
            }
            let (handle, lines) = match compile(&ruby.0, &mine, &mut mrb) {
                Ok(it) => it,
                Err(why) => {
                    error!("{why}");
                    editor.message = first_trouble(&why).to_string();
                    continue;
                }
            };
            if editor.key == Some(species.index() as u64)
                && !editor.changed()
                && let Ok(source) = platform::read(&mine)
            {
                editor.reset_to(source, "the file changed");
            }
            brains.hand_over(species, handle, lines, false);
            let n = restart_species(&mut commands, &mut minds, species, &brains);
            info!("{} changed: {n} restarted", species.file());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The VM panel
// ---------------------------------------------------------------------------------------------

/// `F2` shows and hides the VM panel; `P` stops the world and starts it again.
///
/// **Two halves, and they are a pair.** The Ruby stops because `ScriptWorld::budget` goes to 0:
/// the scheduler returns before handing any task the CPU, so nothing in the VM moves and the
/// panel's numbers stand still while they are read. rubevy `fa37eaa` stops the scheduler's clock
/// with it, so a creature half way through a `sleep 0.2` is still half way through it when the
/// budget comes back. The **world** stops because [`Paused::on`] is false in
/// [`crate::is_still`], the run condition every rule of the garden already carried for the sake
/// of loading a save — so `day_night`, `move_creatures`, `separate`, `startle` and, since W1,
/// the world's own tick (which is where the grass, the hunger, the eating, the breeding and the
/// starving are now: `ruby/world.rb`) are simply not run, and `crate::hold_the_clock` walks
/// `Sky::shift` back by the
/// frame's own length so that the garden's clock does not run on either.
///
/// Stopping only the first half is what G4 did, and the author's third play is why it is not
/// enough: the creatures slid on along the velocity their last thought had written, the day
/// turned over, and a garden left paused for a minute came back with everything starved.
///
/// **`P` does not open the VM panel any more.** From G9 the pause is about the world rather than
/// about the panel, and the panel opens on `F2` (and on `--shot --vm`) — a player who pauses to
/// watch a rabbit should not have a debugger thrown over the middle of the window.
pub fn inspect_keys(
    keys: Res<ButtonInput<KeyCode>>,
    typing: Option<Res<bevy_egui::input::EguiWantsInput>>,
    mut panel: ResMut<VmInspector>,
    mut world: ResMut<ScriptWorld>,
    mut rules: ResMut<ScriptWorld<crate::World>>,
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
                rules.budget = paused.was_world.take().unwrap_or(rules.budget);
                panel.paused = false;
            }
            None => {
                paused.was = Some(world.budget);
                paused.was_world = Some(rules.budget);
                world.budget = 0;
                rules.budget = 0;
                panel.paused = true;
            }
        }
    }
}

/// The VM panel follows the creature being looked at, as the editor follows its file — **or the
/// rules, when the editor is on them** (`F3`, 2026-09-18).
///
/// Until now `F2` could only ever show a creature. The reason was in `rubevy-egui`: the panel's
/// `fill` named `ScriptWorld` by its default marker, so the second VM — the one `world.rb` runs
/// in — was not a thing it could be handed (`docs/worklog/2026-09-17-garden-world.md` §24.1). It
/// takes either now, and this is where the garden decides which: `Watched::world` is the same
/// flag the editor reads, so the two panels stay on the same file and `Tab` or a click brings
/// both back to the creature.
///
/// Everything the panel says about a creature it can say about the rules, because none of it was
/// ever about creatures: **what it is waiting for** is worked out from the frames alone
/// (`rubevy_egui::inspect::why`), and the world's task waits for exactly two things — one pass
/// of `each_frame` ends on `Rubevy.ask("frame")`, which is a `Rubevy::Proxy` ask like any other,
/// and a timer task made by `every` is asleep. **Where it is waiting** is the innermost frame of
/// `world.rb` itself, once the world's prelude has been taken off it (`crate::WorldPrelude`, the
/// world's half of what a creature keeps in its `Mind`). The two numbers are `WorldMeter`'s — the
/// last pass and the middle one — where a creature's are its `Mind`'s.
#[allow(clippy::too_many_arguments)]
pub fn show_vm(
    watched: Res<Watched>,
    world: Res<ScriptWorld>,
    rules: Res<ScriptWorld<crate::World>>,
    meter: Res<crate::WorldMeter>,
    trouble: Res<crate::WorldTrouble>,
    prelude: Res<crate::WorldPrelude>,
    mut panel: ResMut<VmInspector>,
    minds: Query<(&Mind, Option<&ScriptTask>)>,
    world_task: Query<Option<&ScriptTask<crate::World>>, With<crate::WorldScript>>,
) {
    if !panel.open {
        return;
    }
    if watched.world {
        panel.title = format!("the rules  {}", crate::WORLD_FILE);
        let Ok(Some(task)) = world_task.single() else {
            panel.clear(match &trouble.0 {
                Some(why) => format!("the world has no rules: {why}"),
                None => "the rules have no task yet".into(),
            });
            return;
        };
        panel.spent = meter.last_pass;
        panel.per_frame = meter.median();
        // the world's program is `world_prelude.rb` in front of `world.rb`, not `prelude.rb` in
        // front of a creature's, and the panel names the frames that fall in the prelude
        panel.prelude_file = Some(crate::WORLD_PRELUDE_FILE.into());
        let title = panel.title.clone();
        panel.fill(&rules, task.task(), title, prelude.0);
        return;
    }
    let Some(entity) = watched.entity else { return };
    let Ok((mind, script)) = minds.get(entity) else {
        panel.clear("nothing is selected");
        return;
    };
    let Some(script) = script else {
        panel.title = mind.name.clone();
        panel.clear("this creature has no task: it has just been given a new behaviour, or it starved");
        return;
    };
    panel.spent = mind.spent;
    // G9: the panel's top half says insn/frame, and this is the same average the HUD's column is
    panel.per_frame = Some(mind.last_instructions / mind.frames.max(1));
    panel.prelude_file = None;
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
    /// **insn/decision**: what one pass of this creature's behaviour loop costs it in VM
    /// instructions. It stands where G4's *frames* per decision stood, which stopped meaning
    /// anything when a component read stopped costing a frame (`Mind::instructions_per_decision`).
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
            per_decision: mind.instructions_per_decision(),
            at: mind.at.clone(),
            in_memory: mind.in_memory,
        })
        .collect();
    rows.sort_by_key(|r| (r.species.index(), std::cmp::Reverse(r.entity.to_bits())));
    rows
}

/// **What the HUD draws itself with, as one system parameter** (S5b-3).
///
/// `draw_hud` was at fifteen of Bevy's sixteen and the numbers it wanted are two resources, so
/// they travel as one — the answer `VmReport` and `FakePointer` already give to the same limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HudLook<'w> {
    /// the hunger bar: how big it is and where it changes colour
    picture: Res<'w, crate::Picture>,
    /// the night dial: how far it goes
    light: Res<'w, crate::Light>,
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
    mut settings: Option<ResMut<games_shell::Settings>>,
    trouble: Res<crate::WorldTrouble>,
    meter: Res<crate::WorldMeter>,
    rules: Res<ScriptWorld<crate::World>>,
    look: HudLook,
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
            // **W3: what the rules cost, beside what the creatures cost.**
            //
            // The world's script runs exactly one pass of `each_frame` per frame (it waits on
            // `Rubevy.ask("frame")`, and nothing else it asks costs a frame — `world_prelude.rb`),
            // so the instructions it ran between two frames *are* one pass of the rules. That is
            // what makes this a plainer number than the creatures' `insn/decision` column, and it
            // is the number the world VM's budget was chosen from (`crate::WorldMeter`,
            // `install_world_answers`) — so the two are shown together.
            //
            // The budgets are per VM and nothing caps them together (rubevy `docs/host-api.md`,
            // "Two VMs in one app"), which is the other reason this is its own line rather than
            // being added to the one above: `VM … ms` is the creatures' VM, and a garden whose
            // rules have been rewritten into something expensive should be able to say so here
            // without the creatures' figure moving.
            ui.horizontal_wrapped(|ui| {
                let budget = rules.budget;
                let over = budget > 0 && meter.last_pass >= budget;
                let text = egui::RichText::new(format!(
                    "the rules {} / {budget} insn/frame · {:.2} ms",
                    meter.last_pass, meter.mean_ms
                ))
                .monospace();
                ui.label(if over { text.color(amber).strong() } else { text }).on_hover_text(
                    "what one pass of ruby/world.rb's `each_frame` cost this frame — the grass, the hunger, the eating, the pairing and the starving, all of it — against the world VM's own budget, and the wall time its tick took. F3 opens the file",
                );
            });
            // **W1: a garden whose rules will not compile runs anyway, and says so.**
            //
            // `ruby/world.rb` is the rules — the grass, hunger, eating, starving — and it is a
            // file a player is invited to edit. A typo in it leaves a world where the sun still
            // turns and the creatures still walk and nothing else happens, which is a strange
            // thing to be left to work out for oneself. One line, in the colour the panel uses for
            // "something is not right".
            if let Some(why) = &trouble.0 {
                ui.label(
                    egui::RichText::new(format!("world.rb: {why} — nothing grows and nobody gets hungry"))
                        .color(amber),
                );
            }
            night_dial(ui, &look.light, &mut dial, &mut settings);
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
                    "click a creature, or Tab · hunger · insn/frame · insn/decision · the line it is waiting on",
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
                            watched.world = false;
                            editor.open = true;
                        }
                        hunger_bar(ui, &look.picture, row.hunger);
                        ui.label(
                            egui::RichText::new(format!("{:>6} insn/f", row.insn_per_frame))
                                .monospace()
                                .color(egui::Color32::from_gray(170)),
                        );
                        ui.label(
                            egui::RichText::new(match row.per_decision {
                                Some(n) => format!("{n:>5.0} i/dec"),
                                None => "    – i/dec".into(),
                            })
                            .monospace()
                            .color(egui::Color32::from_gray(170)),
                        )
                        .on_hover_text(
                            "VM instructions this creature spends on one pass of its behaviour's loop — everything between one `sleep` and the next: the component reads, the question the game answers, and the `act`. It used to be the frames a decision waited for; a component read is answered inside the tick that asks it now, so a decision costs instructions rather than frames",
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
                    "Tab next · F1 editor · F2 VM · F3 the rules · P pause · F5 save · F9 load · drag to turn, right-drag or WASD to slide, wheel to zoom, Home to reset",
                )
                .weak(),
            );
            // G6: the one line that says the rest of it is explained inside the game. It is drawn
            // in the colour the keys are, not the weak grey the line above it is, because a hint
            // nobody notices is the same as no hint — which is what the author's play found.
            ui.label(
                egui::RichText::new(games_shell::Guide::HINT)
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
    light: &crate::Light,
    dial: &mut crate::NightDial,
    settings: &mut Option<bevy::prelude::ResMut<games_shell::Settings>>,
) {
    let mut value = dial.0;
    let slider = ui
        .add(
            egui::Slider::new(&mut value, light.dial_min..=light.dial_max)
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
fn hunger_bar(ui: &mut egui::Ui, picture: &crate::Picture, hunger: f32) {
    let share = (hunger / crate::HUNGER_MAX).clamp(0.0, 1.0);
    let color = if hunger < picture.hunger_low {
        egui::Color32::from_rgb(220, 90, 80)
    } else if hunger < picture.hunger_warn {
        egui::Color32::from_rgb(230, 180, 80)
    } else {
        egui::Color32::from_rgb(120, 200, 120)
    };
    // one cell of the grid: the bar and the number beside it
    ui.horizontal(|ui| {
        let (rect, _) = ui
            .allocate_exact_size(egui::vec2(picture.hunger_bar[0], picture.hunger_bar[1]), egui::Sense::hover());
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
    /// **What the next step is waiting for besides its clock** (S7, [`Turn`]).
    turn: Turn,
    /// and how many frames it has been waiting for it
    waited: u32,
    /// and how many it may wait — [`scheduler_frames`], worked out from the budget the run is
    /// actually giving the creatures' VM (S5b-3). It is kept here rather than read in
    /// `window_selftest`, which is at Bevy's sixteen parameters.
    frames: u32,
    /// **and the frames that come before the VM's, where there are any** (S5b-5).
    ///
    /// [`scheduler_frames`] counts from the moment something was asked of the VM. A step that
    /// asks by pressing a key has one frame in front of that which is nobody's scheduler:
    /// `rubevy-egui`'s panel reads `Ctrl+Enter` in the egui pass, so `Editor::action` is not set
    /// until the frame after the one the check pressed it in, and `do_editor_actions` takes it
    /// the frame after that. It is the same frame [`Turn::EguiHasThePointer`] is entirely about,
    /// counted here because it comes *before* the wait rather than being the wait.
    ///
    /// Set beside the `turn` it belongs to and cleared with it. It is only ever 0 or 1, and the
    /// 1 is that frame.
    spare: u32,
    /// every beetle there was when Apply was pressed, so that the one born in that very frame
    /// can be told from them (S7)
    beetles_before: Vec<Entity>,
    /// what `beetle.rb` says on disk, to prove Apply did not touch it
    original: String,
    /// the same for `world.rb` (W3)
    world_original: String,
    /// **the day the edited rules are to say**, which is half of whatever `world.rb` was really
    /// carrying when the check read it (S5b-5) rather than a second copy of the 60 it ships with
    day_length: f32,
    /// instructions every creature had run together, for the pause check
    insn: u64,
    /// ticks until the earliest sleeper is due, sampled on the first frame of the pause
    wake: Option<u32>,
    /// where every creature stood when the pause began — the whole world, entity by entity, so
    /// that "nothing moved" is an equality rather than a tolerance (G9)
    places: Vec<(Entity, Vec3)>,
    /// what each of them had left in its meter
    hunger: Vec<(Entity, f32)>,
    /// where the sun stood
    phase: f32,
    /// how far back the camera stood before a wheel message was written (2026-09-18)
    distance: f32,
    /// **and whether egui was holding the pointer in that same frame** (S5b-5).
    ///
    /// The two wheel checks are about one moment: a notch of the wheel, at a place, with a panel
    /// drawn there or not. Whether egui had the pointer is half of what that moment was, so it
    /// is read where the wheel is turned rather than where the verdict is written — a fifth of a
    /// second later, by which time egui may have taken the pointer or let it go for reasons of
    /// its own. Reading it late is what made `and with the panel closed the same wheel in the
    /// same place zooms` fail in four runs of eighty-eight with eight of them at once, both
    /// before S7's changes and after them (`docs/worklog/2026-09-20-window-check-fixes.md`).
    held: bool,
    /// **and whether the editor was in fact drawn in that frame** (S5b-5).
    ///
    /// The control step of the wheel checks shuts the editor and then turns the wheel where the
    /// editor used to be, so its whole premise is that nothing is drawn there. The game can take
    /// that premise away between the two: `choose_watched` opens the editor when the creature
    /// being looked at dies, and a creature starving in that second or two is the garden's own
    /// business. A run where that happened has not failed the thing being checked and has not
    /// passed it either — it is the third verdict.
    shown: bool,
    /// **and what egui's answer was made of, in that same frame** (S5b-5) — `wants_pointer_input`
    /// and `is_pointer_over_area` apart, and whether the editor was open. It is printed only
    /// where the verdict is a FAIL, as a stage direction: a run that fails one of these two
    /// checks has to say *which* of egui's two answers was true, or the next person measures it
    /// all over again.
    held_detail: String,
}

/// **A pointer the checks can put where they like, and a wheel they can turn** (2026-09-18).
///
/// `window_selftest` was already at fourteen system parameters and Bevy's limit is sixteen, so
/// the five the wheel check wants travel as one, the way [`crate::VmReport`] does for the two VMs.
///
/// The pointer is moved by writing the `WindowEvent::CursorMoved` that winit would have written:
/// bevy_egui reads that message in `PreUpdate` and it is the only thing that tells egui where the
/// pointer is (`bevy_egui::input::write_pointer_moved_and_button_messages_system`). Writing it is
/// the same kind of forgery as `keys.press(KeyCode::F2)` above — the input the operating system
/// would have delivered, delivered by the check instead — and it is the only one available: there
/// is no way to ask egui "pretend the pointer is here", and warping the real cursor needs a
/// desktop that will do it.
#[derive(bevy::ecs::system::SystemParam)]
pub struct FakePointer<'w, 's> {
    windows: Query<'w, 's, (Entity, &'static Window)>,
    orbit: Res<'w, crate::Orbit>,
    wheel: MessageWriter<'w, bevy::input::mouse::MouseWheel>,
    events: MessageWriter<'w, bevy::window::WindowEvent>,
    egui: Option<Res<'w, bevy_egui::input::EguiWantsInput>>,
    /// **Where the editor actually is**, which since S5b-1 is a setting and not a constant: the
    /// game — or a player's `garden.settings.txt` — may have asked for a panel of another size,
    /// and a check that read the default would then be pointing at a rectangle nobody drew.
    editor: Res<'w, rubevy_egui::EditorLayout>,
    /// **and the guide, which is drawn in the middle of the window over everything else**
    /// (S5b-5). It starts open, nothing in these checks shuts it, and the control step of the
    /// wheel checks needs a point with nothing under it — see [`FakePointer::hide_the_guide`].
    guide: Option<ResMut<'w, games_shell::Guide>>,
}

impl FakePointer<'_, '_> {
    /// Whether egui is holding the pointer, as [`crate::orbit_camera`] asks it.
    fn egui_has_it(&self) -> bool {
        self.egui.as_ref().is_some_and(|e| e.wants_pointer_input() || e.is_pointer_over_area())
    }

    /// **Shut the guide before the wheel is turned** (S5b-5).
    ///
    /// The guide is anchored to the centre of the window and drawn in `Order::Foreground`, above
    /// every other panel, and it is open when a game starts (`games_shell::Guide`). Nothing in
    /// these checks shuts it, so until now the two wheel checks were relying on the aim point
    /// falling outside it — which it did, by 150 pixels, for as long as the point was the middle
    /// of the editor's rectangle. Taking the point to the rectangle's inner edge put it inside
    /// the guide instead, and the control step failed three runs out of three with
    /// `is_pointer_over_area=true` and the editor shut: the panel under the pointer was the
    /// guide.
    ///
    /// It is set rather than typed (`H` toggles it) because the step wants it *shut*, not
    /// *changed*, and because the same step already puts `Editor::open` where it wants it. What
    /// it buys is that neither wheel check depends on where the guide happens to be.
    fn hide_the_guide(&mut self) {
        if let Some(guide) = self.guide.as_mut() {
            guide.open = false;
        }
    }

    /// The same, taken apart, for a failing check to print (S5b-5).
    fn egui_answers(&self) -> String {
        match self.egui.as_ref() {
            Some(e) => format!(
                "wants_pointer_input={} is_pointer_over_area={}",
                e.wants_pointer_input(),
                e.is_pointer_over_area()
            ),
            None => "egui is not in this run".into(),
        }
    }

    /// **A point inside the editor's rectangle, near the edge that moves when the width does**
    /// (S5b-5).
    ///
    /// It used to be the middle of the rectangle, and S5b-1 found what that is worth: the panel
    /// is pinned to the right of the window, so the middle of the rectangle the *settings*
    /// describe is inside the panel whatever width the panel really has. A run where
    /// `editor_width` had been ignored altogether would have passed — the check would have been
    /// pointing at a rectangle nobody drew and hitting the panel anyway. (The author ran it with
    /// `editor_width=1400` by hand to get round that, which is the check asking to be mended.)
    ///
    /// So the point is taken from the rectangle's **inner edge** — one `margin` inside the left
    /// side, which is the one side that moves when the width changes — and half way down. A panel
    /// narrower than the settings say does not reach it.
    ///
    /// **It is held to the right half of the window.** The two other panels are at the left (the
    /// HUD at the top, the VM inspector at the bottom), and the control step of the wheel checks
    /// needs a point with *nothing* under it once the editor is shut. The clamp only bites for an
    /// editor more than half the window wide, and there the check has already said what it can
    /// about the width.
    fn over_the_editor(&self) -> Option<Vec2> {
        let (_, window) = self.windows.iter().next()?;
        let (margin, width, height) = (self.editor.margin, self.editor.width, self.editor.height);
        let inner_edge = window.width() - margin - width + margin;
        Some(Vec2::new(inner_edge.max(window.width() * 0.5), margin + height * 0.5))
    }

    /// Which window the forged input is about: the primary one, which is the only one these
    /// games open.
    fn the_window(&self) -> Option<Entity> {
        self.windows.iter().next().map(|(entity, _)| entity)
    }

    fn point_at(&mut self, at: Vec2) {
        let Some(window) = self.the_window() else { return };
        self.events.write(bevy::window::WindowEvent::CursorMoved(bevy::window::CursorMoved {
            window,
            position: at,
            delta: None,
        }));
    }

    /// One notch of a PC mouse wheel, towards the garden.
    fn turn_the_wheel(&mut self) {
        let Some(window) = self.the_window() else { return };
        self.wheel.write(bevy::input::mouse::MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }
}

/// **The two VMs, as one system parameter** (S5b-5).
///
/// `window_selftest` was at fifteen of Bevy's sixteen and the pause is about **both** VMs — `P`
/// takes the budget off the creatures' and off the world's together ([`inspect_keys`]) — so the
/// two travel as one rather than as the fifteenth and the sixteenth. It is the answer
/// [`FakePointer`] and `crate::VmReport` give to the same limit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BothVms<'w> {
    /// every creature's script
    creatures: Res<'w, ScriptWorld>,
    /// `ruby/world.rb` (W1)
    rules: Res<'w, ScriptWorld<crate::World>>,
}

/// **What two samples of "one value per creature" say about each other** (S5b-5).
///
/// The pause checks used to be `places() == test.places`, a `Vec` compared with `Vec`, which
/// compares **the order the creatures came out of the `Query` in** as well as the values. That
/// order is per archetype, so one creature moving to another archetype between the two samples —
/// rubevy hanging a `ScriptTask` on it, `dress_animations` hanging an `Animated`, an `Eating`
/// that was on its way when the world stopped — made a still world look like a moving one. S8
/// reproduced it by adding an empty marker to one creature and nothing else: one run in one
/// (`docs/worklog/2026-09-20-writes-landing-in-a-pause.md`).
///
/// So the sample is looked up **by entity**, and what comes out is said in three parts, because
/// they are three different pieces of news: a creature that is not there any more, a creature
/// that was not there before, and a creature whose value moved. There is **no tolerance** —
/// equality is what the check means, and a threshold here would be a number with nothing behind
/// it.
struct Changes {
    /// entities in the first sample and not in the second
    gone: Vec<Entity>,
    /// entities in the second and not in the first
    fresh: Vec<Entity>,
    /// entities in both whose value is not the same, as "who: was -> is"
    moved: Vec<String>,
}

impl Changes {
    /// Whether the same creatures are in both samples, whatever their values did.
    fn same_creatures(&self) -> bool {
        self.gone.is_empty() && self.fresh.is_empty()
    }

    /// The names for a sentence, or nothing at all when there is nothing to say.
    fn cast(&self) -> String {
        match (self.gone.len(), self.fresh.len()) {
            (0, 0) => String::new(),
            _ => format!(" (gone: {:?}; new: {:?})", self.gone, self.fresh),
        }
    }

    fn what_moved(&self) -> String {
        if self.moved.is_empty() { String::new() } else { format!(" ({})", self.moved.join("; ")) }
    }
}

fn what_changed<T: PartialEq + std::fmt::Debug>(
    before: &[(Entity, T)],
    after: &[(Entity, T)],
) -> Changes {
    let mut changes = Changes { gone: Vec::new(), fresh: Vec::new(), moved: Vec::new() };
    for (entity, was) in before {
        match after.iter().find(|(e, _)| e == entity) {
            None => changes.gone.push(*entity),
            Some((_, now)) if now != was => {
                changes.moved.push(format!("{entity}: {was:?} -> {now:?}"))
            }
            Some(_) => {}
        }
    }
    for (entity, _) in after {
        if !before.iter().any(|(e, _)| e == entity) {
            changes.fresh.push(*entity);
        }
    }
    changes
}

impl WindowTest {
    /// Starts once the garden has been running for `at` seconds — long enough for every creature
    /// to have a task and for the editor to be showing one.
    pub fn after(at: f32, budgets: &crate::Budgets) -> WindowTest {
        WindowTest { at, frames: scheduler_frames(budgets), ..WindowTest::default() }
    }

    /// **The frame the editor pressed Apply** — the one frame a forced birth has to land in
    /// ([`birth_in_the_apply_frame`]). Step 7 is the step that presses it and it sets `step` to
    /// 8 as it goes, so the frame to look for is the first frame of step 8.
    fn pressing_apply(&self) -> bool {
        self.step == 8
    }
}

/// **`day_length` as `ruby/world.rb` writes it** (S5b-5), so that a check about what the file
/// says reads the file rather than a number somebody typed twice.
///
/// It is not one of the `def name = number` lines the rest of that file is written in: it is a
/// word and a number, said once inside `world do` and handed over at the start
/// (`garden.rules(day_length:)`), because the sun is drawn in Rust.
fn day_length_in(text: &str) -> Option<f32> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("day_length "))
        .find_map(|rest| rest.split_whitespace().next()?.parse().ok())
}

/// The same line, written back — how the check edits the text it has just read.
fn day_length_line(value: f32) -> String {
    format!("day_length {value:.1}")
}

/// **What a step is waiting for that is not a length of time** (S7).
///
/// A check that has just restarted a script is not waiting for the world to move on; it is
/// waiting for **the VM's scheduler to give the new task a turn**, and that is counted in the
/// VM's frames and not in seconds. The old wait was 0.6 s, which is five frames on a quiet PC,
/// three on a PC that is sharing its CPU and three in a browser — one number standing for three
/// different amounts of the thing actually wanted, which is why the checks around Apply and
/// Revert flaked (`docs/worklog/2026-09-20-window-check-flakes.md`, cause B).
///
/// So the step says what it is waiting **for**, the wait ends the moment it has it, and
/// [`scheduler_frames`] is where it gives up and judges anyway — which is a FAIL, and a true
/// one: the VM has had turns to hand out and has not handed one to a task that is ready, or egui
/// has had the pointer put on it and has not noticed.
///
/// **S5b-5 took the last two seconds out** ([`Turn::TheDayIs`]). S7 changed the four waits that
/// had flaked and left the two around `world.rb`'s own Apply and Revert at 0.6 s, saying they
/// were the same shape and had simply not failed yet. They are the same shape, so they are the
/// same wait now; **the seconds that are left in this file are the ones that mean seconds** — two
/// seconds of a pause, half a second of walking, nine seconds of the VM going on running — and
/// 0.2 s twice for a key that a frame of Bevy's input has to see.
// `Eq` is not derived any more: [`Turn::TheDayIs`] carries the day length the rules are to have
// said, which is the `f32` `Sky::day_length` is. Nothing here wants total equality — the one
// comparison is against [`Turn::NotWaiting`].
#[derive(Default, PartialEq, Clone, Copy)]
enum Turn {
    /// nothing: the step's `at` is a length of the world's time and means what it says
    #[default]
    NotWaiting,
    /// every beetle's task has run an instruction — the restarted ones and anything born since
    RestartedBeetles,
    /// somebody's meter has moved, which only `world.rb`'s `each_frame` can do
    AMeterMoved,
    /// **the rules that have just been applied are running** (S5b-5), said by the one number of
    /// theirs that crosses the boundary: `garden.rules(day_length:)` is written into
    /// [`crate::Sky`] by a world script that has just started, so the day being this long is
    /// "these rules, running". It is the same wait as [`RestartedBeetles`](Turn::RestartedBeetles)
    /// with one script instead of a dozen — Apply on `world.rb` replaces the rules' script
    /// (`crate::wear_the_rules`), and what is left to wait for is rubevy making a task of it and
    /// the scheduler giving it a turn
    TheDayIs(f32),
    /// egui has taken the pointer the check moved over the editor — or let it go again when the
    /// panel was closed. **Not the VM**; the thing being waited for is bevy_egui learning where
    /// the pointer is, which takes a frame of its own (see the two wheel checks, steps 13-17)
    EguiHasThePointer(bool),
}

/// **How many frames a check waits for the thing it is about before it judges anyway** (S7).
/// Not a length of time, and not a number anybody picked. The case it is derived from is the
/// dearest of the three, which is the VM's scheduler reaching a task that has just been made.
///
/// The wait it bounds ends the moment the thing waited for has happened, so this is reached only
/// when it has not. It is the sum of two things that are each already fixed by something else:
///
/// * **Two frames are structural and cannot be shortened.** A restart hangs a new `Script` on the
///   entity through `Commands`, so the component is not there until the end of the frame that
///   asked for it (one); rubevy's `start_scripts` then makes a task out of it and
///   `RubevySet::Tick` runs it (two). S6 measured exactly this: a breath cut to one frame finds
///   the creature with no `ScriptTask` at all (`window-check-flakes.md` §4.4, two runs of two),
///   and `crate::NEWBORN_DEAF_FRAMES` is the same reckoning written down for newborns.
/// * **The rest are one whole frame's budget of the VM's turns.** One frame of the creatures'
///   VM buys `ScriptWorld::budget` instructions — the garden's own [`crate::CREATURE_BUDGET`],
///   41,000 by default since S5b-5 — or `ScriptWorld::frame_time`, 8 ms of wall clock, whichever
///   runs out first. What 8 ms buys was measured in S6: with `frame_time` cut to 300 µs the VM
///   got through 1,708 instructions in its slowest frame (`s6/ft300.log`, f59 — two beetles'
///   first pass of 854 each), which is 5.7 instructions per microsecond, so a full 8 ms frame
///   buys about 45,600 and `ceil(41_000 / 45_600)` is one. Past that the VM has been handed a
///   whole frame's allowance of turns without reaching a task that is ready to run, which is the
///   scheduler having stopped handing them out — the sabiruby 0.5.1 bug these checks are for
///   (`restart_species`) — rather than a machine that is merely busy.
///
/// A machine that is sharing its CPU makes each frame longer, and that is the point: the same
/// three frames are 0.4 s on the quiet PC where a frame is 133 ms and 0.8 s on the loaded one
/// where it is 280 ms (S6 §1.1, §1.2). A number of seconds cannot do that, which is what 0.6 s
/// buying five frames on one machine and three on another was.
///
/// The same number covers the world's VM ([`Turn::AMeterMoved`]), where the reckoning comes out
/// smaller: its script is resumed rather than made, so there are no structural frames, and its
/// budget is 45,000 (`crate::install_world_answers`), so one frame's worth is one frame. It also
/// covers [`Turn::TheDayIs`] (S5b-5), where the world's script **is** made rather than resumed —
/// Apply on `world.rb` replaces it — so that one wants the same two structural frames the
/// creatures' restarts want, and `2 + ceil(45,000 / 45,600)` is three as well. And it covers
/// [`Turn::EguiHasThePointer`], which is not the VM at all and wants **one** frame:
/// bevy_egui reads the forged `CursorMoved` in `PreUpdate` and the pass that sets
/// `EguiWantsInput` is in `EguiPrimaryContextPass`, so the frame after the one the check wrote it
/// in is the frame egui knows. This is the largest of the three, and one number is better than
/// three.
///
/// **S5b-3: it is worked out from the budget the run is really giving, not from a literal.** S7
/// wrote rubevy's then-default 200,000 into the sum as a number, which was right on the day and
/// wrong the moment the budget became something anybody could change (`script_budget` in
/// `garden.settings.txt`): a run given ten times the budget would have been judged after the
/// same seven frames, which is a quarter of what it was promised. The two parts of the sum keep
/// their own sources — [`STRUCTURAL_FRAMES`] is measured in S6 and cannot be shortened,
/// [`INSTRUCTIONS_A_FRAME_BUYS`] is S6's measurement of the wall clock — and what is new is that
/// the division is done at startup instead of in a comment.
///
/// **S5b-5 moved the budget, so this moved with it**, which is the arrangement proving itself:
/// `2 + ceil(41,000 / 45,600)` is **3** where it was 7. The checks are less patient than they
/// were because the VM they are waiting on has less work it is allowed to do in a frame, and
/// nothing here was edited to make that happen.
pub fn scheduler_frames(budgets: &crate::Budgets) -> u32 {
    STRUCTURAL_FRAMES + (budgets.creature as f32 / INSTRUCTIONS_A_FRAME_BUYS).ceil() as u32
}

/// The two frames a restart costs whatever the VM is allowed: the `Script` lands at the end of
/// the frame that asked for it, and rubevy makes a task of it and runs it on the next.
/// **Measured** (S6, `docs/worklog/2026-09-20-window-check-flakes.md` §4.4) and structural.
const STRUCTURAL_FRAMES: u32 = 2;

/// What one frame of the creatures' VM buys, in instructions. It is a number about *this
/// machine's* wall clock, which is why it is the check's and not a setting: a check is allowed to
/// know how fast the machine it is running on is.
///
/// **There are two measurements of it, and this is the slower one** (S5b-5):
///
/// | | how it was measured | rate | 8 ms buys |
/// |---|---|---|---|
/// | S6 | `frame_time` cut to **300 µs**, the VM's slowest frame in that run: 1,708 instructions (`s6/ft300.log`, f59 — two beetles' first pass of 854 each) | 5.7 insn/µs | **45,600** |
/// | S5b-3 | the capped garden at its **own 8 ms**, three runs of a minute | 6.85 insn/µs | 54,800 |
///
/// The difference is the condition, not the machine: a frame cut to 300 µs pays the cost of
/// starting and stopping the tick over a twenty-seventh of the work, so the rate it measures is
/// the rate of a *short* frame. The garden's own frames are the second row, and they are quicker.
///
/// **The slower rate is the one to keep**, and the reason is which way this number's error hurts.
/// It divides the budget to say how many frames a check may wait, so a number that is too **big**
/// makes the wait too short and produces a FAIL for a VM that was merely being slow — a false
/// FAIL, the thing S6 and S7 were called in to remove. A number that is too small only makes a
/// check wait longer before it says what it was going to say. So the rate that buys *less* per
/// frame is the safe side, and 45,600 is it.
///
/// **At the budget the game ships with the two agree anyway**: `ceil(41,000 / 45,600)` and
/// `ceil(41,000 / 54,800)` are both 1, so [`scheduler_frames`] is 3 either way. The choice only
/// shows above 45,600 of budget, which is `script_budget` in somebody's `garden.settings.txt`.
const INSTRUCTIONS_A_FRAME_BUYS: f32 = 45_600.0;

/// **One beetle born in the very frame the editor presses Apply** (S7) — the race the checks
/// cannot otherwise arrange, made to happen on purpose so that a check can watch it.
///
/// Before S7 a creature born in that frame was missed by the hand-over for good
/// ([`catch_up_minds`]). It is a race: whether a birth falls in the one frame Apply is pressed is
/// up to the garden, and S6 measured it hitting about once in a hundred runs on a quiet PC and
/// once in sixteen in a browser — often enough to make the checks flake and far too rarely to be
/// a check of anything. So the run makes it happen: one beetle is asked for in that frame, and
/// step 8 looks at whether it is running the applied text.
///
/// **How, and why it is done this way.** The birth is asked for the way a script's `garden.spawn`
/// asks for it — a [`Birth`] pushed onto [`crate::Births`] — and `children_arrive` builds the
/// body and queues the `Mind` through `Commands`, in this same frame, exactly as a real birth
/// does. Spawning the creature here instead does not reproduce anything: this system would then
/// have `Commands` of its own, and `.before(children_arrive)` would make Bevy put a sync point
/// between them, which flushes the queue and hands the hand-over the very `Mind` it is supposed
/// to miss. S6 walked into that and wrote it down (§2). Writing a resource is not deferred, so
/// nothing is inserted and `children_arrive` keeps the place in the schedule it really has.
///
/// It is registered only by the window's checks (`GARDEN_SELFTEST=1` with a window), so there is
/// no way for it to add a beetle to anybody's garden.
pub fn birth_in_the_apply_frame(test: Res<WindowTest>, mut births: ResMut<crate::Births>, mut done: Local<bool>) {
    if *done || !test.pressing_apply() {
        return;
    }
    *done = true;
    births.waiting.push(crate::Birth {
        species: Species::Beetle,
        // the species' own three numbers: this beetle is about the frame it arrives in and
        // nothing else, so it is the plainest beetle there is and no number is invented for it
        genome: crate::Genome::of(Species::Beetle),
        at: Vec2::ZERO,
        parent: None,
    });
    info!("selftest: a beetle is being born in the frame Apply is pressed");
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
    vms: BothVms,
    sky: Res<Sky>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    minds: Query<(Entity, &Mind, Option<&ScriptTask>)>,
    // G9: the world itself, for the checks about `P` — where everything stands and what it has
    // left in its meter
    bodies: Query<(Entity, &Transform, &Hunger), With<Creature>>,
    tasks: Query<&ScriptTask>,
    // 2026-09-18: the pointer and the wheel, for the last two checks
    mut pointing: FakePointer,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < test.at {
        return;
    }
    let ok = |cond: bool, what: &str| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
    let spent = || tasks.iter().map(|t| vms.creatures.vm.task_instructions(t.task())).sum::<u64>();
    // the world in three numbers: where everybody is, what they have eaten, and the hour
    let places = || -> Vec<(Entity, Vec3)> { bodies.iter().map(|(e, t, _)| (e, t.translation)).collect() };
    let hunger = || -> Vec<(Entity, f32)> { bodies.iter().map(|(e, _, h)| (e, h.0)).collect() };
    let a_beetle = || minds.iter().find(|(_, m, _)| m.species == Species::Beetle);
    let beetles = || minds.iter().filter(|(_, m, _)| m.species == Species::Beetle);
    let rabbits = || minds.iter().filter(|(_, m, _)| m.species == Species::Rabbit);

    // **And then the wait that is not about time** (S7, [`Turn`]). The clock above is the world's
    // — two seconds of a pause, half a second of walking — and it still means what it says. This
    // one is about a frame of somebody else's: a step that has just restarted a script waits
    // here until every one of those tasks has run an instruction, and a step that has just moved
    // the pointer waits until egui knows where it is — however many frames the machine needs to
    // get there, and giving up after [`scheduler_frames`], which `WindowTest::after` worked out
    // from the budget this run is giving.
    if test.turn != Turn::NotWaiting {
        let came_round = match test.turn {
            Turn::NotWaiting => true,
            Turn::RestartedBeetles => {
                beetles().count() > 0 && beetles().all(|(_, m, _)| m.last_instructions > 0)
            }
            Turn::AMeterMoved => hunger()
                .iter()
                .any(|(e, now)| test.hunger.iter().any(|(was, then)| was == e && then != now)),
            Turn::TheDayIs(want) => sky.day_length == want,
            Turn::EguiHasThePointer(want) => pointing.egui_has_it() == want,
        };
        test.waited += 1;
        if !came_round && test.waited < test.frames + test.spare {
            return;
        }
        // a stage direction, not a check: `tools/fixedlines.sh` keeps the lines with a verdict
        info!(
            "selftest: waited {} frame(s) for the thing the next check is about, and it {}",
            test.waited,
            if came_round { "happened" } else { "did not — judging it as it stands" },
        );
        test.turn = Turn::NotWaiting;
        test.waited = 0;
        test.spare = 0;
    }

    match test.step {
        // --- the keys first, while nothing has been restarted -------------
        0 => {
            let Some((entity, mind, _)) = a_beetle() else { return };
            watched.entity = Some(entity);
            test.original = platform::read(&brains.path(&ruby.0, mind.species)).unwrap_or_default();
            // G9: the panel starts closed, so the first key of the run is the one that opens it.
            // Everything below wants to see the frames it draws.
            ok(!panel.open, "the VM panel starts closed");
            keys.press(KeyCode::F2);
            test.step = 1;
            test.at = now + 0.2;
        }
        1 => {
            ok(panel.open, "F2 opens it");
            keys.release(KeyCode::F2);
            keys.press(KeyCode::KeyP);
            test.step = 2;
            test.at = now + 0.2;
        }
        2 => {
            // once the pause has taken hold, and not on the frame it was asked for: the scripts
            // and the rules of *that* frame had already run when the key was read
            test.insn = spent();
            test.wake = vms.creatures.vm.task_next_wakeup_ticks();
            test.places = places();
            test.hunger = hunger();
            test.phase = sky.phase;
            test.step = 3;
            // **two seconds** (G9), not the half second the scripts-only pause was checked over:
            // a creature walks about four units in two seconds, the meters fall by a tenth of
            // themselves, and the day turns by 1/30 of itself — each of them far past anything a
            // comparison could miss
            test.at = now + 2.0;
        }
        3 => {
            // **Both VMs** (S5b-5). `P` takes the budget off the creatures' and off the world's
            // together (`inspect_keys`), and until now the check looked at one of them — a pause
            // that had stopped the creatures and left `world.rb`'s VM running would have passed
            // this line and then failed the three below with no word about why.
            ok(
                panel.paused && vms.creatures.budget == 0 && vms.rules.budget == 0,
                "P pauses: both VMs' budgets are 0",
            );
            ok(spent() == test.insn, "nothing ran while it was paused");
            // **The three the author asked for: the world, not only the VM** — and each of them
            // said by entity rather than by comparing two `Vec`s ([`what_changed`], S5b-5).
            //
            // `P` stops **the rules of the garden, its clock and its two VMs**. It does not
            // freeze Bevy's world, and it was never meant to: a creature can still be given a
            // `ScriptTask`, an `Animated` or an `Eating` in the middle of a pause, which moves it
            // from one archetype to another and so moves it in the order `Query::iter` hands the
            // creatures over in. That is not the garden moving, and a check that compares two
            // lists in order cannot tell the two apart (S8).
            let where_they_are = what_changed(&test.places, &places());
            let meters = what_changed(&test.hunger, &hunger());
            ok(
                where_they_are.same_creatures(),
                &format!(
                    "2 s paused: the same creatures are there{}",
                    where_they_are.cast()
                ),
            );
            ok(
                where_they_are.moved.is_empty(),
                &format!(
                    "2 s paused: every creature is where it was{}",
                    where_they_are.what_moved()
                ),
            );
            ok(
                meters.moved.is_empty(),
                &format!("2 s paused: nobody got hungrier{}", meters.what_moved()),
            );
            ok(sky.phase == test.phase, "2 s paused: the day did not turn");
            ok(panel.open && !panel.frames.is_empty(), "the VM panel has the creature's frames");
            ok(panel.heap.as_ref().is_some_and(|h| h.live > 0), "the panel has the heap counters");
            let what = "nothing that was sleeping woke on the resume frame";
            match test.wake {
                Some(was) if was > 0 => {
                    let left = vms.creatures.vm.task_next_wakeup_ticks();
                    let same = left == Some(was);
                    info!(
                        "selftest: {} {what}: the next one is due in {was} ticks, as it was two seconds ago{}",
                        if same { "ok  " } else { "FAIL" },
                        if same { String::new() } else { format!(" (now {left:?})") },
                    );
                }
                other => info!("selftest: --   {what}: the next wakeup was {other:?} ticks off when the pause began"),
            }
            test.insn = spent();
            test.places = places();
            test.hunger = hunger();
            test.phase = sky.phase;
            keys.release(KeyCode::KeyP);
            keys.press(KeyCode::KeyP);
            test.step = 4;
            test.at = now + 0.5;
            // S7. Half a second is the world's own time and stays: the checks below are about a
            // creature having *walked* and the sun having turned, which want a length of time.
            // `the meters move again` is not like them — a meter is written by `world.rb`'s
            // `each_frame` (`ruby/world.rb`), so what it wants is the world's VM to have been
            // given a turn, and on a machine where half a second is three frames it was
            // sometimes not given one (S6 §6, reproduced by cutting the world VM's `frame_time`
            // to 20 µs). So the step waits for the half second *and* for a meter to move.
            //
            // What it waits for is the check's own question, as the wait after Apply waits for
            // the check's own question there. That is not the check proving itself: the thing
            // being judged is **within how much of the VM's time it happened**, which is what
            // [`scheduler_frames`] says and what a FAIL here now means.
            test.turn = Turn::AMeterMoved;
        }
        4 => {
            // both of them again, as the pause took both
            ok(
                !panel.paused && vms.creatures.budget > 0 && vms.rules.budget > 0,
                "P again gives both budgets back",
            );
            ok(spent() > test.insn, "the creatures are thinking again");
            // and the world with them. Positions are "somebody moved" rather than "everybody
            // did": a creature that is asleep, or one a handler has told to stand still, is
            // allowed to be where it was. This one is by entity too, and always was — a creature
            // that arrived or left while the world was running is neither the news here nor a
            // reason to say nobody walked.
            let walked = !what_changed(&test.places, &places()).moved.is_empty();
            ok(walked, "and the garden moves again: somebody has walked");
            // "changed", not "fell": a creature standing on a plant is *filling* its meter, and
            // over half a second the garden as a whole can go either way. What the check is about
            // is that `get_hungry` and `eat` are running again at all.
            let meters = !what_changed(&test.hunger, &hunger()).moved.is_empty();
            ok(meters, "the meters move again");
            ok(sky.phase > test.phase, "the day turns again");
            keys.release(KeyCode::KeyP);
            keys.press(KeyCode::F2);
            test.step = 5;
            test.at = now + 0.2;
        }
        5 => {
            ok(!panel.open, "F2 hides the VM panel");
            keys.release(KeyCode::F2);
            keys.press(KeyCode::F2);
            test.step = 6;
            test.at = now + 0.2;
        }
        6 => {
            ok(panel.open, "F2 shows it again");
            keys.release(KeyCode::F2);
            test.step = 7;
            test.at = now + 0.2;
        }
        // --- and then the editor, which restarts the scripts ---------------
        7 => {
            ok(
                editor.file == "beetle.rb" && editor.text.contains("creature \"Beetle\""),
                "the editor shows the file of the creature that was clicked",
            );
            // H2. `beetle.rb` opens with a comment in English and holds `def hungry_below`, so
            // one `find` reaches a keyword the lexer has to have seen. `drawn_kind` goes the
            // whole way through the panel's own `listing()` and reads the colour back out of the
            // `LayoutJob`, so what this says is "it is *painted* in the keyword colour" rather
            // than "the table says 1" — without a pixel, which a check with no screen cannot
            // read anyway.
            let def = editor.text.find("def ").unwrap_or(usize::MAX);
            let kind = rubevy_egui::editor::drawn_kind(&editor, def);
            ok(
                kind == Some(1),
                &format!("`def` in the listing is painted in the keyword colour (kind {kind:?})"),
            );
            editor.text = editor.text.replace("sleep 0.2", "sleep 0.9");
            ok(editor.changed(), "typing marks the text edited");
            test.insn = spent();
            test.beetles_before = beetles().map(|(e, _, _)| e).collect();
            editor.action = Some(EditorAction::Apply);
            // Every beetle is handed over in the frame `do_editor_actions` runs — there is no
            // queue any more (see `restart_species`) — so what is left to wait for is the new
            // tasks being made and given a turn, and that is counted in frames (S7). It used to
            // be 0.6 s, which is five frames on one machine and three on another.
            test.step = 8;
            test.at = now;
            test.turn = Turn::RestartedBeetles;
        }
        8 => {
            let on_disk = platform::read(&brains.path(&ruby.0, Species::Beetle)).unwrap_or_default();
            let every = beetles().count();
            let applied = beetles().filter(|(_, m, _)| m.in_memory).count();
            ok(every > 0 && applied == every, "Apply restarts every beetle on the edited text");
            // **The one that was born in the frame Apply was pressed** (S7). A beetle is forced
            // into that frame on purpose ([`birth_in_the_apply_frame`]) because the frames a
            // birth lands in are otherwise the garden's own business: before S7 that beetle was
            // missed by the hand-over and ran the *other* text for the rest of the run, and the
            // check above caught it in about one run in a hundred on a quiet machine and one in
            // sixteen in a browser. It is a check of its own rather than a widening of the one
            // above because it says which beetle and why, and because the sentence above is
            // about Apply while this one is about the race.
            let born_since: Vec<_> =
                beetles().filter(|(e, _, _)| !test.beetles_before.contains(e)).collect();
            ok(
                !born_since.is_empty() && born_since.iter().all(|(_, m, _)| m.in_memory),
                "a beetle born in the very frame of Apply is restarted too",
            );
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
            test.step = 9;
            test.at = now + 9.0;
        }
        9 => {
            // the VM did not stop: every task, restarted or not, is still being given the CPU
            ok(spent() > test.insn, "the whole VM is still running afterwards");
            editor.action = Some(EditorAction::Revert);
            test.step = 10;
            test.at = now;
            // S7: the same wait as after Apply, and for the same reason
            test.turn = Turn::RestartedBeetles;
        }
        10 => {
            ok(
                beetles().count() > 0 && beetles().all(|(_, m, _)| !m.in_memory),
                "Revert puts every beetle back on the file",
            );
            ok(editor.text == test.original, "Revert shows the file again");
            ok(
                platform::read(&brains.path(&ruby.0, Species::Beetle)).unwrap_or_default() == test.original,
                "nothing was written",
            );
            // what the HUD's insn/decision says, with a window and whatever frame rate this
            // machine gives: the number the doc quotes is the headless one, at a steady 60 Hz.
            // The component reads that used to be counted beside the ask round trips are not
            // here any more — rubevy answers a read inside the tick that asks it, so there is no
            // wait to count (`Mind::instructions_per_decision`).
            for (_, mind, _) in minds.iter().take(3) {
                info!(
                    "selftest: {} — {} ask round trips, {} decisions, {} insn/decision",
                    mind.name,
                    mind.ask_trips,
                    mind.decisions,
                    match mind.instructions_per_decision() {
                        Some(n) => format!("{n:.0}"),
                        None => "–".into(),
                    }
                );
            }
            // --- and then the rules, which are the other file in the game (W3) ---
            keys.press(KeyCode::F3);
            test.step = 11;
            test.at = now + 0.2;
        }
        // **The window's half of the twelfth check.** That one takes `world.rb` away and gives it
        // back from Rust (`crate::swap_the_rules`) and watches the meters stop falling; this one
        // goes through the panel a player uses — `F3`, type, `Ctrl+Enter` — and watches a number
        // the rules hand the game.
        //
        // `day_length` is what it watches because it is the one rule that **crosses the boundary
        // as a number**: the sun is drawn in Rust and `garden.rules(day_length:)` is how the file
        // says how long a turn of it takes (`Sky::day_length`). Only a world script that has just
        // started says it, so the sun turning in the time the *edited* text asks for is "these
        // rules are the ones running", with no window to wait for, no threshold and no
        // statistics — where "the grass grew" or "a meter fell" would need all three.
        11 => {
            ok(
                editor.file == "world.rb" && editor.text.contains("world do"),
                "F3 opens the rules of the world",
            );
            ok(
                editor.choices.iter().any(|c| c.label.starts_with("world.rb")),
                "the editor has a third file, and it is not a creature",
            );
            // **What the file says, read out of the file** (S5b-5). This was `sky.day_length ==
            // 60.0` and the edit below was a text replacement of the literal `day_length 60.0`,
            // so a `world.rb` whose day had been made longer would have failed a check whose own
            // sentence says it is about what `world.rb` says — and the edit would quietly have
            // replaced nothing. Both halves read the number the file is really carrying now. It
            // is the shape S5b-4 took out of the mutation rate and S5b-2 out of Battle's turn
            // rate; this stage went looking for the rest of it.
            let said = day_length_in(&editor.text);
            ok(
                said.is_some_and(|n| sky.day_length == n),
                &format!(
                    "the day is what world.rb says it is ({})",
                    match said {
                        Some(n) => format!("{n:.1} s"),
                        None => "and world.rb says nothing about it".into(),
                    }
                ),
            );
            test.world_original = platform::read(&ruby.0.join(crate::WORLD_FILE)).unwrap_or_default();
            // and the edit is that number halved — any other number would do, and half of it is
            // the one that is easiest to recognise in a log beside the original
            let (was, half) = (said.unwrap_or(sky.day_length), said.unwrap_or(sky.day_length) * 0.5);
            test.day_length = half;
            editor.text = editor.text.replace(&day_length_line(was), &day_length_line(half));
            ok(editor.changed(), "typing in the rules marks them edited");
            keys.release(KeyCode::F3);
            // the keys themselves this time, not `Editor::action`: `rubevy-egui`'s panel reads
            // Ctrl+Enter in `PostUpdate` (the egui pass), so the action it sets is taken by
            // `do_editor_actions` on the next frame — which is inside the wait below
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::Enter);
            test.step = 12;
            // **S5b-5: the last two waits that were still seconds.** 0.6 s is five frames on a
            // quiet PC and three on a loaded one or in a browser, and what is wanted here is not
            // a length of time at all: it is `do_editor_actions` taking the key's action, rubevy
            // making a task of the new rules and the scheduler giving it a turn — the same three
            // things the waits after Apply and Revert on a *creature* wait for (S7). These two
            // had simply not failed yet.
            test.at = now;
            test.turn = Turn::TheDayIs(test.day_length);
            // and one frame in front of the VM's own, because this step asks with a key rather
            // than by setting `Editor::action`: the egui pass is where `Ctrl+Enter` is read
            // ([`WindowTest::spare`]). The first run with the wait in it took exactly three
            // frames, which is `scheduler_frames` to the frame — that is the wait being right up
            // against its bound, not room to spare.
            test.spare = 1;
        }
        12 => {
            keys.release(KeyCode::Enter);
            keys.release(KeyCode::ControlLeft);
            ok(
                sky.day_length == test.day_length,
                "Ctrl+Enter: the garden is running the edited rules, without stopping",
            );
            let half = day_length_line(test.day_length);
            ok(
                brains.world().is_some_and(|t| t.contains(&half)),
                "the rules the editor applied are the ones in memory",
            );
            ok(
                platform::read(&ruby.0.join(crate::WORLD_FILE)).unwrap_or_default() == test.world_original,
                "Apply does not touch world.rb",
            );
            ok(!editor.changed(), "after Apply the text is what the world runs");
            editor.action = Some(EditorAction::Revert);
            test.step = 13;
            // and the same wait the other way round: the file's own `day_length` back again
            test.at = now;
            test.turn = Turn::TheDayIs(test.day_length * 2.0);
        }
        13 => {
            ok(sky.day_length == test.day_length * 2.0, "Revert puts the file's rules back");
            ok(editor.text == test.world_original, "Revert shows world.rb again");
            ok(brains.world().is_none(), "and nothing is running a text of its own");
            // --- and the wheel, which is the other thing a panel takes (2026-09-18) ---
            //
            // Last rather than first because it moves the pointer, and every check above is
            // driven by keys and by `Editor::action` and would rather the pointer stayed where
            // the player left it.
            pointing.hide_the_guide();
            let at = pointing.over_the_editor().unwrap_or_default();
            pointing.point_at(at);
            test.step = 14;
            test.at = now;
            // **S7.** bevy_egui learns where the pointer is a frame after the `CursorMoved` was
            // written (`PreUpdate` reads it, the egui pass sets `EguiWantsInput`), and 0.2 s is
            // one frame on a machine running eight of these at once — so the wheel below used to
            // be turned while egui had not yet taken the pointer, `orbit_camera` zoomed, and the
            // check failed saying egui *did* hold it, which by then it did. Three of eight runs,
            // measured 2026-09-20. Waiting for egui to have it says what the step means.
            test.turn = Turn::EguiHasThePointer(true);
        }
        14 => {
            test.distance = pointing.orbit.distance;
            // both halves of the moment, read in the frame the notch is written in
            // ([`WindowTest::held`])
            test.held = pointing.egui_has_it();
            test.held_detail = format!("{} editor.open={}", pointing.egui_answers(), editor.open);
            pointing.turn_the_wheel();
            test.step = 15;
            test.at = now + 0.2;
        }
        15 => {
            // the two halves are in one line on purpose: "the camera did not move" is only worth
            // anything if the pointer really was over the panel, and a run where egui had let go
            // of it would otherwise pass by doing nothing
            let held = test.held;
            let moved = pointing.orbit.distance != test.distance;
            let what = format!(
                "the wheel over the editor scrolls the editor and not the garden (egui holds the pointer: {held}; camera {:.2} -> {:.2})",
                test.distance, pointing.orbit.distance
            );
            if !(held && !moved) {
                info!("selftest: in the frame the wheel was turned: {}", test.held_detail);
            }
            ok(held && !moved, &what);
            // the control: the same wheel, at the same place, with nothing drawn there. Only the
            // editor has to go — the HUD is at the top left and the VM panel at the bottom left,
            // and neither reaches the middle of the editor's rectangle.
            editor.open = false;
            test.step = 16;
            test.at = now;
            // and the same the other way for the control: the panel is shut, and the wheel must
            // not be turned until egui has let the pointer go
            test.turn = Turn::EguiHasThePointer(false);
        }
        16 => {
            test.distance = pointing.orbit.distance;
            test.held = pointing.egui_has_it();
            test.shown = editor.open;
            test.held_detail = format!("{} editor.open={}", pointing.egui_answers(), editor.open);
            pointing.turn_the_wheel();
            test.step = 17;
            test.at = now + 0.2;
        }
        17 => {
            let held = test.held;
            let moved = pointing.orbit.distance != test.distance;
            let what = format!(
                "and with the panel closed the same wheel in the same place zooms (egui holds the pointer: {held}; camera {:.2} -> {:.2})",
                test.distance, pointing.orbit.distance
            );
            // **The premise, before the verdict** ([`WindowTest::shown`]). This step shut the
            // editor; if the game had opened it again by the time the wheel was turned then there
            // *was* a panel under the pointer, and neither `ok` nor `FAIL` would be true of the
            // run that happened.
            if !test.shown && !(!held && moved) {
                info!("selftest: in the frame the wheel was turned: {}", test.held_detail);
            }
            if test.shown {
                info!(
                    "selftest: --   {what}: the editor was open again when the wheel was turned — \
                     the creature being watched died and `choose_watched` opened it"
                );
            } else {
                ok(!held && moved, &what);
            }
            editor.open = true;
            // A PC run was asked for the checks on a command line and should give the prompt
            // back. A page was asked for them in its address, by somebody who is looking at the
            // garden — and `AppExit` there does not end a run, it stops the canvas for good
            // (`platform::CHECKS_EXIT_WHEN_DONE`).
            if platform::CHECKS_EXIT_WHEN_DONE {
                exit.write(AppExit::Success);
            } else {
                info!("selftest: done — the garden keeps running (a page has nothing to exit to)");
            }
            test.step = 18;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{day_length_in, day_length_line};

    /// **The check reads the file's own number** (S5b-5), and it reads the shape `ruby/world.rb`
    /// really writes it in — a word and a number inside `world do`, not one of the
    /// `def name = number` lines the rest of that file uses.
    const WORLD_RB: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/ruby/world.rb"));

    #[test]
    fn the_rules_check_reads_the_day_out_of_the_rules() {
        assert_eq!(day_length_in(WORLD_RB), Some(60.0));
        // and what it writes back is a line of the same shape, which is what the edit replaces
        assert!(WORLD_RB.contains(&day_length_line(60.0)));
        let edited = WORLD_RB.replace(&day_length_line(60.0), &day_length_line(30.0));
        assert_eq!(day_length_in(&edited), Some(30.0));
        assert_ne!(edited, WORLD_RB, "the replacement has to have replaced something");
        // a text with nothing to say says nothing, rather than a number nobody wrote
        assert_eq!(day_length_in("world do\nend\n"), None);
    }
}
