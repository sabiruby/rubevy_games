//! `Genome` — a Rust struct that is also a Ruby class, and the third way the garden crosses the
//! boundary.
//!
//! The other two are already in `main.rs`. A **component** crosses by reflection: it is a Hash
//! built out of Bevy's type registry, the game writes no code for it, and what Ruby gets is a
//! copy. An **answer** crosses as an `Answer`, which is flat — numbers, strings, a table. This is
//! the third: the value **stays in Rust** and Ruby holds a `Data` object naming it, so
//! `child = my_genome.mix(mate).mutate(0.1)` runs three Rust functions and copies nothing but the
//! object handle.
//!
//! Both views of the same three numbers are live at once, which is the thing to look at:
//!
//! ```ruby
//! me[:Creature][:genome]     # {speed: 2.2, sight: 8.0, appetite: 1.0}  — reflection, a Hash
//! my_genome                  # #<Genome>                                — the Rust value itself
//! my_genome.speed            # 2.2                                      — a Rust fn, called
//! ```
//!
//! The Hash is what every other component read already gives (`Genome` is a field of `Creature`,
//! and `bevy_reflect` walks into a nested struct without being told to). The object comes from
//! `Rubevy.ask("genome")`, answered with `ScriptWorld::answer_value(&req, |vm|
//! genome.into_ruby(vm))` — rubevy hands the host the `&mut Vm`, and from there this is ordinary
//! sabiruby (rubevy `docs/host-api.md`, "Answering with an object of the game's own").
//!
//! Everything below the `#[ruby_methods]` line is written by the macro: the class, the store, the
//! `Data` tag, and one `Vm::define_fn` per method with its arity and its conversions. The same
//! class written by hand is in `docs/garden.md`, with the two line counts beside each other.

use bevy::prelude::*;
use sabiruby::error::VmResult;
use sabiruby::{FromRuby, RubyClass, Value, Vm, ruby_methods};
use serde::{Deserialize, Serialize};

use crate::Species;

/// What a creature is born with: how fast it may go, how far it sees, and how quickly it uses
/// itself up. The Rust rules read these three fields directly (`move_creatures`, the `Sight` a
/// creature is spawned with, `get_hungry`); Ruby reads them through the class below, or as a Hash
/// through `Creature`.
///
/// `Reflect` is what puts it in `me[:Creature][:genome]`; `RubyClass` is what makes it a class.
/// They are independent — one is Bevy's reflection, the other is the VM's host store — and the
/// point of this stage is that a type can be both without either knowing. G3 adds a third, which
/// is neither: `Serialize`/`Deserialize` is what puts the same three numbers in the save file and
/// reads a script's Hash back as this struct (`CreatureSpec` below).
#[derive(Clone, Copy, Debug, Reflect, RubyClass, Serialize, Deserialize)]
#[ruby(name = "Genome")]
pub struct Genome {
    pub speed: f32,
    pub sight: f32,
    pub appetite: f32,
}

/// How far a rolled genome may stray from its species' own, up and down. Small enough that a
/// beetle is still a beetle, large enough that two parents have something to average.
const SPREAD: f32 = 0.18;

impl Genome {
    /// The species' own, which is what the numbers were before G2: a beetle walks at 2.2 and sees
    /// eight, a rabbit walks at 3.4 and sees twelve. `appetite` is new and is 1 for both — it
    /// multiplies `HUNGER_RATE`, so 1 is exactly the world G1 ran.
    pub fn of(species: Species) -> Genome {
        match species {
            Species::Beetle => Genome { speed: 2.2, sight: 8.0, appetite: 1.0 },
            Species::Rabbit => Genome { speed: 3.4, sight: 12.0, appetite: 1.0 },
        }
    }

    /// One of the species', jittered, so that the creatures a world starts with are not clones
    /// and `mix` has two different things to average. The dice are the world's (`Dice`, the
    /// SplitMix64 seeded from the clock), not the VM's: this happens while the world is being
    /// built, long before any script runs.
    /// Three numbers, for a log line on either side of the boundary.
    pub fn describe(&self) -> String {
        format!("speed {:.2}, sight {:.1}, appetite {:.2}", self.speed, self.sight, self.appetite)
    }

    pub fn roll(species: Species, mut jitter: impl FnMut(f32, f32) -> f32) -> Genome {
        let base = Genome::of(species);
        let lo = 1.0 - SPREAD;
        let hi = 1.0 + SPREAD;
        Genome {
            speed: base.speed * jitter(lo, hi),
            sight: base.sight * jitter(lo, hi),
            appetite: base.appetite * jitter(lo, hi),
        }
    }
}

#[ruby_methods]
impl Genome {
    /// `Genome.new(2.2, 8.0, 1.0)` — no `self`, so a class method. This is how a script turns the
    /// Hash it read off another creature back into something with methods on it.
    fn new(speed: f64, sight: f64, appetite: f64) -> Self {
        Genome { speed: speed as f32, sight: sight as f32, appetite: appetite as f32 }
    }

    fn speed(&self) -> f64 {
        self.speed as f64
    }

    fn sight(&self) -> f64 {
        self.sight as f64
    }

    fn appetite(&self) -> f64 {
        self.appetite as f64
    }

    /// The average of two. **The other genome arrives as a raw `Value`**, not as a `&Genome`:
    /// what the macro converts an argument with is `FromRuby`, and there is no `FromRuby` for a
    /// reference — a handle can be *borrowed* out of the store (that is what `&self` is) but a
    /// value cannot be taken out of Ruby, because the object naming it would be left stale
    /// (sabiruby `docs/design/macros.md`, "What it does not cover"). So a method that takes
    /// another object of its own class asks for the `&mut Vm` and does the borrow itself, which
    /// is the same thing `Player::greet` does in the VM's own test.
    ///
    /// One consequence is worth knowing: a `&mut Vm` method has **its own** receiver out of the
    /// store for the duration of the call, so `g.mix(g)` raises `RuntimeError: Genome is already
    /// in use by a call on the same object` rather than quietly averaging something with itself.
    fn mix(&self, vm: &mut Vm, other: Value) -> VmResult<Genome> {
        let other = *Genome::borrow(vm, other)?;
        Ok(Genome {
            speed: (self.speed + other.speed) * 0.5,
            sight: (self.sight + other.sight) * 0.5,
            appetite: (self.appetite + other.appetite) * 0.5,
        })
    }

    /// Every gene multiplied by somewhere in `1 ± rate`. A new `Genome`, not a change in place:
    /// `child = my_genome.mix(mate).mutate(0.1)` reads as arithmetic and the parents keep theirs.
    ///
    /// **The dice are the VM's.** A native is handed a `&mut Vm` and nothing else — it cannot
    /// reach Bevy's `World`, so the game's own `Dice` resource is out of reach (rubevy
    /// `docs/host-api.md`, "Adding to the VM": *do not try to touch Bevy's World from a native*).
    /// What it can reach is `Random`, the VM's own generator, and that is the better answer
    /// anyway: it is the same stream the scripts' `rand` draws from, so a world that wants to be
    /// replayed seeds it once with `srand` and the mutations come out the same — whereas a
    /// private generator seeded from `clock_seed` would be a second source of luck that nothing
    /// in Ruby could steer.
    fn mutate(&self, vm: &mut Vm, rate: f64) -> VmResult<Genome> {
        let rate = rate.clamp(0.0, 1.0) as f32;
        let gene = |value: f32, roll: f64| value * (1.0 + rate * (2.0 * roll as f32 - 1.0));
        let (a, b, c) = (vm_rand(vm)?, vm_rand(vm)?, vm_rand(vm)?);
        Ok(Genome {
            speed: gene(self.speed, a),
            sight: gene(self.sight, b),
            appetite: gene(self.appetite, c),
        })
    }

    /// `{speed: 2.31, sight: 8.4, appetite: 0.97}` — the same shape the reflection of
    /// `me[:Creature][:genome]` has, which is what lets a script hand a child's genome back to
    /// the game as an ordinary Hash (`garden.spawn(genome: child.to_h, …)`). G3 replaces the
    /// reading side of that with `Serde<CreatureSpec>`; the writing side stays this.
    fn to_h(&self, vm: &mut Vm) -> VmResult<Value> {
        let h = vm.hash_new();
        for (name, value) in [("speed", self.speed), ("sight", self.sight), ("appetite", self.appetite)] {
            let key = Value::Sym(vm.intern(name));
            vm.hash_set(h, key, Value::Float(value as f64))?;
        }
        Ok(h)
    }

    /// So that a `log` line in a script says something. It is an ordinary Rust method as well,
    /// which is the other half of what the macros are: the `impl` block is handed back unchanged
    /// and the game calls `describe()` from its own systems.
    fn to_s(&self) -> String {
        self.describe()
    }
}

/// `Random.rand` — a Float in `[0, 1)` from the VM's own generator (mruby-random's PCG, seeded
/// once per creature by the `srand` in `run_creature`). Called rather than reimplemented so that
/// there is exactly one stream of luck on the Ruby side of the game.
fn vm_rand(vm: &mut Vm) -> VmResult<f64> {
    let name = vm.intern("Random");
    let Some(class) = vm.const_get(vm.core.object, name) else { return Ok(0.5) };
    let rand = vm.intern("rand");
    let rolled = vm.funcall(class, rand, &[], Value::Nil)?;
    f64::from_ruby(vm, rolled)
}

/// What a script asked the game to make (`garden.spawn(species:, genome:, at:)`), **as serde
/// reads it out of the Ruby Hash** (G3).
///
/// This is the whole of the reading side. `sabiruby_serde::from_value::<CreatureSpec>(vm, hash)`
/// walks the Hash the script passed and fills these three fields, with the field names as the
/// keys — Symbols or Strings, either way round (sabiruby `docs/design/serde.md`: writing has one
/// shape, reading takes both, which is what makes a struct fit a Hash a script wrote however it
/// felt like) — and `Genome` and `Species` are read by their own derives, nested, without this
/// type saying anything about them.
///
/// `deny_unknown_fields` is the game's decision rather than serde's default: a script that
/// misspells a key would otherwise be told nothing and get a creature with a default in it. What
/// it costs is that the message for a typo is serde's list of the fields there are, which is a
/// better message than the one the hand-written reader had.
///
/// G2 read the same Hash by hand, key by key, with `Vm::hash_entries` and `FromRuby`, in 72 lines
/// (`docs/garden.md`, "The spawn Hash, by hand and by serde"). The clamp below is what is left of
/// them, because it is the only part that was never about reading.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatureSpec {
    pub species: Species,
    pub genome: Genome,
    /// where to put it, on the ground: `[x, z]`
    pub at: [f32; 2],
}

/// What a script asked the game to make, once the game has had its say about it: the spec with
/// the genome clamped, and the creature whose script asked.
#[derive(Debug, Clone)]
pub struct Birth {
    pub species: Species,
    pub genome: Genome,
    pub at: Vec2,
    /// the creature whose script asked, for the log and for the selftest
    pub parent: Option<Entity>,
}

impl CreatureSpec {
    /// A script may hand back anything, and the rules are the game's: a creature that asked for
    /// ten times the speed of its species gets its species' limits. This is the one thing the
    /// hand-written reader did that serde does not — deserializing is about shape, and a range is
    /// a rule.
    pub fn into_birth(self, parent: Option<Entity>) -> Birth {
        let genome = Genome {
            speed: self.genome.speed.clamp(0.2, 8.0),
            sight: self.genome.sight.clamp(1.0, 24.0),
            appetite: self.genome.appetite.clamp(0.2, 4.0),
        };
        Birth { species: self.species, genome, at: Vec2::new(self.at[0], self.at[1]), parent }
    }
}
