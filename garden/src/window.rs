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
        && down.distance(up) <= CLICK_SLOP
        && let Some(entity) = creature_under(up, &cameras, &creatures)
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
            let (handle, lines) = match compile(&ruby.0, &path, &mut mrb) {
                Ok(it) => it,
                Err(why) => {
                    error!("{why}");
                    editor.message =
                        format!("saved {}, but it would not compile — {}", species.file(), first_trouble(&why));
                    return;
                }
            };
            restart_species(&mut commands, &mut minds, species, handle, lines, false);
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
            restart_species(&mut commands, &mut minds, species, handle, lines, false);
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
        // S2: the three lines this was — `ScriptTask` off, `ScriptDone` off, the new `Script` on
        // — are rubevy's `replace_script` (R6). Forgetting the `ScriptDone` is the invisible
        // half: a creature whose script had run to its end could never be given another one.
        replace_script(
            commands,
            entity,
            Script::new(handle.clone()).with_name(&mind.name).with_priority(100),
        );
        n += 1;
    }
    n
}

/// A creature file saved from outside the game restarts that species, exactly as Save does — and
/// `world.rb` saved from outside restarts the rules, exactly as their Save does (W3).
#[allow(clippy::too_many_arguments)]
pub fn reload_changed(
    mut commands: Commands,
    watch: Option<Res<Watch>>,
    ruby: Res<RubyDir>,
    brains: Res<Brains>,
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
            let n = restart_species(&mut commands, &mut minds, species, handle, lines, false);
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
                        hunger_bar(ui, row.hunger);
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
    dial: &mut crate::NightDial,
    settings: &mut Option<bevy::prelude::ResMut<games_shell::Settings>>,
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
    /// the same for `world.rb` (W3)
    world_original: String,
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
}

impl FakePointer<'_, '_> {
    /// Whether egui is holding the pointer, as [`crate::orbit_camera`] asks it.
    fn egui_has_it(&self) -> bool {
        self.egui.as_ref().is_some_and(|e| e.wants_pointer_input() || e.is_pointer_over_area())
    }

    /// The middle of the editor panel, where it stands before anybody drags it
    /// (`rubevy_egui::editor`'s own figures, so the check is not guessing the rectangle).
    fn over_the_editor(&self) -> Option<Vec2> {
        let (_, window) = self.windows.iter().next()?;
        use rubevy_egui::editor::{HEIGHT, MARGIN, WIDTH};
        Some(Vec2::new(window.width() - MARGIN - WIDTH * 0.5, MARGIN + HEIGHT * 0.5))
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
    let spent = || tasks.iter().map(|t| world.vm.task_instructions(t.task())).sum::<u64>();
    // the world in three numbers: where everybody is, what they have eaten, and the hour
    let places = || -> Vec<(Entity, Vec3)> { bodies.iter().map(|(e, t, _)| (e, t.translation)).collect() };
    let hunger = || -> Vec<(Entity, f32)> { bodies.iter().map(|(e, _, h)| (e, h.0)).collect() };
    let a_beetle = || minds.iter().find(|(_, m, _)| m.species == Species::Beetle);
    let beetles = || minds.iter().filter(|(_, m, _)| m.species == Species::Beetle);
    let rabbits = || minds.iter().filter(|(_, m, _)| m.species == Species::Rabbit);

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
            test.wake = world.vm.task_next_wakeup_ticks();
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
            ok(panel.paused && world.budget == 0, "P pauses: the scripts' budget is 0");
            ok(spent() == test.insn, "nothing ran while it was paused");
            // the three the author asked for: **the world**, not only the VM
            ok(places() == test.places, "2 s paused: every creature is where it was");
            ok(hunger() == test.hunger, "2 s paused: nobody got hungrier");
            ok(sky.phase == test.phase, "2 s paused: the day did not turn");
            ok(panel.open && !panel.frames.is_empty(), "the VM panel has the creature's frames");
            ok(panel.heap.as_ref().is_some_and(|h| h.live > 0), "the panel has the heap counters");
            let what = "nothing that was sleeping woke on the resume frame";
            match test.wake {
                Some(was) if was > 0 => {
                    let left = world.vm.task_next_wakeup_ticks();
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
        }
        4 => {
            ok(!panel.paused && world.budget > 0, "P again gives the budget back");
            ok(spent() > test.insn, "the creatures are thinking again");
            // and the world with them. Positions are "somebody moved" rather than "everybody
            // did": a creature that is asleep, or one a handler has told to stand still, is
            // allowed to be where it was.
            let moved = places()
                .iter()
                .any(|(e, at)| test.places.iter().any(|(was, place)| was == e && place != at));
            ok(moved, "and the garden moves again: somebody has walked");
            // "changed", not "fell": a creature standing on a plant is *filling* its meter, and
            // over half a second the garden as a whole can go either way. What the check is about
            // is that `get_hungry` and `eat` are running again at all.
            let meters = hunger()
                .iter()
                .any(|(e, now)| test.hunger.iter().any(|(was, then)| was == e && then != now));
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
            editor.action = Some(EditorAction::Apply);
            // Every beetle is handed over in the frame `do_editor_actions` runs — there is no
            // queue any more (see `restart_species`) — so this is a breath for the new tasks to
            // be made and to run their first instructions, not a wait for a queue to drain.
            test.step = 8;
            test.at = now + 0.6;
        }
        8 => {
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
            test.step = 9;
            test.at = now + 9.0;
        }
        9 => {
            // the VM did not stop: every task, restarted or not, is still being given the CPU
            ok(spent() > test.insn, "the whole VM is still running afterwards");
            editor.action = Some(EditorAction::Revert);
            test.step = 10;
            test.at = now + 0.6;
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
        // started says it, so `Sky::day_length == 30` is "these rules are the ones running", with
        // no window to wait for, no threshold and no statistics — where "the grass grew" or "a
        // meter fell" would need all three.
        11 => {
            ok(
                editor.file == "world.rb" && editor.text.contains("world do"),
                "F3 opens the rules of the world",
            );
            ok(
                editor.choices.iter().any(|c| c.label.starts_with("world.rb")),
                "the editor has a third file, and it is not a creature",
            );
            ok(sky.day_length == 60.0, "the day is what world.rb says it is");
            test.world_original = platform::read(&ruby.0.join(crate::WORLD_FILE)).unwrap_or_default();
            editor.text = editor.text.replace("day_length 60.0", "day_length 30.0");
            ok(editor.changed(), "typing in the rules marks them edited");
            keys.release(KeyCode::F3);
            // the keys themselves this time, not `Editor::action`: `rubevy-egui`'s panel reads
            // Ctrl+Enter in `PostUpdate` (the egui pass), so the action it sets is taken by
            // `do_editor_actions` on the next frame — which is inside the breath below
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::Enter);
            test.step = 12;
            test.at = now + 0.6;
        }
        12 => {
            keys.release(KeyCode::Enter);
            keys.release(KeyCode::ControlLeft);
            ok(
                sky.day_length == 30.0,
                "Ctrl+Enter: the garden is running the edited rules, without stopping",
            );
            ok(
                brains.world().is_some_and(|t| t.contains("day_length 30.0")),
                "the rules the editor applied are the ones in memory",
            );
            ok(
                platform::read(&ruby.0.join(crate::WORLD_FILE)).unwrap_or_default() == test.world_original,
                "Apply does not touch world.rb",
            );
            ok(!editor.changed(), "after Apply the text is what the world runs");
            editor.action = Some(EditorAction::Revert);
            test.step = 13;
            test.at = now + 0.6;
        }
        13 => {
            ok(sky.day_length == 60.0, "Revert puts the file's rules back");
            ok(editor.text == test.world_original, "Revert shows world.rb again");
            ok(brains.world().is_none(), "and nothing is running a text of its own");
            // --- and the wheel, which is the other thing a panel takes (2026-09-18) ---
            //
            // Last rather than first because it moves the pointer, and every check above is
            // driven by keys and by `Editor::action` and would rather the pointer stayed where
            // the player left it.
            let at = pointing.over_the_editor().unwrap_or_default();
            pointing.point_at(at);
            test.step = 14;
            test.at = now + 0.2;
        }
        14 => {
            test.distance = pointing.orbit.distance;
            pointing.turn_the_wheel();
            test.step = 15;
            test.at = now + 0.2;
        }
        15 => {
            // the two halves are in one line on purpose: "the camera did not move" is only worth
            // anything if the pointer really was over the panel, and a run where egui had let go
            // of it would otherwise pass by doing nothing
            let held = pointing.egui_has_it();
            let moved = pointing.orbit.distance != test.distance;
            ok(
                held && !moved,
                &format!(
                    "the wheel over the editor scrolls the editor and not the garden (egui holds the pointer: {held}; camera {:.2} -> {:.2})",
                    test.distance, pointing.orbit.distance
                ),
            );
            // the control: the same wheel, at the same place, with nothing drawn there. Only the
            // editor has to go — the HUD is at the top left and the VM panel at the bottom left,
            // and neither reaches the middle of the editor's rectangle.
            editor.open = false;
            test.step = 16;
            test.at = now + 0.2;
        }
        16 => {
            test.distance = pointing.orbit.distance;
            pointing.turn_the_wheel();
            test.step = 17;
            test.at = now + 0.2;
        }
        17 => {
            let held = pointing.egui_has_it();
            let moved = pointing.orbit.distance != test.distance;
            ok(
                !held && moved,
                &format!(
                    "and with the panel closed the same wheel in the same place zooms (egui holds the pointer: {held}; camera {:.2} -> {:.2})",
                    test.distance, pointing.orbit.distance
                ),
            );
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
