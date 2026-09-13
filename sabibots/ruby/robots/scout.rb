# A light robot: circles its enemy at a distance and keeps shooting.
robot "Scout" do
  def run
    loop do
      escape = wall_escape
      if escape
        thrust(escape[0], escape[1])
      else
        target = scan(40)
        if target
          _dx, _dy, dist = target
          aim_and_fire(target)
          if dist < 14
            away_from(target, 1.0)
          else
            strafe(target, 0.9)
          end
        else
          patrol
        end
      end
      sleep 0.05
    end
  end

  def patrol
    @step = (@step || 0) + 1
    angle = @step * 0.25
    thrust(Math.cos(angle) * 0.7, Math.sin(angle) * 0.7)
  end
end
