# SabiRuby Battle — the DSL a match is written in.
#
# A match is a script like a robot is: one task in the same VM, asking the game for what it needs
# and parked while it waits. The difference is what it asks for — it spawns the robots, watches
# what happens to them, and decides when the match is over.

class Match
  TEAMS = %w[red blue green yellow]

  # === the numbers of the game ================================================
  #
  # **Every play number SabiRuby Battle has is here** (S5b-2, 2026-09-21). Until then they were
  # nineteen `const`s and six bare numbers in `sabibots/src/main.rs`, where nobody outside the
  # source could reach them, and two of them — the shot speeds — were written a second time in
  # `prelude.rb` so that a robot could lead a target. Change one of the shot speeds there and the
  # other half of the game quietly disagreed with it.
  #
  # They are the match's now, which is what they always described: how fast a tank is, how hard a
  # shot hits, how wide the field is. The game receives them once, before anybody is on the field,
  # and refuses a match whose numbers make no sense (a tank with no speed, a field smaller than a
  # tank) rather than starting one it cannot play.
  #
  # **None of the values moved.** Each is the number that was in the Rust source, and the comment
  # beside it says where that number came from — which for most of them is *nowhere*: they arrived
  # together in one commit (`978eb34`, "robots become tanks") that says why a robot is a tank and
  # not why it is this fast. `docs/numbers.md` §4 is the full list.
  #
  # A match changes what it likes:
  #
  #     match "Duel", noise: 0.0, numbers: { max_speed: 20.0, arena: 24.0 } do … end
  #
  # A name the game does not know is an error, not a number quietly ignored.
  MODEL = {
    # --- the tank -----------------------------------------------------------
    robot_radius: 1.6,      # how wide a tank is, for hits, for pushing and for the wall. Unknown
    hp_max: 100.0,          # what it starts with and what a full bar means. Unknown
    max_speed: 12.0,        # ahead …
    reverse_speed: 7.0,     # … and back: reverse is slower. Reason only
    turn_rate: 2.6,         # rad/s the hull turns at, at `turn` 1. Reason only
    turret_rate: 4.0,       # rad/s the turret turns at, on its own. Reason only
    grip: 0.2,              # seconds for the speed to catch up with the throttle. Unknown
    out_of_energy: 0.35,    # what is left of the speed when the tank is flat. Unknown

    # --- energy: driving and firing spend it, time gives it back ------------
    energy_max: 100.0,      # reason only: "a robot that fires everything it has cannot also run"
    energy_regen: 12.0,     # per second
    drive_cost: 9.0,        # per second at full throttle
    fire_cost: 16.0,        # a shot costs fire_cost × (fire_cost_base + power)
    fire_cost_base: 0.25,   # unknown

    # --- the gun: power (power_min..1) buys damage and costs speed ----------
    power_min: 0.2,         # the weakest shot worth firing. Unknown
    shot_fast: 55.0,        # how fast the weakest shot flies …
    shot_slow: 30.0,        # … and the heaviest. Reason only; `lead` reads these two
    damage_min: 4.0,        # what the weakest shot takes off …
    damage_max: 16.0,       # … and the heaviest
    cooldown_min: 0.3,      # seconds before the next shot, weakest …
    cooldown_max: 0.8,      # … and heaviest
    base_spread: 0.02,      # rad: even with no noise, a gun is not a laser
    bullet_life: 2.5,       # seconds a shot flies before it is gone. Unknown
    muzzle: 2.8,            # how far in front of the middle a shot appears. Unknown

    # --- what a tank can know, and how far the noise throws it --------------
    radar_range: 60.0,      # the default range of `radar`. Unknown
    incoming_range: 25.0,   # the default range of `incoming`. Unknown
    position_blur: 0.05,    # a contact's place strays by noise × distance × this. Cited
    velocity_blur: 2.0,     # its speed strays by noise × this. Unknown
    shot_blur: 1.5,         # an `incoming` shot's place strays by noise × this. Unknown
    spread_per_noise: 0.1,  # rad of extra spread per unit of noise. Cited

    # --- the field ----------------------------------------------------------
    arena: 32.0,            # half the width of the square, middle to wall. Unknown
    crate_size: 2.6,        # about how far apart the wall's crates stand. Cited: 25 a side
    min_crates: 7.0,        # `shrink` stops here: room for a last fight. Cited
  }

  def initialize(name, noise: 0.0, seed: nil, numbers: {})
    @name = name
    @noise = noise
    @seed = seed
    # the defaults above with whatever this match changed written over them. Built entry by entry
    # rather than with `merge` so that the DSL asks as little of the VM as it can.
    @model = {}
    MODEL.each { |k, v| @model[k] = v }
    numbers.each { |k, v| @model[k] = v }
    @teams = {}
    @rules = []
    @on_destroyed = nil
  end

  # What this match is being played by, after `numbers:` has had its say. A rule written with
  # `rule(…)` may read it — the walls the match shrinks are `crate_size` wide.
  attr_reader :model

  attr_reader :name

  # --- the DSL ---------------------------------------------------------------

  # team :red, robots: %w[scout hunter]
  def team(color, robots:)
    @teams[color.to_s] = robots
  end

  # rule(:sudden_death, after: 20) { ... }             — once, that many seconds in
  # rule(:sudden_death, after: 20, every: 2.0) { ... }  — from then on, every 2 seconds
  def rule(name, after:, every: nil, &body)
    @rules << { name: name.to_s, at: after.to_f, every: every&.to_f, body: body, done: false, runs: 0 }
  end

  def on_destroyed(&body) = @on_destroyed = body

  # --- what the game answers -------------------------------------------------

  def now = Rubevy.ask("clock").pop
  def log(text) = Rubevy.log("match: #{text}")

  # id of the robot that was spawned
  def spawn(file, team, x, y) = Rubevy.ask("spawn", file, team, x, y).pop

  # [[id, team_index, hp, x, y], ...]
  def board = Rubevy.ask("board").pop

  # [[kind, id, other], ...] since the last call; kind 0 = destroyed
  def events = Rubevy.ask("events").pop

  # move every wall in by that many crates (they stop at a small ring); the new half-width
  def shrink(crates = 1) = Rubevy.ask("shrink", crates).pop
  def declare(team) = Rubevy.ask("win", team).pop

  # --- running the match -----------------------------------------------------

  def start
    @ids = {}
    @expected = @teams.values.map(&:size).reduce(0, :+)
    @teams.each_with_index do |(team, robots), t|
      robots.each_with_index do |file, i|
        angle = (t * 2 + i) * 6.28 / (@teams.size * robots.size)
        x = Math.cos(angle) * 20
        y = Math.sin(angle) * 20
        id = spawn(file, team, x, y)
        @ids[id] = [file, team]
        log "#{team} #{file} on the field"
      end
    end
  end

  def run
    log "#{@name} begins"
    # **Before anyone is on the field**: how noisy radars and guns are, the dice, and the numbers
    # the match is played by (S5b-2). Nothing is spawned until this has been answered, which is
    # why the game needs no numbers of its own to fall back on — there is no arena, no wall and no
    # tank until a match has said how big they are.
    #
    # A refusal comes back as a sentence rather than the noise, and it stops the match here: a
    # `numbers:` with a name the game does not know, or with a tank that cannot move, is a mistake
    # to be shown and not a match to be played around.
    answer = Rubevy.ask("rules", @noise.to_f, @seed ? @seed.to_f : -1.0, @model).pop
    raise "the match's numbers were refused: #{answer}" if answer.is_a?(String)
    start
    started = now
    loop do
      events.each do |kind, id, _other|
        next unless kind == 0
        file, team = @ids[id] || ["?", "?"]
        @on_destroyed&.call(file, team)
      end

      field = board
      # the robots appear a frame after they are asked for: no verdict until everyone is here
      if field.size < @expected
        sleep 0.1
        next
      end

      alive = {}
      field.each { |_id, t, hp, _x, _y| alive[TEAMS[t.to_i]] = (alive[TEAMS[t.to_i]] || 0) + 1 if hp > 0 }
      if alive.size <= 1
        winner = alive.keys.first
        log(winner ? "#{winner} wins" : "everyone is down")
        declare(winner || "none")
        return :over
      end

      @rules.each do |r|
        next if r[:done] || now - started < r[:at]
        log "rule: #{r[:name]}" if r[:runs] == 0
        r[:runs] += 1
        if r[:every]
          r[:at] += r[:every]
        else
          r[:done] = true
        end
        r[:body].call(self)
      end

      sleep 0.2
    end
  end
end

# match "Training", noise: 0.3, seed: 7 do … end
#   noise:   0 .. 1, how far radar readings and shots stray (0 is exact, but a gun still spreads)
#   seed:    the same number rolls the same dice; leave it out for a different match every time
#   numbers: what this match changes about the game itself — `Match::MODEL` is the whole list and
#            the defaults, and a name that is not in it is refused rather than ignored
def match(name, noise: 0.0, seed: nil, numbers: {}, &block)
  m = Match.new(name, noise: noise, seed: seed, numbers: numbers)
  m.instance_eval(&block)
  $match = m
end

def run_match
  raise "this file defines no match" if $match.nil?
  $match.run
rescue => e
  Rubevy.log("match: #{e.class}: #{e.message}")
  raise
end
