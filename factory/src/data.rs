//! **The data stage**: what a factory is made of, written in Ruby and read into Rust tables.
//!
//! This is Factorio's data stage at the size of a sample game (`docs/plans/factory-plan.md` §1).
//! `ruby/data.rb` is a file of **declarations** — no loops, no state, no frames — and by the time
//! the first `Update` runs it has become [`Data`] and [`crate::belts::Rules`], and the VM it was
//! read in has given the tables back.
//!
//! ```ruby
//! item   :iron_plate, icon: 1
//! recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 }, time: 2.0, made_in: :furnace
//! ```
//!
//! **The port is `sabiruby_serde::declare`**, and the whole of the reading is eight of them:
//! `Declarations::<T>::install(vm).define(vm, "item")` puts a method on `Object`, the script calls
//! it, and each call's keyword arguments are deserialized into a `T` **inside the native** — which
//! is what puts the error on the declaration's own line.
//!
//! # Why eight words and not three
//!
//! The plan names `item`, `recipe` and `machine`, which are about *what is made*. The fittings the
//! world has built in — the belt, the miner, the chest, the ore in the ground and the inserter —
//! are made by no recipe and in no machine, and each has numbers of its own with names of its own.
//! Giving them one `machine` shape with five optional fields would mean `capacity:` on a furnace
//! deserializing perfectly and being refused afterwards by hand-written code; giving each its own
//! word means **serde** refuses it, at the line, which is the whole reason the declarations are
//! read through serde at all.
//!
//! **`ore` is F2a's and `inserter` is F3's**, and each is a word rather than a field on `miner`
//! for exactly that reason: how much a tile of ground holds is not one of the drill's numbers and
//! neither is how long an arm's swing takes, and putting either there would be the
//! `capacity:`-on-a-furnace shape again ([`OreDecl`], [`InserterDecl`]).
//!
//! # Where an error says it is
//!
//! Two kinds, and only the first gets its line for free:
//!
//! * **inside a declaration** — an unknown field, a missing one, a field of the wrong type, a
//!   number that is not more than zero, a machine no tiles wide. Deserializing runs in the native,
//!   so the raise carries the Ruby frame it happened in and the first line of the exception's
//!   backtrace is `data.rb:4` ([`Trouble::from_raise`]).
//! * **between declarations** — a recipe naming an item nothing declares, a `made_in:` naming no
//!   machine, a machine whose `size` and `sprite` disagree. These can only be asked once every
//!   declaration has been read, and the line comes back with the declaration:
//!   [`sabiruby_serde::declare::Declarations::take_with_lines`] hands over a `Vec<Declared<T>>`
//!   whose `line` is **the line serde's own refusal would land on**, so the two kinds of error
//!   point at the same place. F2 had no such thing — `take` gave names and values and nothing else
//!   — and looked the declaration up in the source text instead, which found the line a
//!   declaration *starts* on where serde says the line it *ends* on. That workaround was reported
//!   (`docs/worklog/2026-09-21-factory-F2.md` §9) and is what F3 took out.
//!
//! # It can be read again
//!
//! [`read_the_declarations`] is a plain function over a VM and a string, and calling it twice on
//! the same VM works: `install` makes a fresh table and `define` puts a fresh method over the old
//! one. F5's editor needs that (edit `data.rb`, load it again); F2 needs it because **the checks
//! load four broken data files through the same door** to prove what the errors say, and they do
//! it in the game's own VM, in a browser as well as on a PC.

use std::collections::BTreeMap;

use bevy::prelude::*;
use sabiruby::error::VmError;
use sabiruby::{Value, Vm};
use sabiruby_serde::declare::{expose, Declarations, Declared};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::belts::Rules;

/// Which item, by its place in [`Data::items`]. The lanes carry one of these per item on a belt,
/// so it is as small as it can be.
pub type ItemId = u16;
/// Which recipe, by its place in [`Data::recipes`].
pub type RecipeId = u16;
/// Which kind of machine, by its place in [`Data::machines`].
pub type MachineId = u16;

// ---------------------------------------------------------------------------------------------
// What a declaration looks like on the way in
// ---------------------------------------------------------------------------------------------
//
// One struct per word. Every one of them is `deny_unknown_fields`, which is what turns a
// misspelled key from silently nothing into a `TypeError` at its own line (sabiruby
// `docs/design/serde.md`: "worth recommending in a data file"), and every number that has to be
// more than zero says so through a `deserialize_with`, so that **the refusal happens inside the
// declaration** and carries its line. A check made afterwards would not have one.

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ItemDecl {
    /// Which 8 px picture of `assets/items/items.png` it is drawn with.
    icon: u32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct RecipeDecl {
    /// What it consumes, by item name. `in` is a keyword in Rust and a perfectly good label in
    /// Ruby, which is why the field is renamed rather than the word changed.
    #[serde(rename = "in", default, deserialize_with = "counts")]
    inputs: BTreeMap<String, u32>,
    #[serde(deserialize_with = "counts")]
    out: BTreeMap<String, u32>,
    /// How long one craft takes, in seconds, in a machine of speed 1.
    #[serde(deserialize_with = "more_than_zero")]
    time: f32,
    made_in: String,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct MachineDecl {
    /// How many tiles it covers, across and up.
    #[serde(deserialize_with = "a_size")]
    size: [u32; 2],
    /// One tile of the sheet per tile it covers, row by row **from the bottom left**, which is
    /// the way the map itself counts. It is always a list, even for a machine one tile big: a
    /// field with two shapes is a field two things can be wrong with.
    sprite: Vec<u16>,
    /// How many times faster than the recipe says it runs. 1 is the identity — the machine takes
    /// the recipe's own time — and a faster one is a later stage's.
    #[serde(deserialize_with = "more_than_zero")]
    speed: f32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct BeltDecl {
    #[serde(deserialize_with = "more_than_zero")]
    tiles_per_second: f32,
    #[serde(deserialize_with = "fits_a_tile")]
    items_per_tile: u32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct MinerDecl {
    #[serde(deserialize_with = "more_than_zero")]
    seconds_per_item: f32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct ChestDecl {
    #[serde(deserialize_with = "at_least_one")]
    capacity: u32,
}

/// **The arm the player writes Ruby for** (F3), and a fitting of the world like the four above
/// it: no recipe makes it and no machine makes it in.
///
/// It has **one** number — how long a swing takes — and the word for it is the miner's, because
/// it means the same thing: how long this thing takes over one item. How far an inserter can
/// reach is not a number here and is not meant to become one: it takes from the tile behind it
/// and puts into the tile in front, which is what "one tile, with a direction" already says
/// (`crate::grid::What::Inserter`). A number for it would be a number with nothing behind it.
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct InserterDecl {
    #[serde(deserialize_with = "more_than_zero")]
    seconds_per_item: f32,
}

/// **How big the world is** (F3a), which is a declaration for the same reason the fittings are:
/// what a map is big enough for is what the game *is* — how many chains fit on it, how far the
/// ore is from the smelters, how much walking there is — and that is played rather than
/// configured. `factory.settings.txt` had it until F3a, where a page had no way of saying it at
/// all, and the author asked for the size to be free before any default was settled.
///
/// **`size:` is the machine's word, because it means the machine's thing**: how many tiles this
/// covers, across and up. The same reason `inserter` borrowed `seconds_per_item:` from `miner` at
/// F3 — one meaning, one spelling — and the reason not to invent `tiles:` for the same idea.
///
/// The floor and the ceiling are not here but in [`tables_of`]: the floor needs the ore's radius
/// and count, which are another declaration's, and a refusal that can name both numbers is worth
/// more than one that can only say "too small".
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct MapDecl {
    /// Tiles across and up, and **they may differ**: a map 96 by 16 is a valley.
    size: [u32; 2],
}

/// **What is in the ground**, which is a fitting of the world like the three above it: no recipe
/// makes it and no machine makes it in.
///
/// **Its name is the item that comes out of it** (the author, 2026-09-21): `ore :iron_ore,
/// per_tile: 60` says what the ground is made of, and a miner brings up whatever the ground it
/// stands on is. F2a wrote it the other way — the name was a label and `miner :drill, digs:
/// :iron_ore` said what came up — which put "what comes out of the ground" in two places and
/// made the miner the one that had to change when a second kind of ore was added. Now there is
/// one place, and a second kind of ore is a second `ore` line.
///
/// F2 left `ore_per_tile` in `factory.settings.txt` because its neighbour — how *wide* a patch is
/// — was wanted in `main`, before there is a VM to have read any Ruby with, and the two were one
/// number in the same struct. F2a moved the first; **F3a moves the other two and the line is
/// gone**, because the map's own size is a declaration now and nothing about the world is worked
/// out before the data stage.
///
/// **Three fields and no defaults.** A patch's width and how many patches there are decide
/// whether a map can be played on at all — four patches on a map of two hundred tiles is a
/// factory with nothing to feed it — so they are the data file's to say, like everything else
/// here (`docs/numbers.md` §9.3: there are no numbers of play in the binary).
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct OreDecl {
    #[serde(deserialize_with = "at_least_one")]
    per_tile: u32,
    /// How wide one patch is, in tiles, measured from its middle.
    #[serde(deserialize_with = "more_than_zero")]
    patch_radius: f32,
    /// **How many patches there are, across and up** — one in the middle of each cell of that
    /// grid ([`crate::grid::Ore::laid_out`]). `[2, 2]` is the four in the corners the game had
    /// before this was something a file could say, tile for tile.
    #[serde(deserialize_with = "at_least_one_each_way")]
    patches: [u32; 2],
}

/// **A number the factory divides by or runs on has to be more than zero**, and this is where
/// that is said, because here it is said at the line the number is written on.
///
/// It is the same contract `crate::belts::Rules` states and `main`'s `positive` keeps for the
/// settings: there is no floor inside the arithmetic, because a floor is a number with nowhere to
/// have come from.
fn more_than_zero<'de, D: Deserializer<'de>>(d: D) -> Result<f32, D::Error> {
    let n = f32::deserialize(d)?;
    if n.is_finite() && n > 0.0 {
        Ok(n)
    } else {
        Err(D::Error::custom(format!("{n} is not more than zero")))
    }
}

/// The same for something counted. Items are whole and there is at least one of them.
fn at_least_one<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let n = u32::deserialize(d)?;
    if n >= 1 { Ok(n) } else { Err(D::Error::custom("this counts items, so it is at least 1")) }
}

/// **How many items fit on one tile of belt**, which is the one number in the file that cannot be
/// anything it likes.
///
/// An item's place on a belt is a whole number of steps and a tile is [`crate::TILE_PX`] of them
/// (`crate::belts`), so the gap between two items — a tile's steps over this number — is only a
/// gap if the division comes out. **The refusal names what can be written**, because "3 will not
/// do" is a thing to argue with and "it is one of 1, 2, 4, 8, 16" is a thing to type.
///
/// It is read as a float and then required to be whole, rather than being a `u32` and letting
/// serde refuse the type: every other number in `data.rb` is written with a decimal point, so
/// `items_per_tile: 2.0` is what a player's hand writes, and `2.5` deserves this sentence rather
/// than "invalid type: floating point".
fn fits_a_tile<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let asked = f32::deserialize(d)?;
    let whole = asked as u32; // saturating, so a negative or a huge number lands off the list
    if asked.is_finite() && whole as f32 == asked && whole >= 1 && crate::TILE_PX.is_multiple_of(whole) {
        return Ok(whole);
    }
    let fits: Vec<String> =
        (1..=crate::TILE_PX).filter(|n| crate::TILE_PX.is_multiple_of(*n)).map(|n| n.to_string()).collect();
    Err(D::Error::custom(format!(
        "an item's place on a belt is one of the {} steps a tile is long, so items_per_tile is one of {} — not {asked}",
        crate::TILE_PX,
        fits.join(", ")
    )))
}

/// `{ iron_ore: 1 }` — and none of the counts is zero, because a recipe that consumes none of
/// something does not mention it.
fn counts<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, u32>, D::Error> {
    let map = BTreeMap::<String, u32>::deserialize(d)?;
    match map.iter().find(|(_, n)| **n == 0) {
        Some((name, _)) => Err(D::Error::custom(format!("{name}: 0 of something is not an amount"))),
        None => Ok(map),
    }
}

/// `[2, 2]` — and a machine covers at least one tile each way, because a machine no tiles wide
/// is on no tile at all.
fn a_size<'de, D: Deserializer<'de>>(d: D) -> Result<[u32; 2], D::Error> {
    let size = <[u32; 2]>::deserialize(d)?;
    if size[0] >= 1 && size[1] >= 1 {
        Ok(size)
    } else {
        Err(D::Error::custom(format!(
            "a machine covers at least one tile each way, not {} by {}",
            size[0], size[1]
        )))
    }
}

/// The same shape for something counted in both directions: `patches: [2, 2]`. A map with no
/// patches of ore across it is a map with no ore on it.
fn at_least_one_each_way<'de, D: Deserializer<'de>>(d: D) -> Result<[u32; 2], D::Error> {
    let n = <[u32; 2]>::deserialize(d)?;
    if n[0] >= 1 && n[1] >= 1 {
        Ok(n)
    } else {
        Err(D::Error::custom(format!("this counts things, so it is at least 1 each way, not {} by {}", n[0], n[1])))
    }
}

// ---------------------------------------------------------------------------------------------
// What the game reads afterwards
// ---------------------------------------------------------------------------------------------

/// One kind of thing that can be on a belt, in a chest or in a machine.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub icon: u32,
}

/// One recipe, with the names turned into numbers: a machine looks its inputs up every time it
/// checks whether it can start, and a name would be a string comparison in that loop.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub inputs: Vec<(ItemId, u32)>,
    pub outputs: Vec<(ItemId, u32)>,
    /// Seconds for one craft in a machine of speed 1.
    pub time: f32,
    pub made_in: MachineId,
}

/// One kind of machine, and the recipes that are made in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    pub name: String,
    /// Tiles across and up. The footprint does **not** turn with the machine: see
    /// [`Data::footprint`].
    pub size: UVec2,
    pub sprite: Vec<u16>,
    pub speed: f32,
    /// Which recipes name this machine in `made_in:`, in the order they were declared.
    pub recipes: Vec<RecipeId>,
}

impl Machine {
    pub fn tiles(&self) -> u32 {
        self.size.x * self.size.y
    }
}

/// **The tables, as one resource.** It exists from the data stage's `Startup` system and never
/// changes afterwards in F2; F5's editor is what will replace it, which is why
/// [`read_the_declarations`] is a function and not a one-shot.
#[derive(Resource, Debug)]
pub struct Data {
    pub items: Vec<Item>,
    pub recipes: Vec<Recipe>,
    pub machines: Vec<Machine>,
    by_item: BTreeMap<String, ItemId>,
    by_machine: BTreeMap<String, MachineId>,
}

impl Data {
    /// **Looking a name up.** Nothing in F2's own game code does — the game holds numbers, and
    /// the names are turned into numbers once, here — so these two are used by the tests and by
    /// the stages that will let a script name a thing: F3's inserters (`wants?(:stone)`) and F4's
    /// goal (`deliver: { science: 10 }`). They are kept rather than deleted and written again
    /// because the index they read is what `tables_of` builds anyway.
    #[allow(dead_code)]
    pub fn item(&self, name: &str) -> Option<ItemId> {
        self.by_item.get(name).copied()
    }

    /// See [`Data::item`].
    #[allow(dead_code)]
    pub fn machine(&self, name: &str) -> Option<MachineId> {
        self.by_machine.get(name).copied()
    }

    pub fn item_name(&self, id: ItemId) -> &str {
        self.items.get(id as usize).map(|i| i.name.as_str()).unwrap_or("?")
    }

    /// **Which tiles a machine built at `origin` covers.** The footprint is in the map's own axes
    /// and does not turn with the machine's direction: the pack's machines are drawn one tile
    /// each and a machine of several tiles is drawn out of several pictures, so turning the
    /// footprint would need a second set of pictures for the turned shape.
    ///
    /// **Since F3 a machine's direction says nothing at all.** It used to say where the machine
    /// pushed what it made; nothing comes out of a machine now but through an inserter's hand,
    /// and an inserter reaches into whichever tile of the footprint it is standing behind. That
    /// is Factorio's shape as well — an assembler there has no direction either — and it is why
    /// `Data::output_of` is gone.
    pub fn footprint(&self, machine: MachineId, origin: UVec2) -> Vec<UVec2> {
        let Some(m) = self.machines.get(machine as usize) else { return Vec::new() };
        let mut tiles = Vec::with_capacity(m.tiles() as usize);
        for dy in 0..m.size.y {
            for dx in 0..m.size.x {
                tiles.push(origin + UVec2::new(dx, dy));
            }
        }
        tiles
    }

}

// ---------------------------------------------------------------------------------------------
// What went wrong, and where
// ---------------------------------------------------------------------------------------------

/// **A data file that will not do**, and the line of it that says so.
///
/// `at` is a line of the author's own file — never of anything the compiler happened to call the
/// file, which in a browser is `playground.rb` for every program the page compiles
/// (`sabiruby-playground/wasm/src/lib.rs:21`). `None` is for the things that have no line at all,
/// which is a file that could not be read.
#[derive(Debug, Clone, PartialEq)]
pub struct Trouble {
    pub at: Option<u32>,
    pub what: String,
}

impl Trouble {
    /// `data.rb:4: unknown field `colour`, expected `icon`` — the one sentence the log, the
    /// screen and the checks all print.
    pub fn say(&self, file: &str) -> String {
        match self.at {
            Some(line) => format!("{file}:{line}: {}", self.what),
            None => format!("{file}: {}", self.what),
        }
    }

    /// What the VM raised while the script was running: serde's refusal of a declaration, a name
    /// declared twice, or anything else the script did.
    ///
    /// The message is `Vm::describe_error` and the line is the first frame of the exception's
    /// backtrace, which is the only way to reach it from Rust — there is no accessor on
    /// [`VmError`] — and the shape of a frame is `file:line` (`Vm::backtrace_text`).
    fn from_raise(vm: &mut Vm, e: &VmError) -> Trouble {
        let what = vm.describe_error(e);
        let at = match e {
            VmError::Raise(exc) => first_frame_line(vm, *exc),
            _ => None,
        };
        Trouble { at, what }
    }
}

/// The line named by the first frame of an exception's backtrace, or `None` if it has none — an
/// exception raised where the program was built without debug information has no frames at all.
fn first_frame_line(vm: &mut Vm, exc: Value) -> Option<u32> {
    let mid = vm.intern("backtrace");
    let frames = vm.funcall(exc, mid, &[], Value::Nil).ok()?;
    let first = vm.ary_vals(frames).and_then(|v| v.first().copied())?;
    let text = vm.str_bytes(first).map(|b| String::from_utf8_lossy(b).into_owned())?;
    line_in(&text)
}

/// **The first `:N` in a piece of text**, which is how both a compiler's `file:line:col: message`
/// and a backtrace's `file:line` name their line. Reading it this way rather than by splitting on
/// the file's name is what makes it not matter what the compiler called the file.
fn line_in(text: &str) -> Option<u32> {
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] != b':' {
            continue;
        }
        let end = i + 1 + bytes[i + 1..].iter().take_while(|b| b.is_ascii_digit()).count();
        if end > i + 1 {
            return text[i + 1..end].parse().ok();
        }
    }
    None
}

/// The same, for a compiler's `FILE:LINE:COL: message`: the line, and where the sentence after
/// the place starts. A line number with no column after it is a backtrace frame and not a
/// diagnostic, so there is nothing to cut.
fn place_in(text: &str) -> (Option<u32>, Option<usize>) {
    let bytes = text.as_bytes();
    let digits = |at: usize| at + bytes[at..].iter().take_while(|b| b.is_ascii_digit()).count();
    for i in 0..bytes.len() {
        if bytes[i] != b':' {
            continue;
        }
        let after_line = digits(i + 1);
        if after_line == i + 1 {
            continue;
        }
        let line = text[i + 1..after_line].parse().ok();
        if bytes.get(after_line) != Some(&b':') {
            return (line, None);
        }
        let after_col = digits(after_line + 1);
        if after_col == after_line + 1 || bytes.get(after_col) != Some(&b':') {
            return (line, None);
        }
        return (line, Some(after_col + 1));
    }
    (None, None)
}

/// A compiler's message, as a [`Trouble`]: the line out of it, and the sentence after the place.
///
/// **A browser's compiler is the page's, and it names every program `playground.rb`** — it is
/// handed a source and nothing else (`games_shell::platform::compile` on wasm, and
/// `sabiruby-playground/wasm/src/lib.rs:21`). The line is right and the name is not, so the name
/// is thrown away here and put back by [`Trouble::say`], and a player is told about the file they
/// were editing in both builds.
fn from_compiler(message: &str) -> Trouble {
    let first = message.lines().next().unwrap_or(message);
    let (at, after) = place_in(first);
    // **not "up to the third colon"**: the message this is handed is already
    // `data.rb: playground.rb:2:9: syntax error`, with the caller's own name on the front, so
    // counting colons cuts in the wrong place. What is cut is the `:LINE:COL:` that `place_in`
    // found, and it is found by its shape.
    let what = match after {
        Some(i) => first[i..].trim().to_string(),
        None => message.to_string(),
    };
    Trouble { at, what }
}

// ---------------------------------------------------------------------------------------------
// Reading one data file
// ---------------------------------------------------------------------------------------------

/// **The whole of the data stage**, as one function over a VM and the text of a data file.
///
/// It may be called as often as anybody likes, on the same VM: `install` makes a new table each
/// time and `define` writes a new method over the old one, so the tables a previous call took out
/// are not in the way. That is what lets F5 reload an edited file, and what lets F2's checks put
/// four broken files through the same door in a running game.
///
/// `compile` is passed in rather than called because it is the one thing that differs between a
/// PC and a page (`crate::platform::compile`), and because a test wants the reference compiler
/// without a `platform` under it.
pub fn read_the_declarations(
    vm: &mut Vm,
    name: &str,
    source: &str,
    compile: impl Fn(&str, &str) -> Result<Vec<u8>, String>,
) -> Result<(Data, Rules), Trouble> {
    let bytes = compile(source, name).map_err(|e| from_compiler(&e))?;

    // `source` is not read past this line any more. F2 kept it to the end so that a check between
    // two declarations could find the line one was written on; `take_with_lines` carries it now.
    let items = Declarations::<ItemDecl>::install(vm).define(vm, "item");
    let recipes = Declarations::<RecipeDecl>::install(vm).define(vm, "recipe");
    let machines = Declarations::<MachineDecl>::install(vm).define(vm, "machine");
    let belts = Declarations::<BeltDecl>::install(vm).define(vm, "belt");
    let miners = Declarations::<MinerDecl>::install(vm).define(vm, "miner");
    let chests = Declarations::<ChestDecl>::install(vm).define(vm, "chest");
    let ores = Declarations::<OreDecl>::install(vm).define(vm, "ore");
    let arms = Declarations::<InserterDecl>::install(vm).define(vm, "inserter");
    let maps = Declarations::<MapDecl>::install(vm).define(vm, "map");

    let ran = vm.load_and_run(&bytes);

    // **Taken whether the run went well or not.** A file that raised half way through has already
    // put its first declarations in the tables, and leaving them in the VM would mean the next
    // call to this function shares a table with a file that failed.
    let items = items.take_with_lines(vm);
    let recipes = recipes.take_with_lines(vm);
    let machines = machines.take_with_lines(vm);
    let belts = belts.take_with_lines(vm);
    let miners = miners.take_with_lines(vm);
    let chests = chests.take_with_lines(vm);
    let ores = ores.take_with_lines(vm);
    let arms = arms.take_with_lines(vm);
    let maps = maps.take_with_lines(vm);

    if let Err(e) = ran {
        return Err(Trouble::from_raise(vm, &e));
    }

    let tables = tables_of(items, recipes, machines, belts, miners, chests, ores, arms, maps)?;
    Ok(tables)
}

/// `a` or `an`, so that a sentence about a word the file might have written reads like a sentence.
/// Four of the words are consonants and `ore` is not.
fn article(word: &str) -> &'static str {
    match word.chars().next() {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// One of a word that has to be declared exactly once — and its name and line, for whatever has
/// to be said about it afterwards.
fn exactly_one<T>(word: &str, mut all: Vec<Declared<T>>) -> Result<Declared<T>, Trouble> {
    match all.len() {
        1 => Ok(all.remove(0)),
        0 => Err(Trouble { at: None, what: format!("nothing declares {} {word}", article(word)) }),
        n => {
            // the second one is the one that is too many, and it is the one to point at
            Err(Trouble {
                at: all[1].line,
                what: format!("{n} {word}s are declared and the game has room for one"),
            })
        }
    }
}

/// **How big the map may be, and what says so** (F3a) — the one check in this file that reads two
/// declarations and the only one whose numbers come from outside the data file altogether.
///
/// The author asked for the size to be free, so **nothing here is a size somebody preferred**.
/// Two things refuse a map, and each is something that actually breaks:
///
/// * **too small for its own ore.** A patch that reaches the border ring is ore nobody can see
///   or stand a miner on, so the floor is [`crate::grid::Ore::smallest_map`] — and it is asked of each side
///   separately, because a map 96 by 16 is two rows of patches on a wide floor and neither number
///   says anything about the other. The floor depends on the radius and the count, which is why
///   the sentence names all three numbers: the size is one of three ways to fix it.
/// * **too big for the picture.** [`crate::draw::MOST_TILES_ACROSS`] is the tile data texture's
///   own limit, which is the drawing's and not this game's.
///
/// **Neither is clamped.** A map quietly made bigger than the file said is a file that lies about
/// the world, and the whole of F3a is that the file says. The refusal lands on the `map` line,
/// with the number that would do.
fn a_map_that_can_be_played_on(
    world: &MapDecl,
    ore: &OreDecl,
    line: Option<u32>,
) -> Result<UVec2, Trouble> {
    let asked = UVec2::new(world.size[0], world.size[1]);
    let patches = UVec2::new(ore.patches[0], ore.patches[1]);
    let most = crate::draw::MOST_TILES_ACROSS;
    for (side, tiles, count, way) in [
        ("across", asked.x, patches.x.max(1), "wide"),
        ("up", asked.y, patches.y.max(1), "tall"),
    ] {
        let least = crate::grid::Ore::smallest_map(ore.patch_radius, count);
        if tiles < least {
            return Err(Trouble {
                at: line,
                what: format!(
                    "a map {tiles} tiles {side} has no room for {count} patches of ore of radius {} without them touching the wall: make it {least} tiles {side}, or ask for fewer patches, or a smaller patch_radius",
                    ore.patch_radius
                ),
            });
        }
        if tiles > most {
            return Err(Trouble {
                at: line,
                what: format!(
                    "a map {tiles} tiles {side} cannot be drawn: the floor is one texture of one texel a tile and {most} is as {way} as one goes"
                ),
            });
        }
    }
    Ok(asked)
}

/// The names turned into numbers, and every reference checked.
#[allow(clippy::too_many_arguments)]
fn tables_of(
    items: Vec<Declared<ItemDecl>>,
    recipes: Vec<Declared<RecipeDecl>>,
    machines: Vec<Declared<MachineDecl>>,
    belts: Vec<Declared<BeltDecl>>,
    miners: Vec<Declared<MinerDecl>>,
    chests: Vec<Declared<ChestDecl>>,
    ores: Vec<Declared<OreDecl>>,
    arms: Vec<Declared<InserterDecl>>,
    maps: Vec<Declared<MapDecl>>,
) -> Result<(Data, Rules), Trouble> {
    if items.is_empty() {
        return Err(Trouble { at: None, what: "nothing declares an item".into() });
    }
    let mut by_item = BTreeMap::new();
    let mut made_items = Vec::new();
    // `Declared` is `#[non_exhaustive]`, so a pattern that takes it apart says `..`: a field the
    // VM's crate adds later is not a change here.
    for (i, Declared { name, value: decl, .. }) in items.into_iter().enumerate() {
        by_item.insert(name.clone(), i as ItemId);
        made_items.push(Item { name, icon: decl.icon });
    }

    let mut by_machine = BTreeMap::new();
    let mut made_machines = Vec::new();
    for (i, Declared { name, value: decl, line, .. }) in machines.into_iter().enumerate() {
        // the one thing about a machine that two of its fields have to agree on, which is why it
        // is here and not in a `deserialize_with`
        let wanted = decl.size[0] * decl.size[1];
        if decl.sprite.len() as u32 != wanted {
            return Err(Trouble {
                at: line,
                what: format!(
                    "{name} covers {} by {} tiles, which is {wanted} pictures, and it gives {}",
                    decl.size[0],
                    decl.size[1],
                    decl.sprite.len()
                ),
            });
        }
        by_machine.insert(name.clone(), i as MachineId);
        made_machines.push(Machine {
            name,
            size: UVec2::new(decl.size[0], decl.size[1]),
            sprite: decl.sprite,
            speed: decl.speed,
            recipes: Vec::new(),
        });
    }

    let mut made_recipes: Vec<Recipe> = Vec::new();
    for Declared { name, value: decl, line, .. } in recipes {
        let amounts = |what: &str, from: BTreeMap<String, u32>| -> Result<Vec<(ItemId, u32)>, Trouble> {
            from.into_iter()
                .map(|(item, n)| match by_item.get(&item) {
                    Some(&id) => Ok((id, n)),
                    None => Err(Trouble {
                        at: line,
                        what: format!("{name} has {item} {what} and nothing declares an item called that"),
                    }),
                })
                .collect()
        };
        let inputs = amounts("in it", decl.inputs)?;
        let outputs = amounts("out of it", decl.out)?;
        if outputs.is_empty() {
            return Err(Trouble { at: line, what: format!("{name} makes nothing") });
        }
        let Some(&made_in) = by_machine.get(&decl.made_in) else {
            return Err(Trouble {
                at: line,
                what: format!(
                    "{name} is made in {} and nothing declares a machine called that",
                    decl.made_in
                ),
            });
        };
        made_machines[made_in as usize].recipes.push(made_recipes.len() as RecipeId);
        made_recipes.push(Recipe { name, inputs, outputs, time: decl.time, made_in });
    }

    let belt = exactly_one("belt", belts)?.value;
    let miner = exactly_one("miner", miners)?.value;
    let chest = exactly_one("chest", chests)?.value;
    let arm = exactly_one("inserter", arms)?.value;
    // **the ground is named after what comes out of it**, so this is the one reference between
    // declarations the ore has — and, since there is one ground, it is what a miner digs
    let Declared { name: ore_name, value: ore, line: ore_line, .. } = exactly_one("ore", ores)?;
    let Some(&digs) = by_item.get(&ore_name) else {
        return Err(Trouble {
            at: ore_line,
            what: format!("the ground is {ore_name} and nothing declares an item called that"),
        });
    };

    let Declared { value: world, line: map_line, .. } = exactly_one("map", maps)?;
    let map_tiles = a_map_that_can_be_played_on(&world, &ore, map_line)?;

    let rules = Rules {
        belt_tiles_per_second: belt.tiles_per_second,
        items_per_tile: belt.items_per_tile,
        mine_seconds: miner.seconds_per_item,
        chest_capacity: chest.capacity,
        digs,
        swing_seconds: arm.seconds_per_item,
        ore_per_tile: ore.per_tile,
        ore_patch_radius: ore.patch_radius,
        ore_patches: UVec2::new(ore.patches[0], ore.patches[1]),
        map_tiles,
    };
    let data = Data {
        items: made_items,
        recipes: made_recipes,
        machines: made_machines,
        by_item,
        by_machine,
    };
    Ok((data, rules))
}

// ---------------------------------------------------------------------------------------------
// Reading the tables back from Ruby
// ---------------------------------------------------------------------------------------------

/// What `item_of(:gear)` answers.
#[derive(Serialize)]
struct ItemSeen {
    icon: u32,
}

/// What `recipe_of(:iron_plate)` answers.
#[derive(Serialize)]
struct RecipeSeen {
    #[serde(rename = "in")]
    inputs: BTreeMap<String, u32>,
    out: BTreeMap<String, u32>,
    time: f32,
    made_in: String,
}

/// What `machine_of(:furnace)` answers.
#[derive(Serialize)]
struct MachineSeen {
    size: [u32; 2],
    speed: f32,
    makes: Vec<String>,
}

/// **The other direction**: three methods a script calls to read the tables back
/// (`sabiruby_serde::declare::expose`).
///
/// The plan writes them `recipes[:iron_plate]` and `items[:gear]`; what `expose` defines is a
/// **method**, so they are `recipe_of(:iron_plate)` and `item_of(:gear)` here, and a prelude that
/// wants the bracket shape can wrap them in three lines when a stage has a use for it (F3's
/// inserters read them through [`crate::inserters`]'s prelude).
///
/// **The names inside an answer are Symbols, all the way down** —
/// `recipe_of(:iron_plate)[:in][:iron_ore]`. F2 met the other answer: a map's keys came back as
/// Strings while a struct's fields were Symbols, so a name a data file had written as `:iron_ore`
/// read back as `"iron_ore"`. That was reported as a gap in the VM's crate and is what
/// `Options::symbols` — which `expose` now uses — fixed (sabiruby
/// `docs/worklog/2026-09-21-serde-lines.md`). It matters here because the prelude an inserter is
/// written in reads these tables, and the spelling a player types is the spelling they wrote.
pub fn expose_the_tables(vm: &mut Vm, data: &Data) {
    let items = data
        .items
        .iter()
        .map(|i| (i.name.clone(), ItemSeen { icon: i.icon }))
        .collect::<Vec<_>>();
    expose(vm, "item_of", items);

    let names = |list: &[(ItemId, u32)]| -> BTreeMap<String, u32> {
        list.iter().map(|&(id, n)| (data.item_name(id).to_string(), n)).collect()
    };
    let recipes = data
        .recipes
        .iter()
        .map(|r| {
            (
                r.name.clone(),
                RecipeSeen {
                    inputs: names(&r.inputs),
                    out: names(&r.outputs),
                    time: r.time,
                    made_in: data.machines[r.made_in as usize].name.clone(),
                },
            )
        })
        .collect::<Vec<_>>();
    expose(vm, "recipe_of", recipes);

    let machines = data
        .machines
        .iter()
        .map(|m| {
            (
                m.name.clone(),
                MachineSeen {
                    size: [m.size.x, m.size.y],
                    speed: m.speed,
                    makes: m
                        .recipes
                        .iter()
                        .map(|&r| data.recipes[r as usize].name.clone())
                        .collect(),
                },
            )
        })
        .collect::<Vec<_>>();
    expose(vm, "machine_of", machines);
}

/// **The tables a test of the factory runs on**, read from a small data file through the same
/// door the game's own file goes through.
///
/// A `Data` built by hand in a test module would be a second way of making one, and the second
/// way is the one that goes on being right after the first has changed. The compiler is
/// `crate::platform`'s, which on a PC — where tests run — is the one linked in.
#[cfg(test)]
pub fn for_a_test(source: &str) -> (Data, Rules) {
    let mut vm = Vm::with_mrblib().expect("a vm");
    match read_the_declarations(&mut vm, "data.rb", source, crate::platform::compile) {
        Ok(tables) => tables,
        Err(trouble) => panic!("the test's own data file: {}", trouble.say("data.rb")),
    }
}

// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The compiler a PC build has, reached the way the game reaches it.
    fn compile(source: &str, name: &str) -> Result<Vec<u8>, String> {
        crate::platform::compile(source, name)
    }

    fn read(source: &str) -> Result<(Data, Rules), Trouble> {
        let mut vm = Vm::with_mrblib().expect("a vm");
        read_the_declarations(&mut vm, "data.rb", source, compile)
    }

    /// A file that is right, so that the wrong ones below differ from it in one thing each.
    const GOOD: &str = concat!(
        "item :iron_ore, icon: 0\n",                                                  // 1
        "item :iron_plate, icon: 1\n",                                                // 2
        "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",                // 3
        "recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 },\n",         // 4
        "       time: 2.0, made_in: :furnace\n",                                      // 5
        "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n",                 // 6
        "miner :drill, seconds_per_item: 1.0\n",                                      // 7
        "chest :crate, capacity: 60\n",                                               // 8
        "ore :iron_ore, per_tile: 60, patch_radius: 3.0, patches: [2, 2]\n",          // 9
        "inserter :arm, seconds_per_item: 1.0\n",                                     // 10
        "map :world, size: [32, 32]\n",                                               // 11
    );

    #[test]
    fn a_data_file_becomes_the_tables_the_game_runs_on() {
        let (data, rules) = read(GOOD).expect("a good file");
        assert_eq!(data.items.len(), 2);
        assert_eq!(data.item("iron_plate"), Some(1));
        assert_eq!(data.items[0].icon, 0);
        assert_eq!(data.recipes.len(), 1);
        assert_eq!(data.recipes[0].inputs, vec![(0, 1)]);
        assert_eq!(data.recipes[0].outputs, vec![(1, 1)]);
        assert_eq!(data.recipes[0].time, 2.0);
        assert_eq!(data.machines[0].recipes, vec![0], "the machine knows what is made in it");
        // and the four numbers of play that used to be settings
        assert_eq!(rules.belt_tiles_per_second, 2.0);
        assert_eq!(rules.items_per_tile, 2);
        assert_eq!(rules.mine_seconds, 1.0);
        assert_eq!(rules.chest_capacity, 60);
        assert_eq!(rules.digs, 0, "what the ground is made of is what a miner brings up");
        assert_eq!(rules.swing_seconds, 1.0, "and an inserter takes a second over one item");
    }

    /// **Each kind of mistake, and the line it is on.** The four the plan names, and the two
    /// `Rules` asked F2 for in its rustdoc (a number that is not more than zero).
    ///
    /// **Since F3, the two kinds of mistake give the same number for the same declaration.** The
    /// recipe in [`GOOD`] is written over lines 4 and 5 on purpose, and both roads — serde
    /// refusing a field inside it, and this game refusing a name between declarations — say 5,
    /// the line it ends on. F2's look-up in the source text said 4 for the second road.
    #[test]
    fn a_mistake_in_a_declaration_says_which_line_it_is_on() {
        let wrong = |line: &str, replacing: &str| GOOD.replace(replacing, line);
        let cases: Vec<(String, u32, &str)> = vec![
            // an unknown field: serde's own message, at the declaration
            (wrong("item :iron_ore, icon: 0, colour: \"red\"\n", "item :iron_ore, icon: 0\n"), 1, "unknown field"),
            // a recipe that names an item nothing declares — the one that needs two declarations,
            // and so the one whose line used to come from a look-up rather than from the VM
            (wrong("recipe :iron_plate, in: { coal: 1 }, out: { iron_plate: 1 },\n", "recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 },\n"), 5, "coal"),
            // a time that is not more than zero
            (wrong("       time: 0.0, made_in: :furnace\n", "       time: 2.0, made_in: :furnace\n"), 5, "not more than zero"),
            // a machine no tiles wide
            (wrong("machine :furnace, size: [0, 1], sprite: [109], speed: 1.0\n", "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n"), 3, "at least one tile"),
            // and a belt that does not move, which is the contract `Rules` states
            (wrong("belt :conveyor, tiles_per_second: 0.0, items_per_tile: 2\n", "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n"), 6, "not more than zero"),
        ];
        for (source, line, says) in cases {
            let trouble = read(&source).expect_err("this file is wrong");
            assert_eq!(trouble.at, Some(line), "{}", trouble.say("data.rb"));
            assert!(
                trouble.what.contains(says),
                "expected {says:?} in {:?}",
                trouble.say("data.rb")
            );
            assert!(trouble.say("data.rb").starts_with(&format!("data.rb:{line}: ")));
        }
    }

    /// **The gaps a belt may have, and the sentence the rest are refused with** (F2a).
    ///
    /// An item's place on a belt is a whole number of the `TILE_PX` steps a tile is long, so a gap
    /// of `TILE_PX ÷ items_per_tile` is only a gap when the division comes out: 1, 2, 4, 8 and 16
    /// are what a file may write. The three refused here are the three kinds of wrong — a whole
    /// number that does not divide (3), one that is not whole (2.5), and one that is not a count
    /// at all (0) — and what is asserted about the message is that **it names what can be
    /// written**, because that is what the player needs and "3 will not do" is not it.
    ///
    /// The other half of this is in `belts.rs`, where a jam at each of the five is counted.
    #[test]
    fn the_gap_between_items_has_to_divide_a_tile() {
        for fits in [1u32, 2, 4, 8, 16] {
            let source = GOOD.replace("items_per_tile: 2", &format!("items_per_tile: {fits}"));
            let (_, rules) = read(&source).expect("a gap that divides a tile");
            assert_eq!(rules.items_per_tile, fits);
            assert!(crate::TILE_PX.is_multiple_of(rules.items_per_tile), "and the gap comes out whole");
        }
        // and the same number written the way the rest of the file writes its numbers
        let (_, rules) = read(&GOOD.replace("items_per_tile: 2", "items_per_tile: 2.0")).expect("2.0");
        assert_eq!(rules.items_per_tile, 2, "a player who writes it as a float means the same thing");

        for asked in ["3", "5", "2.5", "0"] {
            let source = GOOD.replace("items_per_tile: 2", &format!("items_per_tile: {asked}"));
            let trouble = read(&source).expect_err("{asked} is not a gap");
            assert_eq!(trouble.at, Some(6), "{asked}: {}", trouble.say("data.rb"));
            assert!(
                trouble.what.contains("1, 2, 4, 8, 16"),
                "{asked}: the refusal has to say what can be written — {:?}",
                trouble.what
            );
        }
    }

    /// A declaration spread over two lines is reported at its **last** one, which is the line the
    /// `SEND` instruction carries (sabiruby `serde/tests/declare.rs`). The recipe in [`GOOD`] is
    /// written over two lines on purpose so that this is a fact this game has measured and not
    /// one it has read.
    #[test]
    fn a_declaration_over_two_lines_is_reported_at_the_second() {
        let source = GOOD.replace("       time: 2.0, made_in: :furnace\n", "       time: 2.0, made_in: :furnace, colour: :red\n");
        let trouble = read(&source).expect_err("an unknown field");
        assert_eq!(trouble.at, Some(5), "{}", trouble.say("data.rb"));
    }

    /// The machine's two fields that have to agree, and the reference that has to exist.
    #[test]
    fn a_machine_gives_one_picture_for_each_tile_it_covers() {
        let source = GOOD.replace("size: [1, 1], sprite: [109]", "size: [2, 2], sprite: [109]");
        let trouble = read(&source).expect_err("four tiles, one picture");
        assert_eq!(trouble.at, Some(3), "{}", trouble.say("data.rb"));
        assert!(trouble.what.contains("4 pictures"), "{}", trouble.what);

        let source = GOOD.replace("made_in: :furnace", "made_in: :smelter");
        let trouble = read(&source).expect_err("no such machine");
        // **The line a declaration carries and the line serde raises at are the same number**,
        // which is what F3's bump of the VM bought: both are the line the declaration *ends* on,
        // the one the `SEND` instruction holds. The recipe in `GOOD` runs over lines 4 and 5, so
        // this says 5 — and so does
        // [`a_declaration_over_two_lines_is_reported_at_the_second`], which is the same recipe
        // refused by serde instead. F2 said 4 here, because it looked the declaration up in the
        // source text and found the line it *starts* on.
        assert_eq!(trouble.at, Some(5), "{}", trouble.say("data.rb"));
        assert!(trouble.what.contains("smelter"), "{}", trouble.what);
    }

    /// A name declared twice is `declare`'s own refusal, and it lands on the second one.
    #[test]
    fn a_name_declared_twice_is_refused_at_the_second_one() {
        let source = format!("{GOOD}item :iron_ore, icon: 7\n");
        let trouble = read(&source).expect_err("declared twice");
        assert_eq!(trouble.at, Some(12), "{}", trouble.say("data.rb"));
        assert!(trouble.what.contains("already declared"), "{}", trouble.what);
    }

    /// Ruby that will not compile is a `data.rb:LINE` too, and the compiler's own sentence.
    #[test]
    fn ruby_that_will_not_compile_says_which_line() {
        let trouble = read("item :iron_ore, icon:\n").expect_err("not Ruby");
        assert_eq!(trouble.at, Some(1), "{}", trouble.say("data.rb"));
        assert!(trouble.say("data.rb").starts_with("data.rb:1: "), "{}", trouble.say("data.rb"));
    }

    /// **The five fittings and the map are declared exactly once each**, which is the shape the
    /// world has room for. `ore` is F2a's, `inserter` is F3's and `map` is F3a's, and each is one
    /// of them for the same reason the first three are: a thing of the world with a number of its
    /// own, and the game has one of it.
    #[test]
    fn the_world_has_one_belt_one_miner_one_chest_one_ore_one_inserter_and_one_map() {
        let none = GOOD.replace("chest :crate, capacity: 60\n", "");
        assert_eq!(read(&none).expect_err("no chest").what, "nothing declares a chest");
        let none = GOOD.replace("ore :iron_ore, per_tile: 60, patch_radius: 3.0, patches: [2, 2]\n", "");
        assert_eq!(read(&none).expect_err("no ore").what, "nothing declares an ore");
        let none = GOOD.replace("map :world, size: [32, 32]\n", "");
        assert_eq!(read(&none).expect_err("no map").what, "nothing declares a map");
        let two = format!("{GOOD}map :another, size: [40, 40]\n");
        let trouble = read(&two).expect_err("two maps");
        assert_eq!(trouble.at, Some(12), "{}", trouble.say("data.rb"));
        let none = GOOD.replace("inserter :arm, seconds_per_item: 1.0\n", "");
        assert_eq!(read(&none).expect_err("no arm").what, "nothing declares an inserter");
        let two = format!("{GOOD}belt :fast, tiles_per_second: 8.0, items_per_tile: 2\n");
        let trouble = read(&two).expect_err("two belts");
        assert_eq!(trouble.at, Some(12), "{}", trouble.say("data.rb"));
    }

    /// **How big the map may be, and what says so** (F3a).
    ///
    /// The author asked for the size to be free, so what is asserted here is that **the two
    /// refusals are the only two** and that each lands exactly where the thing it is about
    /// breaks: one tile inside each is a map the game starts on, one tile outside is a sentence
    /// with the line on it. The floor is the ore's (a patch on the border ring is ore nobody can
    /// see) and the ceiling is the picture's (the floor is one texture of one texel a tile).
    #[test]
    fn a_map_is_refused_when_its_ore_or_its_picture_says_so_and_not_before() {
        let sized = |w: u32, h: u32| GOOD.replace("size: [32, 32]", &format!("size: [{w}, {h}]"));
        // the floor, exactly: fifteen tiles at radius 3 with two patches each way (`Ore`)
        let least = crate::grid::Ore::smallest_map(3.0, 2);
        assert_eq!(least, 15);
        let (_, rules) = read(&sized(least, least)).expect("the smallest map there is");
        assert_eq!(rules.map_tiles, UVec2::splat(least));
        // …and one tile under it, each way separately, because the two sides are two questions
        for (w, h) in [(least - 1, least), (least, least - 1)] {
            let trouble = read(&sized(w, h)).expect_err("too small for its own ore");
            assert_eq!(trouble.at, Some(11), "{w} by {h}: {}", trouble.say("data.rb"));
            assert!(
                trouble.what.contains(&least.to_string()) && trouble.what.contains("patch"),
                "the refusal has to say what would do, and which numbers make it: {:?}",
                trouble.what
            );
        }
        // **the floor moves with the ore, which is the whole reason it is not a number**: more
        // patches, or wider ones, and the same map is too small
        let more = sized(least, least).replace("patches: [2, 2]", "patches: [3, 2]");
        let trouble = read(&more).expect_err("three patches want more room");
        assert_eq!(trouble.at, Some(11), "{}", trouble.say("data.rb"));
        let wider = sized(least, least).replace("patch_radius: 3.0", "patch_radius: 4.0");
        assert!(read(&wider).is_err(), "a wider patch wants a bigger map");
        // and a smaller one wants less: the floor is a formula and not a constant
        let (_, rules) = read(
            &sized(least, least)
                .replace("patch_radius: 3.0", "patch_radius: 1.0")
                .replace("patches: [2, 2]", "patches: [1, 1]"),
        )
        .expect("one small patch fits easily");
        assert_eq!(rules.map_tiles, UVec2::splat(least));

        // the ceiling, either side of it
        let most = crate::draw::MOST_TILES_ACROSS;
        let (_, rules) = read(&sized(most, 32)).expect("as wide as a texture goes");
        assert_eq!(rules.map_tiles, UVec2::new(most, 32));
        for (w, h) in [(most + 1, 32), (32, most + 1)] {
            let trouble = read(&sized(w, h)).expect_err("too big to draw");
            assert_eq!(trouble.at, Some(11), "{w} by {h}: {}", trouble.say("data.rb"));
            assert!(
                trouble.what.contains(&most.to_string()) && trouble.what.contains("drawn"),
                "the refusal has to say what breaks and where it stops: {:?}",
                trouble.what
            );
        }

        // **and in between, the two sides are free and need not agree** — which is what F3a is
        for (w, h) in [(96u32, 16u32), (15, 512), (33, 32)] {
            let (_, rules) = read(&sized(w, h)).expect("a map between the two");
            assert_eq!(rules.map_tiles, UVec2::new(w, h), "{w} by {h}");
        }
    }

    /// **The ground is named after what comes out of it** (the author, 2026-09-21), and both
    /// ways of getting that wrong are refused at their own line.
    ///
    /// `ore :coal` with no `item :coal` is a reference between two declarations, so the line is
    /// the one `take_with_lines` carried. `miner … digs:` is a field that no longer exists, so it
    /// is serde's own refusal and needs nothing written here at all — which is the whole reason
    /// each fitting has a word of its own (the head of this file).
    #[test]
    fn the_ground_is_named_after_what_comes_out_of_it() {
        let (_, rules) = read(GOOD).expect("a good file");
        assert_eq!(rules.digs, 0, "iron_ore, which is item 0");

        let unknown = GOOD.replace("ore :iron_ore,", "ore :coal,");
        let trouble = read(&unknown).expect_err("nothing declares coal");
        assert_eq!(trouble.at, Some(9), "{}", trouble.say("data.rb"));
        assert!(trouble.what.contains("coal"), "{}", trouble.what);

        let old_spelling =
            GOOD.replace("miner :drill, seconds_per_item: 1.0", "miner :drill, seconds_per_item: 1.0, digs: :iron_ore");
        let trouble = read(&old_spelling).expect_err("digs: is gone");
        assert_eq!(trouble.at, Some(7), "{}", trouble.say("data.rb"));
        assert!(trouble.what.contains("unknown field"), "{}", trouble.what);
    }

    /// **The same VM reads a second file.** This is what F5's reload will be and what the checks
    /// in a running game do: the tables a previous call took out are not in the way.
    #[test]
    fn a_vm_can_be_given_a_data_file_more_than_once() {
        let mut vm = Vm::with_mrblib().expect("a vm");
        let first = read_the_declarations(&mut vm, "data.rb", GOOD, compile).expect("the first");
        assert_eq!(first.0.items.len(), 2);
        let broken = GOOD.replace("time: 2.0", "time: -1.0");
        let second = read_the_declarations(&mut vm, "data.rb", &broken, compile);
        assert_eq!(second.expect_err("wrong").at, Some(5));
        // and a good one again after a bad one
        let third = read_the_declarations(&mut vm, "data.rb", GOOD, compile).expect("the third");
        assert_eq!(third.0.recipes.len(), 1);
    }

    /// **The tables read back from Ruby** (requirement (i) of the stage): what F3's inserters and
    /// F4's control stage will call.
    #[test]
    fn a_script_reads_the_tables_back() {
        let mut vm = Vm::with_mrblib().expect("a vm");
        let (data, _) = read_the_declarations(&mut vm, "data.rb", GOOD, compile).expect("good");
        expose_the_tables(&mut vm, &data);
        let script = concat!(
            "$icon  = item_of(:iron_plate)[:icon]\n",
            "$time  = recipe_of(:iron_plate)[:time]\n",
            "$where = recipe_of(:iron_plate)[:made_in]\n",
            // **a Symbol, as the data file wrote it**: the keys of a map inside an answer are
            // Symbols since `expose` took `Options::symbols` (sabiruby, 2026-09-21). F2 had to
            // write `["iron_ore"]` here, which is the spelling a player would not have guessed.
            "$ore   = recipe_of(:iron_plate)[:in][:iron_ore]\n",
            "$size  = machine_of(:furnace)[:size].inspect\n",
            "$none  = item_of(:gear)\n",
        );
        let bytes = compile(script, "control.rb").expect("compiles");
        vm.load_and_run(&bytes).expect("runs");
        assert_eq!(vm.global_get("$icon"), Value::Int(1));
        assert_eq!(vm.global_get("$time"), Value::Float(2.0));
        assert_eq!(vm.global_get("$ore"), Value::Int(1));
        assert_eq!(vm.global_get("$none"), Value::Nil, "a name the table has not got");
        let says = |v: Value, vm: &mut Vm| {
            String::from_utf8_lossy(vm.str_bytes(v).expect("a String")).into_owned()
        };
        let where_ = vm.global_get("$where");
        assert_eq!(says(where_, &mut vm), "furnace");
        let size = vm.global_get("$size");
        assert_eq!(says(size, &mut vm), "[1, 1]");
    }

    /// The footprint of a machine bigger than one tile, which is the whole of what `size` means
    /// — and, since F3, the whole of what the grid needs to know about one: a machine has no
    /// output tile any more, because nothing comes out of it but through an inserter's hand.
    #[test]
    fn a_machine_of_four_tiles_covers_four_tiles() {
        let source = GOOD
            .replace(
                "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",
                "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\nmachine :big, size: [2, 2], sprite: [1, 2, 3, 4], speed: 1.0\n",
            );
        let (data, _) = read(&source).expect("a good file");
        let big = data.machine("big").expect("declared");
        let at = UVec2::new(10, 4);
        assert_eq!(
            data.footprint(big, at),
            vec![UVec2::new(10, 4), UVec2::new(11, 4), UVec2::new(10, 5), UVec2::new(11, 5)],
            "row by row from the bottom left"
        );
        // and a machine of one tile covers the tile it was built on and no other
        let furnace = data.machine("furnace").expect("declared");
        assert_eq!(data.footprint(furnace, at), vec![at]);
    }
}
