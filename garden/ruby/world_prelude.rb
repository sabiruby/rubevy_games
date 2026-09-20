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

  # The questions the game answers: `garden.rules`, `garden.sprout`, `garden.tell`,
  # `garden.count`, `garden.spawn`, and — on the no-frame road — `garden.within`, `garden.now`
  # and `garden.seed`.
  #
  # **`garden.now` is the garden's own clock, in seconds**, and not the process's: it stands still
  # while the game is paused and goes on from where it was when a save is read back (`Sky::shift`,
  # `hold_the_clock`). A rule that writes a time into a *component* — which is what a breeding
  # cooldown is — has to write it in a clock that outlives this script, because the component
  # does: `world.rb` can be edited and restarted under a garden that is still running, and an
  # `@elapsed` of this object's own would start again at zero with every edit while the cooldowns
  # it had already written would not.
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

  # **Say something to the creatures** (W2).
  #
  #     tell c, "ate", size          # to one of them
  #     tell :all, "season", "dry"   # to every script that listens for it
  #
  # A script cannot publish: the creatures are a **second VM** and their queues are on the other
  # side of the wall, where only the game can reach. So this is a question the game answers by
  # carrying — `world.rb` → `answer_world` → `ScriptWorld::publish` — and what arrives is
  # indistinguishable from what `startle` and `day_night` publish from Rust, because it *is* that.
  #
  # `who` is `:all` or one creature; `payload` is the one value the handler's block is given, and
  # it may be nothing, a number, a string or a creature.
  #
  # **It is a command, not a question**: `ask` without a `pop`, exactly like `sprout`. A rule that
  # waited to hear that its own announcement had been delivered would park the whole pass for a
  # frame, and in a frame where nothing else happened at all — no grass grew, nobody got hungry.
  # What a `pop` would have bought is the game's opinion of the arguments, and that is a thing to
  # get right once while writing the rule rather than sixty times a second while running it.
  def tell(who, name, payload = nil)
    Rubevy.ask("garden.tell", who, name.to_s, payload)
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
  # What it is for is `tell c, "ate", …` in `world.rb` (W2): the one line that turns a table of
  # who is chewing into a message a beetle's `on(:ate)` hears once per meal.
  def starting_to_eat?(who)
    bits = who.to_i
    first = !@eating_before.key?(bits)
    @eating_now[bits] = true
    first
  end

  def log(text)
    Rubevy.log "world: #{text}"
  end

  # The two numbers the **game** builds a creature with, where `world.rb` does not say what they
  # are: nothing, which leaves the game on its own (`CHILD_HUNGER`, `POP_MAX` in `main.rs`). A
  # world that has breeding rules defines them beside the rest of its breeding numbers.
  def child_hunger = nil
  def pop_max = nil

  # And the two **distances** the game has to know about, for the same reason and with the same
  # default of nothing (`REACH`, `MATE_REACH` in `main.rs`): how far from a blade a creature may
  # eat it, and how close two well-fed creatures have to be before the rules tell them about each
  # other. The game does not use either one to *decide* anything — deciding is what this file is
  # for — it uses them to **place** the selftest's meadow corner, which is the one spot in the
  # garden built so that two beetles certainly meet. Where those distances are is a fact about
  # these rules, so the corner has to follow them rather than keep a copy
  # (`docs/worklog/2026-09-18-corner-and-selftest.md`).
  def reach = nil
  def mate_reach = nil

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

  # **Something that happens now and then rather than every frame** (W2).
  #
  #     every 60 do |n|            # n is 1 the first time, 2 the second
  #       turn_to(n.odd? ? "dry" : "wet")
  #     end
  #
  # Each one gets a **task of its own** in the world's VM, and that task does nothing but
  # `sleep`. There is no counter in `each_frame` adding up deltas and no list of due times to
  # walk: the scheduler already keeps a task that is sleeping for nothing, and the thing that
  # wakes it is the same clock that makes `sleep 0.2` mean something in a creature's `run`.
  #
  # Which also settles what a **pause** does to it. rubevy moves the scheduler's clock on by the
  # frame in the VM's tick, so a frame in which the world's tick does not run — `P`, or a garden
  # being read back from a file — is a frame these tasks do not live through: a season with
  # eleven seconds left when the world stops has eleven seconds left when it starts again. A
  # wall-clock timer would have turned the season over while the player was reading the HUD.
  #
  # `define_method`, not the block called directly, for the same reason `each_frame` is
  # (`instance_exec` and `Method#call` run the block in a nested run loop of the VM, and a task
  # cannot park across one — so the first `tell` inside such a block would die). The name is in
  # the source, which is what fixes the slots.
  def self.every(seconds, &block)
    raise "every needs a block" if block.nil?
    raise "every wants a number of seconds" unless seconds.to_f > 0.0
    slot = timers.size
    raise "a world may have #{EVERY_SLOTS} `every` blocks at most" if slot >= EVERY_SLOTS
    define_method("__every_#{slot}", &block)
    timers << [seconds.to_f, slot]
    block
  end

  def self.timers
    @timers ||= []
  end

  # The turn of one `every`, called by its task. It is written out like `run_handler` in
  # `prelude.rb`, and for the same reason: `send` would be a nested run loop.
  def run_timer(slot, n)
    case slot
    when 0 then __every_0(n)
    when 1 then __every_1(n)
    when 2 then __every_2(n)
    when 3 then __every_3(n)
    end
  end

  # How many `every`s one world may have. Four, because the names have to be written out above
  # and the world this game ships with uses one: it is room to play with in the editor, not a
  # budget anybody measured.
  EVERY_SLOTS = 4
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
  # **Which rules are the VM's rules now** (W2), for the timers below.
  #
  # Every task in this VM shares the globals, so this is the one place the `every` tasks of a
  # *replaced* `world.rb` can look to find out that they have been replaced. They have to look,
  # because nothing else will tell them: when the editor swaps the rules (W3, and the twelfth
  # check already) rubevy terminates the script's own task and drops its subscriptions, and a
  # task the script made with `Task.new` is neither of those. Measured before this line was
  # here — the first `world.rb`'s season turned over at sixty seconds although its rules had
  # been taken away at twenty.
  $world_being = being
  srand(being.garden.seed.to_i)

  # **What the rules keep but the game has to build, draw or sweep with**, handed over once.
  #
  # The sun is drawn in Rust and a creature's body is made in Rust, so those numbers have to
  # cross: how long a day is, what a newborn's meter says, and how many creatures the game will
  # make at all. Then the **distances**, which cross for a fourth thing the game builds: the
  # selftest's meadow corner stands where `reach` and `mate_reach` put it (2026-09-18).
  #
  # And since S5b-4 the numbers of the two **sweeps** — `touch_reach`, which is how close a
  # rabbit has to come for a beetle to be told, and `sprout_gap`, which is how far a new blade
  # has to stand from the ones already up — with the four **bodies**. Those six were the last
  # numbers of the garden's play still written in Rust, and they are here for the reason the top
  # of `world.rb` gives about `garden.within`: the number is a rule, the walk over every pair of
  # things in the field is not something Ruby should be doing sixty times a second. So the rules
  # say the number once and the game does the walking.
  #
  # Everything else in `world.rb` is read in `world.rb`. A world that says nothing about one of
  # these sends `nil` and the game keeps its own (`main.rs`, `RuleBook`).
  #
  # **A world that has never heard of one of them does not send the key at all**, which is what
  # `respond_to?` is for: a `world.rb` saved out of the editor before this stage has no
  # `touch_reach` method, and calling it would end the rules with a `NoMethodError` — a garden
  # in somebody's browser going quiet because the game learned a new number. The older keys are
  # asked for outright, because a `world.rb` without `reach` never ran in any published build.
  #
  # It is the one question in this file that is asked for its answer's sake: a number the game
  # will not take (a day of no seconds) comes back as the sentence saying why, in the log.
  numbers = {
    day_length: klass.day_length,
    child_hunger: being.child_hunger,
    pop_max: being.pop_max,
    reach: being.reach,
    mate_reach: being.mate_reach,
  }
  numbers[:touch_reach] = being.touch_reach if being.respond_to?(:touch_reach)
  numbers[:sprout_gap] = being.sprout_gap if being.respond_to?(:sprout_gap)
  numbers[:beetle_radius] = being.beetle_radius if being.respond_to?(:beetle_radius)
  numbers[:rabbit_radius] = being.rabbit_radius if being.respond_to?(:rabbit_radius)
  numbers[:tree_radius] = being.tree_radius if being.respond_to?(:tree_radius)
  numbers[:rock_radius] = being.rock_radius if being.respond_to?(:rock_radius)
  # the Hash goes over as the one positional argument, which is what the keyword form was
  # anyway: `Rubevy::Proxy#method_missing` takes `*args` and the game reads `request.value(0)`
  answer = being.garden.rules(numbers)
  Rubevy.log "world: #{answer}" unless answer == true

  # One task per `every`, started before the first frame — the shape `start_handlers` gives a
  # creature's `on` (`prelude.rb`), with a `sleep` where that one has a queue.
  timers = start_timers(being, klass)

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
ensure
  # the timers are tasks of their own: nothing else stops them when the rules end, and the rules
  # end every time somebody presses Ctrl+Enter
  (timers || []).each { |t| t.terminate }
end

# The `every` blocks, each in a task that sleeps.
#
# A task made with `Task.new` carries the entity of the task that made it, so a timer may `tell`
# and read components exactly as the main pass does (rubevy `docs/host-api.md`, "Events").
#
# The priority is the world task's own and not a step above it, because these are not reflexes:
# a season turning over half a millisecond later than it might have is a season turning over at
# the right time. What a higher priority would buy is that the timer runs before this frame's
# `each_frame` rather than after it, and there is no rule here that would notice.
def start_timers(being, klass)
  klass.timers.map do |seconds, slot|
    Task.new(name: "world-every-#{seconds}") do
      turn = 0
      loop do
        sleep seconds
        # the rules that made this timer are not the rules any more
        break unless $world_being.equal?(being)
        turn += 1
        begin
          being.run_timer(slot, turn)
        rescue => e
          # a timer that raises is one `every` block going quiet, not the world stopping: the
          # grass keeps growing while the author fixes the season
          Rubevy.log "world: every #{seconds}: #{e.class}: #{e.message}"
        end
      end
    end
  end
end
