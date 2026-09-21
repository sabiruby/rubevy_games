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
#   map    :name, size:                      how big the world is, in tiles
#   belt / miner / chest / ore / inserter    the five fittings the world has built in
#
# `icon:` is which picture of `assets/items/items.png` (tools/factory-items.py prints the list),
# and `sprite:` is which tile of `assets/tiles/factory-tiles.png` (docs/factory-tiles.png is the
# same sheet with the numbers on it). One picture per tile the machine covers, top row first.

# ---------------------------------------------------------------------------------------------
# The world
# ---------------------------------------------------------------------------------------------

# **How big the map is, in tiles across and up — and they do not have to be the same.**
# `size:` is the machine's own word because it means the machine's own thing: how many tiles this
# covers, across and up.
#
# **32 by 32 is still the provisional value F0 picked**, and it is here rather than in
# `factory.settings.txt` (where it was until 2026-09-21) because what a map is big enough for is
# what the game is, not how the game is run — and because a page has no settings file to write, so
# the size could not be changed there at all. One chain of the factory below is 2 miners, 4
# furnaces, 1 assembler and 10 inserters — about 30 tiles — so 1,024 tiles is room for about 34 of
# them. Make it bigger and say `patches:` to match, or the ore runs out before the map does.
#
# Two sizes are refused at this line rather than quietly changed:
#
#   * smaller than the ore needs — the patches below would touch the wall, where nothing can be
#     built and nothing can be seen. The message says how many tiles would do;
#   * larger than 2048 either way, which is where the drawing stops: the floor is one texture with
#     one texel per tile, and 2048 is the size every WebGL2 machine is guaranteed to manage.
#
# Everything in between is yours. A big map costs memory — the game logs how many megabytes when
# it lays the land — and that is a fact about your machine rather than a rule of the game.
map :world, size: [32, 32]

# ---------------------------------------------------------------------------------------------
# The things
# ---------------------------------------------------------------------------------------------

item :iron_ore,   icon: 0    # what the miners bring up
item :iron_plate, icon: 1    # one ore, smelted
item :gear,       icon: 2    # two plates, assembled. The end of the line, for now

# ---------------------------------------------------------------------------------------------
# The five fittings
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
# them is worth building. **What comes up is whatever the ground it stands on is** — see `ore`
# below — so a miner has nothing to say about which item it digs.
miner :drill, seconds_per_item: 1.0

# **A minute of one miner** (60 s at one item a second), which is long enough that a chest at the
# end of a line is somewhere to leave things and not a thing to watch.
chest :crate, capacity: 60

# **The name of the ground is what comes out of it.** `ore :iron_ore` is a ground made of iron
# ore, and a miner standing on it brings up iron ore — it has no say in the matter. Put a second
# `ore` line here and you have said there is a second kind of ground; nothing else has to change.
# (Until 2026-09-21 the name was a label and the miner said `digs: :iron_ore`, which meant "what
# comes out of the ground" was written in two places and the miner was the one that had to be
# edited to add a kind of ore.)
#
# **One tile of ore is one chest's worth**, which is that same minute of one miner: a miner
# standing on a tile empties it in a minute. The two numbers are next to each other here on
# purpose — before F2a this one lived in `factory.settings.txt` and the chest did not, so moving
# the chest moved nothing.
#
# **How wide a patch is and how many there are** came here on 2026-09-21 with the map's own size,
# and for the same reason: the radius was the last number of the world's layout left in
# `factory.settings.txt`, kept there because the smallest map it allows was worked out before the
# game had a VM to read this file with. The map is a declaration now, so nothing about the world
# is worked out before this file is read.
#
# **`patches: [2, 2]` is two across and two up** — one in the middle of each quarter of the map,
# which is where the four have always been. It is a grid rather than a count so that the places
# are the same in every run (a scattering would want a seed, which would be a number with nothing
# behind it) and so that a map that is not square gets patches where its cells are: `[6, 2]` on a
# 96 by 16 map is twelve patches, evenly spread, none of them on the wall.
#
# **Radius 3 is about 29 tiles**, which at 60 each is 1,740 ore — six chains' worth of iron for
# the whole patch. Four patches on a map much bigger than this one is a factory with nothing to
# feed it: the rule of thumb is a patch per thousand tiles or so.
ore :iron_ore, per_tile: 60, patch_radius: 3.0, patches: [2, 2]

# **One item a second is a quarter of a full belt** — and it is exactly one miner's output, which
# is the whole derivation: one inserter keeps up with one miner, one keeps two furnaces fed (they
# eat half an ore a second each), and four of them fill a belt. The chain in the comments below
# stays whole numbers with an inserter at every joint.
#
# `seconds_per_item:` is the **miner's own word**, because it means the miner's own thing: how
# long this takes over one item. How far an inserter reaches is not a number — it takes from the
# tile behind it and puts into the tile in front, which is what a one-tile building with a
# direction already says.
#
# **It is also how long an idle inserter waits between looks** (`ruby/prelude.rb`'s `idle`), and
# that is not a second number hiding here: an arm could not have acted sooner than it can swing.
inserter :arm, seconds_per_item: 1.0

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
