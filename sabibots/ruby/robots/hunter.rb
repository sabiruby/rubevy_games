# A heavier robot: closes in, then holds its ground and fires hard, slow shots.
robot "Hunter" do
  def run
    loop do
      target = nearest_enemy(60)

      if near_wall?(5)
        act throttle: 0.8, turn: steer_to(angle_to_center)
      elsif target
        # drive at it until it is close, then back off a little and let the gun work
        throttle = target.distance > 18 ? 1.0 : -0.3
        angle = lead(target, 0.9)
        power = aimed?(angle) && target.distance < 34 && me.energy > 40 ? 0.9 : nil
        act throttle: throttle, turn: steer_to(target.bearing), aim: angle, fire: power
      else
        # nothing on the radar: wander, turret sweeping ahead
        act throttle: 0.6, turn: wander_turn, aim: me.heading + Math.sin(me.time * 2) * 0.8
      end
      sleep 0.08
    end
  end
end
