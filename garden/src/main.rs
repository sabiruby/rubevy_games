//! Garden — a small world, in 3D, whose rules are Rust and whose minds are Ruby.
//!
//! This is stages G0, G0a and G1 of `docs/plans/garden-plan.md`. Grass grows, creatures walk
//! about, get hungry, eat, starve, and the sun goes round once a minute — and **what walks is a
//! Ruby task per creature** (`ruby/creatures/*.rb`), reading and writing its own ECS components
//! by name:
//!
//!     me[:Hunger]                       # → [42.0]
//!     me[:Velocity] = [[vx, vz]]
//!     plant[:Transform][:translation]   # → [x, y, z]
//!
//! There is no per-component glue anywhere in this file, and there is not meant to be: rubevy
//! walks Bevy's type registry (`docs/host-api.md`, "Components by name"), and so does the one
//! answering system here — `answer_garden` resolves `:Plant` through `ReflectComponent` rather
//! than matching on a name. The table in `docs/garden.md` is that registry, written out.
//!
//! The two questions a script asks that are *not* a component read are `garden.nearest(kind)`
//! and `garden.count(kind)`. That is the whole of the game's `ask` surface; SabiRuby Battle's
//! is six kinds, and the difference is the point of the two samples standing side by side.
//!
//!     cargo run -p garden                     # a window
//!     cargo run -p garden -- --headless 90    # no window, 90 seconds, the result on stdout
//!     GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90
//!
//! Mouse: drag to orbit, wheel to zoom.

mod platform;

use std::path::{Path, PathBuf};

use bevy::gltf::GltfAssetLabel;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use rubevy::{Answer, MrbAsset, RubevyPlugin, RubevySet, Script, ScriptTask, ScriptWorld};

// ---------------------------------------------------------------------------------------------
// The field. It lies on XZ with y up, which is the only thing 3D costs the Ruby side: a position
// is `[x, y, z]` and `act` takes `(vx, vz)`.
// ---------------------------------------------------------------------------------------------

/// 40 × 30, as the plan says, measured in world units (one unit is about a rabbit).
const FIELD_W: f32 = 40.0;
const FIELD_D: f32 = 30.0;
const HALF_W: f32 = FIELD_W / 2.0;
const HALF_D: f32 = FIELD_D / 2.0;

/// One turn of the sun. Night is the half of it the sun spends under the ground.
const DAY_LENGTH: f32 = 60.0;
/// Where in that turn the world starts: a little after sunrise, so the first thing a run sees is
/// daylight and the first `"night"` is something that arrives rather than something that was.
const DAWN_OFFSET: f32 = 0.08;

/// Plants
const PLANTS_AT_START: usize = 55;
const PLANTS_MAX: usize = 90;
/// how often a new one comes up, per second
const SPROUT_RATE: f32 = 0.7;
const PLANT_MIN: f32 = 0.18;
const PLANT_MAX: f32 = 1.4;
const PLANT_GROWTH: f32 = 0.06;

/// Creatures
const BEETLES: usize = 6;
const RABBITS: usize = 4;
const HUNGER_MAX: f32 = 100.0;
const HUNGER_RATE: f32 = 1.6;
/// how much plant a creature takes per second of standing on one, and what a unit of plant is
/// worth in hunger
const EAT_RATE: f32 = 1.0;
const FOOD_VALUE: f32 = 60.0;
/// how close is touching, for eating and for a rabbit startling a beetle
const REACH: f32 = 1.1;
const TOUCH_REACH: f32 = 1.3;

/// Solid things. Circles on XZ, pushed apart after the move; no physics crate, because the rule
/// is three lines and a physics crate is a megabyte of wasm and a second vocabulary.
const BEETLE_RADIUS: f32 = 0.40;
const RABBIT_RADIUS: f32 = 0.50;
const TREE_RADIUS: f32 = 0.70;
const ROCK_RADIUS: f32 = 0.60;
const TREES: usize = 7;
const ROCKS: usize = 9;
/// the side of one cell of the neighbour grid: at least twice the largest radius, so two circles
/// that touch are always in the same cell or in neighbouring ones
const CELL: f32 = 1.6;
/// how many times the push is repeated in a frame. One pass settles a pair; a huddle of three or
/// four wants a few, and four is enough that nothing is ever seen overlapping.
const SEPARATE_PASSES: usize = 4;

// ---------------------------------------------------------------------------------------------
// The components. This block is the Ruby API of the game, and the only reason it is an API is
// the two derives and the `register_type` calls in `main`.
// ---------------------------------------------------------------------------------------------

/// Grass. It grows by itself, is eaten down, and is gone when there is nothing left of it.
/// `size` is also the entity's `Transform.scale`, so `e[:Transform][:scale]` says the same thing
/// twice — on purpose: growth is something Ruby can see without the game telling it.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Plant {
    pub size: f32,
}

/// Which sort of creature. A field-less enum reflects as a Symbol (`:Beetle`), which is what a
/// script wants to compare against.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Species {
    #[default]
    Beetle,
    Rabbit,
}

impl Species {
    fn name(self) -> &'static str {
        match self {
            Species::Beetle => "Beetle",
            Species::Rabbit => "Rabbit",
        }
    }
    /// how fast it may go, and how far it sees — the two numbers G2's `Genome` will decide
    fn speed(self) -> f32 {
        match self {
            Species::Beetle => 2.2,
            Species::Rabbit => 3.4,
        }
    }
    fn sight(self) -> f32 {
        match self {
            Species::Beetle => 8.0,
            Species::Rabbit => 12.0,
        }
    }
}

/// A living thing, and how long it has been one.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Creature {
    pub species: Species,
    pub age: f32,
}

/// How full it is: `HUNGER_MAX` is stuffed, 0 is dead. (The name is the plan's; read it as "the
/// hunger meter", which empties.)
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Hunger(pub f32);

/// Where it is going, on the ground plane: `x` is world X and `y` is world Z. Rust integrates it
/// into `Transform` and stops it at the walls; from G1 it is what a brain writes.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Velocity(pub Vec2);

/// How far it can see. G1's `garden.nearest(:Plant)` looks this far and no further.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Sight(pub f32);

/// What it remembers. Empty until G3, where it becomes the Ruby `@memory` written to the save
/// file — it is here from G0 so that the component table does not change shape later.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Memory;

/// How much room a solid thing takes on the ground: a circle on XZ. Creatures, trees and rocks
/// have one; grass does not, because walking into grass is how it is eaten.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct Collider {
    pub radius: f32,
}

/// A tree. Not `Plant`: it is not food, it is something to walk round.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Tree;

/// A rock. The same, lower down.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
#[reflect(Component)]
pub struct Rock;

// ---------------------------------------------------------------------------------------------
// Components Ruby is **not** meant to see: no `Reflect`, no `register_type`. Keeping them out of
// the registry is the whole of the access control there is, and it is enough.
// ---------------------------------------------------------------------------------------------

/// The one directional light.
#[derive(Component)]
struct Sun;

/// What a creature's script is called and what it has cost, for the log (and for G4's HUD). The
/// numbers come from `ScriptWorld::stats`, which is rubevy's, not the VM's public surface.
#[derive(Component)]
struct Mind {
    name: String,
    /// instructions the script had run at the end of the last frame
    last_instructions: u64,
    /// and how many it spent on this one
    spent: u64,
    /// how many frames it has been looked at, so that the log can say what it costs on average —
    /// one frame's number is nearly always zero, because a creature spends nearly every frame
    /// parked on a `sleep` or on an answer
    frames: u64,
    /// the line it is standing on, in the creature's own file where it is in one
    at: String,
    /// how many lines the prelude put in front of the creature's file
    prelude_lines: u32,
}

/// Mid-meal, and for a moment after the last bite so that the animation does not flicker between
/// two blades of grass. Set by `eat`, read by `animate_creatures` — and by nothing else.
#[derive(Component)]
struct Eating {
    until: f32,
}

/// Which of the model's clips a creature is playing, and the entity of the `AnimationPlayer` the
/// scene brought with it. Only the windowed build has either.
#[derive(Component)]
struct Animated {
    player: Entity,
    playing: Gait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gait {
    Idle,
    Walk,
    Eat,
}

/// The selftest's fasting beetle: no script, almost no hunger left.
#[derive(Component)]
struct Fasting;

/// The selftest's probe: a hungry beetle put down within sight of exactly one plant, to see
/// whether a script-driven creature actually walks to its food.
#[derive(Component)]
struct Probe {
    /// the plant it was given
    dinner: Entity,
}

// ---------------------------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------------------------

/// The world's dice, SplitMix64 — the same one sabibots rolls, so a world can be replayed from a
/// seed once anything wants to.
#[derive(Resource)]
struct Dice(u64);

impl Dice {
    /// 0..1
    fn roll(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
    fn between(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.roll() * (hi - lo)
    }
}

/// Where the sun is, and whether the world thinks it is night. The flip is what gets published.
#[derive(Resource, Default)]
struct Sky {
    phase: f32,
    night: bool,
}

/// Which rabbit is standing on which beetle right now, so `"touched"` is published when a contact
/// begins rather than sixty times a second while it lasts.
#[derive(Resource, Default)]
struct Contacts(Vec<(Entity, Entity)>);

/// The same for `"bumped"`: which pairs the separation pass was pushing apart last frame. The key
/// is the two entities' bits, smaller first, so it does not depend on the order the query happened
/// to walk them in.
#[derive(Resource, Default)]
struct Bumps(Vec<(u64, u64)>);

fn pair_key(a: Entity, b: Entity) -> (u64, u64) {
    if a.to_bits() <= b.to_bits() { (a.to_bits(), b.to_bits()) } else { (b.to_bits(), a.to_bits()) }
}

/// The models, loaded once (G0a). Every one of them is a CC0 `.glb` from Kenney in
/// `assets/models/`; what the game had before was Bevy's own primitives, and the swap changed no
/// component at all, because the look was already a **child** of the entity that carries them.
///
/// A headless run has no `Look`: the models are the renderer's business, the components are not,
/// and the checks are all about the components. Every spawn below takes `Option<&Look>` for
/// exactly that reason, the way `day_night` takes `Option<ResMut<GlobalAmbientLight>>`.
#[derive(Resource)]
struct Look {
    ground: Handle<Mesh>,
    turf: Handle<StandardMaterial>,
    tuft: Handle<WorldAsset>,
    bush: Handle<WorldAsset>,
    tree: Handle<WorldAsset>,
    rock: Handle<WorldAsset>,
    beetle: Handle<WorldAsset>,
    rabbit: Handle<WorldAsset>,
    /// the three clips of each animal, in one graph each
    beetle_gaits: Gaits,
    rabbit_gaits: Gaits,
}

/// One animal's walk, idle and eat, as nodes of an `AnimationGraph`. The Kenney "Cube Pets"
/// models carry eight clips each — `static`, `idle`, `walk`, `run`, `eat`, `dance` and two
/// gestures — and the garden uses three of them.
#[derive(Clone)]
struct Gaits {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    eat: AnimationNodeIndex,
}

impl Gaits {
    fn node(&self, gait: Gait) -> AnimationNodeIndex {
        match gait {
            Gait::Idle => self.idle,
            Gait::Walk => self.walk,
            Gait::Eat => self.eat,
        }
    }
}

/// Where the Ruby lives. `ruby/prelude.rb` goes in front of every creature's file.
#[derive(Resource)]
struct RubyDir(PathBuf);

/// Which creatures took a bite last frame, so that `"ate"` is published once per meal rather
/// than sixty times a second — the same decision `"bumped"` and `"touched"` made in G0, and for
/// the same reason: a reflex is for an event, and the queue holds 64.
#[derive(Resource, Default)]
struct Eaters(Vec<Entity>);

/// `--headless N`: how long the world may run.
#[derive(Resource)]
struct Headless {
    until: f32,
}

/// `--shot FILE [SECONDS]`: where the picture goes, and when.
#[derive(Resource)]
struct Shot {
    path: String,
    after: f32,
    taken: bool,
}

/// Where the camera stands, in the only three numbers a look from above needs.
#[derive(Resource)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Orbit { yaw: 0.0, pitch: 0.85, distance: 42.0 }
    }
}

/// `GARDEN_SELFTEST=1`: what the plan asks the world to prove about itself — G0's four things
/// about the rules, and G1's three about the minds.
#[derive(Resource)]
struct SelfTest {
    /// when somebody first ate
    ate_at: Option<f32>,
    /// when night first fell
    night_at: Option<f32>,
    /// the creature that starved, and when
    starved: Option<(Entity, f32)>,
    /// the closest two colliders ever came, as a fraction of the sum of their radii: 1.0 is
    /// touching, below 0.9 is the check failing
    closest: f32,
    /// how many frames had a pair below 0.9, and how many frames were looked at
    overlaps: u32,
    frames: u32,

    // --- G1: the minds -----------------------------------------------------
    /// how far the probe beetle started from the plant it was given, and how close it ever got
    probe_from: f32,
    probe_closest: f32,
    /// when it got there, if it did
    probe_reached: Option<f32>,
    /// beetles that were touched by a rabbit while walking: when, and which way they were going
    touched: Vec<(Entity, f32, Vec2)>,
    /// when each beetle was last put on that list, so that a beetle a rabbit keeps walking into
    /// is looked at once rather than five times: the reflex takes half a second to run and the
    /// second message waits in the queue behind the first, so the answer to "did it turn?" for
    /// touch four is about touch one
    last_touch: Vec<(Entity, f32)>,
    /// how many of those were still there to look at half a second later, and how many had turned
    turn_checked: u32,
    turned: u32,
    /// the fastest anybody moved in the second after night fell, and how many creatures were
    /// looked at in that frame
    asleep_at: Option<f32>,
    awake_speed: f32,
    asleep_counted: u32,
}

impl Default for SelfTest {
    fn default() -> Self {
        // the minima start where nothing can be worse; `awake_speed` is a maximum
        SelfTest {
            ate_at: None,
            night_at: None,
            starved: None,
            closest: f32::INFINITY,
            overlaps: 0,
            frames: 0,
            probe_from: 0.0,
            probe_closest: f32::INFINITY,
            probe_reached: None,
            touched: Vec::new(),
            last_touch: Vec::new(),
            turn_checked: 0,
            turned: 0,
            asleep_at: None,
            awake_speed: 0.0,
            asleep_counted: 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `--headless N`: no window, N seconds, the world reported on stdout. It runs exactly the
    // same systems as the windowed one; only the drawing is missing.
    let headless = args
        .iter()
        .position(|a| a == "--headless")
        .map(|i| args.get(i + 1).and_then(|s| s.parse::<f32>().ok()).unwrap_or(10.0));
    // `--shot FILE [SECONDS]`: a window, a picture of it, and out.
    let shot = args.iter().position(|a| a == "--shot").map(|i| {
        (
            args.get(i + 1).cloned().unwrap_or_else(|| "shot.png".into()),
            args.get(i + 2).and_then(|s| s.parse::<f32>().ok()).unwrap_or(6.0),
        )
    });
    let selftest = std::env::var("GARDEN_SELFTEST").is_ok();

    let mut app = App::new();
    match headless {
        Some(seconds) => {
            app.add_plugins((
                MinimalPlugins.set(bevy::app::ScheduleRunnerPlugin::run_loop(
                    std::time::Duration::from_secs_f32(1.0 / 60.0),
                )),
                bevy::log::LogPlugin { filter: "info,bevy_asset=off".into(), ..default() },
                bevy::asset::AssetPlugin { file_path: platform::assets_dir(), ..default() },
                RubevyPlugin::default(),
            ))
            // no renderer here, and from G0a no models either: the same startup builds the same
            // entities with the same components, and only the child that carries the look is
            // missing. What the headless run tests is the world, not the picture of it.
            .insert_resource(Headless { until: seconds })
            .add_systems(Update, stop_when_over);
        }
        None => {
            app.add_plugins((
                DefaultPlugins
                    .set(AssetPlugin {
                        file_path: platform::assets_dir(),
                        meta_check: bevy::asset::AssetMetaCheck::Never,
                        ..default()
                    })
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            title: "Garden".into(),
                            resolution: (1600u32, 900u32).into(),
                            canvas: Some("#garden".into()),
                            fit_canvas_to_parent: true,
                            ..default()
                        }),
                        ..default()
                    }),
                RubevyPlugin::default(),
            ))
            .init_resource::<Orbit>()
            .add_systems(Startup, (make_look, spawn_camera))
            .add_systems(Update, (orbit_camera, dress_animations, animate_creatures));
        }
    }

    // What Ruby will see, and the only thing it takes. `Transform` is Bevy's own and is
    // registered by `DefaultPlugins`; `MinimalPlugins` does not register it (rubevy's
    // `docs/host-api.md` says so), and a headless run is where the checks live, so it is named
    // here. Registering a type twice is not an error.
    app.register_type::<Transform>()
        .register_type::<Plant>()
        .register_type::<Creature>()
        .register_type::<Species>()
        .register_type::<Hunger>()
        .register_type::<Velocity>()
        .register_type::<Sight>()
        .register_type::<Memory>()
        .register_type::<Collider>()
        .register_type::<Tree>()
        .register_type::<Rock>();

    app.insert_resource(Dice(platform::clock_seed()))
        .insert_resource(RubyDir(platform::ruby_dir()))
        .init_resource::<Sky>()
        .init_resource::<Contacts>()
        .init_resource::<Bumps>()
        .init_resource::<Eaters>()
        .add_systems(Startup, spawn_world)
        .add_systems(
            Update,
            (
                day_night,
                // G0's `wander` stood here, writing `Velocity` so that the world was not a still
                // life. It is gone: the only thing that writes `Velocity` now is
                // `me[:Velocity] = [[vx, vz]]` in a Ruby task, and every system below reads it
                // without caring who wrote it.
                move_creatures,
                separate,
                grow_plants,
                sprout_plants,
                get_hungry,
                eat,
                startle,
                starve,
            )
                .chain(),
        )
        // The one system that answers a script. It goes in `RubevySet::Answer`, which is the
        // only placement where a question is answered on the frame it was asked — anywhere else
        // costs a second frame per round trip, and where it landed used to be luck (rubevy
        // `docs/host-api.md`, "Where the game's systems go in the frame").
        .add_systems(Update, answer_garden.in_set(RubevySet::Answer))
        .add_systems(Update, watch_minds.after(RubevySet::Answer));
    if selftest {
        // after `separate`, so what it measures is the world as the frame leaves it
        app.init_resource::<SelfTest>()
            .add_systems(Update, watch_overlap.after(separate))
            .add_systems(Update, (watch_probe, watch_turning, watch_sleep).after(RubevySet::Answer));
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    app.run();
}

// ---------------------------------------------------------------------------------------------
// Building the world
// ---------------------------------------------------------------------------------------------

/// The models (G0a), loaded once, and the ground they stand on. Only the windowed build runs
/// this: a headless world has no `Look`, and every spawn below simply leaves the child out.
///
/// The grass, the tree and the rock are Kenney's Nature Kit; the rabbit and the beetle are
/// Kenney's Cube Pets, which are node-animated (no skin) and carry `idle`, `walk` and `eat`
/// among their eight clips. `assets/models/` has the two packs' own licence texts beside them,
/// and `CREDITS.md` the sizes and the sources.
fn make_look(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    // the clips of one glb, by their index in it: 0 static, 1 idle, 2 walk, 3 run, 4 eat, and
    // three the garden has no use for
    let mut gaits = |file: &str| {
        let clip = |i: u32| server.load(GltfAssetLabel::Animation(i as usize).from_asset(file.to_string()));
        let (graph, nodes) = AnimationGraph::from_clips([clip(1), clip(2), clip(4)]);
        Gaits { graph: graphs.add(graph), idle: nodes[0], walk: nodes[1], eat: nodes[2] }
    };
    let scene = |file: &str| server.load(GltfAssetLabel::Scene(0).from_asset(file.to_string()));

    commands.insert_resource(Look {
        ground: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(HALF_W, HALF_D))),
        turf: materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.46, 0.25),
            perceptual_roughness: 1.0,
            ..default()
        }),
        tuft: scene("models/grass.glb"),
        bush: scene("models/plant_bush.glb"),
        tree: scene("models/tree_default.glb"),
        rock: scene("models/rock_smallA.glb"),
        beetle: scene("models/animal-crab.glb"),
        rabbit: scene("models/animal-bunny.glb"),
        beetle_gaits: gaits("models/animal-crab.glb"),
        rabbit_gaits: gaits("models/animal-bunny.glb"),
    });
}

fn spawn_world(
    mut commands: Commands,
    look: Option<Res<Look>>,
    ruby: Res<RubyDir>,
    mut mrb: ResMut<Assets<MrbAsset>>,
    mut dice: ResMut<Dice>,
    selftest: Option<Res<SelfTest>>,
) {
    let look = look.as_deref();

    // the ground: one plane, 40 × 30. It is the only mesh left that is not a model, because a
    // lawn made of Kenney's 1 × 1 grass tiles would be twelve hundred entities for a flat green.
    if let Some(look) = look {
        commands.spawn((
            Mesh3d(look.ground.clone()),
            MeshMaterial3d(look.turf.clone()),
            Transform::default(),
        ));
    }

    // the sun. Its rotation, colour and brightness are `day_night`'s from the first frame; what
    // is set here is what does not change — that it casts shadows, and over how much ground.
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: 8_000.0, shadow_maps_enabled: true, ..default() },
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            first_cascade_far_bound: 24.0,
            maximum_distance: 70.0,
            ..default()
        }
        .build(),
        Transform::default(),
    ));

    // the selftest wants a creature that certainly starves, which means one that certainly does
    // not eat: it is spawned in the far corner **with no script at all**, so it never moves, and
    // the grass is kept away from it. Everything else about it is an ordinary beetle — what it
    // lacks is a brain. (In G0 the same beetle was the one without the `Wander` component.)
    let fasting_at = Vec2::new(-HALF_W + 3.0, -HALF_D + 3.0);
    // and one that certainly has something to walk to: a hungry beetle in the other far corner
    // with one plant five units away, which is inside a beetle's `Sight` of eight and outside
    // everything else.
    let probe_at = Vec2::new(HALF_W - 3.0, HALF_D - 3.0);
    let dinner_at = probe_at + Vec2::new(-4.0, -3.0);
    let keep_clear = selftest.is_some();
    let clear_of_fixtures = |at: Vec2| {
        !keep_clear || (at.distance(fasting_at) > 6.0 && at.distance(probe_at) > 7.0)
    };

    // trees and rocks first: they never move, so everything else is placed around them. They are
    // kept apart from each other at the start, because the separation pass moves creatures only
    // and two rocks left inside one another would overlap for the whole run.
    let mut solid: Vec<(Vec2, f32)> = Vec::new();
    for i in 0..(TREES + ROCKS) {
        let tree = i < TREES;
        let radius = if tree { TREE_RADIUS } else { ROCK_RADIUS };
        let mut at = Vec2::ZERO;
        let mut room = false;
        for _ in 0..40 {
            at = Vec2::new(dice.between(-HALF_W + 2.0, HALF_W - 2.0), dice.between(-HALF_D + 2.0, HALF_D - 2.0));
            if clear_of_fixtures(at) && solid.iter().all(|(p, r)| p.distance(at) > r + radius + 1.5) {
                room = true;
                break;
            }
        }
        if !room {
            continue;
        }
        if tree {
            spawn_tree(&mut commands, look, at);
        } else {
            spawn_rock(&mut commands, look, at, dice.between(0.8, 1.25));
        }
        solid.push((at, radius));
    }

    let mut grass: Vec<Vec2> = Vec::new();
    for _ in 0..PLANTS_AT_START {
        let at = Vec2::new(dice.between(-HALF_W + 1.0, HALF_W - 1.0), dice.between(-HALF_D + 1.0, HALF_D - 1.0));
        if !clear_of_fixtures(at) {
            continue;
        }
        // grass under a tree cannot be reached, so it is not put there
        if solid.iter().any(|(p, r)| p.distance(at) < r + 1.2) {
            continue;
        }
        let size = dice.between(0.3, PLANT_MAX);
        let round = dice.roll() < 0.35;
        spawn_plant(&mut commands, look, at, size, round);
        grass.push(at);
    }

    let mut taken: Vec<Vec2> = Vec::new();
    for i in 0..(BEETLES + RABBITS) {
        let species = if i < BEETLES { Species::Beetle } else { Species::Rabbit };
        // not standing on its dinner: a creature that starts inside a plant has eaten before it
        // has moved, and then "somebody ate within 10 s" says nothing about walking or about the
        // contact test. The positions are kept in hand because the plants above are still
        // commands and are not in the world to be queried yet.
        let radius = radius_of(species);
        let mut at = Vec2::ZERO;
        for _ in 0..40 {
            at = Vec2::new(dice.between(-HALF_W + 2.0, HALF_W - 2.0), dice.between(-HALF_D + 2.0, HALF_D - 2.0));
            let clear_of_grass = grass.iter().all(|g| g.distance(at) > 2.5);
            // nothing starts inside anything: the separation pass would otherwise have a pileup
            // to undo on the first frame, and the overlap check looks at that frame too
            let clear_of_solid = solid.iter().all(|(p, r)| p.distance(at) > r + radius + 0.6);
            let clear_of_kin = taken.iter().all(|p: &Vec2| p.distance(at) > 2.0);
            if clear_of_fixtures(at) && clear_of_grass && clear_of_solid && clear_of_kin {
                break;
            }
        }
        taken.push(at);
        let hunger = dice.between(45.0, 90.0);
        let entity = spawn_creature(&mut commands, look, species, at, hunger);
        give_mind(&mut commands, &ruby.0, &mut mrb, entity, species);
    }

    if keep_clear {
        let entity = spawn_creature(&mut commands, look, Species::Beetle, fasting_at, 3.0);
        commands.entity(entity).insert(Fasting);
        info!("selftest: a beetle with no brain and nothing to eat stands at ({:.1}, {:.1})", fasting_at.x, fasting_at.y);

        let dinner = spawn_plant(&mut commands, look, dinner_at, PLANT_MAX, false);
        // hungry enough that its script goes looking rather than wandering (the beetle's own
        // threshold is 55), and far enough that getting there has to be walking
        let probe = spawn_creature(&mut commands, look, Species::Beetle, probe_at, 40.0);
        commands.entity(probe).insert(Probe { dinner });
        give_mind(&mut commands, &ruby.0, &mut mrb, probe, Species::Beetle);
        info!(
            "selftest: a hungry beetle at ({:.1}, {:.1}) with one plant {:.1} away",
            probe_at.x,
            probe_at.y,
            probe_at.distance(dinner_at)
        );
    }
}

/// A plant: the entity carries the component and the scale, the model hangs under it. Splitting
/// them is what lets `Transform.scale` be the plant's size without the model having to know.
fn spawn_plant(commands: &mut Commands, look: Option<&Look>, at: Vec2, size: f32, round: bool) -> Entity {
    let mut plant = commands.spawn((
        Plant { size },
        Transform::from_xyz(at.x, 0.0, at.y).with_scale(Vec3::splat(size)),
        Visibility::default(),
    ));
    if let Some(look) = look {
        // Kenney's grass is about a quarter of a unit high, and a plant here is a tuft a beetle
        // can hide in: the model is scaled up inside the child, where nothing Ruby reads is
        let (model, scale) = if round { (look.bush.clone(), 2.6) } else { (look.tuft.clone(), 2.2) };
        plant.with_children(|plant| {
            plant.spawn((WorldAssetRoot(model), Transform::from_scale(Vec3::splat(scale))));
        });
    }
    plant.id()
}

/// A tree, and a `Collider` that does not move. `Tree`, not `Plant` — walking into grass is
/// eating, walking into a tree is not.
fn spawn_tree(commands: &mut Commands, look: Option<&Look>, at: Vec2) {
    let mut tree = commands.spawn((
        Tree,
        Collider { radius: TREE_RADIUS },
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        tree.with_children(|tree| {
            tree.spawn((WorldAssetRoot(look.tree.clone()), Transform::from_scale(Vec3::splat(2.2))));
        });
    }
}

/// A rock: the same immovable circle, and a boulder squashed a little so that nine of them do
/// not look like nine of one thing.
fn spawn_rock(commands: &mut Commands, look: Option<&Look>, at: Vec2, squash: f32) {
    let mut rock = commands.spawn((
        Rock,
        Collider { radius: ROCK_RADIUS },
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        rock.with_children(|rock| {
            rock.spawn((
                WorldAssetRoot(look.rock.clone()),
                Transform::from_scale(Vec3::new(3.4 * squash, 3.0, 3.4 / squash)),
            ));
        });
    }
}

fn radius_of(species: Species) -> f32 {
    match species {
        Species::Beetle => BEETLE_RADIUS,
        Species::Rabbit => RABBIT_RADIUS,
    }
}

/// A creature: the same split. The entity's `Transform` is position and facing and nothing else,
/// which is what a brain reads and writes; the model is a child — which is exactly why G0a swapped
/// six primitives for six `.glb` files without a single component changing.
fn spawn_creature(commands: &mut Commands, look: Option<&Look>, species: Species, at: Vec2, hunger: f32) -> Entity {
    let mut entity = commands.spawn((
        Creature { species, age: 0.0 },
        Hunger(hunger),
        Velocity(Vec2::ZERO),
        Sight(species.sight()),
        Collider { radius: radius_of(species) },
        Memory,
        Transform::from_xyz(at.x, 0.0, at.y),
        Visibility::default(),
    ));
    if let Some(look) = look {
        // the models face +Z and the world's heading does too (`move_creatures` turns the parent),
        // so the child only sets the size
        let (model, scale) = match species {
            Species::Beetle => (look.beetle.clone(), 0.42),
            Species::Rabbit => (look.rabbit.clone(), 0.55),
        };
        entity.with_children(|body| {
            body.spawn((WorldAssetRoot(model), Transform::from_scale(Vec3::splat(scale))));
        });
    }
    entity.id()
}

/// The mind: `ruby/prelude.rb` and one creature file, compiled together and hung on the entity as
/// a `Script`. rubevy starts it on the next frame, and from then on the creature moves because a
/// Ruby task says so.
fn give_mind(
    commands: &mut Commands,
    ruby: &Path,
    mrb: &mut Assets<MrbAsset>,
    entity: Entity,
    species: Species,
) {
    let file = match species {
        Species::Beetle => "beetle",
        Species::Rabbit => "rabbit",
    };
    let path = ruby.join("creatures").join(format!("{file}.rb"));
    let Some((handle, prelude_lines)) = compile(ruby, &path, mrb) else { return };
    let name = format!("{} {}", species.name(), entity);
    commands.entity(entity).insert((
        Script::new(handle).with_name(&name).with_priority(100),
        Mind { name, last_instructions: 0, spent: 0, frames: 0, at: String::new(), prelude_lines },
    ));
}

/// The prelude and one creature's file, compiled to bytecode in process (the reference compiler),
/// so the game reads `.rb` and nothing has to be built ahead of time. The two are compiled as one
/// program, which is why neither needs a `require`; the answer says how many lines the prelude
/// added, so a line can be reported in the author's own terms.
fn compile(ruby: &Path, creature: &Path, mrb: &mut Assets<MrbAsset>) -> Option<(Handle<MrbAsset>, u32)> {
    let body = match platform::read(creature) {
        Ok(b) => b,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };
    let name = creature.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let prelude = match platform::read(&ruby.join("prelude.rb")) {
        Ok(p) => p,
        Err(e) => {
            error!("{e}");
            return None;
        }
    };
    let src = format!("{prelude}\n# ---- {name} ----\n{body}\nrun_creature\n");
    let prelude_lines = prelude.lines().count() as u32 + 2;
    match platform::compile(&src, &name) {
        Ok(bytes) => Some((mrb.add(MrbAsset { bytes }), prelude_lines)),
        Err(e) => {
            error!("{e}");
            None
        }
    }
}

fn spawn_camera(mut commands: Commands, orbit: Res<Orbit>) {
    commands.spawn((Camera3d::default(), camera_at(&orbit)));
}

fn camera_at(orbit: &Orbit) -> Transform {
    let eye = Vec3::new(
        orbit.distance * orbit.pitch.cos() * orbit.yaw.sin(),
        orbit.distance * orbit.pitch.sin(),
        orbit.distance * orbit.pitch.cos() * orbit.yaw.cos(),
    );
    Transform::from_translation(eye).looking_at(Vec3::ZERO, Vec3::Y)
}

/// Drag to turn round the garden, wheel to come closer. The whole camera, because a world seen
/// from one fixed angle does not look like a world.
fn orbit_camera(
    mut orbit: ResMut<Orbit>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut cameras: Query<&mut Transform, With<Camera3d>>,
) {
    let mut moved = false;
    let dragging = buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right);
    for m in motion.read() {
        if dragging {
            orbit.yaw -= m.delta.x * 0.005;
            orbit.pitch = (orbit.pitch + m.delta.y * 0.005).clamp(0.12, 1.45);
            moved = true;
        }
    }
    for w in wheel.read() {
        orbit.distance = (orbit.distance - w.y * 2.0).clamp(12.0, 90.0);
        moved = true;
    }
    if moved {
        let at = camera_at(&orbit);
        for mut transform in &mut cameras {
            *transform = at;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The rules. Every one of them is a Rust system; none of them knows about Ruby except through
// `ScriptWorld::publish`, which is a no-op while nobody has subscribed.
// ---------------------------------------------------------------------------------------------

/// The sun goes round once a minute: where it is decides the light's direction, its colour and
/// its strength, the ambient light and the colour of the sky. The moment it goes under, `"night"`
/// is published to every script that asked for it; at sunrise, `"day"`.
fn day_night(
    time: Res<Time>,
    mut sky: ResMut<Sky>,
    mut world: ResMut<ScriptWorld>,
    mut test: Option<ResMut<SelfTest>>,
    mut sun: Query<(&mut Transform, &mut DirectionalLight), With<Sun>>,
    ambient: Option<ResMut<GlobalAmbientLight>>,
    clear: Option<ResMut<ClearColor>>,
) {
    let now = time.elapsed_secs();
    sky.phase = (now / DAY_LENGTH + DAWN_OFFSET).fract();
    let angle = sky.phase * std::f32::consts::TAU;
    // the sun's place in the sky: sunrise at phase 0, overhead at 0.25, gone at 0.5
    let up = Vec3::new(angle.cos() * 0.8, angle.sin(), 0.35).normalize();
    let height = up.y;
    let night = height <= 0.0;

    // at night the light comes from where the sun is not: a moon, dim and blue, still casting
    // the shadows that say the world is 3D
    let from = if night { -up } else { up };
    let (color, illuminance) = if night {
        (Color::srgb(0.55, 0.64, 1.0), 300.0)
    } else {
        // low sun is orange, high sun is white
        let noon = height.clamp(0.0, 1.0);
        (
            Color::srgb(1.0, 0.72 + 0.24 * noon, 0.45 + 0.5 * noon),
            1_200.0 + 9_000.0 * noon,
        )
    };
    for (mut transform, mut light) in &mut sun {
        *transform = Transform::from_translation(from * 60.0).looking_at(Vec3::ZERO, Vec3::Y);
        light.color = color;
        light.illuminance = illuminance;
    }
    // `GlobalAmbientLight` and `ClearColor` come with the render plugins; a headless run has
    // neither, and everything above it still runs
    if let Some(mut ambient) = ambient {
        if night {
            ambient.color = Color::srgb(0.35, 0.45, 0.8);
            ambient.brightness = 30.0;
        } else {
            ambient.color = Color::srgb(0.7, 0.8, 1.0);
            ambient.brightness = 120.0 + 260.0 * height.clamp(0.0, 1.0);
        }
    }
    if let Some(mut clear) = clear {
        clear.0 = if night {
            Color::srgb(0.03, 0.04, 0.10)
        } else {
            let noon = height.clamp(0.0, 1.0);
            Color::srgb(0.35 + 0.15 * noon, 0.5 + 0.22 * noon, 0.7 + 0.22 * noon)
        };
    }

    if night != sky.night {
        sky.night = night;
        let name = if night { "night" } else { "day" };
        world.publish(None, name, Answer::Num(now as f64));
        info!("{name} at {now:.1} s");
        if night && let Some(test) = test.as_mut() && test.night_at.is_none() {
            test.night_at = Some(now);
        }
    }
}

/// `Velocity` into `Transform`, on XZ, and the walls stop it. A creature also turns to face the
/// way it is going, which is the only thing in the game that writes `Transform.rotation`.
fn move_creatures(time: Res<Time>, mut creatures: Query<(&Creature, &mut Velocity, &mut Transform)>) {
    let dt = time.delta_secs();
    for (creature, mut velocity, mut transform) in &mut creatures {
        // a brain may ask for more than the body can give; the rule is the body's
        let limit = creature.species.speed();
        if velocity.0.length() > limit {
            velocity.0 = velocity.0.normalize_or_zero() * limit;
        }
        let mut x = transform.translation.x + velocity.0.x * dt;
        let mut z = transform.translation.z + velocity.0.y * dt;
        if x < -HALF_W + 0.5 || x > HALF_W - 0.5 {
            x = x.clamp(-HALF_W + 0.5, HALF_W - 0.5);
            velocity.0.x = 0.0;
        }
        if z < -HALF_D + 0.5 || z > HALF_D - 0.5 {
            z = z.clamp(-HALF_D + 0.5, HALF_D - 0.5);
            velocity.0.y = 0.0;
        }
        transform.translation.x = x;
        transform.translation.z = z;
        if velocity.0.length_squared() > 0.04 {
            transform.rotation = Quat::from_rotation_y((-velocity.0.x).atan2(-velocity.0.y));
        }
    }
}

/// Nothing walks through anything solid. Circles on XZ, pushed apart until they only touch: two
/// creatures give half each, a tree or a rock gives nothing. This is the whole of the physics in
/// the game, and it is deliberately not a physics crate — avian or rapier would be a megabyte of
/// wasm and a second vocabulary for a rule that fits on a screen.
///
/// Neighbours come from a grid of `CELL`-sided squares (`HashMap<(i32, i32), Vec<usize>>`), so the
/// cost is the number of pairs that are actually near each other rather than n². At a dozen
/// creatures it makes no difference; it is here because the world is meant to grow.
///
/// A pair that was not touching last frame and is now gets `"bumped"` published to the creature,
/// with the other thing as a `Rubevy::Entity`.
fn separate(
    mut world: ResMut<ScriptWorld>,
    mut bumps: ResMut<Bumps>,
    mut movers: Query<(Entity, &Collider, &mut Transform), With<Creature>>,
    fixed: Query<(Entity, &Collider, &Transform), Without<Creature>>,
) {
    // one list of everything solid: the movers first, so an index below `mover_count` is one
    let mut at: Vec<Vec2> = Vec::new();
    let mut radius: Vec<f32> = Vec::new();
    let mut who: Vec<Entity> = Vec::new();
    for (entity, collider, transform) in &movers {
        at.push(Vec2::new(transform.translation.x, transform.translation.z));
        radius.push(collider.radius);
        who.push(entity);
    }
    let mover_count = at.len();
    for (entity, collider, transform) in &fixed {
        at.push(Vec2::new(transform.translation.x, transform.translation.z));
        radius.push(collider.radius);
        who.push(entity);
    }
    if at.len() < 2 {
        return;
    }

    let mut touching: Vec<(Entity, Entity)> = Vec::new();
    for pass in 0..SEPARATE_PASSES {
        let mut grid: bevy::platform::collections::HashMap<(i32, i32), Vec<usize>> = default();
        for (i, p) in at.iter().enumerate() {
            grid.entry(((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)).or_default().push(i);
        }
        for i in 0..at.len() {
            let cell = ((at[i].x / CELL).floor() as i32, (at[i].y / CELL).floor() as i32);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let Some(near) = grid.get(&(cell.0 + dx, cell.1 + dz)) else { continue };
                    for &j in near {
                        // each pair once, and two immovable things have nothing to settle
                        if j <= i || (i >= mover_count && j >= mover_count) {
                            continue;
                        }
                        let sum = radius[i] + radius[j];
                        let gap = at[j] - at[i];
                        let distance = gap.length();
                        if distance >= sum {
                            continue;
                        }
                        if pass == 0 {
                            touching.push((who[i], who[j]));
                        }
                        // exactly on top of each other: any direction will do, and it must be the
                        // same one every frame or the pair jitters
                        let dir = if distance > 1e-4 {
                            gap / distance
                        } else {
                            Vec2::new(1.0, 0.0)
                        };
                        let push = sum - distance;
                        match (i < mover_count, j < mover_count) {
                            (true, true) => {
                                at[i] -= dir * push * 0.5;
                                at[j] += dir * push * 0.5;
                            }
                            (true, false) => at[i] -= dir * push,
                            (false, true) => at[j] += dir * push,
                            (false, false) => {}
                        }
                    }
                }
            }
        }
    }

    // back into the world, and back inside the walls: a push can put a creature through one
    let mut i = 0;
    for (_, _, mut transform) in &mut movers {
        transform.translation.x = at[i].x.clamp(-HALF_W + 0.5, HALF_W - 0.5);
        transform.translation.z = at[i].y.clamp(-HALF_D + 0.5, HALF_D - 0.5);
        i += 1;
    }

    // `"bumped"` is published when a contact begins, not on every frame of it: a creature leaning
    // on a tree is pushed a hundred times in the second and a half its heading lasts, and a
    // hundred messages would only fill the queue (rubevy keeps 64 and drops the oldest) and fire
    // G1's `reflex(:bumped)` over and over for one event. The plan says "the frame it is pushed";
    // this is the first of them.
    let mut now: Vec<(u64, u64)> = Vec::with_capacity(touching.len());
    for (a, b) in &touching {
        let key = pair_key(*a, *b);
        now.push(key);
        if bumps.0.contains(&key) {
            continue;
        }
        // the creature is always the first of the pair, by how the list was built
        world.publish(Some(*a), "bumped", Answer::Entity(*b));
        if movers.get(*b).is_ok() {
            world.publish(Some(*b), "bumped", Answer::Entity(*a));
        }
    }
    bumps.0 = now;
}

/// `GARDEN_SELFTEST=1`: nothing ever gets inside anything. Run after `separate`, so what it sees
/// is the world as the frame leaves it; a dozen creatures and sixteen obstacles is few enough to
/// look at every pair, and a check that walks the same grid as the thing it is checking would be
/// checking the grid against itself.
fn watch_overlap(mut test: ResMut<SelfTest>, solids: Query<(&Collider, &Transform, Option<&Creature>)>) {
    let all: Vec<(Vec2, f32, bool)> = solids
        .iter()
        .map(|(c, t, creature)| (Vec2::new(t.translation.x, t.translation.z), c.radius, creature.is_some()))
        .collect();
    let mut worst = f32::INFINITY;
    for (i, (p, r, creature)) in all.iter().enumerate() {
        for (q, s, other_creature) in all.iter().skip(i + 1) {
            // the rule pushes creatures; two rocks are placed apart and are nobody's business
            if !creature && !other_creature {
                continue;
            }
            let ratio = p.distance(*q) / (r + s);
            worst = worst.min(ratio);
        }
    }
    test.frames += 1;
    if worst < f32::INFINITY {
        test.closest = test.closest.min(worst);
        if worst < 0.9 {
            test.overlaps += 1;
        }
    }
}

/// Grass grows, and the growth is the scale: one number, seen two ways.
fn grow_plants(time: Res<Time>, mut plants: Query<(&mut Plant, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut plant, mut transform) in &mut plants {
        plant.size = (plant.size + PLANT_GROWTH * dt).min(PLANT_MAX);
        transform.scale = Vec3::splat(plant.size);
    }
}

/// New grass comes up on an empty patch, now and then, up to a limit.
fn sprout_plants(
    time: Res<Time>,
    mut commands: Commands,
    look: Option<Res<Look>>,
    mut dice: ResMut<Dice>,
    plants: Query<&Transform, With<Plant>>,
) {
    let count = plants.iter().count();
    if count >= PLANTS_MAX || dice.roll() > SPROUT_RATE * time.delta_secs() {
        return;
    }
    let at = Vec2::new(dice.between(-HALF_W + 1.0, HALF_W - 1.0), dice.between(-HALF_D + 1.0, HALF_D - 1.0));
    // not on top of another one
    if plants.iter().any(|t| Vec2::new(t.translation.x, t.translation.z).distance(at) < 1.5) {
        return;
    }
    let round = dice.roll() < 0.35;
    spawn_plant(&mut commands, look.as_deref(), at, PLANT_MIN, round);
}

/// Being alive costs.
fn get_hungry(time: Res<Time>, mut creatures: Query<(&mut Creature, &mut Hunger)>) {
    let dt = time.delta_secs();
    for (mut creature, mut hunger) in &mut creatures {
        creature.age += dt;
        hunger.0 -= HUNGER_RATE * dt;
    }
}

/// Eating is standing on it: a distance test, a bite out of the plant, and the news.
///
/// A bite is taken on every frame of the contact, but `"ate"` is published on the **first** of
/// them, with what that first mouthful was worth. The plan says "publish `ate`", and G0 did it
/// per frame while nothing was listening; a meal lasts a second or two, so a listening script
/// would have had a hundred messages for one event — which is the queue (64, oldest dropped)
/// filled by one creature having lunch, and a `reflex(:ate)` woken sixty times a second to be
/// told the same thing. It is the decision `"bumped"` and `"touched"` already made in G0.
fn eat(
    time: Res<Time>,
    mut commands: Commands,
    mut world: ResMut<ScriptWorld>,
    mut eaters: ResMut<Eaters>,
    mut test: Option<ResMut<SelfTest>>,
    mut creatures: Query<(Entity, &Transform, &mut Hunger), With<Creature>>,
    mut plants: Query<(Entity, &Transform, &mut Plant), Without<Creature>>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    let mut eating_now: Vec<Entity> = Vec::new();
    for (creature, at, mut hunger) in &mut creatures {
        if hunger.0 >= HUNGER_MAX {
            continue;
        }
        let here = Vec2::new(at.translation.x, at.translation.z);
        for (plant_entity, plant_at, mut plant) in &mut plants {
            // another creature may have finished this one off earlier in this same loop: the
            // despawn is a command and has not happened yet, but the component is already empty.
            // Without this a second mouth despawns an entity that is on its way out, which Bevy
            // reports as "the entity … is invalid; its index now has generation 1".
            if plant.size <= 0.02 {
                continue;
            }
            let there = Vec2::new(plant_at.translation.x, plant_at.translation.z);
            if here.distance(there) > REACH + plant.size * 0.5 {
                continue;
            }
            let bite = (EAT_RATE * dt).min(plant.size);
            plant.size -= bite;
            hunger.0 = (hunger.0 + bite * FOOD_VALUE).min(HUNGER_MAX);
            eating_now.push(creature);
            // the creature's own scripts hear it; the grass has none to hear anything. The
            // payload is how big the plant is, not how big the mouthful was: one frame's bite is
            // always the same number and says nothing, while the size of the thing it has just
            // sat down to is what a script would want to remember.
            if !eaters.0.contains(&creature) {
                world.publish(Some(creature), "ate", Answer::Num(plant.size as f64));
            }
            // and the model chews for a moment longer than the last bite, so that walking from
            // one blade of grass to the next does not flicker between two clips
            commands.entity(creature).insert(Eating { until: now + 0.35 });
            if let Some(test) = test.as_mut() && test.ate_at.is_none() {
                test.ate_at = Some(now);
                info!("selftest: first meal at {now:.2} s");
            }
            if plant.size <= 0.02 {
                commands.entity(plant_entity).despawn();
            }
            break; // one plant at a time
        }
    }
    eaters.0 = eating_now;
}

/// A rabbit walking into a beetle is news to the beetle — the material for G1's `reflex(:touched)`
/// — and it is published once per contact, not once per frame.
fn startle(
    time: Res<Time>,
    mut world: ResMut<ScriptWorld>,
    mut contacts: ResMut<Contacts>,
    mut test: Option<ResMut<SelfTest>>,
    creatures: Query<(Entity, &Creature, &Transform, &Velocity)>,
) {
    let now = time.elapsed_secs();
    let mut touching: Vec<(Entity, Entity)> = Vec::new();
    let rabbits: Vec<(Entity, Vec2)> = creatures
        .iter()
        .filter(|(_, c, _, _)| c.species == Species::Rabbit)
        .map(|(e, _, t, _)| (e, Vec2::new(t.translation.x, t.translation.z)))
        .collect();
    for (beetle, creature, at, velocity) in &creatures {
        if creature.species != Species::Beetle {
            continue;
        }
        let here = Vec2::new(at.translation.x, at.translation.z);
        for (rabbit, there) in &rabbits {
            if here.distance(*there) <= TOUCH_REACH {
                touching.push((*rabbit, beetle));
                if !contacts.0.contains(&(*rabbit, beetle)) {
                    world.publish(Some(beetle), "touched", Answer::Entity(*rabbit));
                    // and, for the selftest, which way it was going when it was told
                    if let Some(test) = test.as_mut() && velocity.0.length() > 0.5 && !by_a_wall(at) {
                        // and only when this beetle has been left alone for a while. A rabbit
                        // that keeps walking into one publishes again every time the contact is
                        // remade, the reflex takes half a second over each message, and the rest
                        // wait in the queue — so "did it turn?" asked half a second after the
                        // fourth message is really asking about the first. The clock is reset by
                        // *every* touch, so what is measured is always a beetle that was not
                        // already running from something.
                        let fresh = match test.last_touch.iter_mut().find(|(e, _)| *e == beetle) {
                            Some(seen) => {
                                let fresh = now - seen.1 > 1.5;
                                seen.1 = now;
                                fresh
                            }
                            None => {
                                test.last_touch.push((beetle, now));
                                true
                            }
                        };
                        if fresh {
                            test.touched.push((beetle, now, velocity.0));
                        }
                    }
                }
            }
        }
    }
    contacts.0 = touching;
}

/// An empty meter is the end of it. Despawning takes the entity's `ScriptTask` with it, and
/// rubevy's `on_remove` hook terminates the task and closes the queues anything of its was parked
/// on (`docs/host-api.md`, "Events"). Nothing runs a script in G0; the path is here so that G1
/// does not have to build it.
fn starve(
    time: Res<Time>,
    mut commands: Commands,
    mut test: Option<ResMut<SelfTest>>,
    creatures: Query<(Entity, &Creature, &Hunger)>,
) {
    let now = time.elapsed_secs();
    for (entity, creature, hunger) in &creatures {
        if hunger.0 > 0.0 {
            continue;
        }
        info!("{} {} starved at {now:.1} s (age {:.1} s)", creature.species.name(), entity, creature.age);
        commands.entity(entity).despawn();
        if let Some(test) = test.as_mut() && test.starved.is_none() {
            test.starved = Some((entity, now));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The two questions the game answers (G1)
// ---------------------------------------------------------------------------------------------

/// `garden.nearest(:Plant)` and `garden.count(:Plant)` — the whole of this game's `ask` surface.
/// SabiRuby Battle answers six kinds (`status`, `radar`, `incoming`, `act`, `seed`, `reflex`);
/// the garden answers two, because everything else a creature wants to know is a component it
/// can read for itself.
///
/// **There is no per-component code here either.** The kind of thing to look for arrives as a
/// string, and it is resolved the way rubevy resolves `e[:Hunger]`: through the type registry to
/// a `ReflectComponent`, whose `contains` says whether an entity has one. So `garden.count(:Rock)`
/// works, and so would `garden.nearest(:Whatever)` the day something registers a `Whatever` —
/// without this function being touched. What is typed here is the game's *rule*: how far a
/// creature may see (`Sight`) and that it never finds itself.
///
/// It runs in `RubevySet::Answer`, so a question asked on this frame is answered on this frame
/// and the script wakes with it on the next one.
fn answer_garden(world: &mut World) {
    let Some(registry) = world.get_resource::<AppTypeRegistry>().cloned() else { return };
    let registry = registry.read();
    world.resource_scope(|world: &mut World, mut scripts: Mut<ScriptWorld>| {
        let world = &*world;
        for request in scripts.take_requests() {
            let of_kind = request
                .text(0)
                .and_then(|name| {
                    registry.get_with_short_type_path(name).or_else(|| registry.get_with_type_path(name))
                })
                .and_then(|r| r.data::<bevy::ecs::reflect::ReflectComponent>());
            let asker = request.entity;
            match request.kind.as_str() {
                "garden.nearest" => {
                    let found = of_kind.zip(asker).and_then(|(rc, me)| {
                        let at = world.get::<Transform>(me)?.translation;
                        let here = Vec2::new(at.x, at.z);
                        // how far it may look is its own business, and its own component
                        let reach = world.get::<Sight>(me).map(|s| s.0).unwrap_or(0.0);
                        let mut best: Option<(Entity, f32)> = None;
                        for other in world.iter_entities() {
                            let entity = other.id();
                            if entity == me || !rc.contains(other) {
                                continue;
                            }
                            let Some(there) = other.get::<Transform>() else { continue };
                            let span = here.distance(Vec2::new(there.translation.x, there.translation.z));
                            if span > reach {
                                continue;
                            }
                            if best.is_none_or(|(_, b)| span < b) {
                                best = Some((entity, span));
                            }
                        }
                        best.map(|(e, _)| e)
                    });
                    match found {
                        Some(entity) => scripts.answer(&request, Answer::Entity(entity)),
                        None => scripts.answer(&request, Answer::Nil),
                    }
                }
                "garden.count" => {
                    let count = match of_kind {
                        Some(rc) => world.iter_entities().filter(|e| rc.contains(*e)).count(),
                        None => 0,
                    };
                    scripts.answer(&request, Answer::Num(count as f64));
                }
                other => {
                    warn!("garden: nobody answers {other:?}");
                    scripts.answer(&request, Answer::Nil);
                }
            }
        }
    });
}

/// What a script has cost and where it is standing, read every frame out of
/// `ScriptWorld::stats`. Until G4 puts a panel in the window this is only for the log at the end
/// of a headless run, but it is the same two numbers the panel will show — and the second one,
/// the line a *parked* task is waiting on, is the thing this VM can say and an engine's usual
/// scripting cannot.
fn watch_minds(world: Res<ScriptWorld>, mut minds: Query<(&mut Mind, Option<&ScriptTask>)>) {
    for (mut mind, script) in &mut minds {
        let Some(script) = script else { continue };
        let stats = world.stats(script);
        mind.spent = stats.instructions.saturating_sub(mind.last_instructions);
        mind.last_instructions = stats.instructions;
        mind.frames += 1;
        // the prelude sits in front of the creature's file in the compiled program, so a line
        // past its end is a line of the author's own file, counted from its own first line
        let lines = mind.prelude_lines;
        mind.at = match stats.frames.iter().find(|(_, line)| *line > lines) {
            Some((file, line)) => format!("{file}:{}", line - lines),
            None => match stats.location {
                Some((_, line)) => format!("prelude.rb:{line}"),
                None => "-".into(),
            },
        };
    }
}

// ---------------------------------------------------------------------------------------------
// The models move (G0a). Ruby knows nothing about any of this.
// ---------------------------------------------------------------------------------------------

/// A scene brings its own `AnimationPlayer` when it is spawned, on whichever of its nodes carries
/// the animation. This finds the creature it belongs to by walking up the hierarchy, gives the
/// player the right graph, and remembers where it is.
fn dress_animations(
    mut commands: Commands,
    look: Res<Look>,
    added: Query<Entity, Added<AnimationPlayer>>,
    parents: Query<&ChildOf>,
    creatures: Query<&Creature>,
) {
    for player in &added {
        let mut at = player;
        let mut owner = None;
        loop {
            if creatures.get(at).is_ok() {
                owner = Some(at);
                break;
            }
            match parents.get(at) {
                Ok(parent) => at = parent.parent(),
                Err(_) => break,
            }
        }
        let Some(owner) = owner else { continue };
        let Ok(creature) = creatures.get(owner) else { continue };
        let gaits = match creature.species {
            Species::Beetle => look.beetle_gaits.clone(),
            Species::Rabbit => look.rabbit_gaits.clone(),
        };
        commands
            .entity(player)
            .insert((AnimationGraphHandle(gaits.graph.clone()), AnimationTransitions::new()));
        commands.entity(owner).insert(Animated { player, playing: Gait::Idle });
    }
}

/// Walk when it is moving, eat when it is eating, idle otherwise — decided from `Velocity` and
/// the `Eating` mark, which are the game's own state. A creature's script never knows that any
/// of this happened, and could not reach it if it wanted to: `Animated`, `Eating` and `Gait`
/// derive no `Reflect` and are registered nowhere.
fn animate_creatures(
    time: Res<Time>,
    look: Res<Look>,
    mut creatures: Query<(&Creature, &Velocity, Option<&Eating>, &mut Animated)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let now = time.elapsed_secs();
    for (creature, velocity, eating, mut animated) in &mut creatures {
        let want = if eating.is_some_and(|e| e.until > now) {
            Gait::Eat
        } else if velocity.0.length() > 0.2 {
            Gait::Walk
        } else {
            Gait::Idle
        };
        let Ok((mut player, mut transitions)) = players.get_mut(animated.player) else { continue };
        if animated.playing == want && player.playing_animations().count() > 0 {
            continue;
        }
        let gaits = match creature.species {
            Species::Beetle => &look.beetle_gaits,
            Species::Rabbit => &look.rabbit_gaits,
        };
        transitions
            .play(&mut player, gaits.node(want), std::time::Duration::from_millis(180))
            .repeat();
        animated.playing = want;
    }
}

// ---------------------------------------------------------------------------------------------
// The three checks G1 adds (`GARDEN_SELFTEST=1`)
// ---------------------------------------------------------------------------------------------

/// **A hungry creature with a plant in sight reaches it.** The probe beetle was put down with
/// one plant five units away and nothing else within seven, and forty points of hunger — under
/// its own script's threshold of fifty-five. Nothing in Rust moves it; if it gets there, a Ruby
/// task read `me[:Hunger]`, asked `garden.nearest(:Plant)`, read the answer's
/// `[:Transform][:translation]` and wrote `me[:Velocity]`.
fn watch_probe(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    probes: Query<(&Transform, &Probe)>,
    plants: Query<&Transform, With<Plant>>,
) {
    let now = time.elapsed_secs();
    for (at, probe) in &probes {
        let here = Vec2::new(at.translation.x, at.translation.z);
        let Ok(dinner) = plants.get(probe.dinner) else {
            // the plant is gone, which happens when it has been eaten: it was reached
            if test.probe_reached.is_none() {
                test.probe_reached = Some(now);
                test.probe_closest = 0.0;
            }
            continue;
        };
        let span = here.distance(Vec2::new(dinner.translation.x, dinner.translation.z));
        if test.probe_from == 0.0 {
            test.probe_from = span;
        }
        test.probe_closest = test.probe_closest.min(span);
        if span < REACH + 0.5 && test.probe_reached.is_none() {
            test.probe_reached = Some(now);
            info!("selftest: the hungry beetle reached its plant at {now:.2} s");
        }
    }
}

/// **A beetle touched by a rabbit changes heading within 0.5 s.** `startle` notes the beetle and
/// the way it was going at the moment the game published `"touched"`; half a second later this
/// looks again. Only a beetle that was actually walking is counted, because "it turned" means
/// nothing about one that was standing still or asleep.
fn watch_turning(
    time: Res<Time>,
    mut test: ResMut<SelfTest>,
    creatures: Query<(&Velocity, &Transform)>,
) {
    let now = time.elapsed_secs();
    let mut still_waiting: Vec<(Entity, f32, Vec2)> = Vec::new();
    let mut checked = 0u32;
    let mut turned = 0u32;
    for (beetle, at, was) in std::mem::take(&mut test.touched) {
        if now - at < 0.5 {
            still_waiting.push((beetle, at, was));
            continue;
        }
        let Ok((now_going, place)) = creatures.get(beetle) else { continue }; // starved meanwhile
        // and not against a wall: `move_creatures` zeroes the component of `Velocity` that would
        // take a creature through one, so a beetle in the corner reads as going due west both
        // before the reflex and after it however it turned. The reflex is not what failed there,
        // and a check that says it did would be a check about the walls.
        if by_a_wall(place) {
            continue;
        }
        checked += 1;
        // the angle between the two headings: a flee is roughly a reversal, and anything past a
        // quarter turn is a different course than the one it was on
        let a = was.normalize_or_zero();
        let b = now_going.0.normalize_or_zero();
        if b == Vec2::ZERO || a.dot(b) < 0.7 {
            turned += 1;
        }
    }
    test.touched = still_waiting;
    test.turn_checked += checked;
    test.turned += turned;
}

/// Within a unit of the edge of the field, where `move_creatures` clips `Velocity` and a heading
/// stops being readable from it.
fn by_a_wall(at: &Transform) -> bool {
    at.translation.x.abs() > HALF_W - 1.5 || at.translation.z.abs() > HALF_D - 1.5
}

/// **Creatures sleep at night.** One second after the game published `"night"`, nothing with a
/// script should still be moving: both species have `reflex(:night) { @asleep = true; stop }`,
/// and their run loops keep it that way. The fasting beetle has no script and is standing still
/// anyway, so it proves nothing and is not counted — `Mind` is the mark of a creature that has
/// a brain to fall asleep with.
fn watch_sleep(time: Res<Time>, mut test: ResMut<SelfTest>, creatures: Query<&Velocity, With<Mind>>) {
    let Some(night) = test.night_at else { return };
    if test.asleep_at.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    if now - night < 1.0 {
        return;
    }
    test.asleep_at = Some(now);
    for velocity in &creatures {
        test.awake_speed = test.awake_speed.max(velocity.0.length());
        test.asleep_counted += 1;
    }
}

// ---------------------------------------------------------------------------------------------
// Running without a window
// ---------------------------------------------------------------------------------------------

fn stop_when_over(
    time: Res<Time>,
    headless: Res<Headless>,
    sky: Res<Sky>,
    test: Option<Res<SelfTest>>,
    creatures: Query<(&Creature, &Hunger, &Velocity, &Transform, Option<&Mind>)>,
    plants: Query<&Plant>,
    everything: Query<Entity>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < headless.until {
        return;
    }
    for (creature, hunger, velocity, at, mind) in &creatures {
        // the last two columns are the script's: what it spent on the frame that has just gone,
        // and the line of its own file it is standing on — which is the line it is *waiting* on,
        // since a creature spends most of its life parked on a `sleep` or on an answer
        info!(
            "{:<14} hunger {:>5.1}  age {:>5.1}  at ({:>6.1}, {:>6.1})  v ({:>5.1}, {:>5.1})  {:>6} insn/frame  {}",
            mind.map(|m| m.name.clone()).unwrap_or_else(|| creature.species.name().into()),
            hunger.0,
            creature.age,
            at.translation.x,
            at.translation.z,
            velocity.0.x,
            velocity.0.y,
            mind.map(|m| m.last_instructions / m.frames.max(1)).unwrap_or(0),
            mind.map(|m| m.at.as_str()).unwrap_or("(no brain)")
        );
    }
    info!(
        "{} creatures, {} plants, {} at phase {:.2}",
        creatures.iter().count(),
        plants.iter().count(),
        if sky.night { "night" } else { "day" },
        sky.phase
    );

    if let Some(test) = test {
        let ok = |cond: bool, what: String| info!("selftest: {} {what}", if cond { "ok  " } else { "FAIL" });
        ok(
            test.ate_at.is_some_and(|t| t <= 10.0),
            match test.ate_at {
                Some(t) => format!("somebody ate within 10 s (first at {t:.2} s)"),
                None => "somebody ate within 10 s (nobody ate at all)".into(),
            },
        );
        ok(
            test.night_at.is_some_and(|t| t <= 60.0),
            match test.night_at {
                Some(t) => format!("night arrived by 60 s (at {t:.2} s)"),
                None => "night arrived by 60 s (it never did)".into(),
            },
        );
        match test.starved {
            Some((entity, at)) => ok(
                !everything.contains(entity),
                format!("the starved creature's entity is gone ({entity} starved at {at:.2} s)"),
            ),
            None => ok(false, "a creature starved and its entity is gone (nobody starved)".into()),
        }
        ok(
            test.overlaps == 0 && test.frames > 0,
            format!(
                "nothing walked through anything over {} frames (closest pair {:.3} of the radii, {} frames under 0.9)",
                test.frames, test.closest, test.overlaps
            ),
        );
        // --- G1: the minds -------------------------------------------------
        match test.probe_reached {
            Some(at) => ok(
                true,
                format!(
                    "a hungry creature with a plant in sight reached it (from {:.1} away, at {at:.2} s)",
                    test.probe_from
                ),
            ),
            None => ok(
                false,
                format!(
                    "a hungry creature with a plant in sight reached it (it started {:.1} away and got no closer than {:.1})",
                    test.probe_from, test.probe_closest
                ),
            ),
        }
        ok(
            test.turn_checked > 0 && test.turned == test.turn_checked,
            format!(
                "a beetle touched by a rabbit changed heading within 0.5 s ({}/{})",
                test.turned, test.turn_checked
            ),
        );
        match test.asleep_at {
            Some(at) => ok(
                test.asleep_counted > 0 && test.awake_speed < 0.05,
                format!(
                    "the creatures were asleep a second after night fell ({} of them, fastest {:.3} at {at:.2} s)",
                    test.asleep_counted, test.awake_speed
                ),
            ),
            None => ok(false, "the creatures were asleep a second after night fell (night never came)".into()),
        }
    }
    exit.write(AppExit::Success);
}

fn take_shot(mut commands: Commands, time: Res<Time>, mut shot: ResMut<Shot>, mut exit: MessageWriter<AppExit>) {
    if shot.taken {
        if time.elapsed_secs() > shot.after + 2.0 {
            exit.write(AppExit::Success);
        }
        return;
    }
    if time.elapsed_secs() < shot.after {
        return;
    }
    shot.taken = true;
    let path = shot.path.clone();
    info!("screenshot -> {path}");
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}
