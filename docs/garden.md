# Garden

The second sample in this repository, and the opposite of SabiRuby Battle. In Battle the rules are
Rust and a robot's brain talks to them through one string channel (`Rubevy.ask`); the ECS is never
mentioned on the Ruby side. Here **the Ruby reads and writes the ECS components themselves, by
name** — `me[:Hunger]`, `plant[:Transform][:translation]`, `me[:Velocity] = [[vx, vz]]` — and the
game writes no glue for any of it.

It is 3D: a plane of grass, creatures on it, trees and rocks they cannot walk through, and one sun
that goes round once a minute and casts the shadows. That is Bevy being Bevy; what it costs the Ruby side is two lines, and they are listed
below.

![the garden at 22 seconds: evening, long shadows over a green field, Kenney tufts and bushes of grass, five trees, scattered rocks, and rabbits and beetles walking about](garden.png)

**This file describes stages G0, G0a and G1** (`docs/plans/garden-plan.md`): the world, the models
in it, and the two kinds of mind — a Ruby task per creature and a task per reflex. G2 turns a Rust
struct into a `Genome` class, G3 saves the world and each creature's `@memory`, G4 puts the editor
and the VM panel in the window, and G5 is the browser build.

```
cargo run -p garden                                     # a window
cargo run -p garden -- --headless 90                    # no window, 90 seconds, the result on stdout
GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90  # and the seven checks
cargo run -p garden -- --shot docs/garden.png 22        # a window, one picture at 22 s, and out
```

On WSL without a GPU driver the windowed mode and `--shot` go through the container in `docker/`
(`docs/wsl-gpu.md`); `--headless` needs nothing. `docker/run.sh` is written for sabibots and names
its volumes, so the garden's picture above was taken with the same `docker run` spelled out by
hand against volumes of its own.

## Keys

| | |
|---|---|
| drag (either mouse button) | turn the camera round the garden |
| wheel | closer / further away |

There is nothing else yet. The editor and the VM panel (`rubevy-arena`) arrive in G4.

## The components

This table is the Ruby API. Nothing else in `garden/src/main.rs` mentions Ruby except five
`ScriptWorld::publish` calls, the system that answers the two questions below, and the two lines
that hang a compiled script on a creature. There is **no per-component code at all** — not in
rubevy, which walks Bevy's type registry, and not in the game, which resolves the one component
name a script may pass it the same way. So the whole of what it takes for a component to be
readable from Ruby is `#[derive(Component, Reflect)]`, `#[reflect(Component)]` and
`app.register_type::<T>()` (rubevy `docs/host-api.md`, "Components by name"), and that count — one
line, the registration — is the number the plan asks for.

| component | fields | as Ruby sees it | what the rules do with it |
|---|---|---|---|
| `Plant` | `size: f32` | `{size: 0.42}` | grass: grows by 0.06/s up to 1.4, is eaten down, and the entity goes when there is nothing left. No `Collider` — walking into grass is how it is eaten |
| `Tree` | — (unit) | `{}` | an obstacle, not food. Fixed, with a `Collider` |
| `Rock` | — (unit) | `{}` | the same, lower down |
| `Collider` | `radius: f32` | `{radius: 0.4}` | how much room a solid thing takes on the ground. Creatures, trees and rocks have one |
| `Creature` | `species: Species`, `age: f32` | `{species: :Beetle, age: 12.5}` | `species` fixes the top speed and the sight radius (G2 moves that to `Genome`); `age` counts up |
| `Hunger` | `f32` (tuple) | `[62.3]` | 100 is full, 0 is dead. Falls by 1.6/s, rises by eating |
| `Velocity` | `Vec2` (tuple) | `[[1.2, -0.7]]` | integrated into `Transform` on XZ, clipped to the creature's top speed, stopped by the walls |
| `Sight` | `f32` (tuple) | `[8.0]` | how far `garden.nearest(:Plant)` looks for the creature that asked. 8 for a beetle, 12 for a rabbit |
| `Memory` | — (unit) | `{}` | empty until G3, where it becomes the Ruby `@memory` that is saved. It is here from G0 so the table does not change shape later |
| `Transform` | Bevy's | `{translation: [x, y, z], rotation: [...], scale: [...]}` | position, facing and — for a plant — its size again, as `scale` |

`Species` is a field-less enum and is registered too, so it reads as a Symbol: `:Beetle`,
`:Rabbit`. `Transform` is registered by hand because `MinimalPlugins` (the headless run) does not
register it and `DefaultPlugins` does.

**What is deliberately *not* in the table.** `Sun`, `Mind`, `Eating`, `Animated`, `Fasting` and
`Probe` are components of the game that derive neither `Reflect` nor anything else, and are not
registered. A type nobody registered reads as `nil` from Ruby, answers `false` to `has?` and is
absent from `components` — so *not registering* is the whole of the access control there is, and
it is enough. `Mind` (what a script has cost) and `Animated` (which clip its model is playing)
are the interesting two: a creature cannot read its own instruction count, and cannot tell that
it has a model at all.

## The rules

Every rule is a Rust system in `garden/src/main.rs`, in this order each frame:

| system | what it does | what it publishes |
|---|---|---|
| `day_night` | turns the sun, recolours it, the ambient light and the sky | `"night"` / `"day"` to everyone, at the moment it flips |
| `move_creatures` | `Velocity` into `Transform` on XZ, top speed, walls, facing | |
| `separate` | nothing walks through anything solid (below) | `"bumped"` to each creature pushed, with the other thing |
| `grow_plants` | `Plant.size` up, and into `Transform.scale` | |
| `sprout_plants` | a new plant now and then, up to 90, not on top of another | |
| `get_hungry` | `Hunger` down, `age` up | |
| `eat` | a distance test; a bite out of the plant, the same into `Hunger` | `"ate"` to that creature when a meal *begins*, with the size of the plant it has sat down to |
| `startle` | a rabbit within 1.3 of a beetle | `"touched"` to the beetle, with the rabbit as a `Rubevy::Entity` — once per contact, not once per frame |
| `starve` | `Hunger` at 0 → `despawn` | |
| `answer_garden` | the two questions a script may ask (below) — in `RubevySet::Answer` | |
| `watch_minds` | reads each script's instruction count and the line it stands on | |

`starve` despawns the entity, and that is all it does about the script that was on it: removing an
entity removes its `ScriptTask`, and rubevy's `on_remove` hook terminates the task in the VM and
closes the queues anything of its was parked on, which raises `Rubevy::Unsubscribed` in a reflex
task instead of leaving it standing for ever (rubevy `docs/host-api.md`, "Events"). A creature
that starves therefore takes its brain and its four or five reflex tasks with it, and nothing in
this file says a word about that.

`publish` to a name nobody has subscribed to does nothing at all, which is why the game may
publish freely — all five of these were already wired up in G0, when nothing in the world was
listening, and G1 added no `publish` call at all.

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

## The minds

G0 had a `wander` system in Rust that gave each creature a new heading every second, so that the
garden was not a still life. **G1 deleted it, and that is the only Rust it deleted**: every other
system reads `Velocity` and does not care who wrote it, and what writes it now is

```ruby
me[:Velocity] = [[vx, vz]]
```

in a Ruby task. A creature with no script simply stands where it is — which is how the selftest
arranges a starvation, and what a brainless creature looked like in G0 too.

Each creature is `ruby/prelude.rb` (the DSL) followed by `ruby/creatures/beetle.rb` or
`rabbit.rb`, compiled together as one program and hung on the entity as a `Script`, the way
sabibots does it. The whole of a creature's file:

```ruby
creature "Beetle" do
  def hungry_below = 55.0

  reflex(:night)  { |_at| @asleep = true; stop }
  reflex(:day)    { |_at| @asleep = false }
  reflex(:touched) do |by|                 # a rabbit walked over us; `by` is the rabbit
    next if @asleep
    take_wheel
    flee_from by, swerve: 1.0
    sleep 0.5
    drop_wheel
  end
  reflex(:bumped) { |what| … }             # a tree, a rock, another creature
  reflex(:ate)    { |size| … }             # a meal has started, on a plant this big

  def run
    loop do
      if @asleep then stop; sleep 0.5; next end
      if busy? then sleep 0.1; next end
      if hunger < hungry_below
        plant = garden.nearest(:Plant)
        wander if plant.nil? || head_to(plant).nil?
      else
        wander
      end
      sleep 0.2
    end
  end
end
```

`hunger` is `me[:Hunger][0]`, `here` is `me[:Transform][:translation]`, `head_to` reads the
plant's `[:Transform]` — every one of them a component read, answered by rubevy out of Bevy's type
registry, and the game never sees the question. Each costs one frame: the task is parked and the
other creatures keep running, which is why a pass reads three or four things and then sleeps
rather than reading the same thing sixty times a second.

**The reflexes are tasks of their own.** `reflex(:touched) { … }` becomes an ordinary method
(`define_method`) and a `Task.new` that waits on `Rubevy.subscribe(:touched)`; `run_creature`
starts one per reflex before the brain. The block has to become a *named method* rather than
being run with `instance_exec`, because `instance_exec`, `send` and `Method#call` all go through a
nested run loop of the VM and a task cannot be parked across one — the first `act` inside such a
block dies with "blocking pop cannot be called from within a C function boundary"
(`docs/worklog/2026-09-16-reflex.md`). That is also why there is a fixed number of slots.

**The wheel.** Both tasks are the same object's, so they share instance variables — and they share
the body, which is the problem. A component write is last-writer-wins, a reflex acts in a frame or
two, and the brain takes four or five (read hunger, ask for the nearest plant, read *its*
transform, act). A reflex that fires in the middle of one of the brain's passes is therefore
undone by the `act` at the end of it, which looks exactly like a reflex that never fired. So
`act` has a holder:

```ruby
def act(vx, vz)
  return if @wheel && @wheel != Task.current
  me[:Velocity] = [[vx.to_f, vz.to_f]]
end
```

The holder is a `Task`, not a flag, so the reflex's own `act` still goes through. `take_wheel` /
`drop_wheel` are what a reflex says, and `busy?` is what the brain reads so that it stops thinking
rather than thinks and is ignored. SabiRuby Battle has the same problem and leaves it to each
robot; this is the same idea with the bookkeeping moved into the prelude.

## The two questions

Everything a creature wants to know is a component it can read — except two things, and those go
through a `Rubevy::Proxy`:

```ruby
garden = Rubevy::Proxy.new("garden")
garden.nearest(:Plant)     # => a Rubevy::Entity, or nil — Rubevy.ask("garden.nearest", :Plant).pop
garden.count(:Plant)       # => 58.0
```

`nearest` looks no further than the asking creature's own `Sight` and never answers with the asker
itself. **Neither of them has a line of code per component:** the kind arrives as a string and is
resolved the way `e[:Hunger]` is — through `AppTypeRegistry` to a `ReflectComponent`, whose
`contains` says whether an entity has one. `garden.count(:Rock)` works, and so would
`garden.nearest(:Whatever)` the day something registers a `Whatever`, without `answer_garden`
being touched. What is written out in Rust there is the *rule*: how far a creature may see, and
that it does not find itself.

`answer_garden` is in `RubevySet::Answer`, which is the only placement where a question is
answered on the frame it was asked; anywhere else costs a second frame for every round trip, and
until rubevy had the sets, where such a system landed was luck (rubevy `docs/host-api.md`).

`Rubevy.find` is the third way to reach the world, and the rabbit uses it once: at the start it
asks for every `:Tree`, reads each one's position, and keeps the list in `@memory` — which is what
`Rubevy.find` is for (it walks every entity in the world) and what G3 will save.

## Battle and the Garden, in numbers

| | SabiRuby Battle | the Garden |
|---|---|---|
| `ask` kinds a robot / creature uses | **6** — `status`, `radar`, `incoming`, `act`, `seed`, `reflex` | **2** — `garden.nearest`, `garden.count` |
| Rust glue per component made visible to Ruby | — (the ECS is never mentioned) | **0 lines** — the `register_type::<T>()` line, and nothing else |
| the scripts a player writes | `robots/scout.rb` 62 lines (43 without comments and blanks), `robots/hunter.rb` 22 (18) | `creatures/beetle.rb` 89 (51), `creatures/rabbit.rb` 84 (54) |
| the DSL in front of them | `ruby/prelude.rb` 283 (165) | `ruby/prelude.rb` 366 (201) |

The six kinds in Battle are what a *robot* asks; the match script asks eight more (`board`,
`spawn`, `win`, `rules`, `shrink`, `clock`, `events`, `arena`). The Garden has no match script and
no third kind: a creature that wants to know how full it is reads `me[:Hunger]`.

## What 3D costs the Ruby side

Two things, and the plan says two:

* a position is `[x, y, z]` rather than `[x, y]`. Creatures are on the y = 0 plane, so a brain
  uses elements 0 and 2 of `e[:Transform][:translation]`.
* `act(vx, vz)` — `Velocity` is a `Vec2` whose `x` is world X and whose `y` is world Z.

Nothing else. `Transform.rotation` is decided by the rules from the direction of travel, so no
brain has to touch it, and `Transform.scale` carrying a plant's size would have been true in 2D as
well.

What did cost a line, and is worth knowing before writing a creature, is the **shape** a
reflected component has. `Velocity(pub Vec2)` is a tuple struct with one field, and that field is
a `Vec2`, so it reads as `[[1.2, -0.7]]` and is written the same way — an Array over a struct goes
by position, and the outer one is the tuple's only element. `Hunger(pub f32)` is `[62.3]` for the
same reason. The plan's sketch wrote `me[:Velocity] = [vx, vz]`, which is an Array of two numbers
handed to a struct of one field; the prelude spells the real thing once, inside `act`, and no
creature file ever sees it.

## The models

G0 built the world out of Bevy's own primitives — a cone for a plant, a capsule lying down for a
beetle, a cuboid with two ears for a rabbit — and put every one of them in a **child** of the
entity that carries the components. G0a swapped all six for CC0 glTF from Kenney, and **not one
component changed**: the parent's `Transform` is position, facing and size, and the look hangs
underneath it.

| file | triangles | bytes | pack | licence |
|---|---:|---:|---|---|
| `grass.glb` | 132 | 11,496 | Kenney Nature Kit 2.1 | CC0 1.0 |
| `plant_bush.glb` | 32 | 4,396 | Kenney Nature Kit 2.1 | CC0 1.0 |
| `tree_default.glb` | 114 | 9,428 | Kenney Nature Kit 2.1 | CC0 1.0 |
| `rock_smallA.glb` | 16 | 3,044 | Kenney Nature Kit 2.1 | CC0 1.0 |
| `animal-bunny.glb` | 575 | 131,568 | Kenney Cube Pets 2.0 | CC0 1.0 |
| `animal-crab.glb` | 676 | 150,768 | Kenney Cube Pets 2.0 | CC0 1.0 |
| `Textures/colormap.png` | — | 10,915 | Kenney Cube Pets 2.0 | CC0 1.0 |
| **total** | **1,545** | **321,615** (314 KiB) | 7 files | |

The plan's budget is 2 MB and ten files. Each pack's own `License.txt` sits beside the models in
`garden/assets/models/`, and `CREDITS.md` says where they came from and when.

The Nature Kit pieces carry no texture at all — colour is in the material — which is why a tree is
nine kilobytes. The two animals share one 10 KB palette, and they are **node-animated, with no
skeleton**: eight clips each (`static`, `idle`, `walk`, `run`, `eat`, `dance` and two gestures), of
which the garden plays three.

**Cube Pets has no beetle.** Nothing in Kenney's 49 3D kits does. The crab is the nearest thing in
the same pack — a shell, legs and a scuttle — and using it keeps one palette and one set of clip
names for both animals. It is the only file here whose name is not what it is used as.

**Walk, idle, eat.** A loaded model brings its own `AnimationPlayer`; `dress_animations` finds
which creature it belongs to by walking up the hierarchy and gives it that species'
`AnimationGraph`, and `animate_creatures` picks the clip from `Velocity` and the `Eating` mark and
crossfades over 180 ms. **None of this is visible from Ruby**: `Animated`, `Eating` and the graph
derive no `Reflect` and are registered nowhere, so a creature cannot tell that it has a model, let
alone that it is chewing on screen. It is the clearest example in the game of a thing that is
entirely the engine's.

**What a `.glb` costs the type registry.** In bevy 0.19 a `.glb` loads into a `WorldAsset`, placed
with a `WorldAssetRoot` component (`Scene` and `SceneRoot` were renamed when `bevy_scene` became
the next-generation BSN system), and the spawner that turns one into entities **panics on any
type in the loaded world the app has not registered**. Nothing registers Bevy's own types here —
that is the `reflect_auto_register` feature, which registers every type in the binary that derives
`Reflect`. It would be one line, and it would put a few hundred of Bevy's types in front of Ruby;
the registry would stop being something this game decides, which is most of what the garden is
for. So the windowed build names the plumbing a model brings with it, one line each, the way
`Transform` was already named — twenty-three of them: `GlobalTransform`, `TransformTreeChanged`,
`Visibility` and its two shadows, `VisibilityClass`, `Aabb`, `Name`, `ChildOf`, `Children`,
`Mesh3d`, `MeshMaterial3d<StandardMaterial>`, four animation components and the seven `Gltf*`
ones. They were found one at a time, because the panic names one type and then stops.

A script running in the window can therefore read those too — `e[:GlobalTransform]`, `e[:Name]`.
They are Bevy's, not the game's, and the table above is still the whole of what the *game* offers;
`Mind`, `Animated`, `Eating` and the rest derive no `Reflect` and could not be registered even by
accident. The headless build, which is where every check runs, loads no model and registers none
of them, so what the checks see is exactly the table.

And one feature name is worth writing down: `bevy_animation` gives you `AnimationPlayer`, but the
glTF loader only reads the clips out of a file when **`gltf_animation`** is on as well. Without it
`GltfAssetLabel::Animation(1).from_asset(…)` is an asset that does not exist, and the only sign is
one `ERROR` line per clip.

**A headless run loads none of it.** The `Look` resource is built only in the windowed build, and
every spawn takes `Option<&Look>` — the same shape `day_night` uses for `GlobalAmbientLight`,
which the renderer brings and `MinimalPlugins` does not. The world is built identically either
way; only the child that carries the look is missing. Loading glTF without a renderer would mean
adding four more plugins to the headless app and would put Bevy's asset loader inside the checks,
and there is nothing to check: no test looks at a child, and every component a script can see is
on the parent.

That `Option` is load-bearing in the other direction too, and it is why `spawn_world` is ordered
`after(MakeLook)`. Without the ordering the windowed build's `spawn_world` can run *before*
`make_look` and see `None` — and a `None` there is not an error, it is "no models". The result is
a garden with no ground, no trees and no creatures, in which the only things with a model are the
plants that sprouted later, because `sprout_plants` runs in `Update`. It renders, it does not warn,
and it is wrong.

## Day and night

One turn of the sun is 60 seconds; the world starts a little after sunrise. Where the sun stands
decides the direction of the one `DirectionalLight`, its colour (orange low, white high) and its
brightness, the ambient light, and the colour of the sky. At night the light comes from the other
side — a moon, dim and blue — so there are still shadows and the world still looks like a solid
place.

That is what makes `"night"` legible on screen rather than a number in a log: the creatures stop
where they stand when it arrives, and the picture says so.

## Running without a window, and the seven checks

`--headless N` runs exactly the same systems for N seconds with no renderer and prints every
creature — where it is, what it has cost its script, and the line of its own file that script is
standing on. `GARDEN_SELFTEST=1` adds the checks the plan asks for, printed as `selftest: ok` /
`selftest: FAIL` lines the way sabibots does. Four are about the world (G0):

1. **somebody ate within 10 s** — the world has to be dense enough, and the contact test has to work.
2. **night arrived by 60 s** — the clock, and the publish on the flip.
3. **the starved creature's entity is gone** — the despawn path.
4. **nothing walked through anything** — over every frame of the run, no two colliders' centres
   came closer than 90% of the sum of their radii, where at least one of the two is a creature.

and three are about the minds (G1), which means they fail if the Ruby does not run, does not read
its components, does not get its answers, or does not hear an event:

5. **a hungry creature with a plant in sight reaches it.** A beetle is put in a far corner with
   forty points of hunger — under its own script's threshold of fifty-five — and one plant five
   units away, inside a beetle's `Sight` of eight and with nothing else within seven. Nothing in
   Rust moves it. If it arrives, a Ruby task read `me[:Hunger]`, asked `garden.nearest(:Plant)`,
   read the answer's `[:Transform][:translation]` and wrote `me[:Velocity]`.
6. **a beetle touched by a rabbit changes heading within 0.5 s.** `startle` notes which way the
   beetle was going at the moment it published `"touched"`; half a second later the heading must
   be more than 45° off it. Three sorts of touch are not counted, and each of them says something:
   a beetle that was **standing still** (nothing to turn from), a beetle **against a wall**
   (`move_creatures` zeroes the component of `Velocity` that would take a creature through one, so
   a beetle in the corner reads as going due west whatever it does), and a beetle that has been
   **touched at all in the last 1.5 s** — a rabbit that keeps walking into one publishes every
   time the contact is remade, the reflex takes half a second over each message and the rest wait
   in the queue, so half a second after the fourth message the answer is really about the first.
   What is left is a beetle that was walking, in the open, and not already running from anything.
7. **creatures sleep at night.** One second after `"night"` is published, nothing that has a
   script may still be moving. The fasting beetle has no script and proves nothing, so it is not
   counted.

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90
selftest: a beetle with no brain and nothing to eat stands at (-17.0, -12.0)
selftest: a hungry beetle at (17.0, 12.0) with one plant 5.0 away
[script] Rabbit: 7 trees, and 39 plants to start with
selftest: first meal at 0.70 s
selftest: the hungry beetle reached its plant at 1.77 s
Beetle 105v0 starved at 1.9 s (age 1.9 s)
[script] Beetle: meal 1, a plant of 1.4
night at 25.2 s
day at 55.2 s
night at 85.2 s
Beetle 98v0    hunger  82.7  age  90.0  at (   2.6,    8.4)  v (  0.0,   0.0)       6 insn/frame  beetle.rb:67
Rabbit 101v0   hunger  75.3  age  90.0  at (   0.7,   -9.5)  v (  0.0,   0.0)      17 insn/frame  rabbit.rb:55
…
11 creatures, 41 plants, night at phase 0.58
selftest: ok   somebody ate within 10 s (first at 0.70 s)
selftest: ok   night arrived by 60 s (at 25.20 s)
selftest: ok   the starved creature's entity is gone (105v0 starved at 1.89 s)
selftest: ok   nothing walked through anything over 5369 frames (closest pair 0.954 of the radii, 0 frames under 0.9)
selftest: ok   a hungry creature with a plant in sight reached it (from 5.0 away, at 1.77 s)
selftest: ok   a beetle touched by a rabbit changed heading within 0.5 s (32/32)
selftest: ok   the creatures were asleep a second after night fell (11 of them, fastest 0.000 at 26.21 s)
```

Three runs of that, all seven ok each time: the turn check counted 28, 28 and 24 touches and every
one of them turned.

The starvation needs a creature that certainly starves, and a creature with a brain in a garden of
fifty-five plants usually does not. So `GARDEN_SELFTEST=1` puts one beetle in the far corner with
3 points of hunger left **and no script at all**: it never moves, nothing grows within 6 units of
it, and it is dead in about two seconds. Nothing else about the world changes, and the beetle
itself is an ordinary beetle — what it lacks is a brain. (In G0 the same beetle was the one
without the `Wander` component, which was the same thing said in Rust.)

**The last two columns of a creature's line are the VM's.** `insn/frame` is the script's whole
instruction count divided by the frames it has lived — a beetle costs 5 to 8, a rabbit 16 to 18,
because the rabbit asks two questions a pass and the beetle one. It is an average because one
frame's figure is nearly always zero: a creature spends nearly every frame *parked*, on a `sleep`
or on an answer. The other column is where it is parked (`ScriptStats::frames`, walked back to
the first line that belongs to the creature's own file rather than the prelude), which is the
thing this VM can tell a HUD and an engine's usual scripting cannot. At the end of a 90-second run
it is night, so everybody is standing on the `sleep 0.5` of their `@asleep` branch.

## What is not here yet

G2 makes a `Genome` class out of a Rust struct with the macros, and has the creatures mix and
mutate it to breed. G3 saves and loads the world and each creature's `@memory` through serde —
the rabbit already keeps one. G4 puts the editor and the VM panel in the window, so that a
creature's file can be rewritten while the garden runs, with the insn/frame and the waiting line
above as a panel rather than a log. G5 is the browser build, where the 314 KiB of models are
fetched beside the wasm. `docs/plans/garden-plan.md` has all of it.
