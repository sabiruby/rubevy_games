# A heavier robot: closes in, then holds its ground and fires.
robot "Hunter" do
  def run
    loop do
      escape = wall_escape
      if escape
        thrust(escape[0], escape[1])
      else
        target = scan(50)
        if target
          _dx, _dy, dist = target
          aim_and_fire(target) if dist < 30
          dist > 16 ? toward(target, 1.0) : strafe(target, 0.5)
        else
          sweep
        end
      end
      sleep 0.08
    end
  end

  def sweep
    @dir ||= 1
    @dir = -@dir if rand < 0.08
    thrust(@dir * 0.8, 0.25)
  end
end
