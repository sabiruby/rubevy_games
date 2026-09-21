# The control stage: what this factory is **for**.
#
# `data.rb` says what the world is made of; an inserter's script says what one arm does; this says
# what you are playing for and what the game should say about it. It is one script for the whole
# factory, and it hears about the factory at the granularity of a machine — something was built,
# something was made, something went into a chest, a machine is backed up. The words are in
# `ruby/control_prelude.rb`.
#
# Ideas, in the order they get interesting:
#
#   * change the goal below and the game is a different game. `goal deliver: { iron_plate: 100 }`
#     is a smelting game; `gear: 1` is a tutorial.
#   * `on(:jammed)` is the one event that is about something being *wrong*. Make it say what to do
#     about it — a jammed machine wants an arm taking what it made away.
#   * nothing says a goal has to be about delivering. Count what you like in a handler of your own
#     and call `win!`.
#   * `look_at(column, row)` points the camera at a tile — it is rubevy's optional `Rubevy::Camera`
#     layer, which is Ruby over an entity's `Transform` and not something the game answers. Winning
#     already does it (the chest the goal was finished in); a handler may do it whenever it likes.

# **Sixty gears.**
#
# The chain this factory is built round is 2 miners -> 4 furnaces -> 1 assembler -> **one gear a
# second** (`ruby/data.rb` works every number of it out against a full belt). So sixty of them is
# **one chain running for one minute** — and a minute is the unit the rest of the file is already
# measured in: a chest holds sixty, which is a minute of one miner, and a tile of ore holds sixty,
# which is a minute of digging it. So this is also **one chestful of gears**.
#
# It counts from the moment this script starts, not from the moment the world was made: apply a
# new control.rb and the goal starts again, because a new goal is a new game.
goal deliver: { gear: 60 }

on(:built) do |what, n, x, y|
  log "#{n} #{what} at #{x}, #{y}"
end

# **A machine that is holding what it made and has the parts for another craft.** It is not
# waiting for anything you have to bring it; it is waiting for somebody to take away what it has
# already made, which is an arm on the other side of it.
on(:jammed) do |what, n, x, y|
  log "the #{what} at #{x}, #{y} is full of what it made — it needs an arm taking it away"
end

# **The first of each thing this factory ever makes** is worth saying; the ones after it are a
# number on the screen. `@made` is an ordinary instance variable of the object your handlers run
# in, so a handler may remember whatever it likes between events.
on(:crafted) do |item, n, x, y|
  @made ||= {}
  next if @made[item]
  @made[item] = true
  log "the first #{item} came out of the machine at #{x}, #{y}"
end

# The two that happen all the time say nothing unless it is the thing the goal is about: a line in
# the log for every plate a factory of three hundred arms makes is a log nobody can read, and the
# game counts them for the screen anyway (`heard …` on the HUD).
on(:delivered) do |item, n, x, y|
  log "#{n} #{item} into the chest at #{x}, #{y}" if item == :gear
end
