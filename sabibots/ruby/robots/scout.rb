# A light robot: circles its enemy, keeps shooting light shots, and gets out of the way of
# anything coming at it.
robot "Scout" do
  # Being hit is news the behaviour would only hear on its next pass, up to 0.05 s later. This block
  # runs in a task of its own the moment the game says so: throw the wheel over and hold it for a
  # third of a second, so the shot that follows the first one misses.
  #
  # One `act` is enough to throw it. `act` is last-writer-wins and the handler task runs at a
  # *lower* priority than the behaviour, so within a frame the handler asks last and the game answers
  # it last (docs/sabiruby-battle.md). It used to have to say it again every 0.05 s to win the
  # wheel back from a behaviour that was ahead of it in the frame.
  #
  # `@swerve` is how the two tasks agree who is steering. They are the same object, so an
  # instance variable is all it takes — the behaviour leaves the controls alone while it is set, so
  # that the decision it already had in flight when the hit landed does not undo the swerve.
  on(:hit) do |by, damage|
    begin
      @swerve = rand < 0.5 ? 1.0 : -1.0
      act throttle: 1.0, turn: @swerve
      sleep 0.3
    ensure
      @swerve = nil
    end
  end

  def run
    @side = rand < 0.5 ? 1 : -1
    loop do
      # while a handler has the wheel the behaviour keeps its hands off it
      if @swerve
        sleep 0.05
        next
      end

      target = nearest_enemy(45)
      threat = incoming(18).find { |shot| on_collision?(shot) }

      if near_wall?(6)
        heading, throttle = angle_to_center, 1.0
      elsif threat
        heading, throttle = dodge_angle(threat), 1.0
      elsif target
        # keep the enemy off to one side and go round it; now and then, the other way
        @side = -@side if rand < 0.01
        heading = target.bearing + Math::PI / 2 * @side
        throttle = target.distance < 14 ? 0.7 : 1.0
      else
        heading, throttle = me.heading + wander_turn, 0.7
      end

      # The agreement is checked again *here*, where the controls are actually touched. A hit can
      # land while the behaviour is asking its questions, and this decision was made before the
      # handler took the wheel: acting on it now would undo the swerve a few frames after it
      # started. Checking only at the top of the loop is a frame too early.
      next sleep(0.05) if @swerve

      if target
        angle = lead(target, 0.3)
        act throttle: throttle, turn: steer_to(heading), aim: angle,
            fire: (aimed?(angle, 0.2) && me.energy > 25) ? 0.3 : nil
      else
        act throttle: throttle, turn: steer_to(heading), aim: me.heading
      end
      sleep 0.05
    end
  end
end
