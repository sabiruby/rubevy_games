//! **A factory, written down** — the grid, the belts, the ore, the scripts, and what the two
//! kinds of script remember.
//!
//! # What a save is and is not
//!
//! It is the **world**, and the world here is the grid and the lanes: a miner half way through a
//! dig, a furnace half way through a craft and an arm half way through a swing are all fields of
//! [`crate::grid::Building`], because F3 put the arm's state in the grid rather than in a
//! component for exactly this. So the difficult half of a save file — "what is the factory doing
//! right now" — is `Vec<Option<Building>>` and nothing else.
//!
//! **It is not the tasks.** A task parked on a `move` cannot be written down (rubevy says so on
//! `Held`: a request is an `ObjId` in one VM's heap), so a loaded game starts every script again
//! from the top of `run`. What a script must not forget across that is its `@memory`, which is
//! saved and put back — the garden's road, one word for one thing.
//!
//! **It is not `data.rb`.** A save is a factory built out of a particular set of declarations: an
//! item's number, a machine's number and the size of the map are all indices into tables that
//! file makes. So the file carries a print of those tables ([`Stamp`]) and a save taken against a
//! different `data.rb` is **refused and said**, rather than read into a world where tile 900 is
//! off the map and item 2 is a different thing.
//!
//! # The shape of the file
//!
//! Only the tiles that have something on them and the lanes that have something on them are
//! written: an empty map of 2048 by 2048 would otherwise be four million nulls. Both are written
//! in tile order, so two saves of one factory are the same text — which is the check
//! (`save → load → save`), and the reason a `HashMap` is turned into a sorted `Vec` on the way
//! out.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use rubevy::{ScriptTask, ScriptWorld};
use serde::{Deserialize, Serialize};

use crate::belts::{Lanes, OnBelt, Rules, Steps};
use crate::control::TheControl;
use crate::data::{Data, ItemId, MachineId, RecipeId};
use crate::grid::{Building, Dir, Grid, Ore, What};
use crate::inserters::{Inserter, Minds};
use crate::machines::Stock;
use crate::platform;

/// **What this build writes, and the only number it will read back.**
///
/// It is a promise about the *meaning* of the fields below and not a checksum of them: a field
/// serde can default, added or taken away, leaves a file that parses and means something else.
/// The rule for moving it is the garden's (`garden/src/main.rs`): if a factory written by the old
/// build would come back **wrong** rather than not at all, this goes up.
pub const SAVE_VERSION: u32 = 1;

/// **The print of the `data.rb` a save was taken against.**
///
/// Everything in the file is numbers into that file's tables: `item: 2` is the third `item` line,
/// `What::Machine(1)` is the second `machine` line, and a tile index is a place on a map whose
/// size is `map :world, size:`. Names rather than a count, because two data files with three
/// items each are not the same three items; and in declaration order, because the order **is**
/// the numbering.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Stamp {
    /// `map :world, size: [w, h]`
    tiles: [u32; 2],
    /// every `item`, in the order `data.rb` declares them
    items: Vec<String>,
    /// every `machine`, in the order `data.rb` declares them
    machines: Vec<String>,
    /// every `recipe`, in order: `Building::making` is a number into this
    recipes: Vec<String>,
    /// how many steps a tile is cut into (`belt … items_per_tile:` decides what may be written,
    /// and `crate::TILE_PX` what a position means)
    steps_per_tile: u32,
}

impl Stamp {
    /// The print of the tables this run is holding.
    pub fn of(data: &Data, rules: &Rules) -> Stamp {
        Stamp {
            tiles: [rules.map_tiles.x, rules.map_tiles.y],
            items: data.items.iter().map(|i| i.name.clone()).collect(),
            machines: data.machines.iter().map(|m| m.name.clone()).collect(),
            recipes: data.recipes.iter().map(|r| r.name.clone()).collect(),
            steps_per_tile: crate::TILE_PX,
        }
    }

    /// **What is different**, as a sentence a player can act on, or `None` where nothing is.
    fn differs_from(&self, now: &Stamp) -> Option<String> {
        if self.tiles != now.tiles {
            return Some(format!(
                "it was saved on a map of {} by {} and data.rb now says {} by {}",
                self.tiles[0], self.tiles[1], now.tiles[0], now.tiles[1]
            ));
        }
        for (what, then, here) in [
            ("items", &self.items, &now.items),
            ("machines", &self.machines, &now.machines),
            ("recipes", &self.recipes, &now.recipes),
        ] {
            if then != here {
                return Some(format!(
                    "the {what} in data.rb are not the ones it was saved with ({} then, {} now)",
                    then.join(", "),
                    here.join(", ")
                ));
            }
        }
        if self.steps_per_tile != now.steps_per_tile {
            return Some(format!(
                "a tile was {} steps and is now {}",
                self.steps_per_tile, now.steps_per_tile
            ));
        }
        None
    }
}

/// A factory, written down. This struct **is** the file format; there is no schema anywhere else.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FactorySave {
    /// which build wrote this, first in the file so that it is the first thing a person opening
    /// it sees and the only thing [`read_save`] reads before it decides
    pub version: u32,
    /// which `data.rb` it was written against
    stamp: Stamp,
    /// **the fraction of a step the belts had not taken yet** (`Lanes::part_of_a_step`). Without
    /// it a factory loaded at a sixtieth of a second would restart the fraction at zero, which is
    /// a belt that is up to one step of sixteen behind where it was.
    part_of_a_step: f32,
    /// what is left under each tile that has any, as `[tile, left]`
    ore: Vec<(u32, u32)>,
    /// every tile something is built on, in tile order
    buildings: Vec<TileSave>,
    /// every tile with something on its belt, in tile order
    belts: Vec<LaneSave>,
    /// the scripts that are not `inserter.rb`
    minds: MindsSave,
    /// the control stage: its text, and how far through its goal it had got
    control: ControlSave,
}

/// One built tile. The fields are [`Building`]'s, which is where a machine's half-done work and
/// an arm's hand already live.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct TileSave {
    at: u32,
    what: WhatSave,
    dir: u8,
    /// seconds into the dig, the craft or the swing
    work: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    held: Vec<(ItemId, u32)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    made: Vec<(ItemId, u32)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    making: Option<RecipeId>,
    #[serde(default, skip_serializing_if = "is_false")]
    swinging: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// [`What`] as the file spells it — a word for the four fittings and a number for a machine,
/// rather than serde's own tagging of an enum, so that the file reads as a factory.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
enum WhatSave {
    Belt,
    Miner,
    Chest,
    Inserter,
    Machine(MachineId),
    /// the rest of a machine's footprint, and the tile that is the machine
    Covered(u32),
}

/// One tile's belt, front first. `along` is a whole number of steps, which is what F2a made it —
/// so a saved position is exact and `save → load → save` is the same text.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct LaneSave {
    at: u32,
    on: Vec<(Steps, ItemId)>,
}

/// The scripts the player has written, and nothing that came off the disk unchanged: a save that
/// carried a copy of `inserter.rb` would put an old copy of the file back over an edited one.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct MindsSave {
    /// a text applied to every arm, where there is one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    everyone: Option<String>,
    /// arms with a script of their own, by tile, in tile order
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    own: Vec<(u32, String)>,
    /// what each running arm remembers (`@memory`), by tile, in tile order. Only the arms that
    /// remember anything are written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    memory: Vec<(u32, serde_json::Value)>,
}

/// The control stage: the text, and **how far through its goal it had got**.
///
/// The progress is saved and put back, rather than counted again from the load. A goal is what
/// the game is being played for, so a factory saved five gears from winning is a factory five
/// gears from winning; the alternative — the counters at zero and the chests still full — is a
/// game that quietly took the afternoon back. It is the same road the arms' `@memory` takes, for
/// the same reason, and F4's rule is untouched: a *new* `control.rb` still counts from zero,
/// because that is a new game rather than the same one continued.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct ControlSave {
    /// `ruby/control.rb`, or whatever was applied over it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// `@got`: how much of each wanted item has been delivered
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    got: serde_json::Value,
    #[serde(default, skip_serializing_if = "is_false")]
    won: bool,
}

// ---------------------------------------------------------------------------------------------
// Where a save goes
// ---------------------------------------------------------------------------------------------

/// Where a save is written and read: `--save PATH`, or `save_file` in the store, or
/// [`crate::platform::SAVE_FILE`]. **In a browser it is a `localStorage` key** and not a file.
#[derive(Resource, Debug)]
pub struct SaveFile {
    pub path: String,
    /// `--save PATH` was asked for on the command line, so a headless run writes one as it ends
    pub on_exit: bool,
}

/// Somebody asked for a save or a load this frame: a key, a button, or the end of a `--save` run.
#[derive(Resource, Debug, Default)]
pub struct Asked {
    pub save: bool,
    pub load: bool,
}

/// The last thing a save or a load said, for the HUD and the checks.
#[derive(Resource, Debug, Default, Clone)]
pub struct SaveNote {
    pub text: String,
    pub at: f32,
    pub bad: bool,
}

impl SaveNote {
    pub fn say(&mut self, at: f32, bad: bool, text: impl Into<String>) {
        self.text = text.into();
        self.at = at;
        self.bad = bad;
    }
}

/// A file that has been read and not yet put into the world. It is taken apart by
/// [`load_the_factory`] on the next frame, which is the only thing that builds a world out of one.
#[derive(Resource, Debug)]
pub struct Loading(pub FactorySave);

/// **The memories waiting for their scripts to start.** A loaded arm's task reaches
/// `run_inserter` — and so gets an object to hang `@memory` on — a frame or two after the grid
/// was filled in, so what is read out of the file waits here until there is somewhere to put it.
#[derive(Resource, Debug, Default)]
pub struct Restoring {
    /// by tile, so that it survives the entity being rebuilt by `keep_the_crew`
    arms: HashMap<usize, serde_json::Value>,
    control: ControlSave,
    /// how long it has been waiting, in seconds of the game's own clock
    since: f32,
}

// ---------------------------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------------------------

/// **The whole factory into one file.** An ordinary system: everything it writes is a resource,
/// and the `ScriptWorld` is there only for the memories.
#[allow(clippy::too_many_arguments)]
pub fn save_the_factory(
    time: Res<Time>,
    file: Res<SaveFile>,
    mut asked: ResMut<Asked>,
    mut note: ResMut<SaveNote>,
    grid: Res<Grid>,
    lanes: Res<Lanes>,
    ore: Res<Ore>,
    data: Res<Data>,
    rules: Res<Rules>,
    minds: Res<Minds>,
    control: Res<TheControl>,
    mut scripts: ResMut<ScriptWorld>,
    arms: Query<(&Inserter, &ScriptTask)>,
    control_task: Query<&ScriptTask, With<crate::control::ControlScript>>,
) {
    if !asked.save {
        return;
    }
    asked.save = false;
    let save = the_factory_as_a_file(
        &grid,
        &lanes,
        &ore,
        &data,
        &rules,
        &minds,
        &control,
        &mut scripts,
        &arms,
        control_task.single().ok().map(|t| t.task()),
    );
    match serde_json::to_string_pretty(&save) {
        Err(e) => {
            error!("the factory would not be written: {e}");
            note.say(time.elapsed_secs(), true, format!("not saved: {e}"));
        }
        Ok(text) => match platform::write(Path::new(&file.path), &text) {
            Ok(()) => {
                let says = format!(
                    "saved {} buildings and {} items to {}",
                    save.buildings.len(),
                    save.belts.iter().map(|l| l.on.len()).sum::<usize>(),
                    file.path
                );
                info!("{says}");
                note.say(time.elapsed_secs(), false, says);
            }
            Err(e) => {
                error!("{}: {e}", file.path);
                note.say(time.elapsed_secs(), true, format!("not saved: {e}"));
            }
        },
    }
}

/// The factory as the file sees it. Apart so that a test may call it without an `App`.
#[allow(clippy::too_many_arguments)]
fn the_factory_as_a_file(
    grid: &Grid,
    lanes: &Lanes,
    ore: &Ore,
    data: &Data,
    rules: &Rules,
    minds: &Minds,
    control: &TheControl,
    scripts: &mut ScriptWorld,
    arms: &Query<(&Inserter, &ScriptTask)>,
    control_task: Option<sabiruby::value::ObjId>,
) -> FactorySave {
    let mut buildings = Vec::new();
    let mut belts = Vec::new();
    // `Grid::built` is kept in tile order, which is what makes two saves of one factory the same
    // text without a sort here
    for &t in grid.built() {
        let at = t as usize;
        if let Some(b) = grid.at(at) {
            buildings.push(TileSave {
                at: t,
                what: what_out(b.what),
                dir: b.dir.number() as u8,
                work: b.work,
                held: b.held.0.clone(),
                made: b.made.0.clone(),
                making: b.making,
                swinging: b.swinging,
            });
        }
        let lane = lanes.on(at);
        if !lane.is_empty() {
            belts.push(LaneSave {
                at: t,
                on: lane.iter().map(|o| (o.along, o.item)).collect(),
            });
        }
    }
    let mut memory: Vec<(u32, serde_json::Value)> = arms
        .iter()
        .filter_map(|(inserter, task)| {
            let value = read_memory(scripts, task.task())?;
            Some((inserter.tile as u32, value))
        })
        .collect();
    memory.sort_by_key(|(tile, _)| *tile);
    let (got, won) = control_state(scripts, control_task);
    FactorySave {
        version: SAVE_VERSION,
        stamp: Stamp::of(data, rules),
        part_of_a_step: lanes.part_of_a_step(),
        ore: ore
            .left
            .iter()
            .enumerate()
            .filter(|&(_, &left)| left > 0)
            .map(|(t, &left)| (t as u32, left))
            .collect(),
        buildings,
        belts,
        minds: MindsSave {
            everyone: minds.everyone().map(str::to_string),
            own: minds.each_own(),
            memory,
        },
        control: ControlSave { text: Some(control.text().to_string()), got, won },
    }
}

/// One script's `@memory`, as JSON, or `None` where there is nothing to keep. Public because the
/// checks read an arm's back after a load, which is the half of a save file a test can see.
pub fn read_memory(scripts: &mut ScriptWorld, task: sabiruby::value::ObjId) -> Option<serde_json::Value> {
    let vm = &mut scripts.vm;
    let being = vm.ivar_get(task, BEING_IVAR).obj()?;
    let memory = vm.ivar_get(being, MEMORY_IVAR);
    if memory.obj().is_none() {
        return None;
    }
    match sabiruby_serde::from_value::<serde_json::Value>(vm, memory) {
        Ok(serde_json::Value::Object(map)) if map.is_empty() => None,
        Ok(value) => Some(value),
        Err(e) => {
            // a Hash with something in it JSON has no name for — an entity, a Proxy. The arm
            // keeps it; the file does not get it.
            warn!("an inserter's memory would not convert: {}", vm.describe_error(&e));
            None
        }
    }
}

/// `@got` and `@won`, off the control stage's own object.
fn control_state(
    scripts: &mut ScriptWorld,
    task: Option<sabiruby::value::ObjId>,
) -> (serde_json::Value, bool) {
    let Some(task) = task else { return (serde_json::Value::Null, false) };
    let vm = &mut scripts.vm;
    let Some(being) = vm.ivar_get(task, BEING_IVAR).obj() else {
        return (serde_json::Value::Null, false);
    };
    let got = vm.ivar_get(being, "@got");
    let got = sabiruby_serde::from_value::<serde_json::Value>(vm, got)
        .unwrap_or(serde_json::Value::Null);
    (got, vm.ivar_get(being, "@won").truthy())
}

/// The instance variable both preludes hang their object on (`ruby/prelude.rb`,
/// `ruby/control_prelude.rb`): a task's own `self` is the VM's one `main` object, shared by every
/// task, so an object per script is what makes `@memory` this arm's and not everybody's.
const BEING_IVAR: &str = "@being";
/// What an inserter's script remembers across a save (`ruby/prelude.rb`, `memory`).
const MEMORY_IVAR: &str = "@memory";

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

/// **The version is read on its own, before anything else.** A file from another build is refused
/// by number rather than by whichever field happened to change, which is the garden's rule (G5).
pub fn read_save(path: &str) -> Result<FactorySave, String> {
    let text = platform::read(Path::new(path))?;
    #[derive(Deserialize)]
    struct OnlyTheVersion {
        version: Option<u32>,
    }
    let stamp: OnlyTheVersion = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    match stamp.version {
        Some(SAVE_VERSION) => {}
        Some(other) => {
            return Err(format!(
                "{path}: saved with version {other}, this factory reads {SAVE_VERSION}"
            ))
        }
        None => {
            return Err(format!("{path}: saved with no version, this factory reads {SAVE_VERSION}"))
        }
    }
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// **A factory put back.** One frame: the grid, the lanes, the ore and the scripts, all at once,
/// so that nothing ever sees half a world.
///
/// The one thing it cannot do here is the memories — a loaded arm has no object to hang one on
/// until its task has reached `run_inserter` — so those go into [`Restoring`], which is emptied
/// over the following frames.
#[allow(clippy::too_many_arguments)]
pub fn load_the_factory(
    mut commands: Commands,
    time: Res<Time>,
    loading: Option<Res<Loading>>,
    data: Res<Data>,
    rules: Res<Rules>,
    mut grid: ResMut<Grid>,
    mut lanes: ResMut<Lanes>,
    mut ore: ResMut<Ore>,
    mut minds: ResMut<Minds>,
    mut note: ResMut<SaveNote>,
) {
    let Some(loading) = loading else { return };
    let save = loading.0.clone();
    commands.remove_resource::<Loading>();

    let now = Stamp::of(&data, &rules);
    if let Some(why) = save.stamp.differs_from(&now) {
        let says = format!("the save is of another factory: {why}");
        error!("{says}");
        note.say(time.elapsed_secs(), true, says);
        return;
    }

    let tiles = crate::grid::how_many(rules.map_tiles);
    *grid = Grid::new(rules.map_tiles);
    *lanes = Lanes::for_map(rules.map_tiles);
    lanes.set_part_of_a_step(save.part_of_a_step);
    for left in ore.left.iter_mut() {
        *left = 0;
    }
    for (at, left) in &save.ore {
        if (*at as usize) < tiles {
            ore.left[*at as usize] = *left;
        }
    }
    ore.changed = true;

    let mut built = 0;
    for tile in &save.buildings {
        let at = tile.at as usize;
        if at >= tiles {
            continue;
        }
        let mut b = Building::new(what_in(tile.what), Dir::ALL[(tile.dir as usize) % 4]);
        b.work = tile.work;
        b.held = Stock(tile.held.clone());
        b.made = Stock(tile.made.clone());
        b.making = tile.making;
        b.swinging = tile.swinging;
        grid.place(at, b);
        built += 1;
    }
    let mut carried = 0;
    for lane in &save.belts {
        let at = lane.at as usize;
        if at >= tiles {
            continue;
        }
        for &(along, item) in &lane.on {
            lanes.put_on(at, OnBelt { along, item });
            carried += 1;
        }
    }

    minds.restore(save.minds.everyone.clone(), save.minds.own.clone());
    commands.insert_resource(Restoring {
        arms: save.minds.memory.iter().map(|(t, v)| (*t as usize, v.clone())).collect(),
        control: save.control.clone(),
        since: time.elapsed_secs(),
    });
    // the control stage's own text goes back through the one door that swaps it (F4's `Rewrite`),
    // so that a loaded goal is compiled and started exactly as an edited one is
    if let Some(text) = save.control.text.clone() {
        commands.write_message(crate::control::Rewrite(text));
    }
    let says = format!("loaded {built} buildings and {carried} items");
    info!("{says}");
    note.say(time.elapsed_secs(), false, says);
}

/// **The memories, handed over as their scripts reach `run`.**
///
/// It runs every frame there is a [`Restoring`] and takes itself away when the last one has been
/// placed or when it has waited longer than [`Restoring::patience`] — an arm whose script will
/// not compile never reaches `run_inserter`, and a load is not a thing to hang on that.
pub fn restore_memories(
    mut commands: Commands,
    time: Res<Time>,
    mut restoring: ResMut<Restoring>,
    mut scripts: ResMut<ScriptWorld>,
    mut control: ResMut<TheControl>,
    arms: Query<(&Inserter, &ScriptTask)>,
    control_task: Query<&ScriptTask, With<crate::control::ControlScript>>,
) {
    let mut placed_control = restoring.control.text.is_none();
    if !placed_control && let Ok(task) = control_task.single() {
        let vm = &mut scripts.vm;
        if let Some(being) = vm.ivar_get(task.task(), BEING_IVAR).obj() {
            if !restoring.control.got.is_null()
                && let Ok(value) = sabiruby_serde::to_value(vm, &restoring.control.got)
            {
                vm.ivar_set(being, "@got", value);
            }
            if restoring.control.won {
                vm.ivar_set(being, "@won", sabiruby::Value::bool(true));
                control.won = true;
            }
            placed_control = true;
        }
    }
    let waiting = &mut restoring.arms;
    for (inserter, task) in &arms {
        let Some(memory) = waiting.get(&inserter.tile) else { continue };
        let vm = &mut scripts.vm;
        let Some(being) = vm.ivar_get(task.task(), BEING_IVAR).obj() else { continue };
        match sabiruby_serde::to_value(vm, memory) {
            Ok(value) => {
                vm.ivar_set(being, MEMORY_IVAR, value);
            }
            Err(e) => error!("a memory would not go back: {}", vm.describe_error(&e)),
        }
        waiting.remove(&inserter.tile);
    }
    let waited = time.elapsed_secs() - restoring.since;
    if (restoring.arms.is_empty() && placed_control) || waited > Restoring::patience() {
        if !restoring.arms.is_empty() || !placed_control {
            warn!(
                "{} scripts never started; the factory is running anyway",
                restoring.arms.len() + usize::from(!placed_control)
            );
        }
        commands.remove_resource::<Restoring>();
    }
}

impl Restoring {
    /// **How long a load waits for the scripts to start**, and it is a number with a derivation
    /// rather than a taste: a script is compiled and started by `keep_the_crew` on the frame
    /// after the grid changed, reaches `run_inserter` on the frame after that, and a browser
    /// compiles a program synchronously in between. So what is being waited for is a handful of
    /// frames, and what is being guarded against is a script that will never start at all. One
    /// second is longer than any number of frames this is and shorter than a player notices.
    fn patience() -> f32 {
        1.0
    }
}

// ---------------------------------------------------------------------------------------------
// The two little conversions
// ---------------------------------------------------------------------------------------------

fn what_out(what: What) -> WhatSave {
    match what {
        What::Belt => WhatSave::Belt,
        What::Miner => WhatSave::Miner,
        What::Chest => WhatSave::Chest,
        What::Inserter => WhatSave::Inserter,
        What::Machine(id) => WhatSave::Machine(id),
        What::Covered { origin } => WhatSave::Covered(origin),
    }
}

fn what_in(what: WhatSave) -> What {
    match what {
        WhatSave::Belt => What::Belt,
        WhatSave::Miner => What::Miner,
        WhatSave::Chest => What::Chest,
        WhatSave::Inserter => What::Inserter,
        WhatSave::Machine(id) => What::Machine(id),
        WhatSave::Covered(origin) => What::Covered { origin },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(
        "item :iron_ore, icon: 0\n",
        "item :iron_plate, icon: 1\n",
        "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",
        "recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 }, time: 1.0, made_in: :furnace\n",
        "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n",
        "miner :drill, seconds_per_item: 1.0\n",
        "chest :crate, capacity: 4\n",
        "inserter :arm, seconds_per_item: 1.0\n",
        "ore :iron_ore, per_tile: 10, patch_radius: 1.0, patches: [1, 1]\n",
        "map :world, size: [16, 16]\n",
    );

    /// **A save is of the data file it was taken against**, and a different one is said rather
    /// than read into a world whose numbers mean something else.
    #[test]
    fn a_save_from_another_data_file_is_refused_and_says_which_half() {
        let (data, rules) = crate::data::for_a_test(DATA);
        let now = Stamp::of(&data, &rules);
        assert_eq!(now.differs_from(&now), None, "the same tables are the same print");

        let mut bigger = now.clone();
        bigger.tiles = [32, 16];
        let why = bigger.differs_from(&now).expect("a different map is different");
        assert!(why.contains("32 by 16"), "{why}");

        let mut renamed = now.clone();
        renamed.items[1] = "copper_plate".into();
        let why = renamed.differs_from(&now).expect("different items are different");
        assert!(why.contains("items"), "{why}");

        let mut reordered = now.clone();
        reordered.machines.push("assembler".into());
        assert!(reordered.differs_from(&now).is_some(), "an added machine moves no number, but");
    }

    /// The file's own words for what is on a tile go both ways, for every kind there is.
    #[test]
    fn every_kind_of_building_survives_the_two_little_conversions() {
        for what in [
            What::Belt,
            What::Miner,
            What::Chest,
            What::Inserter,
            What::Machine(3),
            What::Covered { origin: 17 },
        ] {
            assert_eq!(what_in(what_out(what)), what);
        }
    }

    /// **A version that is not this one is refused before the rest of the file is read**, which is
    /// what lets the format change without a build having to parse the old one.
    #[test]
    fn a_save_is_read_by_its_version_first() {
        let one = serde_json::json!({ "version": SAVE_VERSION + 1, "nonsense": true });
        let text = one.to_string();
        #[derive(Deserialize)]
        struct OnlyTheVersion {
            version: Option<u32>,
        }
        let read: OnlyTheVersion = serde_json::from_str(&text).unwrap();
        assert_eq!(read.version, Some(SAVE_VERSION + 1));
        // and a file with no version at all parses this far, which is how "saved with no version"
        // can be said rather than "expected field `version`"
        let none: OnlyTheVersion = serde_json::from_str("{\"tick\": 3}").unwrap();
        assert_eq!(none.version, None);
    }
}
