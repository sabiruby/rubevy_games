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

  # breeding (W2). Every one of these was a `const` too, with the same value, and the paragraph
  # that was written above each of them is in `garden/src/main.rs`'s history.
  def mate_hunger   = 75.0 # MATE_HUNGER: three quarters full, both of them, before a pair is one
  def mate_reach    = 2.0  # MATE_REACH: and about two body-lengths apart — which is what two
                           #   creatures eating at the same clump of grass actually measure
  def court_retry   = 2.0  # COURT_RETRY: how often the rules may tell the same creature again
  def mate_cost     = 30.0 # MATE_COST: what a child takes off each parent's meter…
  def mate_cooldown = 20.0 # MATE_COOLDOWN: …and how long it may not have another, whatever it
                           #   eats. **Both are charged when the child arrives**, not when this
                           #   file says "mate": whether a creature does anything at all with the
                           #   message is its own script's business, and a rule that charged for
                           #   the message would be charging for one the script may never have
                           #   listened for. The rabbit, for instance, has no `on(:mate)`.
  def child_hunger  = 50.0 # CHILD_HUNGER: what a newborn's meter says. Well under `mate_hunger`,
                           #   so nothing is born breeding
  def pop_max       = 24   # POP_MAX: how many creatures the garden holds

  # The two of those the **game** needs, because the game builds the body: a newborn's meter and
  # the cap. They go over once, at the start, with `day_length` — the same handover and the same
  # reason, which is that Rust cannot ask this file a question (`world_prelude.rb`, `run_world`).
  # Everything else above is read here and nowhere else.

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
    # The garden's own clock, once a pass: the cooldown below is written into a component, which
    # outlives this script, so it has to be written in a clock that outlives it too.
    now = garden.now
    # Who is well fed enough, rested enough and standing close enough to be told about a partner
    # — filled in below and paired off at the foot of the pass, which is where `court` stood in
    # the Rust chain (after `eat`, so a creature that has just eaten its way over the line is in).
    ready = (@ready ||= []).clear
    # What a parent owes for a child: written down on the pass that first sees the child and
    # taken off the meter on the **next** one.
    #
    # Not written into `Hunger` where the child is found: the parent's own turn of this loop is
    # going to write its meter too, and of two writes to one component in one frame only the
    # second one ever happened (`world_prelude.rb`). Charging it a pass later is a sixtieth of a
    # second later than Rust's `hatch` charged it, and it does not depend on the order the two
    # creatures happen to come out of `Rubevy.find` in.
    #
    # Two tables, swapped: `owed` is what the last pass wrote down and this one collects, `@owed`
    # is what this pass writes down. So a debt whose parent starved in between is forgotten after
    # one pass instead of being kept for ever.
    owed = @owed ||= {}
    @owed = @owing ||= {}
    @owing = owed
    @owed.clear
    creatures.each do |c|
      body = c[:Creature]
      next if body.nil? # gone between the list being taken and this line

      # --- a child has arrived ----------------------------------------------
      #
      # Nothing has aged it yet and it names the creature that asked for it, and the thing that
      # ages a creature is this loop — so "age is exactly zero and somebody asked for me" is a
      # newborn, seen once, on the first pass after the game built its body. The world's own first
      # creatures and everything read back from a save have no parent and are not this.
      born = body[:parent]
      if body[:age] == 0.0 && born.is_a?(Hash)
        mother = born[:Some][0]
        # and the other parent is the one the rules told it about, which is on the creature
        # itself: a pairing is written into both of their `Breeding`s below
        pair = mother && mother[:Breeding]
        @owed[mother.to_i] = true if mother
        @owed[pair[:partner].to_i] = true if pair
        # a newborn is not told about a partner for as long as its parents are not
        c[:Breeding] = { ready_at: now + mate_cooldown }
        c[:Creature] = { age: dt }
        next
      end

      h = c[:Hunger][0] - hunger_rate * dt * body[:genome][:appetite]

      # what it owes for a child that arrived on the last pass. A parent is left under its own
      # script's "go and find something to eat" line, which is most of the reason the population
      # does not run away.
      if !owed.empty? && owed.delete(c.to_i)
        h -= mate_cost
        h = 1.0 if h < 1.0 # a child does not kill its parent
        c[:Breeding] = { ready_at: now + mate_cooldown }
      end

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

      # and whether the rules may have anything to say to it about a partner
      next if h < mate_hunger
      breeding = c[:Breeding]
      next if breeding && now < breeding[:ready_at]
      spot = c[:Transform][:translation]
      ready << [c, body[:species], spot[0], spot[2]]
    end

    # --- two of a sort, well fed, close enough, and the news -----------------
    #
    # `court`, line for line, in the language the rest of the rules are in. The whole of what it
    # decides is who may breed with whom, how often, and how many creatures the garden holds; what
    # a child *is* is worked out in the creature's own script, which is where `Genome#mix` lives
    # — so this is the boundary G2 drew, with the other side of it rewritten.
    #
    # The pair is found by walking the well-fed against each other rather than by asking
    # `garden.within`: there are rarely more than two or three of them at once, and a question per
    # candidate to save a handful of squared distances would be the expensive way round.
    if ready.size > 1
      room = pop_max - creatures.size
      spoken = (@spoken ||= {}).clear
      span = mate_reach * mate_reach
      i = 0
      while i < ready.size && room > 0
        a = ready[i]
        i += 1
        next if spoken[a[0].to_i]
        j = i
        while j < ready.size
          b = ready[j]
          j += 1
          next if b[1] != a[1] || spoken[b[0].to_i]
          dx = a[2] - b[2]
          dz = a[3] - b[3]
          next if dx * dx + dz * dz > span
          # the lower entity id is the one told, so that a meeting is one message and not two —
          # and one child and not two, of the same two parents, in the same frame
          one, two = a[0].to_i <= b[0].to_i ? [a[0], b[0]] : [b[0], a[0]]
          tell one, "mate", two
          # what each of them is now waiting on, and who it was told about. The second is what
          # says who else pays when the child arrives, a frame or two from now; it is on the
          # creature rather than in a table here because the rules can be edited and restarted
          # under a garden that is still running, and this has to outlive that.
          a[0][:Breeding] = { ready_at: now + court_retry, partner: b[0] }
          b[0][:Breeding] = { ready_at: now + court_retry, partner: a[0] }
          spoken[a[0].to_i] = true
          spoken[b[0].to_i] = true
          room -= 1
          break
        end
      end
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
