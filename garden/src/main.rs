//! Garden — a small world, in 3D, whose rules are Rust and whose minds will be Ruby.
//!
//! This is stage G0 of `docs/plans/garden-plan.md`: the world alone. Grass grows, creatures walk
//! about, get hungry, eat, starve, and the sun goes round once a minute. **No script runs yet.**
//! What G0 is really for is the shape of the data: every component a creature has is
//! `#[derive(Component, Reflect)] #[reflect(Component)]` and is handed to `register_type`, which
//! is the whole of what it takes for G1's Ruby to say
//!
//!     me[:Hunger]                       # → 42.0
//!     me[:Velocity] = [vx, vz]
//!     plant[:Transform][:translation]   # → [x, y, z]
//!
//! There is no per-component glue anywhere in this file, and there is not meant to be: rubevy
//! walks Bevy's type registry (`docs/host-api.md`, "Components by name"). The table in
//! `docs/garden.md` is that registry, written out.
//!
//!     cargo run -p garden                     # a window
//!     cargo run -p garden -- --headless 90    # no window, 90 seconds, the result on stdout
//!     GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90
//!
//! Mouse: drag to orbit, wheel to zoom.

mod platform;

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use rubevy::{Answer, RubevyPlugin, ScriptWorld};

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

/// **The placeholder brain — this is what G1 deletes.** Until a Ruby task writes `Velocity`,
/// something has to, or the world is a still life. It picks a new heading every second or so and
/// walks. Nothing else in this file knows it exists; a creature spawned without it simply stands
/// where it is (which is how the selftest arranges a starvation).
#[derive(Component)]
struct Wander {
    until: f32,
}

/// The selftest's fasting beetle: no `Wander`, almost no hunger left.
#[derive(Component)]
struct Fasting;

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

/// The meshes and materials, made once. Plants come up while the world runs, so the handles have
/// to outlive `Startup`.
#[derive(Resource)]
struct Look {
    tuft: Handle<Mesh>,
    bush: Handle<Mesh>,
    beetle: Handle<Mesh>,
    rabbit_body: Handle<Mesh>,
    rabbit_ear: Handle<Mesh>,
    trunk: Handle<Mesh>,
    canopy: Handle<Mesh>,
    boulder: Handle<Mesh>,
    leaf: Handle<StandardMaterial>,
    shell: Handle<StandardMaterial>,
    fur: Handle<StandardMaterial>,
    bark: Handle<StandardMaterial>,
    needle: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
}

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

/// `GARDEN_SELFTEST=1`: the four things the plan asks the world to prove about itself.
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
}

impl Default for SelfTest {
    fn default() -> Self {
        // `closest` is a minimum, so it starts where nothing can be worse
        SelfTest { ate_at: None, night_at: None, starved: None, closest: f32::INFINITY, overlaps: 0, frames: 0 }
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
            // no renderer here, but the same startup builds the same entities, so what the
            // headless run tests is the world the window shows
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
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
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, orbit_camera);
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
        .init_resource::<Sky>()
        .init_resource::<Contacts>()
        .init_resource::<Bumps>()
        .add_systems(Startup, (make_look, spawn_world).chain())
        .add_systems(
            Update,
            (
                day_night,
                wander,        // G0 only; G1's Ruby writes `Velocity` instead
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
        );
    if selftest {
        // after `separate`, so what it measures is the world as the frame leaves it
        app.init_resource::<SelfTest>().add_systems(Update, watch_overlap.after(separate));
    }
    if let Some((path, after)) = shot {
        app.insert_resource(Shot { path, after, taken: false }).add_systems(Update, take_shot);
    }
    app.run();
}

// ---------------------------------------------------------------------------------------------
// Building the world
// ---------------------------------------------------------------------------------------------

fn make_look(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(Look {
        tuft: meshes.add(Cone { radius: 0.38, height: 1.1 }),
        bush: meshes.add(Sphere::new(0.45)),
        beetle: meshes.add(Capsule3d::new(0.32, 0.55)),
        rabbit_body: meshes.add(Cuboid::new(0.55, 0.5, 0.85)),
        rabbit_ear: meshes.add(Cuboid::new(0.1, 0.45, 0.08)),
        trunk: meshes.add(Cylinder::new(0.18, 2.0)),
        canopy: meshes.add(Cone { radius: 1.0, height: 2.4 }),
        boulder: meshes.add(Sphere::new(ROCK_RADIUS)),
        leaf: materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.62, 0.22),
            perceptual_roughness: 0.9,
            ..default()
        }),
        shell: materials.add(StandardMaterial {
            base_color: Color::srgb(0.24, 0.16, 0.12),
            perceptual_roughness: 0.35,
            metallic: 0.25,
            ..default()
        }),
        fur: materials.add(StandardMaterial {
            base_color: Color::srgb(0.88, 0.84, 0.78),
            perceptual_roughness: 0.95,
            ..default()
        }),
        bark: materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.24, 0.16),
            perceptual_roughness: 1.0,
            ..default()
        }),
        needle: materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.33, 0.16),
            perceptual_roughness: 0.9,
            ..default()
        }),
        stone: materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.52, 0.55),
            perceptual_roughness: 0.8,
            ..default()
        }),
    });
}

fn spawn_world(
    mut commands: Commands,
    look: Res<Look>,
    mut dice: ResMut<Dice>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    selftest: Option<Res<SelfTest>>,
) {
    // the ground: one plane, 40 × 30
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(HALF_W, HALF_D)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.46, 0.25),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::default(),
    ));

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
    // not eat: it is spawned in the far corner without the placeholder brain, so it never moves,
    // and the grass is kept away from it. Everything else about it is an ordinary beetle.
    let fasting_at = Vec2::new(-HALF_W + 3.0, -HALF_D + 3.0);
    let keep_clear = selftest.is_some();

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
            let clear_of_fasting = !keep_clear || at.distance(fasting_at) > 6.0;
            if clear_of_fasting && solid.iter().all(|(p, r)| p.distance(at) > r + radius + 1.5) {
                room = true;
                break;
            }
        }
        if !room {
            continue;
        }
        if tree {
            spawn_tree(&mut commands, &look, at);
        } else {
            spawn_rock(&mut commands, &look, at, dice.between(0.8, 1.25));
        }
        solid.push((at, radius));
    }

    let mut grass: Vec<Vec2> = Vec::new();
    for _ in 0..PLANTS_AT_START {
        let at = Vec2::new(dice.between(-HALF_W + 1.0, HALF_W - 1.0), dice.between(-HALF_D + 1.0, HALF_D - 1.0));
        if keep_clear && at.distance(fasting_at) < 6.0 {
            continue;
        }
        // grass under a tree cannot be reached, so it is not put there
        if solid.iter().any(|(p, r)| p.distance(at) < r + 1.2) {
            continue;
        }
        let size = dice.between(0.3, PLANT_MAX);
        let round = dice.roll() < 0.35;
        spawn_plant(&mut commands, &look, at, size, round);
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
            if clear_of_grass && clear_of_solid && clear_of_kin {
                break;
            }
        }
        taken.push(at);
        let hunger = dice.between(45.0, 90.0);
        let entity = spawn_creature(&mut commands, &look, species, at, hunger);
        commands.entity(entity).insert(Wander { until: dice.between(0.3, 1.4) });
    }

    if keep_clear {
        let entity = spawn_creature(&mut commands, &look, Species::Beetle, fasting_at, 3.0);
        commands.entity(entity).insert(Fasting);
        info!("selftest: a beetle with nothing to eat stands at ({:.1}, {:.1})", fasting_at.x, fasting_at.y);
    }
}

/// A plant: the entity carries the component and the scale, the mesh hangs under it. Splitting
/// them is what lets `Transform.scale` be the plant's size without the mesh having to know.
fn spawn_plant(commands: &mut Commands, look: &Look, at: Vec2, size: f32, round: bool) {
    let (mesh, y) = if round { (look.bush.clone(), 0.45) } else { (look.tuft.clone(), 0.55) };
    commands
        .spawn((
            Plant { size },
            Transform::from_xyz(at.x, 0.0, at.y).with_scale(Vec3::splat(size)),
            Visibility::default(),
        ))
        .with_children(|plant| {
            plant.spawn((Mesh3d(mesh), MeshMaterial3d(look.leaf.clone()), Transform::from_xyz(0.0, y, 0.0)));
        });
}

/// A tree: a trunk and a cone of needles, and a `Collider` that does not move. `Tree`, not
/// `Plant` — walking into grass is eating, walking into a tree is not.
fn spawn_tree(commands: &mut Commands, look: &Look, at: Vec2) {
    commands
        .spawn((
            Tree,
            Collider { radius: TREE_RADIUS },
            Transform::from_xyz(at.x, 0.0, at.y),
            Visibility::default(),
        ))
        .with_children(|tree| {
            tree.spawn((
                Mesh3d(look.trunk.clone()),
                MeshMaterial3d(look.bark.clone()),
                Transform::from_xyz(0.0, 1.0, 0.0),
            ));
            tree.spawn((
                Mesh3d(look.canopy.clone()),
                MeshMaterial3d(look.needle.clone()),
                Transform::from_xyz(0.0, 3.0, 0.0),
            ));
        });
}

/// A rock: a squashed sphere, and the same immovable circle.
fn spawn_rock(commands: &mut Commands, look: &Look, at: Vec2, squash: f32) {
    commands
        .spawn((
            Rock,
            Collider { radius: ROCK_RADIUS },
            Transform::from_xyz(at.x, 0.0, at.y),
            Visibility::default(),
        ))
        .with_children(|rock| {
            rock.spawn((
                Mesh3d(look.boulder.clone()),
                MeshMaterial3d(look.stone.clone()),
                Transform::from_xyz(0.0, ROCK_RADIUS * 0.55, 0.0)
                    .with_scale(Vec3::new(squash, 0.62, 1.0 / squash)),
            ));
        });
}

/// A creature: the same split. The entity's `Transform` is position and facing and nothing else,
/// which is what a brain reads and writes; the shape is a child, and G0a swaps it for a `.glb`
/// without a single component changing.
fn radius_of(species: Species) -> f32 {
    match species {
        Species::Beetle => BEETLE_RADIUS,
        Species::Rabbit => RABBIT_RADIUS,
    }
}

fn spawn_creature(commands: &mut Commands, look: &Look, species: Species, at: Vec2, hunger: f32) -> Entity {
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
    match species {
        Species::Beetle => {
            entity.with_children(|body| {
                body.spawn((
                    Mesh3d(look.beetle.clone()),
                    MeshMaterial3d(look.shell.clone()),
                    // a capsule stands up by default; a beetle lies along the way it walks
                    Transform::from_xyz(0.0, 0.32, 0.0)
                        .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                ));
            });
        }
        Species::Rabbit => {
            entity.with_children(|body| {
                body.spawn((
                    Mesh3d(look.rabbit_body.clone()),
                    MeshMaterial3d(look.fur.clone()),
                    Transform::from_xyz(0.0, 0.3, 0.0),
                ));
                for side in [-1.0f32, 1.0] {
                    body.spawn((
                        Mesh3d(look.rabbit_ear.clone()),
                        MeshMaterial3d(look.fur.clone()),
                        Transform::from_xyz(side * 0.14, 0.72, -0.24),
                    ));
                }
            });
        }
    }
    entity.id()
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

/// **The placeholder brain (G0 only).** A heading, held for a second or so, then another one.
/// G1 deletes this system and the `Wander` component with it; what writes `Velocity` from then on
/// is `me[:Velocity] = [vx, vz]` in a Ruby task, and nothing else in this file changes.
fn wander(time: Res<Time>, mut dice: ResMut<Dice>, mut creatures: Query<(&Creature, &mut Wander, &mut Velocity)>) {
    let now = time.elapsed_secs();
    for (creature, mut wander, mut velocity) in &mut creatures {
        if now < wander.until {
            continue;
        }
        wander.until = now + dice.between(0.6, 1.8);
        let heading = dice.between(0.0, std::f32::consts::TAU);
        let speed = creature.species.speed() * dice.between(0.4, 1.0);
        velocity.0 = Vec2::new(heading.cos(), heading.sin()) * speed;
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
    look: Res<Look>,
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
    spawn_plant(&mut commands, &look, at, PLANT_MIN, round);
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
fn eat(
    time: Res<Time>,
    mut commands: Commands,
    mut world: ResMut<ScriptWorld>,
    mut test: Option<ResMut<SelfTest>>,
    mut creatures: Query<(Entity, &Transform, &mut Hunger), With<Creature>>,
    mut plants: Query<(Entity, &Transform, &mut Plant), Without<Creature>>,
) {
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
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
            // the creature's own scripts hear it; the grass has none to hear anything
            world.publish(Some(creature), "ate", Answer::Num((bite * FOOD_VALUE) as f64));
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
}

/// A rabbit walking into a beetle is news to the beetle — the material for G1's `reflex(:touched)`
/// — and it is published once per contact, not once per frame.
fn startle(
    mut world: ResMut<ScriptWorld>,
    mut contacts: ResMut<Contacts>,
    creatures: Query<(Entity, &Creature, &Transform)>,
) {
    let mut now: Vec<(Entity, Entity)> = Vec::new();
    let rabbits: Vec<(Entity, Vec2)> = creatures
        .iter()
        .filter(|(_, c, _)| c.species == Species::Rabbit)
        .map(|(e, _, t)| (e, Vec2::new(t.translation.x, t.translation.z)))
        .collect();
    for (beetle, creature, at) in &creatures {
        if creature.species != Species::Beetle {
            continue;
        }
        let here = Vec2::new(at.translation.x, at.translation.z);
        for (rabbit, there) in &rabbits {
            if here.distance(*there) <= TOUCH_REACH {
                now.push((*rabbit, beetle));
                if !contacts.0.contains(&(*rabbit, beetle)) {
                    world.publish(Some(beetle), "touched", Answer::Entity(*rabbit));
                }
            }
        }
    }
    contacts.0 = now;
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
// Running without a window
// ---------------------------------------------------------------------------------------------

fn stop_when_over(
    time: Res<Time>,
    headless: Res<Headless>,
    sky: Res<Sky>,
    test: Option<Res<SelfTest>>,
    creatures: Query<(&Creature, &Hunger, &Velocity, &Transform)>,
    plants: Query<&Plant>,
    everything: Query<Entity>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    if now < headless.until {
        return;
    }
    for (creature, hunger, velocity, at) in &creatures {
        info!(
            "{:<7} hunger {:>5.1}  age {:>5.1}  at ({:>6.1}, {:>6.1})  v ({:>5.1}, {:>5.1})",
            creature.species.name(),
            hunger.0,
            creature.age,
            at.translation.x,
            at.translation.z,
            velocity.0.x,
            velocity.0.y
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
