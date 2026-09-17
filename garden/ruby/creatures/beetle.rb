# The beetle. It is slow, it eats grass, and a rabbit walking over it is the worst thing that
# happens to it all day.
#
# Everything the beetle knows about itself is a component read — `hunger`, `here`, and the
# `[:Transform]` of whatever `garden.nearest` handed back. Nothing in `garden/src/main.rs`
# knows this file exists.

creature "Beetle" do
  # Below this it stops wandering and goes looking for grass. 100 is full, 0 is dead.
  def hungry_below = 55.0

  # --- the handlers: one task each, waiting on a queue ----------------------

  # The sun has gone. A beetle sleeps where it stands.
  on(:night) do |_at|
    @asleep = true
    stop
  end

  on(:day) do |_at|
    @asleep = false
  end

  # A rabbit has walked over us. `by` is the rabbit, as a `Rubevy::Entity`: the handler reads its
  # position out of the ECS like anything else, and runs the other way. It keeps the wheel for
  # half a second, so that the behaviour's next pass does not quietly steer back.
  on(:touched) do |by|
    next if @asleep
    take_wheel
    # aside as well as away: a rabbit that is chasing comes from behind, and running straight
    # away from something behind you is carrying on
    flee_from by, swerve: 1.0
    sleep 0.5
    drop_wheel
  end

  # Walked into something solid — a tree, a rock, another creature. Turn off it rather than lean
  # on it, gently (this is not a fright) and briefly.
  on(:bumped) do |what|
    next if @asleep || busy?
    take_wheel
    flee_from what, Creature::CRUISE, swerve: 1.0
    sleep 0.25
    drop_wheel
  end

  # A meal has started, on a plant of this size. Published once per meal, not once per frame of
  # it — a beetle chews for a second or two, and sixty messages would be one event filling a
  # queue that holds sixty-four.
  #
  # This is also where a beetle's memory is written (G3). `memory` is a plain Hash that lives on
  # this object; the game reads it out of the VM when the garden is saved and puts it back when
  # the garden is loaded, so a beetle picked up an hour later still knows how many meals it has
  # had and where the best grass it ever found was. The keys are Strings because JSON's are.
  on(:ate) do |size|
    meals = (memory["meals"] || 0) + 1
    memory["meals"] = meals
    # the best plant it has ever eaten, and where. `@dinner` is where `run` was walking to — the
    # position it already read, so remembering it costs no round trip.
    best = memory["favorite"]
    if @dinner && (best.nil? || size > best["size"])
      memory["favorite"] = { "at" => [@dinner[0], @dinner[2]], "size" => size }
    end
    # every fifth meal, out loud, as JSON — which is in the VM because the game put it there
    remember_out_loud if meals % 5 == 1
  end

  # Two well-fed beetles have bumped into each other and the rules picked this one to work out
  # the child. `partner` is the other beetle, as a `Rubevy::Entity`.
  #
  # **This is the whole of G2.** The rule — who may breed with whom, how often, and how many
  # creatures the garden holds — is Rust, in `court`. What the child *is* is worked out here, by
  # calling three methods on a Rust struct: `mix` averages two genomes, `mutate` nudges every gene
  # by up to a tenth with the VM's own dice, and `to_h` turns the result into the Hash the game
  # takes back. The partner's genome comes the other way, out of its `Creature` component as a
  # plain Hash — so both views of `Genome` are used in one line each, three lines apart.
  on(:mate) do |partner|
    next if @asleep
    mate = genome_of(partner)
    next if mate.nil? # it starved between the rule speaking and this waking up
    child = my_genome.mix(mate).mutate(0.1)
    spot = here
    answer = garden.spawn(species: name, genome: child.to_h, at: [spot[0] + 1.2, spot[2] + 1.2])
    if answer == true
      memory["children"] = (memory["children"] || 0) + 1
      log "child #{memory["children"]}: #{child.to_s}"
    else
      # G3: the game answers a Hash it cannot read with what was wrong with it, in serde's words
      log "no child: #{answer}"
    end
  end

  # The world has turned over into a dry or a wet season — `world.rb`'s `every 60`, carried across
  # from the world's VM by `tell :all` (W2). A beetle can do nothing whatever about the weather;
  # what it can do is know which one it is in, and a save file opened an hour later says so.
  on(:season) do |season|
    memory["season"] = season
  end

  # --- the behaviour --------------------------------------------------------

  def run
    loop do
      if @asleep
        # `stop` again, and not only in the night handler: the handler fires while this loop is
        # somewhere in the middle of a pass, so the pass that was already under way gets its
        # `act` in after it. One more `act 0, 0` here is what settles it.
        stop
        sleep 0.5
        next
      end
      if busy?
        # a handler has the wheel: thinking now would only cost round trips for an `act` that is
        # going to be dropped
        sleep 0.1
        next
      end
      if hunger < hungry_below
        # The one question that is not a component: the rules know what is near, a script does
        # not. The radius is the creature's own `Sight`, which the game reads off the entity.
        plant = garden.nearest(:Plant)
        # where it is, read once and then walked to: `head_to` takes a place as happily as a
        # thing, so this costs one round trip rather than two — and the place is what
        # `on(:ate)` remembers if the meal turns out to be the best one yet
        @dinner = place_of(plant)
        # nil where the plant was eaten in the frame between the question and the answer, which
        # happens in a garden of ten mouths
        wander if @dinner.nil? || head_to(@dinner).nil?
      else
        wander
      end
      sleep 0.2
    end
  end
end
