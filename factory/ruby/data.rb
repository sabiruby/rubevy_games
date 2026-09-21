# The data stage: what this factory is made of.
#
# Everything here is a **declaration**. There are no loops, no frames and no state: the game runs
# this file once, before the first frame, and what it leaves behind is a table
# (`factory/src/data.rs`). If a line of it is wrong the game says which line and does not start.
#
# **This is where the numbers of play live**, and every one of them is written against something
# else rather than picked, so moving one moves the meaning of the others. The comment on each
# line says what it is a number *of*. Change them and play; nothing in the Rust has an opinion.
#
#   item   :name, icon:                      something that can be on a belt
#   recipe :name, in:, out:, time:, made_in: what a machine turns things into
#   machine :name, size:, sprite:, speed:    what runs a recipe
#   belt / miner / chest / ore               the four fittings the world has built in
#
# `icon:` is which picture of `assets/items/items.png` (tools/factory-items.py prints the list),
# and `sprite:` is which tile of `assets/tiles/factory-tiles.png` (docs/factory-tiles.png is the
# same sheet with the numbers on it). One picture per tile the machine covers, top row first.

# ---------------------------------------------------------------------------------------------
# The things
# ---------------------------------------------------------------------------------------------

item :iron_ore,   icon: 0    # what the miners bring up
item :iron_plate, icon: 1    # one ore, smelted
item :gear,       icon: 2    # two plates, assembled. The end of the line, for now

# ---------------------------------------------------------------------------------------------
# The four fittings
# ---------------------------------------------------------------------------------------------

# **The belt is the unit everything else is measured in**: a full one carries
# `tiles_per_second × items_per_tile` = 4 items a second, and every rate below is a fraction of
# that. The speed itself has no reason — it is what felt right to play (docs/numbers.md §9.3 says
# so rather than inventing one) — and `items_per_tile` follows the art: an item's icon is 8 px
# and a tile is 16, so two sit on a tile without touching.
#
# **`items_per_tile` is one of 1, 2, 4, 8, 16.** An item's place on a belt is a whole number of
# the 16 steps a tile is long (one step to one pixel of the art), so the gap between two items —
# 16 over this number — has to come out whole. Anything else is refused at this line.
belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2

# One item a second is **a quarter of a full belt**, so four miners fill one belt and a line of
# them is worth building. `digs:` is what comes out of the ground.
miner :drill, seconds_per_item: 1.0, digs: :iron_ore

# **A minute of one miner** (60 s at one item a second), which is long enough that a chest at the
# end of a line is somewhere to leave things and not a thing to watch.
chest :crate, capacity: 60

# **One tile of ore is one chest's worth**, which is that same minute of one miner: a miner
# standing on a tile empties it in a minute. The two numbers are next to each other here on
# purpose — before F2a this one lived in `factory.settings.txt` and the chest did not, so moving
# the chest moved nothing. **What comes out of the ground is the miner's `digs:`**, above; the
# name here is a label, the way `:conveyor` and `:crate` are.
#
# **How wide a patch is stayed a setting** (`ore_patch_radius` in factory.settings.txt): the
# smallest map the four patches fit on without touching the border is worked out from it before
# the game has a VM to read this file with, so it cannot be here. This number is read once the
# world is laid out, which is after.
ore :patch, per_tile: 60

# ---------------------------------------------------------------------------------------------
# The machines, and what is made in them
# ---------------------------------------------------------------------------------------------

# `speed:` multiplies: 1 is "the recipe's own time". `sprite:` is one tile per tile covered.
#
# The furnace is Kenney's copper arch (tile 109) and one tile big. The assembler covers two by
# two and is Kenney's green machine stretched to fit (tools/factory-machine.py, tiles 138-141):
# the thing that eats four furnaces' worth ought to look bigger than one of them.
machine :furnace,   size: [1, 1], sprite: [109],                speed: 1.0
machine :assembler, size: [2, 2], sprite: [138, 139, 140, 141], speed: 1.0

# **Two seconds a plate is an eighth of a full belt.** So: one miner keeps two furnaces going,
# and eight furnaces fill one belt with plates. Two seconds rather than one because a furnace
# that kept up with its own miner would make smelting a thing you put in the line and forget.
recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 },
       time: 2.0, made_in: :furnace

# **One assembler eats two plates a second**, which is four furnaces — or two miners, all the way
# back — and makes one gear a second, which is a quarter of a belt. The whole chain, then:
#
#   2 miners  ->  4 furnaces  ->  1 assembler  ->  1 gear a second
#
# and a full belt of gears is four of those chains. That is the number the map has to be big
# enough for, and it is why `time:` here is 1.0 and not something rounder: it makes the chain
# whole numbers all the way down.
recipe :gear, in: { iron_plate: 2 }, out: { gear: 1 },
       time: 1.0, made_in: :assembler
