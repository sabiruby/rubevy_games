# A light robot: circles its enemy, keeps shooting light shots, and gets out of the way of
# anything coming at it.
robot "Scout" do
  # Being hit is news the brain would only hear on its next pass, up to 0.05 s later. This block
  # runs in a task of its own the moment the game says so: throw the wheel over and hold it for a
  # third of a second, so the shot that follows the first one misses.
  #
  # `@swerve` is how the two tasks agree who is steering. They are the same object, so an
  # instance variable is all it takes — and it is needed, because `act` is last-writer-wins and
  # two tasks steering at once would only fight (docs/sabiruby-battle.md).
  reflex(:hit) do |by, damage|
    begin
      @swerve = rand < 0.5 ? 1.0 : -1.0
      # The wheel is held over rather than set once. The brain may already have a question in
      # flight — it looked at `@swerve`, then asked for its radar — and the `act` it makes when
      # that answer arrives would undo this one. Saying it again every 0.05 s wins those frames
      # back, and from its next pass the brain leaves the controls alone by itself.
      6.times do
        act throttle: 1.0, turn: @swerve
        sleep 0.05
      end
    ensure
      @swerve = nil
    end
  end

  def run
    @side = rand < 0.5 ? 1 : -1
    loop do
      # while a reflex has the wheel the brain keeps its hands off it
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
