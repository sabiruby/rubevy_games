# The rabbit. Faster and further-sighted than a beetle, it eats the same grass, and when it is
# not hungry it goes to have a look at whatever else is alive — which is what a beetle feels as
# `"touched"`.
#
# This file is the other half of the point: where the beetle only ever asks about itself and the
# nearest plant, the rabbit uses `Rubevy.find` once at the start to learn the shape of the
# garden, and `garden.nearest(:Creature)` to find its neighbours. Both are ordinary reads of
# registered components; neither needed a line of Rust.

creature "Rabbit" do
  def hungry_below = 60.0

  # how close it wants to get to a beetle before it is satisfied
  def nosey_range = 1.2

  on(:night) do |_at|
    @asleep = true
    stop
  end

  on(:day) do |_at|
    @asleep = false
  end

  # Walked into a tree, a rock or another creature. `what` is the thing, as a `Rubevy::Entity`
  # — a rabbit backs off it and picks a new heading.
  on(:bumped) do |what|
    next if @asleep || busy?
    take_wheel
    flee_from what, Creature::CRUISE, swerve: 1.0
    sleep 0.3
    drop_wheel
  end

  on(:ate) do |_size|
    memory["meals"] = (memory["meals"] || 0) + 1
  end

  def run
    # `Rubevy.find` walks every entity in the world, so it is a thing to do now and then rather
    # than every frame — once, here, is what it is for. The trees never move, so where they are
    # is worth remembering — and from G3 that memory is in the save file, so a rabbit that is
    # loaded from one knows the garden's trees before it has looked at anything (the keys are
    # Strings because that is what comes back out of JSON).
    trees = memory["trees"]
    if trees.nil?
      trees = Rubevy.find(:Tree).map { |t| place_of(t) }.compact
      memory["trees"] = trees
      log "#{trees.size} trees, and #{garden.count(:Plant).to_i} plants to start with"
    else
      log "#{trees.size} trees, remembered: #{JSON.generate(trees.first)}"
    end

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
      spot = here
      if hunger < hungry_below
        plant = garden.nearest(:Plant)
        wander(avoid: memory["trees"], from: spot) if plant.nil? || head_to(plant, Creature::CRUISE * 1.6).nil?
      else
        # Not hungry: go and see who else is about. `nearest(:Creature)` never answers with the
        # asker itself, so this is always somebody else — and `[:Creature]` says which sort. Both
        # reads can answer nil, because the answer is a frame old and a creature can starve in it.
        other = garden.nearest(:Creature)
        kind = other && other[:Creature]
        if kind && kind[:species] == :Beetle
          left = head_to other, Creature::CRUISE * 1.4
          wander(1.0, avoid: memory["trees"], from: spot) if left.nil? || left < nosey_range
        else
          wander avoid: memory["trees"], from: spot
        end
      end
      sleep 0.25
    end
  end
end
