# The rules of the garden.
#
# Until 2026-09-17 every line of this was Rust: five systems in `garden/src/main.rs`
# (`grow_plants`, `sprout_plants`, `get_hungry`, `eat`, `starve`) and a dozen `const`s nobody
# outside the source could reach. The rules are here now, and **the numbers with them** — change
# one, press Ctrl+Enter, and the garden goes on running under the new one without stopping.
#
# What is still Rust is the mechanics rather than the rules: where a creature ends up when it is
# pushed (`separate`), how the sun is drawn (`day_night`, on the `day_length` below), who is
# touching whom (`startle`), and the spatial question this file asks every frame —
# `garden.within`, which is a closure walking the world in Rust because 24 creatures against 90
# plants is a loop Ruby should not be writing sixty times a second.
#
# One pass of `each_frame` is one frame; `world_prelude.rb` says why, and why nothing in here may
# read back what it has just written.

world do
  # One turn of the sun, in seconds: the sun spends half of it under the ground and that half is
  # the night. Handed to the game once, at the start, because the sun is drawn there.
  # (`main.rs`, `DAY_LENGTH`.)
  day_length 60.0

  # === the numbers ==========================================================
  #
  # Every one of these was a `const` in `garden/src/main.rs` on the line named beside it, and
  # every one of them is the same number it was: the rules moved languages, they did not change
  # their minds.

  # the grass
  def growth      = 0.06   # PLANT_GROWTH: how much bigger a blade gets per second
  def plant_max   = 1.4    # PLANT_MAX: and how big it may get
  def plant_cap   = 90     # PLANTS_MAX: how many blades the field holds
  def crumb       = 0.02   # `eat`: under this there is nothing left of a plant

  # How many new blades come up per second — **and the one number in this file that is not a
  # `const`'s** (W2).
  #
  # `SPROUT_RATE` was 0.7 all year round. The two seasons are this build's *play*, not a
  # measurement: a garden whose rules can be edited while it runs wants something in it that
  # changes without anybody editing it, and rain is the oldest one there is. The two values sit
  # either side of the 0.7 the garden was tuned at, the same distance from it, so a year of them
  # averages the world W1 measured its budget on; what a season changes is *when* the grass
  # comes, not how much of it there is in the end.
  def sprout_rate = @season == "dry" ? 0.5 : 0.9

  # The seasons, and what saying so means. `@season` is an ordinary instance variable of this
  # object — the world keeps no state in the save file (`docs/plans/garden-world-plan.md` §2,
  # default 10), so a garden read back from F9 opens in the wet season whatever it was in when
  # it was written — and `tell :all` is how every script that cares hears about it.
  def turn_to(season)
    @season = season
    tell :all, "season", season
    log "the #{season} season"
  end

  # A season is a day long, which is the `day_length` above. `n` is 1 the first time this runs,
  # so the first turn is to the dry one and the world opens in the other.
  every 60 do |n|
    turn_to(n.odd? ? "dry" : "wet")
  end

  # being alive
  def hunger_max  = 100.0  # HUNGER_MAX: stuffed. 0 is dead
  def hunger_rate = 1.6    # HUNGER_RATE: per second, times the creature's own `appetite`

  # eating
  def eat_rate    = 1.0    # EAT_RATE: how much plant a mouth takes per second
  def food_value  = 60.0   # FOOD_VALUE: what one unit of plant is worth in hunger
  def reach       = 1.1    # REACH: how close is close enough to eat, plus half the plant's size

  # === the rules ============================================================
  each_frame do |dt|
    # The world opens in the wet season, and says so: a creature that starts before the first
    # `every 60` has gone round still has somewhere to read the season from. It is written this
    # way rather than as a `start do … end` of its own because one line that runs once is not a
    # part of the DSL worth having.
    turn_to("wet") if @season.nil?

    # Three tables, kept between frames and emptied here rather than made again: at 90 plants and
    # sixty frames a second, a Hash a frame is five thousand of them a minute for the collector.
    #
    #   `size`  — what each plant's size will be when this pass is over: grown, then bitten.
    #   `who`   — the entity that size belongs to. `garden.within` answers with the raw bits of an
    #             entity (a row of an `Answer::Rows` is numbers), and this is how a row becomes
    #             something that can be written to.
    #   `changed` — the plants whose size has to be written back at the end: the ones that grew
    #             and the ones that were bitten. One list does both jobs, because a plant that
    #             changed is a plant that grew or was bitten.
    #
    # There used to be a fourth, `bitten`, so that one blade fed one mouth per frame (the plan's
    # §2, default 5). Its reason was that two writes to one plant in one frame would be the
    # second one only — and that stopped being true the moment the sizes were worked out in these
    # tables and written once at the foot of the pass: two mouths at one blade now take two bites
    # out of the same number, which is what Rust's `eat` did (W2, `docs/worklog/…-garden-world.md`).
    size = (@size ||= {}).clear
    who = (@who ||= {}).clear
    changed = (@changed ||= {}).clear

    # --- the grass grows, up to a point -------------------------------------
    cap = plant_max
    taller = growth * dt
    plants.each do |p|
      bits = p.to_i
      who[bits] = p
      s = p[:Plant][:size]
      if s < cap
        s += taller
        s = cap if s > cap
        changed[bits] = true
      end
      size[bits] = s
    end

    # --- hunger, a mouthful of grass, and the end of it ---------------------
    #
    # The order is the order the five Rust systems ran in, and it matters at both ends: a creature
    # whose meter has just gone under zero still eats if there is grass under it (`eat` ran before
    # `starve`), and a creature that ate its way back above zero lives.
    full = hunger_max
    mouthful = eat_rate * dt
    # how far a creature can possibly be from a plant and still be able to eat it: the plant's own
    # size is half of the answer, so the question is asked at the largest a plant may be and the
    # rows that come back are each measured against their own plant
    arm = reach + cap * 0.5
    creatures.each do |c|
      body = c[:Creature]
      next if body.nil? # gone between the list being taken and this line
      h = c[:Hunger][0] - hunger_rate * dt * body[:genome][:appetite]

      if h < full
        # The rows are nearest first, so the first one in reach is the one it would have walked
        # into. (`eat` took the first in the query's own order, which is nobody's order.)
        garden.within(c, arm, :Plant).each do |row|
          bits = row[0].to_i
          s = size[bits]
          # gone, or eaten down to nothing
          next if s.nil? || s <= crumb
          # and in reach of *this* plant, which is wider the bigger the plant is
          next if row[1] > reach + s * 0.5
          bite = mouthful
          bite = s if s < bite
          size[bits] = s - bite
          changed[bits] = true
          h += bite * food_value
          h = full if h > full
          # **The news of a meal** (W2), on the first frame of it and not on the sixtieth: a
          # mouthful is taken on every frame of a meal, and a rule that announced each one would
          # fill the sixty-four the queue holds with one beetle having lunch. The payload is what
          # is *left* of the plant, as `eat` published it — one frame's bite is always the same
          # number and says nothing, while the size of the thing it has sat down to is what a
          # script might want to remember.
          tell c, "ate", size[bits] if starting_to_eat?(c)
          break # one plant at a time
        end
      end

      if h <= 0.0
        # An empty meter is the end of it. Despawning takes the creature's `ScriptTask` with it,
        # and rubevy's remove hook ends its behaviour and its handlers.
        Rubevy.despawn(c)
        next
      end
      c[:Hunger] = [h]
      c[:Creature] = { age: body[:age] + dt }
    end

    # --- and the grass, written back once -----------------------------------
    #
    # Once, at the end, rather than as it is worked out: a write lands at the end of the frame, so
    # two writes to one plant in one pass would be the second one only — and the first of them is
    # the growth.
    changed.each_key do |bits|
      s = size[bits]
      if s <= crumb
        Rubevy.despawn(who[bits])
      else
        who[bits][:Plant] = { size: s }
      end
    end

    # --- a new blade, now and then ------------------------------------------
    sprout if plants.size < plant_cap && rand < sprout_rate * dt
  end
end
