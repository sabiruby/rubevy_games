# SabiRuby Battle — the DSL a match is written in.
#
# A match is a script like a robot is: one task in the same VM, asking the game for what it needs
# and parked while it waits. The difference is what it asks for — it spawns the robots, watches
# what happens to them, and decides when the match is over.

class Match
  TEAMS = %w[red blue green yellow]

  def initialize(name, noise: 0.0, seed: nil)
    @name = name
    @noise = noise
    @seed = seed
    @teams = {}
    @rules = []
    @on_destroyed = nil
  end

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
    # before anyone is on the field: how noisy radars and guns are, and the dice
    Rubevy.ask("rules", @noise.to_f, @seed ? @seed.to_f : -1.0).pop
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
#   noise: 0 .. 1, how far radar readings and shots stray (0 is exact, but a gun still spreads)
#   seed:  the same number rolls the same dice; leave it out for a different match every time
def match(name, noise: 0.0, seed: nil, &block)
  m = Match.new(name, noise: noise, seed: seed)
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
