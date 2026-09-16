# Garden

The second sample in this repository, and the opposite of SabiRuby Battle. In Battle the rules are
Rust and a robot's brain talks to them through one string channel (`Rubevy.ask`); the ECS is never
mentioned on the Ruby side. Here **the Ruby reads and writes the ECS components themselves, by
name** — `me[:Hunger]`, `plant[:Transform][:translation]`, `me[:Velocity] = [vx, vz]` — and the
game writes no glue for any of it.

It is 3D: a plane of grass, creatures on it, trees and rocks they cannot walk through, and one sun
that goes round once a minute and casts the shadows. That is Bevy being Bevy; what it costs the Ruby side is two lines, and they are listed
below.

![the garden at 22 seconds: evening, long shadows, grass, trees, rocks, four rabbits and six beetles](garden.png)

**This file describes stage G0** (`docs/plans/garden-plan.md`), which is the world without any
script in it. G1 puts the minds in.

```
cargo run -p garden                                     # a window
cargo run -p garden -- --headless 90                    # no window, 90 seconds, the result on stdout
GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90  # and the three checks
cargo run -p garden -- --shot docs/garden.png 22        # a window, one picture at 22 s, and out
```

On WSL without a GPU driver the windowed mode and `--shot` go through the container in `docker/`
(`docs/wsl-gpu.md`); `--headless` needs nothing.

## Keys

| | |
|---|---|
| drag (either mouse button) | turn the camera round the garden |
| wheel | closer / further away |

There is nothing else yet. The editor and the VM panel (`rubevy-arena`) arrive in G4.

## The components

This table is the Ruby API. Nothing else in `garden/src/main.rs` mentions Ruby except five
`ScriptWorld::publish` calls, and there is no per-component code at all: rubevy walks Bevy's type
registry, so **the whole of what it takes for a component to be readable from Ruby is
`#[derive(Component, Reflect)]`, `#[reflect(Component)]` and `app.register_type::<T>()`**
(rubevy `docs/host-api.md`, "Components by name").

| component | fields | as Ruby sees it | what the rules do with it |
|---|---|---|---|
| `Plant` | `size: f32` | `{size: 0.42}` | grass: grows by 0.06/s up to 1.4, is eaten down, and the entity goes when there is nothing left. No `Collider` — walking into grass is how it is eaten |
| `Tree` | — (unit) | `{}` | an obstacle, not food. Fixed, with a `Collider` |
| `Rock` | — (unit) | `{}` | the same, lower down |
| `Collider` | `radius: f32` | `{radius: 0.4}` | how much room a solid thing takes on the ground. Creatures, trees and rocks have one |
| `Creature` | `species: Species`, `age: f32` | `{species: :Beetle, age: 12.5}` | `species` fixes the top speed and the sight radius (G2 moves that to `Genome`); `age` counts up |
| `Hunger` | `f32` (tuple) | `[62.3]` | 100 is full, 0 is dead. Falls by 1.6/s, rises by eating |
| `Velocity` | `Vec2` (tuple) | `[[1.2, -0.7]]` | integrated into `Transform` on XZ, clipped to the creature's top speed, stopped by the walls |
| `Sight` | `f32` (tuple) | `[8.0]` | how far G1's `garden.nearest(:Plant)` may look. Nothing in G0 reads it |
| `Memory` | — (unit) | `{}` | empty until G3, where it becomes the Ruby `@memory` that is saved. It is here from G0 so the table does not change shape later |
| `Transform` | Bevy's | `{translation: [x, y, z], rotation: [...], scale: [...]}` | position, facing and — for a plant — its size again, as `scale` |

`Species` is a field-less enum and is registered too, so it reads as a Symbol: `:Beetle`,
`:Rabbit`. `Transform` is registered by hand because `MinimalPlugins` (the headless run) does not
register it and `DefaultPlugins` does.

**What is deliberately *not* in the table.** `Wander`, `Sun` and `Fasting` are components of the
game that derive neither `Reflect` nor anything else, and are not registered. A type nobody
registered reads as `nil` from Ruby, answers `false` to `has?` and is absent from `components` —
so *not registering* is the whole of the access control there is, and it is enough. `Wander` in
particular is the placeholder brain (below), and Ruby must not find it.

## The rules

Every rule is a Rust system in `garden/src/main.rs`, in this order each frame:

| system | what it does | what it publishes |
|---|---|---|
| `day_night` | turns the sun, recolours it, the ambient light and the sky | `"night"` / `"day"` to everyone, at the moment it flips |
| `wander` | **the placeholder brain — G1 deletes it** | |
| `move_creatures` | `Velocity` into `Transform` on XZ, top speed, walls, facing | |
| `separate` | nothing walks through anything solid (below) | `"bumped"` to each creature pushed, with the other thing |
| `grow_plants` | `Plant.size` up, and into `Transform.scale` | |
| `sprout_plants` | a new plant now and then, up to 90, not on top of another | |
| `get_hungry` | `Hunger` down, `age` up | |
| `eat` | a distance test; a bite out of the plant, the same into `Hunger` | `"ate"` to that creature, with what the bite was worth |
| `startle` | a rabbit within 1.3 of a beetle | `"touched"` to the beetle, with the rabbit as a `Rubevy::Entity` — once per contact, not once per frame |
| `starve` | `Hunger` at 0 → `despawn` | |

`starve` despawns the entity, and that is all it does about the script that was on it: removing an
entity removes its `ScriptTask`, and rubevy's `on_remove` hook terminates the task in the VM and
closes the queues anything of its was parked on, which raises `Rubevy::Unsubscribed` in a reflex
task instead of leaving it standing for ever (rubevy `docs/host-api.md`, "Events"). Nothing runs a
script in G0; the path is here so G1 does not have to build it.

`publish` to a name nobody has subscribed to does nothing at all, which is why the game may
publish freely — and why all of the above is already wired up with no script in the world.

## Solid things

Creatures, trees and rocks do not pass through one another. There is no physics crate: avian or
rapier would be a megabyte of wasm, a second vocabulary and a set of rules the reader cannot see.
What there is instead is a circle on XZ per solid thing (`Collider { radius }`) and one system that
runs after the move:

* neighbours come out of a grid of 1.6-unit cells (`HashMap<(i32, i32), Vec<usize>>`), so the work
  is the pairs that are actually near each other rather than every pair. At a dozen creatures that
  is a wash; it is written that way because the world is meant to grow;
* a pair whose centres are closer than the sum of their radii is pushed apart until they only
  touch — **half each between two creatures, all of it on the creature when the other thing is a
  tree or a rock**, which is what makes an obstacle an obstacle;
* four passes a frame, because a huddle of three or four takes more than one;
* and then the walls again, since a push can send a creature through one.

Grass has no collider on purpose: walking into grass is eating it.

**`"bumped"`** is published to a creature when a contact *begins*, with the other thing as a
`Rubevy::Entity`, and it is the material for G1's `reflex(:bumped) { turn away }`. The plan says
"on the frame it is pushed"; a creature leaning on a tree is pushed on every frame of the second
and a half its heading lasts, and a hundred messages for one event would only fill the queue
(rubevy keeps 64 and drops the oldest) and fire the reflex over and over. So it is the first of
those frames, the way `"touched"` is.

## The placeholder brain

Until a Ruby task writes `Velocity`, something has to, or the garden is a still life. `wander`
gives each creature a new heading every second or so. It is marked in the source as the thing G1
replaces, and it is the *only* thing G1 replaces: every other system reads `Velocity` and does not
care who wrote it.

A creature spawned without the `Wander` component simply stands where it is. That is how the
selftest arranges a starvation, and it is also, in advance, what a creature with no brain will
look like in G1.

## What 3D costs the Ruby side

Two things, and the plan says two:

* a position is `[x, y, z]` rather than `[x, y]`. Creatures are on the y = 0 plane, so a brain
  uses elements 0 and 2 of `e[:Transform][:translation]`.
* `act(vx, vz)` — `Velocity` is a `Vec2` whose `x` is world X and whose `y` is world Z.

Nothing else. `Transform.rotation` is decided by the rules from the direction of travel, so no
brain has to touch it, and `Transform.scale` carrying a plant's size would have been true in 2D as
well.

The meshes are Bevy's own primitives with a `StandardMaterial` and no assets at all: a `Plane3d`
of 40 × 30 for the ground, a `Cone` or a `Sphere` for a plant, a `Capsule3d` lying down for a
beetle, a `Cuboid` with two smaller ones for a rabbit's ears, a `Cylinder` and a `Cone` for a
tree, a squashed `Sphere` for a rock. Every one of them is a **child** of
the entity that carries the components, so the parent's `Transform` is position, facing and size
and nothing else — which is what keeps the "lie the capsule down" rotation out of what Ruby reads,
and is what lets G0a swap in a `.glb` without a single component changing.

## Day and night

One turn of the sun is 60 seconds; the world starts a little after sunrise. Where the sun stands
decides the direction of the one `DirectionalLight`, its colour (orange low, white high) and its
brightness, the ambient light, and the colour of the sky. At night the light comes from the other
side — a moon, dim and blue — so there are still shadows and the world still looks like a solid
place.

That is what makes `"night"` legible on screen rather than a number in a log: in G1 the creatures
sleep when it arrives, and the picture says so.

## Running without a window, and the three checks

`--headless N` runs exactly the same systems for N seconds with no renderer and prints every
creature, the plant count and where in the day it stopped. `GARDEN_SELFTEST=1` adds the plan's
four checks, printed as `selftest: ok` / `selftest: FAIL` lines the way sabibots does:

1. **somebody ate within 10 s** — the world has to be dense enough, and the contact test has to work.
2. **night arrived by 60 s** — the clock, and the publish on the flip.
3. **the starved creature's entity is gone** — the despawn path.
4. **nothing walked through anything** — over every frame of the run, no two colliders' centres
   came closer than 90% of the sum of their radii, where at least one of the two is a creature.

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90
selftest: a beetle with nothing to eat stands at (-17.0, -12.0)
selftest: first meal at 1.04 s
Beetle 198v0 starved at 1.9 s (age 1.9 s)
night at 25.2 s
Beetle 174v0 starved at 52.9 s (age 52.9 s)
day at 55.2 s
night at 85.2 s
Beetle  hunger  87.6  age  90.0  at (  14.9,    5.1)  v (  0.8,   1.5)
…
9 creatures, 50 plants, night at phase 0.58
selftest: ok   somebody ate within 10 s (first at 1.04 s)
selftest: ok   night arrived by 60 s (at 25.21 s)
selftest: ok   the starved creature's entity is gone (198v0 starved at 1.88 s)
selftest: ok   nothing walked through anything over 5363 frames (closest pair 0.998 of the radii, 0 frames under 0.9)
```

The second starvation in that run is nobody's fixture: a beetle that wandered badly for 53 seconds
and died of it. The world does kill on its own; the fixture is only there so the *check* does not
depend on it happening.

The third one needs a creature that certainly starves, and a creature that wanders at random in a
garden of 55 plants usually does not. So `GARDEN_SELFTEST=1` also puts one beetle in the far
corner with 3 points of hunger left **and no `Wander` component**: it never moves, nothing grows
within 6 units of it, and it is dead in about two seconds. Nothing else about the world changes,
and the beetle itself is an ordinary beetle — what it lacks is a brain.

## What is not here yet

G0a swaps the primitives for light CC0 glTF models. G1 is the point of the whole thing: two kinds
of creature written in Ruby over `Entity#[]`, `Rubevy.find` and `subscribe`, with the reflexes in
tasks of their own. G2 makes a `Genome` class out of a Rust struct with the macros, G3 saves and
loads the world and each creature's `@memory` through serde, G4 puts the editor and the VM panel
in the window, and G5 is the browser build. `docs/plans/garden-plan.md` has all of it.
