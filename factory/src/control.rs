//! **The control stage** — the one script that says what the factory is *for*.
//!
//! `data.rb` is what the world is made of (`crate::data`) and an inserter's script is what one arm
//! does (`crate::inserters`); this is the third of the three and the smallest: **one script, five
//! events and a goal**. It is Factorio's control stage with the same job and a tenth of the
//! surface — `script.on_event` and nothing that builds anything.
//!
//! # The five events, and why they are the size they are
//!
//! | name | what it is about |
//! |---|---|
//! | `built` / `removed` | something was put on the map or taken off it |
//! | `crafted` | machines finished crafts |
//! | `delivered` | things went into chests |
//! | `jammed` | a machine is holding what it made **and has the parts for another craft** |
//!
//! **A subscription holds sixty four messages** before the oldest is dropped
//! (`ScriptWorld::queue_limit`), and a script is woken once a frame, so sixty four is what may be
//! published between two looks. That is the whole of the granularity argument, and it is worked
//! backwards: one message a frame **per kind of thing**, with how many of it there were and where
//! the last of them was ([`Counted`]). How many kinds there are is what `ruby/data.rb` declares —
//! three items and two machines in the file this game ships — so the ceiling cannot be reached by
//! anything a data file can say. A message per delivered item would reach it at about sixty arms.
//!
//! **Nothing here is ever told about one item on one belt.** There are thousands of those and the
//! plan says so in as many words (§3.4); the belts are Rust and stay Rust.
//!
//! **A jam is a machine's and not a belt's.** A working factory has jammed belts in it all the
//! time — a belt in front of a machine slower than it *is* a jam — so publishing those would be
//! publishing that the factory is running. A machine that is holding what it made while it has
//! the parts for the next craft is a chain that has stopped, and it stopped because nobody is
//! taking its output away, which is an arm the player has not built (`crate::machines::craft`).
//!
//! # What the script says back
//!
//! **Nothing, in the sense of a request.** The game reads four things off the control object with
//! two `ivar_get`s each — whether it has won, the line it wants on the screen, how much of each
//! event it has heard, and how many messages its own subscriptions dropped — which is the
//! garden's way with a creature's `@asleep` and `mutation_rate`. So the control stage's only
//! effect on the game is `win!`, and that is a decision rather than an oversight: what a *goal*
//! is is a thing to say, not a thing to do, and a script that could build would need to say what
//! happens when it cannot — which is the editor's and the save's business (F5).
//!
//! # A control.rb that will not run
//!
//! The factory keeps running with no goal in it. Whatever is wrong is said with the line of the
//! player's own file it is on, exactly as a broken inserter's script is: a compiler's complaint
//! goes through `in_the_authors_lines`, and an exception's place comes with the ending rubevy
//! sends ([`crate::inserters::where_it_broke`]).

use std::collections::HashSet;

use bevy::prelude::*;
use rubevy::{
    in_the_authors_lines, replace_script, Answer, MrbAsset, Program, Script, ScriptEnded,
    ScriptStatus, ScriptTask, ScriptWorld,
};

use crate::belts::{Move, Moves, Rules};
use crate::data::{Data, ItemId};
use crate::grid::{Grid, What};
use crate::platform;

/// The player's file, and the name its troubles are reported under — in a browser the compiler
/// calls every program `playground.rb`.
pub const SCRIPT_FILE: &str = "control.rb";
/// The DSL it is written in. Its name only shows up when something goes wrong *in* it.
pub const PRELUDE_FILE: &str = "control_prelude.rb";

/// **What the game publishes to the control stage**, in the order the counters below are in.
///
/// The names are what `ruby/control_prelude.rb` subscribes to, spelled the same; they are plain
/// words rather than `factory.built` because a script writes them (`on(:built)`) and this game's
/// VM publishes nothing else.
pub const EVENTS: [&str; 5] = ["built", "removed", "crafted", "delivered", "jammed"];

/// Where each of [`EVENTS`] is in the counters, for the two places that name one.
const BUILT: usize = 0;
const REMOVED: usize = 1;
const CRAFTED: usize = 2;
const DELIVERED: usize = 3;
const JAMMED: usize = 4;

// ---------------------------------------------------------------------------------------------
// What happened this frame
// ---------------------------------------------------------------------------------------------

/// **One kind of thing that happened this frame**, which is the shape of every event: what it
/// was, how many of it there were, and the tile of the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counted {
    /// An item's number for `crafted` and `delivered`, a building's ([`number_of`]) for the rest.
    pub what: u32,
    pub n: u32,
    pub at: usize,
}

/// **The frame's happenings, added up by kind** — filled by the factory's own step
/// ([`crate::items::run_the_factory`]) and by the builder ([`crate::build::orders`]), and emptied
/// by [`tell_the_control_stage`] the moment they have been published.
#[derive(Resource, Debug, Default)]
pub struct Happenings {
    lists: [Vec<Counted>; EVENTS.len()],
    /// **Which machines were backed up at the end of the last frame.** A jam is a state and an
    /// event is an edge: `crate::machines::craft` says a machine is jammed every frame it is,
    /// and what the control stage hears is the frame it *became* so. Without this a factory with
    /// one stopped furnace in it would publish a message every frame for ever.
    backed_up: HashSet<usize>,
    /// The same, being built for this frame. Two sets rather than one so that a machine that has
    /// come unjammed falls out of it by not being put back in.
    still: HashSet<usize>,
}

impl Happenings {
    /// One more of a kind of thing, at a tile. The lists are a handful of entries long — a kind
    /// is a declaration of `data.rb` — so finding one is a walk of three.
    fn add(&mut self, event: usize, what: u32, n: u32, at: usize) {
        let list = &mut self.lists[event];
        match list.iter_mut().find(|c| c.what == what) {
            Some(already) => {
                already.n += n;
                already.at = at;
            }
            None => list.push(Counted { what, n, at }),
        }
    }

    /// Something was built or taken away. A covered tile is not a building of its own, so
    /// nothing is said about one.
    pub fn was_built(&mut self, what: What, at: usize) {
        if let Some(number) = number_of(what) {
            self.add(BUILT, number, 1, at);
        }
    }

    pub fn was_removed(&mut self, what: What, at: usize) {
        if let Some(number) = number_of(what) {
            self.add(REMOVED, number, 1, at);
        }
    }

    /// **What one step of the factory did, at the granularity the control stage hears about.**
    ///
    /// It is one walk of the moves, in the same system that was already walking them for the
    /// tally (`crate::items::run_the_factory`).
    pub fn watch(&mut self, moves: &Moves, grid: &Grid, data: &Data) {
        self.still.clear();
        for m in &moves.0 {
            match *m {
                // a delivery is a thing arriving in a chest, whether a belt handed it over or an
                // arm put it there — which is what a player means by having delivered something
                Move::Taken { into, item, .. } => self.delivery(grid, item, into),
                Move::Made { at, item } => self.delivery(grid, item, at),
                Move::Crafted { at, recipe } => {
                    if let Some(recipe) = data.recipes.get(recipe as usize) {
                        for &(item, n) in &recipe.outputs {
                            self.add(CRAFTED, item as u32, n, at);
                        }
                    }
                }
                Move::Jammed { at } => {
                    self.still.insert(at);
                    if !self.backed_up.contains(&at)
                        && let Some(What::Machine(kind)) = grid.at(at).map(|b| b.what)
                        && let Some(number) = number_of(What::Machine(kind))
                    {
                        self.add(JAMMED, number, 1, at);
                    }
                }
                Move::Carried { .. } | Move::Swung { .. } => {}
            }
        }
        core::mem::swap(&mut self.backed_up, &mut self.still);
    }

    fn delivery(&mut self, grid: &Grid, item: ItemId, into: usize) {
        if grid.at(into).map(|b| b.what) == Some(What::Chest) {
            self.add(DELIVERED, item as u32, 1, into);
        }
    }
}

/// **A building as a number**, which is what a published message carries and what the prelude
/// turns back into the name a player wrote: the four fittings the world has built in, and then
/// the machines `data.rb` declares, in the order it declares them — the same order
/// [`the_building_names`] writes.
///
/// `None` for a covered tile, which is not a building but the rest of one.
pub fn number_of(what: What) -> Option<u32> {
    Some(match what {
        What::Belt => 0,
        What::Miner => 1,
        What::Chest => 2,
        What::Inserter => 3,
        What::Machine(kind) => FITTINGS.len() as u32 + kind as u32,
        What::Covered { .. } => return None,
    })
}

/// The buildings that are not `data.rb`'s, in the order [`number_of`] counts them.
const FITTINGS: [&str; 4] = ["belt", "miner", "chest", "inserter"];

// ---------------------------------------------------------------------------------------------
// The script
// ---------------------------------------------------------------------------------------------

/// **The control stage, as the game holds it**: the text, the entity its task lives on, and
/// everything the script has said.
#[derive(Resource, Debug)]
pub struct TheControl {
    /// `ruby/control.rb`, or whatever has been applied over it.
    text: String,
    /// `ruby/control_prelude.rb`, read once.
    prelude: String,
    /// How far down the program the player's first line is, for the compiler's complaints.
    prelude_lines: u32,
    /// The entity the script runs on. It exists whether or not there is a script on it.
    entity: Option<Entity>,
    /// **What is wrong with it**, if anything: a file that would not be read, a program that
    /// would not compile, or a task that stopped — with the line of `control.rb` it was on.
    pub trouble: Option<String>,
    /// What the script says the goal is, in its own words (`@saying`).
    pub saying: Option<String>,
    /// Whether it has said it has won (`@won`).
    pub won: bool,
    /// How much of each of [`EVENTS`] it has heard (`@seen_built` and its four sisters).
    pub seen: [u32; EVENTS.len()],
    /// What its own subscriptions dropped (`Rubevy::Subscription#dropped`, summed in the
    /// prelude). The VM's own total is `ScriptWorld::dropped`, and the two are shown together.
    pub dropped: u64,
    /// So that the log says the win once rather than every frame.
    said_won: bool,
    /// **Whether what is running came out of the panel rather than out of the file** (F5). The
    /// editor draws the same mark it draws for an arm; Save is what clears it.
    pub in_memory: bool,
}

impl TheControl {
    /// `ruby/control.rb`, or whatever has been applied over it — for the editor and the save.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How far down the program the player's first line is, for the VM panel.
    pub fn prelude_lines(&self) -> u32 {
        self.prelude_lines
    }
}

/// **Another control.rb, please** — the game's own message for "run this text instead", which the
/// checks write and F5's editor will.
///
/// It is a message and not a function anybody may call, for the reason F3 wrote one for building
/// (`crate::build::Order`): what is *meant* is written down, and one system carries it out, so
/// whoever asks does not have to hold a `Commands` and the asset store to do it.
#[derive(Message, Debug, Clone)]
pub struct Rewrite(pub String);

/// The one system that swaps the control stage's script, before the factory steps.
pub fn follow_the_rewrites(
    mut commands: Commands,
    mut asked: MessageReader<Rewrite>,
    data: Res<Data>,
    rules: Res<Rules>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut control: ResMut<TheControl>,
) {
    for Rewrite(text) in asked.read() {
        let _ = give_it_another_script(
            &mut commands,
            &mut control,
            &data,
            &rules,
            &mut mrb,
            text.clone(),
        );
    }
}

/// **`Startup`, after the data stage**: the prelude, the file, the program, and the task.
///
/// It is after the data stage because the block the game writes in front of the prelude is made
/// of `data.rb`'s own names, and because a control stage with no factory to watch would have
/// nothing to say.
pub fn read_the_control_stage(
    mut commands: Commands,
    ruby: Res<crate::RubyDir>,
    data: Res<Data>,
    rules: Res<Rules>,
    mut mrb: ResMut<Assets<MrbAsset>>,
) {
    let prelude = platform::read(&ruby.0.join(PRELUDE_FILE)).unwrap_or_default();
    let (text, read_trouble) = match platform::read(&ruby.0.join(SCRIPT_FILE)) {
        Ok(text) => (text, None),
        Err(why) => (String::new(), Some(format!("{SCRIPT_FILE}: {why}"))),
    };
    let entity = commands.spawn(ControlScript).id();
    let mut control = TheControl {
        text,
        prelude,
        prelude_lines: 0,
        entity: Some(entity),
        trouble: read_trouble,
        saying: None,
        won: false,
        seen: [0; EVENTS.len()],
        dropped: 0,
        said_won: false,
        in_memory: false,
    };
    if control.trouble.is_none() {
        match compile(&control.prelude, &data, &rules, &control.text, &mut mrb) {
            Ok((handle, lines)) => {
                control.prelude_lines = lines;
                commands.entity(entity).insert(the_script(handle, lines));
                info!("{SCRIPT_FILE}: the control stage is running");
            }
            Err(why) => control.trouble = Some(why),
        }
    }
    if let Some(why) = &control.trouble {
        error!("{why}");
        error!("the factory runs with no goal until {SCRIPT_FILE} is fixed");
    }
    commands.insert_resource(control);
}

/// The entity the control stage's task lives on. There is exactly one and it is never despawned:
/// a script that is replaced keeps its entity, which is what `replace_script` is for.
#[derive(Component)]
pub struct ControlScript;

/// **The script, with the priority the plan asks for** (§5): **one ahead of the inserters**.
///
/// A smaller number is looked at first, and what is wanted is that three thousand arms cannot
/// starve the one script that says whether the game has been won — so the number is read off
/// what an arm runs at rather than written down, and one is the whole of the distance: *ahead* is
/// the property, and by how much says nothing. The arms are all at one priority among themselves,
/// which is the other half of §5: a script that is behind another is a script that goes hungry
/// first when the budget runs out.
fn the_script(handle: Handle<MrbAsset>, prelude_lines: u32) -> Script {
    let what_an_arm_runs_at = Script::new(handle.clone()).priority;
    Script::new(handle)
        .with_name("control")
        .with_priority(what_an_arm_runs_at.saturating_sub(1))
        // **how far down the program the player's first line is**, wrapper included, so that
        // `ScriptEnded::at` says the line of `control.rb` and not of the program
        .with_prelude_lines(prelude_lines)
}

/// One program: what the game wrote, the prelude, and the player's own file.
fn compile(
    prelude: &str,
    data: &Data,
    rules: &Rules,
    body: &str,
    mrb: &mut Assets<MrbAsset>,
) -> Result<(Handle<MrbAsset>, u32), String> {
    // **Once**, for the same reason the inserters' compile is once since 2026-09-22: rubevy says
    // where a script stopped (`ScriptEnded::at`), so the program no longer has to carry its own
    // length for a `rescue` in the prelude to read. What is still added by hand is the wrapper —
    // `Program::new` counts what is in front of the *body* it was given, and the body it is given
    // here is the player's file already inside a method.
    let front = format!("{}{prelude}", names_and_numbers(data, rules));
    let program = Program::new(&front, SCRIPT_FILE, &in_a_method(body), "run_control");
    let prelude_lines = program.prelude_lines + WRAPPER_LINES;
    match platform::compile(&program.source, SCRIPT_FILE) {
        Ok(bytes) => Ok((mrb.add(MrbAsset { bytes }), prelude_lines)),
        Err(why) => Err(in_the_authors_lines(&why, prelude_lines, PRELUDE_FILE)),
    }
}

/// **The player's file, put inside a method** — one line in front of it and one after.
///
/// An inserter's file is `inserter "…" do … end`, so everything in it runs inside the block the
/// prelude calls and an exception in it is inside the prelude's own `rescue`, which is where the
/// line a player is shown comes from ([`crate::inserters`]). A control.rb is written at the top
/// level — `goal`, `on`, and nothing to indent — so without this its exceptions would happen
/// while the *program* was being loaded, before `run_control` was reached, and the game could
/// only say `control.rb:?`. One line of wrapping buys the same sentence both files get.
fn in_a_method(body: &str) -> String {
    format!("def the_control_file\n{body}\nend\n")
}

/// How many lines [`in_a_method`] puts in front of the player's first one. The one after does not
/// count: nothing is reported against a line past the end.
const WRAPPER_LINES: u32 = 1;

/// **The Ruby the game writes**, from its own tables, in front of the prelude: the names a number
/// in a published message stands for, and how far down the program the player's first line is.
///
/// They are methods on the class rather than constants for the same reason the inserters' are: a
/// program is compiled again every time a text changes, and a constant written twice is a warning
/// nobody asked for. A name is quoted so that a data file may call a thing whatever it likes.
fn names_and_numbers(data: &Data, rules: &Rules) -> String {
    let quoted = |name: &str| format!(":\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
    let items: Vec<String> = data.items.iter().map(|i| quoted(&i.name)).collect();
    let buildings: Vec<String> = FITTINGS
        .iter()
        .map(|f| quoted(f))
        .chain(data.machines.iter().map(|m| quoted(&m.name)))
        .collect();
    format!(
        "# ---- written by the game from ruby/data.rb (factory/src/control.rs) ----\n\
         class Control\n\
         \x20 def self.declared_items\n\
         \x20   [{}]\n\
         \x20 end\n\
         \x20 def self.declared_buildings\n\
         \x20   [{}]\n\
         \x20 end\n\
         \x20 def self.map_size\n\
         \x20   [{}, {}]\n\
         \x20 end\n\
         \x20 def self.tile_px\n\
         \x20   {}\n\
         \x20 end\n\
         end\n",
        items.join(", "),
        buildings.join(", "),
        rules.map_tiles.x,
        rules.map_tiles.y,
        crate::TILE_PX,
    )
}

/// **Another control.rb, over the one that is running** — which is what the checks drive and what
/// F5's editor will.
///
/// Everything the old script said is forgotten, because a new goal is a new game: it counts what
/// has been delivered **from now**, not from when the world was made. The old task is terminated
/// by `replace_script`, which closes the subscriptions it held, which is what ends its handler
/// tasks (`ruby/control_prelude.rb`).
pub fn give_it_another_script(
    commands: &mut Commands,
    control: &mut TheControl,
    data: &Data,
    rules: &Rules,
    mrb: &mut Assets<MrbAsset>,
    text: String,
) -> Result<(), String> {
    let Some(entity) = control.entity else { return Err("there is no control stage".into()) };
    let made = compile(&control.prelude, data, rules, &text, mrb);
    control.text = text;
    control.saying = None;
    control.won = false;
    control.said_won = false;
    control.seen = [0; EVENTS.len()];
    control.dropped = 0;
    match made {
        Ok((handle, lines)) => {
            control.prelude_lines = lines;
            control.trouble = None;
            replace_script(commands, entity, the_script(handle, lines));
            Ok(())
        }
        Err(why) => {
            error!("{why}");
            error!("the factory runs with no goal until {SCRIPT_FILE} is fixed");
            control.trouble = Some(why.clone());
            Err(why)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Telling it, and hearing it
// ---------------------------------------------------------------------------------------------

/// **After the factory's step: what happened, published.**
///
/// It runs after the step and before the next frame's tick, so a script hears about a frame at
/// the head of the frame after it. Publishing to a name nobody subscribed to is one lookup
/// (rubevy's subscriptions are filed by name), so a run whose `control.rb` will not compile pays
/// nothing for these.
///
/// **The plan's trap about `publish` is not this game's** (§5: "a value is built once per
/// subscriber, so do not have every script subscribe to the same name"). There is one control
/// script and one handler per name in it, so a message is built once or twice; the inserters
/// subscribe to nothing at all — their `move` is a request they wait on, which is the other road.
pub fn tell_the_control_stage(
    mut scripts: ResMut<ScriptWorld>,
    grid: Res<Grid>,
    mut happenings: ResMut<Happenings>,
) {
    for (event, name) in EVENTS.iter().enumerate() {
        for c in core::mem::take(&mut happenings.lists[event]) {
            // **four flat numbers**: what, how many, and the column and row of the last of them —
            // a published message is a value built in the VM, and a list of numbers is the one
            // shape that costs nothing to build (`Answer::List`). The names are looked up in the
            // prelude, out of the lists the game wrote from `data.rb`.
            let tile = grid.tile_of(c.at);
            scripts.publish(
                None,
                name,
                Answer::List(vec![c.what as f64, c.n as f64, tile.x as f64, tile.y as f64]),
            );
        }
    }
}

/// **What the script says, read off the object it hangs on its task** — once a frame, with two
/// `ivar_get`s and no question for the script to answer.
///
/// It is the garden's road (`read_memory`, `mutation_rate_of`): a script that had to *tell* the
/// game these would be a script parked on a request every time a gear was delivered, and a
/// request costs a frame.
pub fn hear_the_control_stage(
    mut scripts: ResMut<ScriptWorld>,
    mut control: ResMut<TheControl>,
    task: Query<&ScriptTask, With<ControlScript>>,
) {
    let Ok(task) = task.single() else { return };
    let task = task.task();
    let Some(being) = scripts.vm.ivar_get(task, "@being").obj() else { return };
    for (i, event) in EVENTS.iter().enumerate() {
        control.seen[i] = whole_number(&mut scripts, being, &format!("@seen_{event}")) as u32;
    }
    control.dropped = whole_number(&mut scripts, being, "@dropped");
    let saying = scripts.vm.ivar_get(being, "@saying");
    control.saying = scripts
        .vm
        .str_bytes(saying)
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    control.won = scripts.vm.ivar_get(being, "@won").truthy();
    if control.won && !control.said_won {
        control.said_won = true;
        info!(
            "the control stage says the game is won: {}",
            control.saying.clone().unwrap_or_else(|| "no goal".into())
        );
    }
}

/// One `ivar_get` as a whole number, and zero for anything else — an instance variable that has
/// never been written answers nil.
fn whole_number(scripts: &mut ScriptWorld, being: sabiruby::value::ObjId, name: &str) -> u64 {
    match scripts.vm.ivar_get(being, name) {
        sabiruby::Value::Int(n) if n >= 0 => n as u64,
        _ => 0,
    }
}

/// **A control stage that stopped says where** — the same road a stopped inserter takes
/// ([`crate::inserters::where_it_broke`]): rubevy hands the line the script stopped on with the
/// ending, in the player's own numbering, because the `Script` was told how far down the program
/// the player's first line is.
pub fn watch_the_control_ending(
    mut ended: MessageReader<ScriptEnded>,
    mut control: ResMut<TheControl>,
    task: Query<(), With<ControlScript>>,
) {
    for end in ended.read() {
        if !task.contains(end.entity) {
            continue;
        }
        let at = crate::inserters::where_it_broke(end, SCRIPT_FILE);
        let says = format!("{at}: {}", end.value);
        match end.status {
            ScriptStatus::Failed => error!("the control stage stopped at {says}"),
            ScriptStatus::Finished => info!("the control stage ran to its end at {says}"),
        }
        error!("the factory runs with no goal until {SCRIPT_FILE} is fixed");
        control.trouble = Some(says);
    }
}

// ---------------------------------------------------------------------------------------------
// What it says
// ---------------------------------------------------------------------------------------------

/// **What the control stage has to say**, worked out from what the script said and what the VM
/// counted — one string, whoever is showing it.
///
/// F4 put it on a `Text` node of its own in the top left. F5's HUD is egui and has this line in
/// it ([`crate::window::draw_hud`]), so the node and the two systems that kept it are gone and
/// this is what is left: the sentence, with nothing about where it is drawn.
pub fn what_it_says(control: &TheControl, dropped_in_the_vm: u64) -> String {
    let mut lines: Vec<String> = Vec::new();
    match (&control.trouble, &control.saying) {
        (Some(why), _) => lines.push(format!("no goal — {why}")),
        (None, Some(saying)) => lines.push(saying.clone()),
        (None, None) => lines.push("control.rb sets no goal".into()),
    }
    lines.push(format!(
        "heard {} built, {} crafted, {} delivered, {} jammed — dropped {} ({} in the VM)",
        control.seen[BUILT],
        control.seen[CRAFTED],
        control.seen[DELIVERED],
        control.seen[JAMMED],
        control.dropped,
        dropped_in_the_vm,
    ));
    lines.join("\n")
}

/// **A run with no window has the same two numbers in its log, said once.**
///
/// Once, and not every frame: the counters move every time anything is delivered, and a line a
/// gear is a log nobody can read. What is worth saying out loud is the thing that should never
/// happen — a message published to a script that the script never saw, which says the events are
/// too fine for what this factory does and is the one number the plan asks for on the HUD (§3.4).
/// Everything else the line holds is on the screen in a window and in the checks everywhere.
pub fn say_if_anything_was_dropped(
    control: Res<TheControl>,
    scripts: Res<ScriptWorld>,
    mut said: Local<bool>,
) {
    if *said {
        return;
    }
    let (mine, vm) = (control.dropped, scripts.dropped());
    if mine == 0 && vm == 0 {
        return;
    }
    *said = true;
    warn!(
        "the control stage missed {mine} of what was published to it ({vm} dropped in the VM): \
         the events are finer than a subscription's {} a frame, or the script is behind",
        ScriptWorld::QUEUE_LIMIT
    );
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::machines;

    /// **Whether a machine is what this game calls jammed**, asked of a tile rather than watched
    /// for — the same two halves `crate::machines::craft` puts together when it says so, written
    /// here so that the test can ask a grid rather than watch a frame.
    fn is_jammed(grid: &Grid, data: &Data, tile: usize) -> bool {
        match grid.at(tile).map(|b| b.what) {
            Some(What::Machine(kind)) => {
                grid.at(tile).is_some_and(|b| !b.made.is_empty())
                    && machines::could_start(grid, data, tile, kind)
            }
            _ => false,
        }
    }
    use crate::belts::{Lanes, OnBelt};
    use crate::grid::{Building, Dir, Ore};
    use bevy::math::UVec2;

    /// The same little data file the belts' tests run on, through the same door.
    const DATA: &str = concat!(
        "item :ore, icon: 0\n",
        "item :plate, icon: 1\n",
        "machine :furnace, size: [1, 1], sprite: [109], speed: 1.0\n",
        "recipe :plate, in: { ore: 1 }, out: { plate: 1 }, time: 1.0, made_in: :furnace\n",
        "belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2\n",
        "miner :drill, seconds_per_item: 1.0\n",
        "chest :crate, capacity: 4\n",
        "ore :ore, per_tile: 10, patch_radius: 1.0, patches: [1, 1]\n",
        "map :world, size: [16, 16]\n",
        "inserter :arm, seconds_per_item: 1.0\n",
    );

    /// **A jam is said once, on the frame it starts**, and a machine that is merely waiting for
    /// parts is not jammed at all.
    ///
    /// A belt runs into an arm, the arm feeds a furnace, and there is **no arm on the other side**
    /// — so the furnace smelts one plate, holds it, and has the ore for another. That is the one
    /// thing this game calls a jam, and it is a state: the factory says it every frame it is true
    /// (`crate::machines::craft`) and [`Happenings`] turns it into the one frame it *became* true.
    #[test]
    fn a_jam_is_a_machine_that_cannot_get_rid_of_what_it_made_and_is_said_once() {
        let (data, rules) = crate::data::for_a_test(DATA);
        let tiles = UVec2::splat(8);
        let mut grid = crate::grid::Grid::new(tiles);
        let mut ore = Ore { left: vec![0; crate::grid::how_many(tiles)], changed: false };
        let mut lanes = Lanes::for_map(tiles);
        let feed = grid.index(UVec2::new(1, 1));
        let arm = grid.index(UVec2::new(2, 1));
        let furnace = grid.index(UVec2::new(3, 1));
        grid.place(feed, Building::new(crate::grid::What::Belt, Dir::East));
        grid.place(arm, Building::new(crate::grid::What::Inserter, Dir::East));
        let kind = data.machine("furnace").expect("declared");
        grid.place(furnace, Building::new(crate::grid::What::Machine(kind), Dir::East));
        for i in 0..3 {
            lanes.put_on(feed, OnBelt { along: -i * rules.spacing(), item: 0 });
        }

        let mut happenings = Happenings::default();
        let mut jams = 0;
        let mut said_while_waiting_for_parts = 0;
        for _ in 0..600 {
            // the arm, driven from Rust: what a script decides is not what is being measured here
            if grid.at(arm).is_some_and(|b| !b.swinging) {
                crate::machines::start_swing(&mut grid, &mut lanes, arm);
            }
            let holding = grid.at(furnace).is_some_and(|b| !b.made.is_empty());
            let moves = crate::belts::step(
                &mut grid,
                &mut ore,
                &mut lanes,
                &rules,
                &data,
                1.0 / 60.0,
            );
            happenings.watch(&moves, &grid, &data);
            let said = happenings.lists[JAMMED].len();
            jams += said;
            if !holding {
                said_while_waiting_for_parts += said;
            }
            happenings.lists[JAMMED].clear();
        }

        assert!(is_jammed(&grid, &data, furnace), "the furnace is holding a plate and has ore");
        assert_eq!(jams, 1, "the jam is one event and not one a frame");
        assert_eq!(said_while_waiting_for_parts, 0, "and it is not said before it is true");

        // and the moment somebody takes the plate away it is not a jam any more — and becoming
        // one again is another event
        crate::machines::take_from(&mut grid, &mut lanes, furnace);
        assert!(!is_jammed(&grid, &data, furnace), "nothing is waiting to come out of it now");
        for _ in 0..600 {
            if grid.at(arm).is_some_and(|b| !b.swinging) {
                crate::machines::start_swing(&mut grid, &mut lanes, arm);
            }
            let moves =
                crate::belts::step(&mut grid, &mut ore, &mut lanes, &rules, &data, 1.0 / 60.0);
            happenings.watch(&moves, &grid, &data);
        }
        assert_eq!(happenings.lists[JAMMED].len(), 1, "it jammed again, and said so again");
    }

    /// **A frame's happenings are one message per kind**, which is the whole of the granularity
    /// argument: a hundred deliveries of two kinds are two messages and not a hundred.
    #[test]
    fn a_frame_is_one_message_a_kind_however_much_happened() {
        let mut happenings = Happenings::default();
        for i in 0..100 {
            happenings.add(DELIVERED, 0, 1, i);
            happenings.add(DELIVERED, 1, 2, i);
        }
        assert_eq!(happenings.lists[DELIVERED].len(), 2, "two kinds, two messages");
        assert_eq!(happenings.lists[DELIVERED][0], Counted { what: 0, n: 100, at: 99 });
        assert_eq!(happenings.lists[DELIVERED][1], Counted { what: 1, n: 200, at: 99 });
        assert!(
            happenings.lists[DELIVERED].len() < ScriptWorld::QUEUE_LIMIT,
            "and a frame cannot fill a subscription's queue"
        );
    }

    /// The numbers a message carries a building as, and the names the prelude turns them back
    /// into, are one list read from both ends.
    #[test]
    fn a_building_and_its_number_agree_with_the_names_the_game_writes() {
        let (data, rules) = crate::data::for_a_test(DATA);
        let written = names_and_numbers(&data, &rules);
        let names: Vec<&str> = written
            .lines()
            .find(|l| l.contains(":\"belt\""))
            .expect("the buildings")
            .split('"')
            .filter(|p| p.chars().all(|c| c.is_ascii_lowercase() || c == '_') && !p.is_empty())
            .collect();
        assert_eq!(names, ["belt", "miner", "chest", "inserter", "furnace"]);
        assert_eq!(number_of(What::Belt), Some(0));
        assert_eq!(number_of(What::Inserter), Some(3));
        assert_eq!(number_of(What::Machine(0)), Some(4), "the first machine follows the four");
        assert_eq!(number_of(What::Covered { origin: 0 }), None, "not a building of its own");
    }
}
