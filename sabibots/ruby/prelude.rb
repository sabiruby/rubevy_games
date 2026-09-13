# SabiRuby Battle — the DSL every robot file is written in.
# Loaded in front of the robot's own file, so `robot "…" do … end` is all a robot needs.
#
# Everything a robot can do goes through Rubevy.ask: the question is answered by the game,
# and the robot's task is parked until it is. That is why this code reads as plain sequential
# Ruby with no callbacks in it.

class Robot
  # --- what the game answers -------------------------------------------------

  # [x, y, hp, heading] of this robot
  def me = Rubevy.ask("me").pop

  # the nearest enemy within `range` as [dx, dy, distance], or nil
  def scan(range = 40.0) = Rubevy.ask("scan", range).pop

  # push in a direction (the game caps the speed); returns true
  def thrust(dx, dy) = Rubevy.ask("thrust", dx, dy).pop

  def stop = thrust(0.0, 0.0)

  # fire in a direction; false while the gun is still cooling down
  def fire(dx, dy) = Rubevy.ask("fire", dx, dy).pop

  def log(text) = Rubevy.log("#{name}: #{text}")

  # --- what a robot is -------------------------------------------------------

  def name = self.class.robot_name
  def x = me[0]
  def y = me[1]
  def hp = me[2]
  def alive? = hp > 0

  # Turn towards a target and shoot while it is in front of us. `target` is what `scan` answered.
  def aim_and_fire(target)
    dx, dy, _dist = target
    fire(dx, dy)
  end

  def toward(target, speed = 1.0)
    dx, dy, dist = target
    return stop if dist.nil? || dist < 0.001
    thrust(dx / dist * speed, dy / dist * speed)
  end

  # how far the arena reaches from the middle (the game answers, so the DSL need not know)
  def arena = @arena ||= Rubevy.ask("arena").pop

  # nil unless we are close to a wall, in which case: the way back in
  def wall_escape(margin = 6.0)
    px, py = me
    limit = arena - margin
    ex = px > limit ? -1.0 : (px < -limit ? 1.0 : 0.0)
    ey = py > limit ? -1.0 : (py < -limit ? 1.0 : 0.0)
    (ex == 0.0 && ey == 0.0) ? nil : [ex, ey]
  end

  # circle a target rather than walking into it
  def strafe(target, speed = 1.0)
    dx, dy, dist = target
    return stop if dist.nil? || dist < 0.001
    thrust(-dy / dist * speed, dx / dist * speed)
  end

  def away_from(target, speed = 1.0)
    dx, dy, dist = target
    return stop if dist.nil? || dist < 0.001
    thrust(-dx / dist * speed, -dy / dist * speed)
  end
end

# `robot "Scout" do … end` — makes a subclass of Robot, evaluates the block in it (so `def run`
# lands where it belongs), and remembers it as the one this file defines.
def robot(name, &block)
  klass = Class.new(Robot)
  klass.define_singleton_method(:robot_name) { name }
  klass.class_eval(&block)
  Object.const_set(:CURRENT_ROBOT, klass) if Object.const_defined?(:CURRENT_ROBOT) == false
  $robot_class = klass
end

# Called by the game after the robot's file has been read.
def run_robot
  klass = $robot_class
  raise "this file defines no robot" if klass.nil?
  bot = klass.new
  bot.log "online"
  bot.run
rescue => e
  Rubevy.log "#{klass ? klass.robot_name : '?'}: #{e.class}: #{e.message}"
  raise
end
