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

**This file describes stages G0, G0a, G1, G2 and G3** (`docs/plans/garden-plan.md`): the world, the
models in it, the two kinds of mind — a Ruby task per creature and a task per reflex — the
`Genome`, a Rust struct that is also a Ruby class, which the creatures mix and mutate to breed, and
the save file, which is the world and every creature's own memory as JSON. G4 puts the editor and
the VM panel in the window, and G5 is the browser build.

```
cargo run -p garden                                     # a window
cargo run -p garden -- --headless 90                    # no window, 90 seconds, the result on stdout
GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90  # and the nine checks
cargo run -p garden -- --headless 30 --save g.json      # and write the garden down at the end
cargo run -p garden -- --load g.json                    # and pick it up again
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
| F5 | write the garden to `garden.save.json` (G3) |
| F9 | read it back |

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
| `Creature` | `species: Species`, `age: f32`, `genome: Genome` | `{species: :Beetle, age: 12.5, genome: {speed: 2.31, sight: 8.4, appetite: 0.97}}` | `age` counts up; `genome` is what the rules read for the top speed, the sight radius and the hunger rate (G2, below) |
| `Hunger` | `f32` (tuple) | `[62.3]` | 100 is full, 0 is dead. Falls by 1.6/s, rises by eating |
| `Velocity` | `Vec2` (tuple) | `[[1.2, -0.7]]` | integrated into `Transform` on XZ, clipped to the creature's top speed, stopped by the walls |
| `Sight` | `f32` (tuple) | `[8.0]` | how far `garden.nearest(:Plant)` looks for the creature that asked. It is set at birth from the creature's own `genome.sight`, which starts near 8 for a beetle and 12 for a rabbit |
| `Memory` | — (unit) | `{}` | that this creature remembers things. It stayed a marker at G3, which is the finding rather than an omission: what it remembers is a Ruby Hash in the VM, and the save file reads it from there (below) rather than keeping a second copy here |
| `Transform` | Bevy's | `{translation: [x, y, z], rotation: [...], scale: [...]}` | position, facing and — for a plant — its size again, as `scale` |

`Species` is a field-less enum and is registered too, so it reads as a Symbol: `:Beetle`,
`:Rabbit`. `Genome` is a struct nested inside `Creature`, and reflection walks into it without
being asked — which is how the same three numbers that a Ruby object has methods for are also a
plain Hash (see **The genome** below). `Transform` is registered by hand because `MinimalPlugins` (the headless run) does not
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
| `court` | two creatures of one species within 2.0 of each other, both over 75 full, neither on a cooldown, and fewer than 24 creatures alive | `"mate"` to **one** of the two, with the other as a `Rubevy::Entity` |
| `starve` | `Hunger` at 0 → `despawn` | |
| `answer_garden` | the four questions a script may ask (below) — in `RubevySet::Answer` | |
| `hatch` | the children `garden.spawn` asked for this frame, and what they cost their parents | |
| `watch_minds` | reads each script's instruction count and the line it stands on | |

and one system runs at `Startup` and never again: `install_genome`, which is
`Genome::register(&mut world.vm)` — the whole of putting a class of the game's own in the VM.

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
  reflex(:mate) do |partner|               # G2: we are both full, and standing together
    child = my_genome.mix(genome_of(partner)).mutate(0.1)
    garden.spawn(species: name, genome: child.to_h, at: [here[0] + 1.2, here[2] + 1.2])
  end

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
(`docs/worklog/2026-09-16-reflex.md`). That is also why there is a fixed number of slots: G1 had
five and the beetle used all five, so G2 made it six, which is one `when` in `run_reflex` and one
`__reflex_5`.

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

## The questions

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

G2 adds two more, and they are the two a *component* could not have been:

```ruby
Rubevy.ask("genome").pop                                     # => #<Genome>, the Rust value itself
garden.spawn(species: name, genome: child.to_h, at: [x, z])  # => true, or a string saying why not
```

`"genome"` is answered with `ScriptWorld::answer_value`, which hands the host the `&mut Vm` so
that the answer can be an **object** rather than one of `Answer`'s flat shapes. `garden.spawn` is
the other direction: its argument is a Hash, which arrives as `Arg::Value` — the Ruby value
itself, not a copy — and the game reads it with `Vm::hash_entries` and `FromRuby`. Both are below.

## The genome (G2)

`Genome` is a Rust struct — three `f32`s — with **two derives on it**, and the two derives are
what this stage is about:

```rust
#[derive(Clone, Copy, Debug, Reflect, RubyClass)]
#[ruby(name = "Genome")]
pub struct Genome { pub speed: f32, pub sight: f32, pub appetite: f32 }
```

`Reflect` is Bevy's and puts it in the type registry, so it is readable from Ruby as a Hash like
any other component field. `RubyClass` is sabiruby's and gives the type a store inside the VM, so
a Ruby object can *name* a Rust value instead of copying it. Neither knows about the other, and
the result is that the same three numbers are visible two ways at once:

```ruby
me[:Creature][:genome]      # => {speed: 2.31, sight: 8.4, appetite: 0.97}   — reflection, a copy
my_genome                   # => #<Genome>                                   — the Rust value
my_genome.speed             # => 2.31                                        — a Rust fn, called
my_genome.mix(other)        # => #<Genome>                                   — and so is this
```

A creature uses both, and which one it uses says something. Its **own** genome it asks for once
(`Rubevy.ask("genome")`) and keeps as the object, because it is going to do arithmetic with it. A
**partner's** it reads as the Hash out of `partner[:Creature]`, because reading a component of
another entity is something it can already do, and asking the game a second question would be a
question the game did not need to answer. `Genome.new(h[:speed], h[:sight], h[:appetite])` turns
the Hash back into something with methods on it — which is the class method the macro wrote.

### What the rules read it for

Every one of the three genes is a number a Rust system had hard-coded per species in G1:

| gene | the rule that reads it | before G2 |
|---|---|---|
| `speed` | `move_creatures` clips `Velocity` to it | `Species::speed()` — 2.2 or 3.4 |
| `sight` | the `Sight` component a creature is spawned with, which `garden.nearest` obeys | `Species::sight()` — 8 or 12 |
| `appetite` | `get_hungry` multiplies `HUNGER_RATE` by it | did not exist |

A creature is born with its species' numbers jittered by up to a sixth either way, so the world
starts with ten different creatures rather than two kinds of clone — and `mix` has something to
average. `appetite` is the price of the other two: a fast, far-sighted creature that also eats
fast is not obviously better off, which is what makes the three numbers something to select rather
than a wish list. A run prints the mean genome of each species at the end.

### The macros, and the same class by hand

Everything under `#[ruby_methods]` is written for the game: the class, its `Data` tag, the store
the values live in, and one `Vm::define_fn` per method with its arity and its conversions
(sabiruby `docs/design/macros.md`). What the game writes is the arithmetic.

The plan asks for the comparison, so the same class was written a second time without the macros
— `Vm::define_closure`, `Vm::install_host_store`, `Vm::data_new`, `Vm::data_of`, by hand — and
**compiled**, because a line count of code that does not build is not a line count. It was a fair
version rather than a straw man: `Genome` is `Copy`, so it copies the value out of the store
instead of borrowing it (no `take_out` / `give_back` pair), and the three getters share one
closure. Then it was deleted; what survives is the number.

| | with the macros | by hand |
|---|---:|---:|
| the struct and the eight methods (`new`, three getters, `mix`, `mutate`, `to_h`, `to_s`) | **61 lines** | **113 lines** |
| installing it in the VM | `Genome::register(&mut world.vm)` | `genome_by_hand::register(&mut world.vm)` |

Lines of code: comments and blank lines are not counted on either side, and the arithmetic — the
bodies of the eight methods, which are the same on both sides — *is*. The 52 lines of difference
are all plumbing: the store and its tag, the class and its singleton class, a `data_new` per value
returned, a `data_of` and a type error per value received, an argument count per method, and a
`FromRuby` per argument. None of it is hard and all of it is the sort of thing that is wrong in
one method out of eight for a week.

Two honest notes on the number. The hand version uses `Vm::define_fn`'s sibling `define_closure`,
which is the raw `|vm, recv, args, blk|` form; `define_fn` is part of the VM and not of the macro
crate, and a host that reached for it would get the argument counting and the conversions back
without any macro, at the cost of naming every parameter's type. And the macro version's 61 lines
include `Genome#to_s` and the `Random.rand` helper, which the hand version also has.

### Breeding: the rule is Rust, the child is Ruby

```ruby
reflex(:mate) do |partner|
  mate = genome_of(partner)                                  # its Creature component, as a Hash
  child = my_genome.mix(mate).mutate(0.1)                    # three Rust methods, called
  garden.spawn(species: name, genome: child.to_h, at: [x, z])
end
```

Rust decides **who may breed with whom**: two creatures of one species, both over 75 full, within
two units of each other, neither on a cooldown, and fewer than twenty-four creatures alive. It
publishes `"mate"` to *one* of the pair — the one with the lower entity id, so that a meeting is
one message and one child rather than two — with the other as a `Rubevy::Entity`.

Ruby decides **what the child is**, and does it by calling Rust: `mix` averages two genomes,
`mutate` multiplies every gene by somewhere in `1 ± rate` using the VM's own `Random`, and `to_h`
turns the result into the Hash the game takes back. Nothing about the child's numbers is written
in `garden/src/main.rs`.

Then Rust again: `garden.spawn` is answered in `RubevySet::Answer`, which reads the Hash and hands
a `Birth` to `hatch`, which makes the creature, gives it a script, and charges both parents
`MATE_COST` from their meter and a twenty-second cooldown. The cost is charged when the **child
arrives**, not when the rule speaks, because whether a creature does anything at all with
`"mate"` is its script's business — the rabbit has no `reflex(:mate)`, hears the message, and
nothing happens.

The population holds itself down without a rule about population: a parent is left under the
fifty-five at which its own script goes looking for grass, so it has to eat its way back up past
seventy-five before it can do this again. The cap of twenty-four is there for the case the rules
do not cover, and in a ninety-second run it is never reached (ten creatures become twelve to
nineteen, with three to six born and some starved).

### What it took in rubevy: nothing

`ScriptWorld::vm` is public and the resource exists as soon as `RubevyPlugin` is added, so
registering the class is an ordinary `Startup` system. `ScriptWorld::answer_value` hands the host
the `&mut Vm`, so `Genome { … }.into_ruby(vm)` is the whole of answering with one. `Arg::Value`
already carries the Hash itself. All three were in rubevy `fa37eaa` before this stage started
(`docs/host-api.md`, "Answering with an object of the game's own" and "Adding to the VM"), and
`tests/host_data.rs` there is this arrangement in twelve frames.

The one thing to know about the store: the value is dropped when the Ruby object naming it is
collected, and that is the VM's doing, not rubevy's — `set_on_free`, which rubevy uses for its own
`Rubevy::Entity`, is a different place. A creature that has asked for its genome holds it for its
life; a child's genome, once read out into a component, is the game's.

## Saving and loading (G3)

**F5** writes the garden to `garden.save.json`, **F9** reads it back; without a window it is
`--save PATH` (written when the run ends) and `--load PATH` (read before the first frame). In the
browser there is no file — `platform::write` puts the same text in `localStorage` under
`garden:garden.save.json`, which is where a saved creature file already goes.

```
cargo run -p garden -- --headless 30 --save garden.save.json   # a garden, written down
cargo run -p garden -- --load garden.save.json                 # and picked up again
```

The file is one struct with `#[derive(Serialize, Deserialize)]` on it (`GardenSave` in
`garden/src/main.rs`), and there is no schema anywhere else. A real one, cut down to one plant and
one creature:

```json
{
  "tick": 30.013807,
  "day_phase": 0.5802301,
  "night": true,
  "plants": [ { "at": [-18.510439, -12.502432], "size": 1.4 } ],
  "trees": [ [-16.647482, -9.710527] ],
  "rocks": [ [-10.369152, 4.2812824] ],
  "creatures": [
    {
      "species": "Beetle",
      "at": [-19.01399, -7.1129208],
      "hunger": 76.82623,
      "age": 30.013792,
      "genome": { "speed": 2.3462436, "sight": 8.815449, "appetite": 0.9204452 },
      "memory": {
        "meals": 3,
        "favorite": { "at": [-17.096527099609375, 1.6607341766357422], "size": 0.8215940594673157 }
      }
    }
  ]
}
```

`species` is a `Species` and `genome` is the same `Genome` the component holds and a script mixes
— three derives on one type now, and none of them knows about the others: `Reflect` for
`me[:Creature][:genome]`, `RubyClass` for the object with `mix` on it, serde for this file.

### The memory is the script's, and the game reads it out of the VM

`memory` is the one field that is not the game's data. It is the Ruby Hash a creature keeps in
`@memory`, and the save reads it **out of the VM without asking the script**:

```rust
let being = vm.ivar_get(task, "@being").obj()?;        // the creature object, on the task
let memory = vm.ivar_get(being, "@memory");            // its Hash
sabiruby_serde::from_value::<serde_json::Value>(vm, memory)?
```

There is no `Rubevy.ask("memory.dump")` and no `to_json` in any creature's file: the save button
does not wake a creature up. Loading is the same two entry points the other way round —
`to_value` builds the Hash again and `ivar_set` hangs it back on the object.

The one line of arrangement it needed is in the prelude:

```ruby
Task.current.instance_variable_set(:@being, being)
```

A task's `self` is the VM's `main` object and **every task shares it**, so a top-level `@ivar` in
one creature's file would be the same variable in every other creature's — which is why a creature
is an object of its own in the first place, and why the host cannot find that object without being
shown it. rubevy hangs the entity on the task in exactly the same way (`@rubevy_entity`). One line
of Ruby, two `ivar_get`s of Rust, and a game can read anything a script remembers.

**Two things about the timing, both of which the round-trip check made visible.** The object a
memory hangs on does not exist until the script's first frame — `run_creature` makes it — so the
restore happens *at the end of that frame*, and a `run` that reads `memory` on its first line
would read an empty Hash and have its work replaced a moment later. The prelude therefore sleeps
once (`sleep 0.05`) between taking the subscriptions and the first thought; before that line was
there, a loaded rabbit walked the whole garden with `Rubevy.find(:Tree)` to learn what it already
remembered. And while a loaded garden's minds are starting, **the world does not move**: the rules
have a run condition, and the world's clock (`Sky::shift`) is held, so the garden that is read back
is the garden that was written down rather than that garden plus two frames of walking.

**The keys of `@memory` are Strings.** JSON has no others, so a Hash written with Symbol keys would
quietly become a different Hash after a load — `memory[:meals]` finding nothing where `"meals"`
now sits. Everywhere else in the garden a Hash key is a Symbol, because everywhere else it never
leaves the VM.

### The spawn Hash, by hand and by serde

`garden.spawn(species:, genome:, at:)` is the one question whose argument has a shape. G2 read it
key by key with `Vm::hash_entries` and `FromRuby`; G3 reads it with one call.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureSpec {
    pub species: Species,
    pub genome: Genome,
    pub at: [f32; 2],
}

// in `answer_garden`, where `read_birth(&mut scripts.vm, asked)` used to be:
sabiruby_serde::from_value::<CreatureSpec>(&mut scripts.vm, asked).map_err(|e| scripts.vm.describe_error(&e))
```

| | by hand (G2) | by serde (G3) |
|---|---:|---:|
| reading the Hash | **72 lines** (`read_birth` 47 + `read_genome` 25) | **7 lines** — the struct |
| the rule about it (the genome clamped to what a body can be) | 3 of those lines | 10 lines (`CreatureSpec::into_birth`) |
| the message for `{speed:, appetite:}` | `"genome: wants speed, sight and appetite; got 2 of them"` | `"missing field \`sight\` (TypeError)"` |
| the message for a key nobody knows | `"spawn: unknown key \"colour\""` | serde's, naming the three fields there are |

Lines of code as G2 counted them: comments and blank lines are not counted, and the 72 were
written to be read rather than to be short. What the 65 that went were doing was matching key
names against strings, turning a Symbol into a `Species` with a `match`, counting how many genes
had turned up, and building a different error sentence for each way of getting it wrong. serde's
derive writes all of that, and it writes the same thing for `Genome` and `Species` without
`CreatureSpec` mentioning them, which is the part that does not show in a line count: the
hand-written reader had a `read_genome` because a nested Hash is a second function, and there is
no `read_species` here at all.

The clamp stayed. Deserializing is about shape, and a range is a rule — `into_birth` is where a
script that asks for ten times its species' speed is told what it is actually getting.

**It is still an `ask`, not a native.** `Serde<T>` is made for `define_fn`, and
`vm.define_fn(garden, "spawn", |spec: Serde<CreatureSpec>| …)` would have been one line shorter
again — but a native is handed the `&mut Vm` and nothing else. It could not count the creatures
already in the garden, could not spawn anything, and could not answer `true` or the reason: it
would have to leave a note behind for a system to pick up, which is exactly what a `Request`
already is. So `garden.spawn` stays in `RubevySet::Answer` with the other questions, and the serde
crate is used for the half it is needed for — reading the argument.

### What is not in the file, and why

A rock's squash and whether a plant is a bush or a tuft are the model's business and are rolled
again on the way in. A breeding cooldown, who bumped into whom last frame, which rabbit is standing
on which beetle are bookkeeping about a *run*, not about a world. `Velocity` is not saved either: a
creature that is picked up again stands still until its brain says otherwise. `Memory` stayed the
empty marker component it has been since G0 — what a creature remembers lives in the VM, and a
component would be a second copy of it with nobody to keep it in step.

### The round trip, measured

```
$ ./target/release/garden --headless 30 --save s1.json
saved 10 creatures, 41 plants at 30.0 s to s1.json (11338 bytes)
$ ./target/release/garden --load s1.json --headless 0 --save s2.json
loaded 10 creatures, 41 plants, 7 trees, 9 rocks at 30.0 s (0 entities made way)
saved 10 creatures, 41 plants at 30.0 s to s2.json (11338 bytes)
$ diff s1.json s2.json && echo IDENTICAL
IDENTICAL
```

Two processes: one builds a world, runs it for thirty seconds and writes it down; the other starts
with nothing, reads that file, hands every creature its memory back and writes the world out again
— and the two files are the same bytes. Positions, hunger, age, every genome, the day's phase and
every `@memory` down to the rabbit that remembers where seven trees are.

It was not the same bytes at first, and the reason is worth keeping. Four numbers differed in the
last digit — `9.595357894897461` came back as `9.59535789489746`, one unit in the last place lower
— and all four were in a rabbit's remembered tree positions, which are the only `f64`s in the file
that are not re-rounded to `f32` on the way in. **serde_json's parser is not exactly the inverse of
its writer** unless it is asked to be: the `float_roundtrip` feature is what makes a parsed float
the nearest one to the text (measured in a four-line program against the same version, 1.0.151).
The writer was always exact; it is the reading side that rounds. The feature costs parsing speed,
which a file read once has none to spare.

F9 — a load into a garden that is *already running*, where everything alive is despawned first —
takes the same `load_world` the command line does, and it was run once headlessly with a throwaway
hook (not committed): seventy-three entities made way, the world came back at the hour it was saved
at, the rabbits had their trees again, and nothing was logged above `INFO`. What has not been
pressed is the keys themselves; this machine has no GPU driver and the window goes through the
container in `docker/` (`docs/wsl-gpu.md`).

## Battle and the Garden, in numbers

| | SabiRuby Battle | the Garden |
|---|---|---|
| `ask` kinds a robot / creature uses | **6** — `status`, `radar`, `incoming`, `act`, `seed`, `reflex` | **4** — `garden.nearest`, `garden.count` (G1), `genome`, `garden.spawn` (G2). G3 added none: the save file goes *round* the scripts, not through them |
| Rust glue per component made visible to Ruby | — (the ECS is never mentioned) | **0 lines** — the `register_type::<T>()` line, and nothing else |
| Rust for one type made into a Ruby class | — (none) | **61 lines** with the macros, **113** by hand |
| the scripts a player writes | `robots/scout.rb` 62 lines (43 without comments and blanks), `robots/hunter.rb` 22 (18) | `creatures/beetle.rb` 128 (68), `creatures/rabbit.rb` 89 (57) |
| the DSL in front of them | `ruby/prelude.rb` 283 (165) | `ruby/prelude.rb` 449 (218) |
| reading one structured argument out of Ruby | — | **72 lines** by hand (G2), **7** with serde (G3) |

The six kinds in Battle are what a *robot* asks; the match script asks eight more (`board`,
`spawn`, `win`, `rules`, `shrink`, `clock`, `events`, `arena`). The Garden had two after G1, and
G2's two are exactly the two that a component read could not have been: one wants an **object of
the game's own** back (`genome`), and the other wants the game to **make something** from a
structured argument (`garden.spawn`). A creature that only wants to know how full it is still
reads `me[:Hunger]`.

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

## Running without a window, and the nine checks

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
   G2 added a fourth exclusion, because G2 added creatures that are **younger than the world**: a
   newborn is spawned with a `Script`, which rubevy turns into a task on a later frame, and that
   task's first act is to subscribe — so for the first moments of a life there is nobody listening
   and an event published then is dropped. A creature under two seconds old is not counted.

   **This check is the flaky one, and it was flaky before G2.** It looks at thirty or forty
   touches in a run and fails when one of them turned by less than 45°. Measured on this machine,
   in ninety-second runs: the merged G1 binary (`a61a611`, with none of G2 in it) failed once in
   four (32/33); this branch failed twice in eight (35/37 and 37/39). The failing case is a beetle
   that *did* change course, by about 39°, which is what the brain's own `wander` looks like — so
   the likeliest reading is that the message was dropped or answered late, not that the reflex is
   broken. It is G1's check and G1's problem; nothing was changed here to chase it.
7. **creatures sleep at night.** One second after `"night"` is published, nothing that has a
   script may still be moving. The fasting beetle has no script and proves nothing, so it is not
   counted.

and one is about the genome (G2), which is the whole round trip in a single line of the log:

8. **a child was born whose genome is its parents', mixed and mutated.** The rule records both
   parents' genomes when it publishes `"mate"`; when the child arrives, every one of its three
   genes has to lie within the mutation rate (a tenth) of the parents' mean, and at least one has
   to differ from both parents — a child exactly on the mean would mean `mutate` did nothing, and
   a child on a parent would mean `mix` did nothing. It fails if the class did not register, if
   the `"mate"` was not heard, if `Genome#mix` or `#mutate` did not run, if `to_h` made the wrong
   Hash, or if `garden.spawn` could not read it.

   Left to chance this is a coin toss, so it is arranged the way the starvation and the probe are:
   two hungry beetles with **different genomes** are put four and a half units either side of a
   tight clump of four plants, in a corner nothing else is within eight units of. Not one rule is
   bent for them. They walk to the grass because they are hungry, eat it because they are standing
   on it, are full because they ate, and are told about each other because they are full and
   standing together. In four consecutive runs the first child arrived at 1.95, 1.96, 2.00 and
   6.42 seconds. Wild pairings happen too, and are counted in the same line.

and one is about the save file (G3):

9. **a spawn Hash with a gene missing names the gene.** The only script in the game that is not a
   creature — four lines of Ruby, in `main.rs` rather than in `ruby/`, started under
   `GARDEN_SELFTEST=1` — asks for `garden.spawn(species: "Beetle", genome: {speed: 2.0,
   appetite: 1.0}, at: [0.0, 0.0])`, and what it is told has to name `sight`. It does, in serde's
   own words and nobody else's: **``missing field `sight` (TypeError)``**. What it is really
   checking is that a Hash of the wrong shape fails *legibly* on the way from a script into a Rust
   struct, which is the half of `sabiruby-serde` this game leans on.

```
$ GARDEN_SELFTEST=1 ./target/release/garden --headless 90
selftest: a beetle with no brain and nothing to eat stands at (-17.0, -12.0)
selftest: a hungry beetle at (17.0, 12.0) with one plant 5.0 away
selftest: two hungry beetles 9.0 apart, with four plants between them at (-14.0, 9.0)
selftest: a tester script will ask for a creature with a gene missing
[script] Tester: a genome with no sight: missing field `sight` (TypeError)
[script] Rabbit: 7 trees, and 41 plants to start with
selftest: first meal at 1.08 s
selftest: the hungry beetle reached its plant at 1.84 s
Beetle 109v0 starved at 1.9 s (age 1.9 s)
[script] Beetle: {"meals":1,"favorite":{"at":[-13.300000190734863,9.199999809265137],"size":1.3832240104675293}}
a Beetle was born at 5.4 s (121v0) — speed 2.17, sight 8.1, appetite 0.98
night at 25.2 s
day at 55.2 s
a Beetle was born at 60.4 s (153v0) — speed 1.97, sight 7.9, appetite 0.96
Beetle 117v0 starved at 66.8 s (age 66.8 s)
a Beetle was born at 83.9 s (169v0) — speed 1.98, sight 7.8, appetite 1.03
night at 85.2 s
Beetle 153v0   hunger  62.6  age  29.6  at (  10.6,   -5.7)  v (  0.0,   0.0)      10 insn/frame  beetle.rb:102
Rabbit 106v0   hunger  54.8  age  90.0  at (  -0.8,    1.5)  v (  0.0,   0.0)      16 insn/frame  rabbit.rb:60
…
19 creatures, 40 plants, night at phase 0.58
15 × Beetle: mean genome speed 2.13, sight 8.0, appetite 0.96 (its species' own is speed 2.20, sight 8.0, appetite 1.00)
4 × Rabbit: mean genome speed 3.29, sight 10.5, appetite 1.04 (its species' own is speed 3.40, sight 12.0, appetite 1.00)
selftest: ok   somebody ate within 10 s (first at 1.08 s)
selftest: ok   night arrived by 60 s (at 25.21 s)
selftest: ok   the starved creature's entity is gone (109v0 starved at 1.89 s)
selftest: ok   nothing walked through anything over 5358 frames (closest pair 0.956 of the radii, 0 frames under 0.9)
selftest: ok   a hungry creature with a plant in sight reached it (from 5.0 away, at 1.84 s)
selftest: ok   a beetle touched by a rabbit changed heading within 0.5 s (42/42)
selftest: ok   the creatures were asleep a second after night fell (15 of them, fastest 0.000 at 26.22 s)
selftest: ok   a child was born whose genome is its parents' mixed and mutated (at 5.39 s: speed 2.166 vs 2.184/2.069, mean 2.127; sight 8.124 vs 8.757/8.667, mean 8.712; appetite 0.985 vs 0.966/1.107, mean 1.037 (mutated off both parents)) [8 pairings, 7 children]
selftest: ok   a spawn Hash with a gene missing names the gene (missing field `sight` (TypeError))
```

Eight runs of that at G2, and four more at G3. The genome check has passed every time — the first
child arrives between 1.95 and 16.47 seconds, with three to seven born in ninety seconds — and so
has everything else except check 6, which failed twice in the eight and once in the four (39/40),
the pre-existing flake described above (the same binary from `main`, with none of G2 or G3 in it,
fails once in four). The G3 check has not failed: all four runs were told the same sentence,
naming the gene that was not there.

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

G4 puts the editor and the VM panel in the window, so that a creature's file can be rewritten
while the garden runs, with the insn/frame and the waiting line above as a panel rather than a log.
G5 is the browser build, where the 314 KiB of models are fetched beside the wasm — and where the
save file above becomes a `localStorage` key, which the code already does and nothing has yet run.
`docs/plans/garden-plan.md` has all of it.
