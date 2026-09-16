# Garden — the DSL every creature file is written in.
# Loaded in front of the creature's own file, so `creature "Beetle" do … end` is all a file needs.
#
# This is the opposite of SabiRuby Battle's prelude. There, everything a robot can do is a
# question the game answers (`Rubevy.ask("radar", …)`) and the ECS is never mentioned. Here a
# creature **reads and writes its own components by name**:
#
#     me[:Hunger]                       # → [62.3]
#     me[:Velocity] = [[vx, vz]]
#     plant[:Transform][:translation]   # → [x, y, z]
#
# and the game writes no glue for any of it — a component is reachable because it derives
# `Reflect` and is handed to `register_type`, and for no other reason (rubevy
# `docs/host-api.md`, "Components by name"). The two questions that are *not* a component —
# "what is the nearest plant", "how many are there" — go through `garden`, which is a
# `Rubevy::Proxy`, and they are the only two the game answers.
#
# What a creature remembers (`memory`, G3) is the other way round again: a plain Ruby Hash that
# the ECS knows nothing about, which the game reads out of the VM when the garden is saved.
#
# Every read costs one frame: the question goes out with the frame's commands, the game answers
# it in `RubevySet::Answer`, and the task wakes in the next frame with the answer. The task is
# parked meanwhile and costs nothing, which is why this reads as plain sequential Ruby — but it
# is also why a pass of `run` reads three or four things and then sleeps, rather than reading
# the same thing sixty times a second.

# ---------------------------------------------------------------------------------------------
# `Rubevy::Proxy`, copied from rubevy's `assets/scripts/proxy.rb`.
#
# rubevy ships it as an example script, not in its own prelude, because a proxy is a decision a
# game makes: a real method says what it takes and fails at the call, a proxy accepts every name
# and turns it into a question the game has to recognise. The garden makes that decision for one
# object — `garden` — and for nothing else.
# ---------------------------------------------------------------------------------------------
module Rubevy
  class Proxy
    def initialize(kind)
      @kind = kind
    end

    attr_reader :kind

    # `garden.nearest(:Plant)` is `Rubevy.ask("garden.nearest", :Plant).pop`. It rests on the VM
    # dispatching a Ruby `method_missing` in the frame the call was made in, so the body may park
    # the task on `pop`.
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
# What a creature is.
# ---------------------------------------------------------------------------------------------
class Creature
  TWO_PI = Math::PI * 2

  # A creature may ask for more speed than its body has; `move_creatures` clips it to what the
  # species can do. Asking for the limit and letting the rule decide is the shape of the whole
  # boundary here: the script says what it wants, Rust says what happens.
  CRUISE = 2.0
  DASH = 9.0

  # === the body, as components ==============================================
  #
  # There is no `Rubevy.ask` in any of these. `me[:Hunger]` is a component read, answered by
  # rubevy itself out of Bevy's type registry, and the game never sees the question.

  # The entity this script is attached to. No round trip: rubevy hangs it on the task.
  def me
    @me ||= Rubevy.entity
  end

  # The one thing that is not a component: the two questions only the rules can answer.
  def garden
    @garden ||= Rubevy::Proxy.new("garden")
  end

  def name
    self.class.creature_name
  end

  # Where to go, on the ground. `Velocity` is `Velocity(Vec2)` — a tuple struct whose single
  # field is a `Vec2` — so it reads as `[[vx, vz]]` and is written the same way: an Array over a
  # struct goes by position, and the outer one is the tuple's only element. `vx` is world X and
  # `vz` is world Z; y is up and nothing on the ground touches it.
  #
  # The write is deferred like every component write — it lands after this frame's scripts have
  # run — so two tasks of the same creature that both call `act` are last-writer-wins. **That is
  # why there is a wheel.** A reflex acts in a frame or two; the brain takes four or five, because
  # it reads `me[:Hunger]`, asks `garden.nearest`, reads that plant's `[:Transform]` and only then
  # acts. A reflex that fires in the middle of one of the brain's passes is therefore undone by
  # the `act` at the end of it, a moment later — which looks exactly like a reflex that did not
  # fire. So a reflex takes the wheel, and `act` from anybody else does nothing while it has it.
  #
  # The holder is a `Task`, not a flag, so the reflex's own `act` still goes through and the test
  # is "is this the task that took it" rather than "is anybody busy".
  #
  # It is also **the one place `@course` is set**. `@course` is "where I am going", and a reflex
  # picks the side it swerves to from it (`flee_from` below), so a `@course` that says something
  # the creature is not doing is a reflex that turns the wrong way. Writing it here — and only
  # where the velocity actually went out — is what keeps it true: a `wander` whose `act` was
  # dropped because a reflex has the wheel now changes nothing, where before it left `@course`
  # pointing along a heading nobody ever took.
  def act(vx, vz)
    return if @wheel && @wheel != Task.current
    vx = vx.to_f
    vz = vz.to_f
    me[:Velocity] = [[vx, vz]]
    @course = Math.atan2(vz, vx) unless vx == 0.0 && vz == 0.0
    nil
  end

  # A reflex that wants the wheel for a while says so, and gives it back.
  def take_wheel
    @wheel = Task.current
  end

  def drop_wheel
    @wheel = nil if @wheel == Task.current
  end

  def stop
    act(0.0, 0.0)
  end

  # Each of these is one frame.
  def hunger
    me[:Hunger][0]
  end

  def sight
    me[:Sight][0]
  end

  def here
    me[:Transform][:translation]
  end

  def body
    me[:Creature]
  end

  def species
    body[:species]
  end

  # === the genome: the same three numbers, twice (G2) ========================
  #
  # `Genome` is a Rust struct with two derives on it (`garden/src/genome.rs`). One of them is
  # Bevy's `Reflect`, which is why it is readable as a Hash like any other component field:
  #
  #     me[:Creature][:genome]     # => {speed: 2.31, sight: 8.4, appetite: 0.97}
  #
  # The other is sabiruby's `RubyClass`, which makes it a Ruby class whose objects **are** the
  # Rust value — not a copy of it. `Rubevy.ask("genome")` is answered with one of those, so
  # `my_genome.mix(other).mutate(0.1)` is three Rust functions called from Ruby, and the only
  # thing that crosses the boundary is a handle.
  #
  # Which one to use is a real choice. The Hash is free of ceremony and is already there; the
  # object has the arithmetic in it. This creature uses both: its own genome as the object,
  # because it is going to do sums with it, and a partner's as the Hash, because reading a
  # component of another entity is something it can already do and asking the game a second
  # question would not be.

  # Ours, as the object. It never changes, so it is asked for once.
  def my_genome
    @my_genome ||= Rubevy.ask("genome").pop
  end

  # Somebody else's, out of their `Creature` component and back into a `Genome`. One frame, no
  # question asked of the game, and `Genome.new` is the class method the macro wrote.
  def genome_of(thing)
    body = thing && thing[:Creature]
    genes = body && body[:genome]
    genes && Genome.new(genes[:speed], genes[:sight], genes[:appetite])
  end

  def log(text)
    Rubevy.log "#{name}: #{text}"
  end

  # === what it remembers (G3) ===============================================
  #
  # An ordinary Ruby Hash, on this object, belonging to nobody but the script. The game reads it
  # when the garden is saved — from the outside, without asking, with two `ivar_get`s into the VM
  # (`garden/src/main.rs`, `read_memory`) — and puts it back the same way when the garden is
  # loaded. So a creature does not implement a save format and cannot refuse to be saved; it just
  # remembers things.
  #
  # **The keys are Strings.** A Hash that goes through JSON comes back with String keys — that is
  # what JSON has — so a memory written with Symbol keys would quietly become a *different* Hash
  # after a load, and the creature's `memory[:meals]` would find nothing where `"meals"` sits. The
  # rule is only for `@memory`; everywhere else in the garden a Hash key is a Symbol, because
  # everywhere else it never leaves the VM.
  #
  # It is read lazily, and that matters on the way back in: the object this Hash hangs on does not
  # exist until this file's `run_creature` has run, so the game cannot put a memory back before the
  # script's first frame. It puts it back *during* that frame — and a creature that touched
  # `memory` before then finds the empty Hash below, which the restore then replaces.
  def memory
    @memory ||= {}
  end

  # What the log shows when a creature is asked what it remembers: `JSON` is in the VM because the
  # game put it there (`install_host_api`), not because the VM has one.
  def remember_out_loud
    log JSON.generate(memory)
  end

  # === a library of helpers, in plain Ruby ==================================
  #
  # Arithmetic is not a rule, so it lives here rather than in Rust. A creature file may use
  # these, copy and change them, or do its own sums.

  # Where a thing is: a position is already `[x, y, z]`, an entity has to be asked (one frame).
  #
  # nil where the entity is gone. That is not a rare case: `garden.nearest` answered a frame ago,
  # and in between a rabbit may have eaten the plant or the creature may have starved — the read
  # of a component on an entity that is no longer there answers nil, and so does this.
  def place_of(thing)
    return thing if thing.is_a?(Array)
    return nil if thing.nil?
    tf = thing[:Transform]
    tf && tf[:translation]
  end

  def flat_distance(a, b)
    Math.sqrt((a[0] - b[0])**2 + (a[2] - b[2])**2)
  end

  # the turn from `a` to `b`, the short way round: -PI .. PI
  def angle_diff(a, b)
    d = (b - a) % TWO_PI
    d > Math::PI ? d - TWO_PI : d
  end

  # A heading held for a while, then another one — the walk the Rust placeholder did in G0,
  # written where a walk belongs. `avoid` is a list of places to steer clear of when a new
  # heading is picked, and `from` is where we are (the caller usually knows already, and asking
  # again would cost another frame).
  def wander(change = 0.25, avoid: nil, from: nil)
    course = @course
    if course.nil? || rand < change
      course = rand * TWO_PI
      course = turn_away(course, from || here, avoid) if avoid && !avoid.empty?
    end
    act Math.cos(course) * CRUISE, Math.sin(course) * CRUISE
    course
  end

  # Straight at something, as fast as asked for. Answers the distance that was left, or nil where
  # the thing has gone since it was found.
  def head_to(target, speed = CRUISE)
    from = here
    to = place_of(target)
    return nil if to.nil?
    dx = to[0] - from[0]
    dz = to[2] - from[2]
    span = Math.sqrt(dx * dx + dz * dz)
    if span < 0.01
      stop
      return 0.0
    end
    # `@course` follows from the write: `act` sets it, and only where the write went out.
    act dx / span * speed, dz / span * speed
    span
  end

  # Straight away from something — what a reflex does with what it was handed.
  #
  # `swerve` turns the escape aside by that many radians, **on the side that takes it further
  # from the course it was already on**. A rabbit that is chasing a beetle catches it from
  # behind, and "straight away from a thing behind you" is "carry on": without the swerve the
  # reflex fires, writes a velocity, and the beetle walks on exactly as it was. With it, the
  # heading always changes by at least `swerve` — which is what a startled beetle looks like,
  # and what makes "it turned" a thing that can be checked.
  #
  # "The course it was already on" is `@course`, and the guarantee is only worth as much as that
  # is. `act` is the one thing that sets it (above) and the wheel is taken before this is called,
  # so nothing can move it between the two reads below and the write at the end: the turn away
  # from the heading the creature really has is `|angle to the escape| + swerve`, which is never
  # less than `swerve`. When `@course` could be written by an `act` that never went out, it was
  # sometimes the *wrong* side, and the beetle turned by 39° instead of 57° or more — which is
  # exactly the sixth check's old flakiness (`docs/worklog/2026-09-17-garden-G4.md`).
  def flee_from(thing, speed = DASH, swerve: 0.0)
    from = here
    away = place_of(thing)
    return wander(1.0, from: from) if away.nil?
    dx = from[0] - away[0]
    dz = from[2] - away[2]
    span = Math.sqrt(dx * dx + dz * dz)
    return wander(1.0, from: from) if span < 0.01
    course = Math.atan2(dz, dx)
    if swerve != 0.0
      side = @course.nil? || angle_diff(@course, course) >= 0 ? 1.0 : -1.0
      course += swerve.abs * side
    end
    act Math.cos(course) * speed, Math.sin(course) * speed
    span
  end

  # A heading, turned until it does not point within a quarter turn of anything in `places`
  # that is close. Four tries and then whatever we have — a creature is allowed to walk into a
  # tree, it only bounces off it.
  def turn_away(course, from, places, near = 5.0)
    4.times do
      bad = places.find do |p|
        span = flat_distance(from, p)
        next false if span > near
        diff = (Math.atan2(p[2] - from[2], p[0] - from[0]) - course) % TWO_PI
        diff = diff - TWO_PI if diff > Math::PI
        diff.abs < Math::PI / 4
      end
      return course unless bad
      course = (course + Math::PI / 3) % TWO_PI
    end
    course
  end

  # === reflexes =============================================================
  #
  # `reflex(:touched) { |by| … }` in a creature's body registers a block that runs in a task of
  # its own, the moment the game publishes the event — not on the next pass of `run`.
  # `run_creature` starts one task per reflex before `run`.
  #
  # It is the same shape SabiRuby Battle uses, and for the same reason: `instance_exec`, `send`
  # and `Method#call` all run the block in a nested run loop of the VM, and a task cannot be
  # parked across one — the first `act` inside such a block dies with "blocking pop cannot be
  # called from within a C function boundary". An ordinary call written out in Ruby is a frame
  # in this task, which can wait. So the names have to be in the source, and that is what fixes
  # the number of slots (rubevy_games `docs/worklog/2026-09-16-reflex.md`).
  REFLEX_SLOTS = 6

  def self.reflexes
    @reflexes ||= []
  end

  def self.reflex(event, &block)
    raise "reflex needs a block" if block.nil?
    slot = reflexes.size
    raise "a creature may have #{REFLEX_SLOTS} reflexes at most" if slot >= REFLEX_SLOTS
    define_method("__reflex_#{slot}", &block)
    reflexes << [event.to_sym, slot]
    block
  end

  # A payload the game published: `Answer::Num` and `Answer::Entity` arrive as one value,
  # `Answer::List` as an Array, and a reflex block writes the parameters it wants either way.
  def run_reflex(slot, payload)
    args = payload.is_a?(Array) ? payload : [payload]
    case slot
    when 0 then __reflex_0(*args)
    when 1 then __reflex_1(*args)
    when 2 then __reflex_2(*args)
    when 3 then __reflex_3(*args)
    when 4 then __reflex_4(*args)
    when 5 then __reflex_5(*args)
    end
  end

  # Both tasks are this same object's, so an instance variable is how the brain and a reflex
  # agree about anything. `@asleep` is set by the night reflex and read by `run`; `busy?` is a
  # reflex saying "leave me alone for a moment", and the brain uses it to stop thinking rather
  # than to think and be ignored.
  def asleep?
    @asleep
  end

  def busy?
    !@wheel.nil?
  end
end

# `creature "Beetle" do … end` — a subclass of Creature with the block evaluated in it (so
# `def run` lands where it belongs), remembered as the one this file defines.
def creature(name, &block)
  klass = Class.new(Creature)
  klass.define_singleton_method(:creature_name) { name }
  klass.class_eval(&block)
  $creature_class = klass
end

# Called by the game after the creature's file has been read.
def run_creature
  tasks = []
  klass = $creature_class
  raise "this file defines no creature" if klass.nil?
  being = klass.new
  # The creature, where the game can find it (G3).
  #
  # A task's `self` is the VM's `main` object and **every task shares it**, so a top-level
  # `@ivar` in one creature's file is the same variable in every other creature's — which is why
  # a creature is an object of its own in the first place. The host has the task (rubevy's
  # `ScriptTask`) and nothing else, so the way from a task to the creature has to be laid down
  # here, in one line: the save reads `@being` off the task and `@memory` off that.
  # rubevy hangs the entity on the task in exactly the same way (`@rubevy_entity`).
  Task.current.instance_variable_set(:@being, being)
  # Each creature rolls its own luck, from the bits of its own entity — no question asked, and
  # two beetles spawned in the same frame do not walk in step.
  srand(being.me.to_i)
  start_reflexes(being, klass, tasks)
  # One breath before the first thought (G3).
  #
  # A creature that comes out of a save file is handed its `@memory` by the game, and the game
  # cannot do that until this object exists — which is now, in the middle of this task's first
  # frame. The hand-over happens at the end of that frame (`restore_memory`), so a `run` that
  # started reading `memory` on the line below would read the empty Hash and then have its work
  # replaced. Measured, before this line was here: a loaded rabbit walked the whole garden with
  # `Rubevy.find(:Tree)` to learn what it already remembered.
  #
  # The subscriptions are taken first, above, so nothing published in this moment is missed.
  sleep 0.05
  being.run
rescue => e
  Rubevy.log "#{klass ? klass.creature_name : '?'}: #{e.class}: #{e.message}"
  raise
ensure
  # the reflexes are tasks of their own: nothing else stops them when the brain ends
  tasks.each { |t| t.terminate }
end

# One task per `reflex`, started before the brain.
#
# The subscription is taken here, in the script's own task, because `Rubevy.subscribe` belongs to
# the entity whose task asks; the queue it answers is an ordinary object and is simply handed to
# the block that reads it. A `Task.new` task carries the entity of the task that made it, so a
# reflex may `act` for the creature (rubevy `docs/host-api.md`, "Events").
def start_reflexes(being, klass, tasks)
  here = Task.current
  # a smaller number is a higher priority: a reflex is looked at before the brain
  priority = here.priority - 20
  priority = 0 if priority < 0
  klass.reflexes.each do |event, slot|
    queue = Rubevy.subscribe(event)
    task = Task.new(name: "#{klass.creature_name}-#{event}", priority: priority) do
      begin
        loop do
          args = queue.pop # parked here, costing nothing, until it happens
          begin
            being.run_reflex(slot, args)
          rescue => e
            Rubevy.log "#{being.name}: reflex #{event}: #{e.class}: #{e.message}"
          end
        end
      rescue Rubevy::Unsubscribed
        # the creature is gone — starved, or its file saved. The game let the subscription go
        # and closed the queue, so the `pop` raised instead of parking for ever. This is the
        # ordinary end of a reflex task.
      end
    end
    tasks << task
  end
  tasks
end
