# SabiRuby Battle — the DSL every robot file is written in.
# Loaded in front of the robot's own file, so `robot "…" do … end` is all a robot needs.
#
# Everything a robot can do goes through Rubevy.ask: the question is answered by the game, and
# the robot's task is parked on `pop` until it is (a frame, usually). That is why this code
# reads as plain sequential Ruby with no callbacks in it — and why `act` sets every control in
# one question rather than one question each.
#
# The file has two halves. The first is what the game offers, and it is raw on purpose: a tank's
# controls and noisy readings. The second is a library of helpers written on top of it in plain
# Ruby — leading a target, steering, dodging. A robot can use them, copy and change them, or
# ignore them and do its own arithmetic.

# What a robot knows about itself. Angles are radians, 0 pointing right, counterclockwise.
class Status
  attr_reader :x, :y, :hp, :team, :heading, :speed, :turret, :energy, :cooldown, :arena, :time

  def initialize(row)
    @x, @y, @hp, @team, @heading, @speed, @turret, @energy, @cooldown, @arena, @time = row
    @team = @team.to_i
  end

  def alive? = @hp > 0
  def gun_ready? = @cooldown <= 0
end

# Another robot, as the radar saw it. Position and velocity carry the match's noise, more of it
# the further away the robot is.
class Contact
  attr_reader :id, :team, :hp, :x, :y, :vx, :vy, :heading, :turret, :distance, :bearing

  def initialize(row, mine)
    @id, @team, @hp, @x, @y, @vx, @vy, @heading, @turret, @distance, @bearing = row
    @team = @team.to_i
    @mine = mine
  end

  def enemy? = @team != @mine
  def friend? = !enemy?
end

# A shot from another team on its way.
class Shot
  attr_reader :x, :y, :vx, :vy, :distance

  def initialize(row)
    @x, @y, @vx, @vy, @distance = row
  end
end

class Robot
  UNSET = -999.0
  # how fast a shot of a given power flies (the game's numbers, for leading a target)
  SHOT_FAST = 55.0
  SHOT_SLOW = 30.0

  # === what the game answers =================================================

  # Yourself, fresh from the game.
  def status = @me = Status.new(Rubevy.ask("status").pop)

  # Yourself as of the last `status` or `radar` — no question asked.
  def me = @me || status

  # Every other robot still running within `range`, friends included. Also refreshes `me`.
  def radar(range = 60.0)
    rows = Rubevy.ask("radar", range).pop
    @me = Status.new(rows.shift)
    rows.map { |row| Contact.new(row, @me.team) }
  end

  # Shots from other teams within `range`, the nearest first.
  def incoming(range = 25.0) = Rubevy.ask("incoming", range).pop.map { |row| Shot.new(row) }

  # The controls, all in one question. Leave out what should stay as it is.
  #   throttle: -1 (full reverse) .. 1 (full ahead)
  #   turn:     -1 (clockwise) .. 1 (counterclockwise), at the tank's turning rate
  #   aim:      the angle the turret should turn to (it turns at its own rate)
  #   fire:     power 0.2 .. 1 — harder hits, flies slower, reloads longer, costs more energy
  # Returns true when a shot left the barrel.
  def act(throttle: nil, turn: nil, aim: nil, fire: nil)
    fired, _energy, _cooldown = Rubevy.ask("act", throttle || UNSET, turn || UNSET, aim || UNSET, fire || UNSET).pop
    fired > 0
  end

  def drive(throttle, turn = 0.0) = act(throttle: throttle, turn: turn)
  def aim(angle) = act(aim: angle)
  def fire(power = 0.5) = act(fire: power)
  def stop = act(throttle: 0.0, turn: 0.0)

  def log(text) = Rubevy.log("#{name}: #{text}")

  # === what a robot is ======================================================

  def name = self.class.robot_name

  # === reflexes =============================================================
  #
  # `reflex(:hit) { |by, damage| … }` in a robot's class body registers a block that runs in a
  # task of its own, the moment the game says the event happened — not on the brain's next pass.
  # `run_robot` starts one task per reflex before `run`.
  #
  # The controls are last-writer-wins: the brain and a reflex both call `act`, and whichever
  # question the game answers last in a frame is what the tank does. A reflex that wants the
  # wheel for a while has to say so and the brain has to leave it alone — the scout does it with
  # an instance variable, which is shared because both tasks are the same object's.
  # How many reflexes one robot may have. There is a limit because of how a reflex is called:
  # see `run_reflex`.
  REFLEX_SLOTS = 4

  def self.reflexes
    @reflexes ||= []
  end

  def self.reflex(event, &block)
    raise "reflex needs a block" if block.nil?
    slot = reflexes.size
    raise "a robot may have #{REFLEX_SLOTS} reflexes at most" if slot >= REFLEX_SLOTS
    # the block becomes an ordinary method of this robot's class, which is what lets it wait
    define_method("__reflex_#{slot}", &block)
    reflexes << [event.to_sym, slot]
    block
  end

  # Runs reflex `slot` with the arguments the game published.
  #
  # The `case` is not decoration. `instance_exec`, `send` and `Method#call` all run the block in
  # a nested run loop of the VM, and a task cannot be parked across one: the first `act` inside
  # such a block dies with "blocking pop cannot be called from within a C function boundary".
  # An ordinary call written out in Ruby is a frame in this task, which can wait — so the name
  # has to be there in the source, and that is what fixes the number of slots.
  def run_reflex(slot, args)
    case slot
    when 0 then __reflex_0(*args)
    when 1 then __reflex_1(*args)
    when 2 then __reflex_2(*args)
    when 3 then __reflex_3(*args)
    end
  end

  # === a library of helpers, in plain Ruby ======================================

  # the angle from `a` to `b`, the short way round: -PI .. PI
  def angle_diff(a, b)
    d = (b - a) % (Math::PI * 2)
    d > Math::PI ? d - Math::PI * 2 : d
  end

  def angle_to(x, y) = Math.atan2(y - me.y, x - me.x)
  def distance_to(x, y) = Math.sqrt((x - me.x)**2 + (y - me.y)**2)
  def angle_to_center = angle_to(0.0, 0.0)

  # A turn value that brings the hull round to `angle`, gently as it gets close.
  def steer_to(angle) = (angle_diff(me.heading, angle) * 2.0).clamp(-1.0, 1.0)

  def enemies(range = 60.0) = radar(range).select(&:enemy?)
  def nearest_enemy(range = 60.0) = enemies(range).min_by(&:distance)

  def shot_speed(power) = SHOT_FAST + (SHOT_SLOW - SHOT_FAST) * power

  # Where to aim to hit `target` if it keeps going the way it is: one guess at the flight time,
  # corrected once.
  def lead(target, power = 0.5)
    speed = shot_speed(power)
    t = target.distance / speed
    2.times do
      fx = target.x + target.vx * t
      fy = target.y + target.vy * t
      t = distance_to(fx, fy) / speed
    end
    angle_to(target.x + target.vx * t, target.y + target.vy * t)
  end

  # Whether the turret points at `angle` closely enough, and the gun could fire now.
  def aimed?(angle, tolerance = 0.12)
    me.gun_ready? && angle_diff(me.turret, angle).abs < tolerance
  end

  def near_wall?(margin = 6.0)
    limit = me.arena - margin
    me.x.abs > limit || me.y.abs > limit
  end

  # Which way to steer to get out of `shot`'s path: across it, on the side we are already on.
  def dodge_angle(shot)
    along = Math.atan2(shot.vy, shot.vx)
    side = (me.x - shot.x) * -shot.vy + (me.y - shot.y) * shot.vx
    along + (side >= 0 ? Math::PI / 2 : -Math::PI / 2)
  end

  # Whether `shot` will pass within `radius` of us if neither of us changes course.
  def on_collision?(shot, radius = 2.5)
    rx, ry = me.x - shot.x, me.y - shot.y
    v2 = shot.vx**2 + shot.vy**2
    return false if v2 < 0.001
    t = (rx * shot.vx + ry * shot.vy) / v2
    return false if t < 0
    cx, cy = shot.x + shot.vx * t - me.x, shot.y + shot.vy * t - me.y
    cx * cx + cy * cy < radius * radius
  end

  # a turn value that wanders: keeps a direction for a while, then picks another
  def wander_turn(change = 0.04)
    @wander = rand * 2.0 - 1.0 if @wander.nil? || rand < change
    @wander
  end
end

# `robot "Scout" do … end` — makes a subclass of Robot, evaluates the block in it (so `def run`
# lands where it belongs), and remembers it as the one this file defines.
def robot(name, &block)
  klass = Class.new(Robot)
  klass.define_singleton_method(:robot_name) { name }
  klass.class_eval(&block)
  $robot_class = klass
end

# Called by the game after the robot's file has been read.
def run_robot
  tasks = []
  klass = $robot_class
  raise "this file defines no robot" if klass.nil?
  # a robot's `rand` is rolled from the match's dice, so a match with a seed repeats its luck
  srand(Rubevy.ask("seed").pop.to_i)
  bot = klass.new
  start_reflexes(bot, klass, tasks)
  bot.log "online"
  bot.run
rescue => e
  Rubevy.log "#{klass ? klass.robot_name : '?'}: #{e.class}: #{e.message}"
  raise
ensure
  # the reflexes are tasks of their own: nothing else stops them when the brain ends. (A task
  # the game terminates from outside does not run this — see docs/sabiruby-battle.md.)
  tasks.each { |t| t.terminate }
end

# One task per `reflex`, started before the brain.
#
# The subscription has to be taken *here*, in the script's own task: a task made with `Task.new`
# carries no entity, so rubevy refuses to subscribe for it. The queue is an ordinary object, so
# it is simply handed to the block that reads it.
#
# The same missing entity would stop a reflex from asking the game anything at all — `act` from a
# reflex would be a question from nobody — so the new task is given the entity this one carries.
def start_reflexes(bot, klass, tasks)
  here = Task.current
  entity = here.instance_variable_get(:@rubevy_entity)
  # a smaller number is a higher priority: a reflex is looked at before the brain is
  priority = here.priority - 20
  priority = 0 if priority < 0
  klass.reflexes.each do |event, slot|
    queue = Rubevy.subscribe(event)
    task = Task.new(name: "#{klass.robot_name}-#{event}", priority: priority) do
      loop do
        args = queue.pop                       # parked here, costing nothing, until it happens
        Rubevy.log "#{bot.name}: reflex #{event} #{args.inspect}"
        # the HUD's mark. A question nobody waits for is a command: `Rubevy.ask` answers the
        # queue to wait on, and a reflex that never pops it is not parked for a frame
        Rubevy.ask("reflex", :begin)
        begin
          bot.run_reflex(slot, args)
        rescue => e
          Rubevy.log "#{bot.name}: reflex #{event}: #{e.class}: #{e.message}"
        ensure
          Rubevy.ask("reflex", :end)
        end
      end
    end
    # the new task is ready but cannot run before this one yields, so it is safe to hand it the
    # entity here rather than inside it
    task.instance_variable_set(:@rubevy_entity, entity)
    tasks << task
  end
  Rubevy.ask("reflex", :ready, klass.reflexes.size)
  tasks
end
