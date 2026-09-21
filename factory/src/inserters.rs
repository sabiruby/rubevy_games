//! **The inserters** — the one machine in this game with a mind, and the whole reason it exists.
//!
//! Everything else in the factory is Rust and asks nobody anything: belts carry, miners dig,
//! furnaces smelt. An inserter takes one thing from the tile behind it and puts it in the tile in
//! front, and **when** it does that is a line of Ruby the player can rewrite while the game runs.
//! Nothing goes into or out of a machine any other way ([`crate::machines`]), so a line that ends
//! at a furnace ends there until somebody writes an arm for it.
//!
//! # The two halves
//!
//! | the arm | the mind |
//! |---|---|
//! | [`crate::machines::swing`] — how long a swing takes, what happens when it arrives | `ruby/inserter.rb` on top of `ruby/prelude.rb` |
//! | the grid: `held` is the hand, `work` is how far across, `swinging` is whether it is moving | a task in rubevy's VM, one per inserter, all on one irep |
//!
//! The arm's state is in the **grid** and not in a component, for three reasons that all say the
//! same thing: a save file is the grid (F5), the factory's own step is `App`-free so its tests can
//! drive an arm with no Bevy at all, and the sum that says nothing is made or lost out of nothing
//! walks the grid (`crate::belts`'s minutes-long test). What is on the entity is the *script*.
//!
//! # Three questions and one act
//!
//! The three questions — what is behind, what is in the hand, would the front take this — are
//! answered **inside the tick** (`ScriptWorld::answer_in_tick`), so they cost no frame and the
//! answer is there in the line that asked. They are answered out of the world as it stands in
//! `RubevySet::Tick`, which is after the last step of the factory and before the next one, so a
//! script reads a still picture and the swing it starts happens in that same picture.
//!
//! `move` is the act, and it is **the thing that waits**: the request is kept — in [`Arms`], by
//! the tile it belongs to — for as long as the arm takes, and answered when the arm arrives. Why
//! that and not an event or a `sleep` is written out in
//! `docs/worklog/2026-09-21-factory-F3.md` §3; the short of it is that a kept request is the only
//! one of the three where the game does not have to tell the script a number it already knows,
//! and the only one where an inserter taken away mid-swing takes its own loose ends with it.
//!
//! # What a data file cannot reach
//!
//! The item names and how long a swing takes are written **into the program** the game compiles,
//! as a few lines of Ruby in front of `prelude.rb` ([`names_and_numbers`]). It is the cheapest
//! honest way to turn an item number into the name a player wrote: an in-tick answer can only
//! hand back a flat value, so the game answers numbers, and the table that turns one into
//! `:iron_ore` has to be in the VM already.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use rubevy::{
    in_the_authors_lines, replace_script, Answer, MrbAsset, Program, Request, Script, ScriptEnded,
    ScriptStatus, ScriptTask, ScriptWorld,
};

use crate::belts::{Lanes, Rules};
use crate::data::{Data, ItemId};
use crate::grid::{Building, Dir, Grid, What};
use crate::{machines, platform};

/// The file every inserter starts on, and the name its errors are reported under.
pub const SCRIPT_FILE: &str = "inserter.rb";
/// The DSL it is compiled in front of. Its name only shows up when something goes wrong *in* it.
pub const PRELUDE_FILE: &str = "prelude.rb";

/// **Which inserter this script is the mind of.** The tile, because that is what everything about
/// an inserter is looked up by — the grid, [`Arms`], the editor.
#[derive(Component, Debug)]
pub struct Inserter {
    pub tile: usize,
    /// **Which text it is running**, as a hash of it. It is what [`keep_the_crew`] compares to
    /// see that this one arm has been given a new mind — and a hash rather than the text so that
    /// a component on a thousand entities is a number and not a thousand copies of a file.
    pub running: u64,
}

/// The hash a [`Inserter::running`] is. `DefaultHasher` is not stable between releases of Rust
/// and does not need to be: nothing is written down, it is compared with itself inside one run.
fn hash_of(text: &str) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------------------------
// What each inserter is running
// ---------------------------------------------------------------------------------------------

/// **The texts, and the programs they were compiled to.**
///
/// Three places a text can come from, in the order they are asked: one this inserter was given on
/// its own, one that was applied to every inserter, and the file. It is the garden's `Brains` with
/// the species taken out and a tile put in.
///
/// **One text is one program is one irep.** rubevy keeps a program by its bytes, so a hundred
/// inserters on one text load it once (`docs/host-api.md`, "One program, one irep"); what is
/// cached here is the *compiling*, which is 7 ms in a browser and would otherwise happen a
/// hundred times.
#[derive(Resource)]
pub struct Minds {
    /// `ruby/inserter.rb`, as it was on disk when the game started.
    file: String,
    /// What was applied to every inserter, and is not saved.
    everyone: Option<String>,
    /// An inserter that was given a script of its own. **By tile**, so an inserter taken away and
    /// built again in the same place gets the same mind back — which is what a player who
    /// rebuilt a belt by mistake would want, and is written down here because it is a choice.
    own: HashMap<usize, String>,
    /// text → what it compiled to, and how far down it the player's first line is.
    programs: HashMap<String, (Handle<MrbAsset>, u32)>,
    /// **How many times a running arm has been handed a new program.** Every distinct text is an
    /// irep the VM keeps for ever (SabiRuby has no way to drop one), so this is a number worth
    /// seeing beside `loaded_programs` — an editor used for an hour spends it.
    pub swaps: u64,
    /// Bumped whenever a text changes, so that the system that keeps the crew knows to look.
    generation: u64,
    /// What the file said when it would not compile or would not be read. F3-2's editor is what
    /// shows it; the log has it from the moment it happens.
    #[allow(dead_code)]
    pub trouble: Option<String>,
}

impl Minds {
    /// What the inserter at `tile` should be running.
    pub fn text_for(&self, tile: usize) -> &str {
        self.own
            .get(&tile)
            .or(self.everyone.as_ref())
            .map(|s| s.as_str())
            .unwrap_or(self.file.as_str())
    }

    /// The file as it was read, for Revert.
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Whether this inserter is running a text that is not the file's.
    pub fn in_memory(&self, tile: usize) -> bool {
        self.own.contains_key(&tile) || self.everyone.is_some()
    }

    /// Whether `tile` was given a script of its own.
    pub fn is_its_own(&self, tile: usize) -> bool {
        self.own.contains_key(&tile)
    }

    /// Give one inserter a text of its own.
    pub fn give_to_one(&mut self, tile: usize, text: String) {
        self.own.insert(tile, text);
        self.generation += 1;
    }

    /// Give every inserter that has no text of its own this one — and take away the ones that
    /// have, because "every inserter on this script" means every one of them.
    pub fn give_to_all(&mut self, text: String) {
        self.own.clear();
        self.everyone = Some(text);
        self.generation += 1;
    }

    /// Back to the file, for everybody.
    pub fn back_to_the_file(&mut self) {
        self.own.clear();
        self.everyone = None;
        self.generation += 1;
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// **What the arms are waiting on, and which of them have stopped.**
///
/// Everything here is keyed by tile, which is what an inserter *is*: an entity may come and go
/// under one — it is rebuilt when the grid changes — and the tile is what the player clicked.
#[derive(Resource, Default)]
pub struct Arms {
    /// The `move` each arm's script is parked on. A request kept across frames is rubevy's own
    /// road for an answer that is not ready (`docs/host-api.md`), and keeping it *here* is what
    /// makes an inserter taken away mid-swing tidy: the tile goes out of the grid, this entry
    /// goes with it, and the task went with the entity.
    waiting: HashMap<usize, Request>,
    /// Arms that arrived this step, and whether they put down what they were carrying. Filled by
    /// the factory's own step and emptied by [`finish_swings`].
    pub finished: Vec<(usize, bool)>,
    /// **Inserters whose script is not running any more**, and where each of them stopped — it
    /// raised, it overran, it would not compile, or it simply came back from `run`. The picture
    /// draws a mark over these and the log says the place; keeping the place here as well is what
    /// lets a check say more than "it stopped".
    stopped: HashMap<usize, String>,
}

impl Arms {
    pub fn has_stopped(&self, tile: usize) -> bool {
        self.stopped.contains_key(&tile)
    }

    /// Where the script on `tile` stopped, as `file:line`, for whoever is showing it.
    pub fn stopped_at(&self, tile: usize) -> Option<&str> {
        self.stopped.get(&tile).map(|s| s.as_str())
    }

    pub fn how_many_stopped(&self) -> usize {
        self.stopped.len()
    }

    /// Everything this tile was in the middle of, forgotten: the inserter is gone, or is being
    /// given a new mind.
    fn forget(&mut self, tile: usize) {
        self.waiting.remove(&tile);
        self.stopped.remove(&tile);
    }

    fn mark_stopped(&mut self, tile: usize, at: String) {
        self.stopped.insert(tile, at);
    }

    /// How many arms are parked on a `move` right now — the number a HUD would show beside the
    /// frame's statistics, and what the checks read to say an arm that was taken away left
    /// nothing behind.
    pub fn how_many_waiting(&self) -> usize {
        self.waiting.len()
    }
}

/// **The inserters, as one system parameter** — and the panel their scripts are edited in, which
/// is here for a reason worth writing down rather than for tidiness: a system may take sixteen
/// parameters and the checks already take sixteen, so a check that is about an arm *and* its
/// panel has to reach both through one of them. The panel is an `Option` because a run with no
/// window has no `Editor` at all (`crate::window`).
#[derive(bevy::ecs::system::SystemParam)]
pub struct Crew<'w, 's> {
    pub minds: ResMut<'w, Minds>,
    pub arms: ResMut<'w, Arms>,
    pub standing: Query<'w, 's, (Entity, &'static Inserter)>,
    pub panel: Option<ResMut<'w, rubevy_egui::Editor>>,
}

impl Crew<'_, '_> {
    /// The entity whose script is the mind of the inserter on `tile`, where there is one.
    pub fn at(&self, tile: usize) -> Option<Entity> {
        self.standing.iter().find(|(_, i)| i.tile == tile).map(|(e, _)| e)
    }

    /// How many arms have a script.
    pub fn how_many(&self) -> usize {
        self.standing.iter().count()
    }
}

// ---------------------------------------------------------------------------------------------
// Compiling
// ---------------------------------------------------------------------------------------------

/// **The Ruby the game writes**, from its own tables, in front of the prelude.
///
/// Two things, and both of them are things an in-tick answer cannot hand over: the item names, so
/// that a number the game answers with can be said as the Symbol the data file wrote, and how long
/// a swing takes, so that `idle` is one swing and not a number of its own.
///
/// They are **methods on the class and not constants**, because a program is compiled again every
/// time a text changes and a constant written twice is a warning a player did not ask for. A name
/// is quoted (`:"…"`) so that a data file may call an item whatever it likes.
fn names_and_numbers(data: &Data, rules: &Rules, stagger: f32, prelude_lines: u32) -> String {
    let names: Vec<String> = data
        .items
        .iter()
        .map(|item| format!(":\"{}\"", item.name.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!(
        "# ---- written by the game from ruby/data.rb (factory/src/inserters.rs) ----\n\
         class Inserter\n\
         \x20 def self.declared_items\n\
         \x20   [{}]\n\
         \x20 end\n\
         \x20 def self.swing_seconds\n\
         \x20   {:?}\n\
         \x20 end\n\
         \x20 def self.stagger\n\
         \x20   {:?}\n\
         \x20 end\n\
         \x20 def self.prelude_lines\n\
         \x20   {}\n\
         \x20 end\n\
         end\n",
        names.join(", "),
        rules.swing_seconds,
        stagger,
        prelude_lines,
    )
}

/// **How much of a swing a script's first wait is spread over**, and the one knob here that is a
/// measuring instrument rather than a number of play.
///
/// The default is a whole swing: every arm looks for the first time somewhere in the first swing
/// of its life, so a thousand of them started in one frame do not wake in one frame ever after
/// (`ruby/prelude.rb`, and rubevy's measurement of what that costs). **Zero turns it off**, which
/// is not a way to play — it is how the stress run shows what the spreading is worth
/// (`factory.settings.txt`'s `inserter_stagger`, `FACTORY_STAGGER`, `?stagger=0`).
#[derive(Resource, Debug, Clone, Copy)]
pub struct Stagger(pub f32);

impl Default for Stagger {
    fn default() -> Self {
        Stagger(1.0)
    }
}

/// **How many lines the block above is.** It does not depend on what is in it — every field is on
/// a line of its own — which is what makes [`compile`]'s two passes exact rather than a guess:
/// the number the block *carries* is how many lines are in front of the player's first one, and
/// putting it there changes nothing about how many that is.
fn lines_of_the_block(data: &Data, rules: &Rules, stagger: f32) -> u32 {
    names_and_numbers(data, rules, stagger, 0).lines().count() as u32
}

/// One inserter's program: what the game wrote, the prelude, and the player's own file.
fn compile(
    prelude: &str,
    data: &Data,
    rules: &Rules,
    stagger: f32,
    body: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    // **Twice, because the program has to carry its own length.** A script that raises is the one
    // case where nobody but the VM knows where it was — a task that has ended has no frames left
    // to ask (measured, `docs/worklog/2026-09-21-factory-F3.md` §4) — so the *prelude* reads the
    // exception's backtrace and says the line in the player's own numbering, which means it has
    // to know how far down the program the player's first line is. `Program::new` counts that off
    // the text; the block in front adds exactly its own lines, and that count does not change
    // when the number written into it does, so one extra `Program::new` settles it.
    let without = Program::new(prelude, SCRIPT_FILE, "", "run_inserter").prelude_lines;
    let prelude_lines = without + lines_of_the_block(data, rules, stagger);
    let front = format!("{}{prelude}", names_and_numbers(data, rules, stagger, prelude_lines));
    let program = Program::new(&front, SCRIPT_FILE, body, "run_inserter");
    debug_assert_eq!(program.prelude_lines, prelude_lines, "the block's own length moved");
    match platform::compile(&program.source, SCRIPT_FILE) {
        Ok(bytes) => Ok((mrb.add(MrbAsset { bytes }), program.prelude_lines)),
        Err(why) => Err(in_the_authors_lines(&why, program.prelude_lines, PRELUDE_FILE)),
    }
}

/// **Whether a text would compile**, which is what the editor asks before it writes anything
/// down: a refusal has to leave every arm running what it was running.
///
/// It compiles and keeps the result, so the Apply that follows costs nothing — which is also why
/// it takes the same `&mut Minds` the real thing does.
pub fn would_compile(
    minds: &mut Minds,
    prelude: &str,
    data: &Data,
    rules: &Rules,
    stagger: Stagger,
    text: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(), String> {
    program_of(minds, prelude, data, rules, stagger, text, mrb).map(|_| ())
}

/// The program for a text, compiled if this is the first time it has been seen.
fn program_of(
    minds: &mut Minds,
    prelude: &str,
    data: &Data,
    rules: &Rules,
    stagger: Stagger,
    text: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    if let Some(ready) = minds.programs.get(text) {
        return Ok(ready.clone());
    }
    let made = compile(prelude, data, rules, stagger.0, text, mrb)?;
    minds.programs.insert(text.to_string(), made.clone());
    Ok(made)
}

/// **Where the player's file is**, read once at startup — and the prelude beside it, which is read
/// every time something is compiled because a browser's copy is a table in the binary and a PC's
/// is a file somebody may be editing.
#[derive(Resource, Debug)]
pub struct Prelude(pub String);

/// `Startup`, after the data stage: the file, the prelude, and nothing compiled yet.
pub fn read_the_scripts(mut commands: Commands, ruby: Res<crate::RubyDir>) {
    let prelude = platform::read(&ruby.0.join(PRELUDE_FILE)).unwrap_or_default();
    let (file, trouble) = match platform::read(&ruby.0.join(SCRIPT_FILE)) {
        Ok(text) => (text, None),
        Err(why) => {
            error!("{SCRIPT_FILE}: {why} — inserters will have no mind");
            (String::new(), Some(format!("{SCRIPT_FILE}: {why}")))
        }
    };
    commands.insert_resource(Prelude(prelude));
    commands.insert_resource(Minds {
        file,
        everyone: None,
        own: HashMap::new(),
        programs: HashMap::new(),
        swaps: 0,
        generation: 1,
        trouble,
    });
}

// ---------------------------------------------------------------------------------------------
// Keeping the crew in step with the grid
// ---------------------------------------------------------------------------------------------

/// **One entity with a script per inserter on the map**, and none left over — and each of them
/// running the text it is supposed to be running.
///
/// The grid is what a player builds on and the entity is only where the script lives, so the grid
/// leads: a tile that has become an inserter gets an entity, and an entity whose tile is not an
/// inserter any more is despawned — which takes its `ScriptTask` with it, which terminates the
/// task in the VM and closes anything it had subscribed to (rubevy, "Replacing and removing a
/// script"). That is the whole of "an inserter that was taken away leaves no task behind".
///
/// **An arm whose text has changed is not despawned**, it is handed a new script
/// (`replace_script`), so that Apply in the editor is one arm starting over and not an arm
/// disappearing and coming back. Which arms those are is read off the text each is running
/// ([`Inserter::running`]), so applying to one leaves the others exactly as they were — which is
/// a thing the checks measure.
///
/// It runs when the grid has changed or when a text has, and does nothing at all otherwise.
#[allow(clippy::too_many_arguments)]
pub fn keep_the_crew(
    mut commands: Commands,
    grid: Res<Grid>,
    data: Res<Data>,
    rules: Res<Rules>,
    prelude: Res<Prelude>,
    stagger: Res<Stagger>,
    mut minds: ResMut<Minds>,
    mut arms: ResMut<Arms>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    standing: Query<(Entity, &Inserter)>,
    mut seen: Local<(u64, u64)>,
) {
    if *seen == (grid.changes, minds.generation()) {
        return;
    }
    *seen = (grid.changes, minds.generation());

    let wanted: HashSet<usize> = grid
        .built()
        .iter()
        .filter(|&&t| grid.at(t as usize).map(|b| b.what) == Some(What::Inserter))
        .map(|&t| t as usize)
        .collect();
    let prelude = prelude.0.clone();
    let mut here: HashSet<usize> = HashSet::new();

    for (entity, inserter) in &standing {
        if !wanted.contains(&inserter.tile) {
            commands.entity(entity).despawn();
            arms.forget(inserter.tile);
            continue;
        }
        here.insert(inserter.tile);
        let text = minds.text_for(inserter.tile).to_string();
        if inserter.running == hash_of(&text) {
            continue;
        }
        // this one has been given a new mind
        match program_of(&mut minds, &prelude, &data, &rules, *stagger, &text, &mut mrb) {
            Ok((handle, _)) => {
                minds.swaps += 1;
                arms.forget(inserter.tile);
                commands.entity(entity).insert(Inserter {
                    tile: inserter.tile,
                    running: hash_of(&text),
                });
                replace_script(
                    &mut commands,
                    entity,
                    Script::new(handle).with_name(name_of(&grid, inserter.tile)),
                );
            }
            Err(why) => {
                error!("{why}");
                arms.mark_stopped(inserter.tile, first_line(&why));
            }
        }
    }

    for tile in wanted {
        if here.contains(&tile) {
            continue;
        }
        let text = minds.text_for(tile).to_string();
        match program_of(&mut minds, &prelude, &data, &rules, *stagger, &text, &mut mrb) {
            Ok((handle, _)) => {
                minds.swaps += 1;
                arms.forget(tile);
                commands.spawn((
                    Inserter { tile, running: hash_of(&text) },
                    Script::new(handle).with_name(name_of(&grid, tile)),
                ));
            }
            Err(why) => {
                // **the arm stands still and the game goes on**, which is the rule for every
                // broken script here: it is the player's file and a typo in it is not a crash
                error!("the inserter at {} will not start: {why}", grid.tile_of(tile));
                arms.mark_stopped(tile, first_line(&why));
            }
        }
    }
}

/// **The first line of a compiler's complaint**, which is the one with the place in it — the
/// rest are the diagnostics after it, and a mark over a building has room for one sentence.
fn first_line(why: &str) -> String {
    why.lines().next().unwrap_or(why).to_string()
}

/// What a script is called in the log and in the VM panel: the tile it stands on.
fn name_of(grid: &Grid, tile: usize) -> String {
    let at = grid.tile_of(tile);
    format!("inserter {},{}", at.x, at.y)
}

// ---------------------------------------------------------------------------------------------
// The three questions
// ---------------------------------------------------------------------------------------------

/// The tile and the way it faces of whichever inserter asked.
fn asking(world: &World, request: &Request) -> Option<(usize, Dir)> {
    let who = request.entity.or_else(|| request.entity_arg(0))?;
    let tile = world.get::<Inserter>(who)?.tile;
    let grid = world.get_resource::<Grid>()?;
    match grid.at(tile) {
        Some(&Building { what: What::Inserter, dir, .. }) => Some((tile, dir)),
        _ => None,
    }
}

/// An item number as an [`Answer`], and `Nil` for nothing — which is what the prelude turns into
/// a Symbol or `nil`.
fn as_item(item: Option<ItemId>) -> Answer {
    match item {
        Some(id) => Answer::Num(id as f64),
        None => Answer::Nil,
    }
}

/// `Startup`: the three questions an inserter's script asks, answered inside the tick.
pub fn install_answers(mut scripts: ResMut<ScriptWorld>) {
    scripts.answer_in_tick(
        "factory.behind",
        Box::new(|world: &World, request: &Request| {
            let Some((tile, dir)) = asking(world, request) else { return Answer::Nil };
            let (Some(grid), Some(lanes)) =
                (world.get_resource::<Grid>(), world.get_resource::<Lanes>())
            else {
                return Answer::Nil;
            };
            let Some(from) = grid.step_from(tile, dir.back()) else { return Answer::Nil };
            as_item(machines::would_give(grid, lanes, from))
        }),
    );
    scripts.answer_in_tick(
        "factory.holding",
        Box::new(|world: &World, request: &Request| {
            let Some((tile, _)) = asking(world, request) else { return Answer::Nil };
            let Some(grid) = world.get_resource::<Grid>() else { return Answer::Nil };
            as_item(grid.at(tile).and_then(|b| b.held.first()))
        }),
    );
    scripts.answer_in_tick(
        "factory.front_takes",
        Box::new(|world: &World, request: &Request| {
            let Some((tile, dir)) = asking(world, request) else { return Answer::Bool(false) };
            // the prelude answers -1 for a name nothing declares, which is no item at all
            let asked = request.num_or(0, -1.0);
            // finite and not negative, said that way round because a NaN is neither
            if !asked.is_finite() || asked < 0.0 {
                return Answer::Bool(false);
            }
            let item = asked as ItemId;
            let (Some(grid), Some(lanes), Some(rules), Some(data)) = (
                world.get_resource::<Grid>(),
                world.get_resource::<Lanes>(),
                world.get_resource::<Rules>(),
                world.get_resource::<Data>(),
            ) else {
                return Answer::Bool(false);
            };
            let Some(into) = grid.step_from(tile, dir) else { return Answer::Bool(false) };
            Answer::Bool(machines::would_take(grid, lanes, rules, data, into, item))
        }),
    );
}

// ---------------------------------------------------------------------------------------------
// The one act
// ---------------------------------------------------------------------------------------------

/// **`RubevySet::Answer`: a `move` starts a swing**, and the request is kept until it arrives.
///
/// What is done here is the picking up, and it is done **now** rather than at the end of the
/// swing so that the item travels with the arm — it is out of the belt and in the hand, which is
/// where the picture draws it and where the sum that says nothing is lost counts it.
///
/// A `move` with nothing behind it and nothing in the hand answers `false` at once, which reaches
/// the script on the next frame; so a `loop { move }` is one frame an iteration and cannot spin
/// the VM, and it is also an arm swinging at nothing, which is what `behind` is for.
pub fn answer_moves(
    mut scripts: ResMut<ScriptWorld>,
    mut arms: ResMut<Arms>,
    mut grid: ResMut<Grid>,
    mut lanes: ResMut<Lanes>,
    standing: Query<&Inserter>,
) {
    for request in scripts.take_requests() {
        if request.kind != "factory.move" {
            // **every request is answered**, or the task that asked it is parked for ever. A
            // question nothing here knows is `nil`, which is what rubevy answers for one nobody
            // registered at all.
            scripts.answer(&request, Answer::Nil);
            continue;
        }
        let tile = request.entity.and_then(|e| standing.get(e).ok()).map(|i| i.tile);
        let Some(tile) = tile else {
            scripts.answer(&request, Answer::Bool(false));
            continue;
        };
        // an empty hand reaches behind; a full one is carrying on with what it was refused
        if !machines::start_swing(&mut grid, &mut lanes, tile) {
            scripts.answer(&request, Answer::Bool(false));
            continue;
        }
        // a second `move` from the same tile would be a second task on one arm, which cannot
        // happen with one script; if it ever does, the older one is answered rather than lost
        if let Some(old) = arms.waiting.insert(tile, request) {
            scripts.answer(&old, Answer::Bool(false));
        }
    }
}

/// **After the factory's step: the arms that arrived are the answers.**
pub fn finish_swings(mut scripts: ResMut<ScriptWorld>, mut arms: ResMut<Arms>) {
    if arms.finished.is_empty() {
        return;
    }
    let finished = core::mem::take(&mut arms.finished);
    for (tile, placed) in finished {
        if let Some(request) = arms.waiting.remove(&tile) {
            scripts.answer(&request, Answer::Bool(placed));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// A script that stopped
// ---------------------------------------------------------------------------------------------

/// **An inserter whose script has ended stands still, and says where it broke.**
///
/// Every way a script can stop comes through here: an exception it did not rescue, a
/// `Task::Overrun` (which is an `Exception` and not a `StandardError`, so a `rescue => e` does not
/// keep it going), and a `run` that simply came back. The game goes on — one arm is one arm — and
/// the picture puts a mark over it ([`Arms::has_stopped`]).
pub fn watch_endings(
    mut ended: MessageReader<ScriptEnded>,
    mut arms: ResMut<Arms>,
    mut scripts: ResMut<ScriptWorld>,
    standing: Query<(&Inserter, Option<&ScriptTask>)>,
    grid: Res<Grid>,
) {
    for end in ended.read() {
        let Ok((inserter, task)) = standing.get(end.entity) else { continue };
        let at = task
            .and_then(|t| where_it_broke(&mut scripts, t))
            .unwrap_or_else(|| format!("{SCRIPT_FILE}:?"));
        arms.mark_stopped(inserter.tile, at.clone());
        let tile = grid.tile_of(inserter.tile);
        match end.status {
            ScriptStatus::Failed => error!(
                "the inserter at {}, {} stopped: {at}: {}",
                tile.x, tile.y, end.value
            ),
            ScriptStatus::Finished => info!(
                "the inserter at {}, {} ran to its end at {at} ({})",
                tile.x, tile.y, end.value
            ),
        }
    }
}

/// **`file:line`, in the player's own terms** — read out of the VM, off the task that ended.
///
/// **A task that has ended has no frames left.** `ScriptWorld::stats` answers an empty list and
/// no location at the moment `ScriptEnded` arrives, which is the one moment this is wanted
/// (measured 2026-09-21; it is why this is not the four lines it looks like it should be). So the
/// place is worked out where the exception still has a backtrace — in the prelude's own `rescue`,
/// which knows how far down the program the player's first line is because the game wrote that
/// number into it ([`names_and_numbers`]) — and left on the task as an ordinary instance
/// variable. This reads it back with one `ivar_get`, the way the garden reads a creature's memory
/// out of a task it never asks anything of.
///
/// `None` where there is nothing there: a script that ran to its end and never raised, or one
/// stopped by something a `rescue => e` does not catch — `Task::Overrun` is an `Exception` and
/// not a `StandardError`, so an arm that overran says `inserter.rb:?` and the VM's own message.
pub fn where_it_broke(scripts: &mut ScriptWorld, task: &ScriptTask) -> Option<String> {
    let value = scripts.vm.ivar_get(task.task(), BROKE_AT_IVAR);
    let bytes = scripts.vm.str_bytes(value)?;
    Some(String::from_utf8_lossy(bytes).into_owned())
}

/// The instance variable the prelude's `rescue` leaves the place in, on its own task.
const BROKE_AT_IVAR: &str = "@broke_at";

#[cfg(test)]
mod tests {
    use super::*;

    /// **What the game writes in front of the prelude is Ruby**, and it carries the names a data
    /// file wrote however they were spelled.
    #[test]
    fn the_game_writes_the_item_names_and_the_swing_into_the_program() {
        let (data, rules) = crate::data::for_a_test(concat!(
            "item :iron_ore, icon: 0\n",
            "item :\"a name with spaces\", icon: 1\n",
            "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",
            "recipe :iron_ore, in: {}, out: { iron_ore: 1 }, time: 1.0, made_in: :furnace\n",
            "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n",
            "miner :drill, seconds_per_item: 1.0\n",
            "chest :crate, capacity: 4\n",
            "ore :iron_ore, per_tile: 10\n",
            "inserter :arm, seconds_per_item: 0.25\n",
        ));
        let written = names_and_numbers(&data, &rules, 1.0, 123);
        assert!(written.contains(":\"iron_ore\""), "{written}");
        assert!(written.contains(":\"a name with spaces\""), "{written}");
        assert!(written.contains("0.25"), "{written}");
        assert!(written.contains("123"), "and how far down the player's first line is: {written}");
        // **the block's length does not depend on what is in it**, which is what the two passes
        // in `compile` rest on
        assert_eq!(
            names_and_numbers(&data, &rules, 1.0, 0).lines().count(),
            names_and_numbers(&data, &rules, 0.0, 999_999).lines().count()
        );
        // and it compiles, which is the only thing that says the quoting is right
        let bytes = platform::compile(&format!("{written}Inserter"), "written.rb");
        assert!(bytes.is_ok(), "{:?}", bytes.err());
    }
}
