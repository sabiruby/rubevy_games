# A light robot: circles its enemy, keeps shooting light shots, and gets out of the way of
# anything coming at it.
robot "Scout" do
  def run
    @side = rand < 0.5 ? 1 : -1
    loop do
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
