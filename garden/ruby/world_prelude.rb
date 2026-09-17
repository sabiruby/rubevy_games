# Garden — the DSL `ruby/world.rb` is written in: the **rules** of the world, in Ruby.
#
# This is the other half of `prelude.rb`. There, a creature's script decides what one creature
# does; here, one script decides what the world does to all of them — the grass growing, hunger,
# eating, starving. It runs in a **second VM** (`struct World` in `garden/src/main.rs`), so it
# shares nothing with the creatures: not the heap, not the globals, not this `Rubevy::Proxy`
# below, which is why the class is written out again rather than required from anywhere.
#
# **One pass of `each_frame` is one frame.** That is the whole shape of this file, and it rests on
# which questions cost a frame and which do not (rubevy `docs/host-api.md`):
#
#   * a component read (`p[:Plant]`), a write (`c[:Hunger] = …`), `Rubevy.find`, `Rubevy.despawn`
#     and `garden.within` all cost **no frame at all** — the tick answers the reads while the
#     script is still running, `find` is one of the four rubevy answers itself, a write and a
#     despawn are commands the script does not wait for, and `garden.within` is a closure the game
#     registered with `answer_in_tick`;
#   * `Rubevy.ask("frame")` is the one question the *game* answers in a system, and it therefore
#     costs exactly one frame. So `run_world` at the foot of this file waits on it, once, and that
#     is what makes the loop turn once per frame — not `sleep 0`, which would run several times in
#     one frame, and not `sleep 1.0/60`, which would drift against the frame it is meant to be.
#
# **A write lands at the end of the frame**, as it always did, so a value written here cannot be
# read back here. The rules are written as `dt` integrations for that reason: everything a pass
# needs about the world it reads once, at the top, and everything it decided it writes once, at
# the bottom.

# ---------------------------------------------------------------------------------------------
# `Rubevy::Proxy`, again.
#
# `ruby/prelude.rb` has one of these too, and this is not a file that could be shared with it: the
# two VMs have two heaps, two sets of constants and two `Rubevy` modules. Copying eleven lines is
# what "the two VMs share nothing" costs, and it is the cheapest place to pay it.
# ---------------------------------------------------------------------------------------------
module Rubevy
  class Proxy
    def initialize(kind)
      @kind = kind
    end

    attr_reader :kind

    def method_missing(name, *args, &blk)
      Rubevy.ask("#{@kind}.#{name}", *args).pop
    end

    def respond_to_missing?(name, include_private = false)
      true
    end

    def inspect
      "#<Rubevy::Proxy #{@kind}>"
    end
    alias to_s inspect
  end
end

# ---------------------------------------------------------------------------------------------
# What a world is.
# ---------------------------------------------------------------------------------------------
class World
  # The entity this script sits on. It is not a creature and has no body; it is where rubevy hangs
  # the task, which is what `Rubevy.ask` and `garden.within` need to know who is asking.
  def me
    @me ||= Rubevy.entity
  end

  # The questions the game answers: `garden.rules`, `garden.sprout`, `garden.count`,
  # `garden.spawn`, and — on the no-frame road — `garden.within`.
  def garden
    @garden ||= Rubevy::Proxy.new("garden")
  end

  # This frame's number, as the game answered it.
  attr_reader :frame

  # **Every plant, every creature — once a frame.**
  #
  # `Rubevy.find` walks the whole world (in the windowed build that is every entity a model
  # brought with it as well), so it is a thing to do now and then, not four times a pass. These
  # two are therefore memoised, and `run_world` throws the memo away at the head of every frame:
  # a rule that wants the list twice in one pass gets the same list, and never the world of two
  # frames ago.
  #
  # It does not cost a frame — `entities.with` is one of the four questions rubevy answers itself,
  # inside the tick — it costs a walk.
  def plants
    @plants ||= Rubevy.find(:Plant)
  end

  def creatures
    @creatures ||= Rubevy.find(:Creature)
  end

  # Put a seed down somewhere: the game picks the spot (and refuses one on top of another plant),
  # because where a plant may be and what a plant is made of are the world's furniture rather than
  # its rules. **It is a command, not a question**: `ask` without a `pop`, so the pass is not
  # parked for a frame waiting to be told where the seed went. A pass that waited would be a frame
  # in which no grass grew and nobody got hungry.
  def sprout
    Rubevy.ask("garden.sprout")
    nil
  end

  # Whether this is the **first** frame of a meal for that creature.
  #
  # A creature standing on a blade of grass takes a bite on every frame of it, and a meal lasts a
  # second or two — so a rule that announced every bite would be sixty messages for one event,
  # which is the queue (rubevy keeps 64) filled by one creature having lunch. The Rust rules kept
  # the same list in an `Eaters` resource and this is that list, in the language the rule is now
  # written in.
  #
  # W1 defines it and nothing calls it: what it is for is `tell c, "ate", …`, which is W2's
  # (`docs/plans/garden-world-plan.md` §4). It is here because the bookkeeping belongs beside the
  # per-frame caches it rides on, not beside the thing that will use it.
  def starting_to_eat?(who)
    bits = who.to_i
    first = !@eating_before.key?(bits)
    @eating_now[bits] = true
    first
  end

  def log(text)
    Rubevy.log "world: #{text}"
  end

  # What the rules are where `world.rb` did not say. A world that defines no `each_frame` is a
  # world that stands still, which is also what a `world.rb` that will not compile leaves behind
  # (the game says so on the HUD rather than refusing to start).
  def each_frame(dt)
  end

  # Called by `run_world` at the head of every frame, before `each_frame`.
  def begin_frame(n)
    @frame = n
    @plants = nil
    @creatures = nil
    @eating_before = @eating_now || {}
    @eating_now = {}
  end

  # === the DSL ===============================================================
  #
  # `each_frame` is `define_method` and not `instance_exec`, for the same reason `on` is in
  # `prelude.rb`: `instance_exec` runs the block in a nested run loop of the VM, and a task cannot
  # be parked across one — the first `Rubevy.ask` inside such a block would die with "blocking pop
  # cannot be called from within a C function boundary". A method defined from the block is an
  # ordinary Ruby frame, which can wait.

  # How long one turn of the sun takes, in seconds. The sun itself is still drawn by Rust
  # (`day_night`), so this is handed over once, at the start, as `garden.rules(day_length:)`.
  def self.day_length(seconds = nil)
    @day_length = seconds.to_f unless seconds.nil?
    @day_length
  end

  def self.each_frame(&block)
    raise "each_frame needs a block" if block.nil?
    define_method(:each_frame, &block)
  end
end

# `world do … end` — a subclass of World with the block evaluated in it, remembered as the one
# this file defines. The shape is `creature "Beetle" do … end`'s, one name shorter: there is only
# ever one world.
def world(&block)
  klass = Class.new(World)
  klass.class_eval(&block)
  $world_class = klass
end

# Called by the game after `world.rb` has been read; the two are compiled as one program, which is
# why neither needs a `require`.
#
# **The dice are seeded from the game's** (W2). sabiruby's default generator starts from a
# constant and the VM has no clock to mix in — `srand` with no argument would mix `gc_clock`,
# which rubevy does not set — so every seeding available from *inside* the VM gives the same
# sequence on every run, and W1 ran a world whose grass came up at the same moments every time.
# The one generator in this game that differs between runs is the game's own `Dice`, seeded from
# the clock (`platform::clock_seed`), so the number `garden.seed` answers is one of its rolls and
# the world's luck is the garden's luck. It costs no frame: the game registered it with
# `answer_in_tick`, like `garden.within`.
def run_world
  klass = $world_class
  raise "this file defines no world" if klass.nil?
  being = klass.new
  # Where the game can find the object, exactly as a creature's task carries its `@being`
  # (`prelude.rb`, `run_creature`). The world keeps no memory in the save file
  # (`docs/plans/garden-world-plan.md` §2), so nothing reads this yet; it costs one line and it is
  # the line that would have to be there.
  Task.current.instance_variable_set(:@being, being)
  srand(being.garden.seed.to_i)

  seconds = klass.day_length
  being.garden.rules(day_length: seconds) unless seconds.nil?

  loop do
    # **The one round trip, and the reason there is one.** The game answers this in
    # `RubevySet::<World>::answer()`, so the task wakes in the next frame's tick: one pass of the
    # loop is one frame, exactly, with nothing to keep in step and nothing to drift.
    n = Rubevy.ask("frame").pop
    break if n.nil?
    being.begin_frame(n)
    # `$rubevy` is refreshed at the head of this VM's own tick, so this is this frame's delta and
    # it costs no question at all.
    being.each_frame($rubevy[:delta])
  end
rescue => e
  Rubevy.log "world: #{e.class}: #{e.message}"
  raise
end
