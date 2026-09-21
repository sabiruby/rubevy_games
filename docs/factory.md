# Factory

The third game. **This file says what exists, not what is planned** — the plan is
[`plans/factory-plan.md`](plans/factory-plan.md), and everything this page does not mention is
not written yet.

What exists (stages **F0** to **F4**, 2026-09-22):

* a crate, `factory/`, in the workspace;
* a **floor**: one `TilemapChunk` — Bevy's own, one draw call for the whole grid — laid out of
  Kenney's Tiny Factory tiles, on a PC and **in a browser**, with **ore** in it;
* a **grid you build on**: one tile holds one building, a click puts one down and a click with
  nothing in hand takes it away;
* **conveyors** that carry, jam, merge and turn; **miners** that dig the ore they stand on;
  **chests** that hold what arrives;
* a **camera the player drives** (`games_shell::CameraPlugin`: drag, wheel, `WASD`, `Home`)
  whose zoom is **rounded to a whole number of screen pixels per pixel of the art**;
* a **data stage** — `ruby/data.rb`, where the items, the recipes, the machines and every number
  the factory is played by are written, read once before the first frame into Rust tables;
* **furnaces and assemblers** that make what a recipe says in the time it says, including machines
  that cover more than one tile;
* **inserters** — the one machine with a mind. One takes a thing from the tile behind it and puts
  it in the tile in front, **and when it does that is a line of Ruby**
  (`ruby/inserter.rb` on top of `ruby/prelude.rb`). Nothing goes into or out of a machine any
  other way, so a line that ends at a furnace ends there until an arm is put beside it;
* a **panel to write that Ruby in**, in a window: click an arm and its script is there, Apply runs
  it in that one or in all of them, Revert puts them back on the file;
* a **control stage** — `ruby/control.rb` on top of `ruby/control_prelude.rb`, one script for the
  whole factory that hears five things at the grain of a *machine* (`built`, `removed`, `crafted`,
  `delivered`, `jammed`), says what the factory is for (`goal deliver: { gear: 60 }`) and says when
  it has been won, with a line of text on the screen for the goal and for **what was dropped**;
* `--headless N`, `--shot`, `--stress N`, `--arms N`, and the checks (`FACTORY_SELFTEST`,
  `?selftest`).

What does not exist: a HUD (F4's goal is one line of `bevy_ui` text, and F5 makes it egui), a VM
panel, a guide, a save file, and Save in the panel (it says so rather than doing half of it).

```
cargo run -p factory                          # a window
cargo run -p factory -- --headless 66         # no window (66 is the default; see numbers.md)
cargo run -p factory -- --stress 4000         # a loop of belt, measured (sizes its own map)
cargo run -p factory -- --arms 1000           # a thousand inserters, measured (the same)
FACTORY_SELFTEST=1 cargo run -p factory -- --headless
docker/build.sh factory release && docker/run.sh factory release   # a window, in the container
web/build.sh factory && web/serve.sh          # then http://localhost:8080/factory/
```

## Playing it

The game says the building keys in the log when it starts, and `H` says the rest of it in English
or Japanese:

| | |
|---|---|
| `1` `2` `3` `4` | a belt, a miner, a chest, an **inserter** in hand |
| `5` onwards | the machines `data.rb` declares, in the order it declares them (`5` the furnace, `6` the assembler) — the game prints the list when it starts |
| `0` | nothing in hand: a click takes away what is there |
| `R` | turn what is in hand a quarter turn anticlockwise — **a machine has no direction**, so with one in hand it does nothing and says why (nothing goes in or out of a machine but through an arm) |
| click | build it, or take it away |
| click an inserter with an inserter in hand | **open its script in the panel** |
| drag, wheel, `WASD`, `Home` | the camera |
| `H` `?` | the guide, English and Japanese |
| `F1` `F2` | the editor, the VM panel |
| `F5` `F9` | write the factory down, read it back |
| `P` | **the whole factory stands still** — the belts, the arms and the scripts. Building and editing go on, because looking at a jammed line and laying the belt it wanted is what a pause is for |
| Ctrl+Enter, Ctrl+S | apply what is in the editor, write it to its file |

**Opening an arm needs no mode and no key.** A click on a tile that already has an inserter cannot
have meant "build an inserter", so it means "show me this one" — and building a new one opens it
too, which is the order a player does it in. It also means a second click never resets an arm in
the middle of a swing.

**A miner has to stand on ore**, which is what makes a patch worth finding; everything else goes
anywhere, and building over something replaces it. **A machine may cover more than one tile** —
the assembler is two by two — and it is built on the tile that was clicked, with the rest of its
footprint going up and to the right; a click on any of its tiles takes the whole thing away. A
belt may feed it through **any** of its tiles, and what it makes comes out one step the way it
faces, from the tile it was built on. A belt carries what is on it the way it faces
and takes things from any side but its own front, so two belts pointing at each other jam rather
than passing the same item back and forth. A miner puts what it digs into whatever it faces — a
belt or a chest — and if there is nowhere to put it, **it keeps the dig and takes nothing out of
the ground** until there is.

**A machine has no other door than an inserter.** A belt running into a furnace jams; what a
furnace made stays in it. A chest still takes from a belt and a miner still puts a dig straight
onto one, so the first line a player builds — a miner, a belt, a chest — needs no Ruby at all.
Factorio wants an inserter for the chest too; this game does not, because the line that has to be
joined up by a script is the line with a *machine* in it, and that is what the game is about.
**A machine's direction says nothing** (an arm reaches into whichever of its tiles it stands
behind), which is Factorio's shape as well.

## The window

One editor with three buttons, and the three files it switches between are three different kinds
of thing (`factory/src/window.rs`):

| | what Apply does | what Ctrl+S writes |
|---|---|---|
| **the inserter** | hands this one arm, or every arm, a new script | `ruby/inserter.rb` |
| **`control.rb`** | swaps the control stage's script — and its goal counts from now, because a new goal is a new game | `ruby/control.rb` |
| **`data.rb`** | **reads the declarations again, and may lay the world out anew** | `ruby/data.rb` |

**Nothing writes a file but Ctrl+S.** Apply is for trying something; the file on disk is untouched
until you say so, and in a browser "the file" is a `localStorage` key, so a page keeps what you
wrote.

**`data.rb` while the game runs** is the point of the stage, and it is what makes `map :world,
size:` — a declaration since F3a — a thing you can *turn*, in a page, without restarting anything.
Three outcomes, and the first two leave the factory exactly as it is:

1. **It will not read.** The line it is wrong on goes in the panel and in the log; the tables in
   use are still the ones that built the world, so nothing on the map moves.
2. **It reads, and the world still fits it.** The tables and the numbers are swapped and nothing
   is rebuilt: a belt that was carrying goes on carrying, at the new speed.
3. **It reads, and the world does not fit it** — a different map size, different ore, or the
   items, machines or recipes are not the same list. Every tile and every item in the world is a
   number into those, so the world has to be laid out again, **which loses what is built**. The
   panel says what would change and asks for the Apply a second time. A player does not lose a
   factory by pressing a button once.

The HUD (egui, top left) is four rows: what is built and carried and how many arms; what one frame
of scripts spent against the budget, how many arms are parked on a `move` and what the tick took
against `frame_time`; what the control stage says and **what was dropped**, which should be zero;
and how many programs the VM is holding, because every distinct text a script is started from is
an irep SabiRuby keeps for the life of the process.

**The guide is `H`**, English and Japanese, and the Japanese is a font cut down to the characters
those strings use (`tools/subset-font.sh`, 81,480 B for 396 glyphs). Edit the Japanese and re-cut
it, or the new character is a blank box.

## The save file

`factory.save.json` beside the game, or the `localStorage` key `factory:factory.save.json` in a
page; `F5` writes it, `F9` reads it, and `--save PATH` / `--load PATH` do the same for a run with
no keyboard. `factory/src/save.rs` **is** the format; there is no schema anywhere else.

**What it is** is the grid and the lanes — which is the whole of the difficult half, because F3
put a miner's half-finished dig, a furnace's half-finished craft and an arm's half-finished swing
in `Building` rather than in components, for exactly this. Beside them: what is left of the ore,
the fraction of a step the belts had not taken yet (without it a loaded factory is up to one step
of sixteen behind), the scripts that were applied in memory, and what each script remembers.

**What it is not** is the tasks. A task parked on a `move` is a request in one VM's heap and
cannot be written down, so a loaded game starts every `run` again from the top. What a script must
not forget across that is `memory` — an ordinary Hash, `@memory`, which the save carries and the
load hands back — and for the control stage, how far through its goal it had got (`@got`, `@won`).
A factory saved five gears from winning comes back five gears from winning.

**A save is of the `data.rb` it was taken against.** The file carries a print of that file's
tables (the map's size, the items, the machines, the recipes, and how many steps a tile is), and a
save whose print does not match is refused with the half that differs rather than read into a
world where tile 900 is off the map. A file from another build is refused by its `version`, which
is read on its own before the rest of the file is looked at.

**Two saves of one factory are the same text**, which is what the check measures: `save → load →
save` compares the two files. That holds because everything written in tile order is *kept* in
tile order and the `HashMap`s are sorted on the way out — and because the check holds the world
still while it does it, since a belt moves half a step a frame.

## The camera, from Ruby

`look_at(column, row)` in `ruby/control_prelude.rb` is rubevy's optional `Rubevy::Camera` layer
(`ScriptWorld::load_and_run(rubevy::layers::CAMERA)`) — Ruby over an entity's `Transform`, and
nothing the game answers. Winning uses it: `win!` points the camera at the chest the last delivery
the goal counted went into.

The joining is one system. The camera here is driven by a resource (`games_shell`'s `CameraView`)
and its `Transform` is written from that every frame, so a script's `move_to` would be painted
over before anybody saw it; `let_a_script_move_the_camera` takes what the transform says now,
minus what the panels cover, as the new view. It **settles** rather than drifting: after a script
has moved the camera it sets `focus = t - shift`, the camera plugin writes `t = focus + shift`
back, and the next frame finds the transform where it left it and does nothing.

## What one step of the factory is

`factory/src/belts.rs`, and the tests at the bottom of it run the whole of it with no `App` at
all. A belt tile is **sixteen steps** long and an item on it is a whole number of them: how far
through that tile it has come, one step to one pixel of the art. A lane is the items on one tile,
front first, and **every item is at least a gap behind the one in front of it**. That invariant is
the model: carrying is adding a step to each number, jamming is a number that cannot grow,
merging is two tiles handing items to one and each finding the gap the other left.

The steps are F2a's; F1 held the position as a fraction of a tile and the fraction is what made
"a jammed tile holds one more than the gaps that fit" a statement about `f32` rather than about
the factory (below). What a whole number of steps buys is that the sentence is now true of every
gap a belt can have, that a frame's leftover fraction of a step is carried rather than rounded —
so the same seconds carry an item the same distance at five frames a second and at sixty — and
that an item's place on the screen is a pixel of the art rather than a multiply and a rounding
away from one.

**Six passes**, and their order is the model:

1. **the tails as they were** — what a belt may push into its neighbour is read from a snapshot
   taken before anything moved, so two belts merging into a third both see the same picture and
   the answer does not depend on which tile is walked first;
2. **carry** — every item forward by a step, the front one no further than the room ahead of it;
3. **hand over** — an item at the end of its tile goes to the next one **if there is still room
   when its own turn comes**, which is where a merge is decided (the lower tile index gets the
   gap) and where a chest takes an item off the belt;
4. **dig** — a miner that has finished puts an item down, or holds it;
5. **swing** — an inserter whose arm is crossing moves it on and puts down what it is carrying
   when it arrives (F3: the half of an inserter that is not Ruby);
6. **craft** — a machine with nothing waiting to go out takes a recipe it has the parts for,
   consumes them, and works at it for `time / speed`.

**There is no `deliver` pass**, and there was one until F3: a machine used to push what it had
made into whatever it faced. Nothing goes into or out of a machine now but through an arm, which
is what makes the player's Ruby the thing that joins a factory up. A machine still holding what it
made does not start another craft, which is exactly the rule a blocked miner keeps with its
finished dig — and a machine in that state **with the parts for the next craft** is the one thing
this game calls a jam (the control stage, below). A machine holds **one craft's worth in and one
out** — the smallest buffer that lets it run without a gap, which is why it is not a number
anybody chose.

**A frame's work is the size of the factory and not the size of the map** (F4). Two of the passes
above used to walk every tile of the world — the tails were written over the whole map before the
real ones went in, and what is on the belts was counted by adding up every lane — which cost 5.9 ms
a frame on a map of 2048 by 2048 with nothing built on it. The tails are written for the tiles
something stands on and a tile that is taken away is put back to the sentinel there and then; the
count is kept by the four doors items go in and out by (`Lanes::put_on`, `take_off`, `forget`, and
`moving`, which only moves them). An empty map of any size now steps in **1 µs**.

**Where an item lives was measured, not chosen.** The plan would not settle on paper whether an
item should be an entity or a number in a lane, so F1 built both, ran the same rule over both at
1,000, 4,000 and 16,000 items, and kept the lanes: 2.2 to 2.9 times cheaper with no window, and
— the reason that outlives the numbers — the rule needs *the item in front*, which an ECS query
has no notion of, so the entity way round has to rebuild the order every frame.
`worklog/2026-09-21-factory-F1.md` §3 has the numbers, including why the browser measurement
could not tell the two apart and is not quoted as if it could.

---

## The floor, and the one thing that nearly stopped it

The plan's riskiest assumption was that `TilemapChunk` — added to Bevy in 0.17, one chunk of
tiles drawn in a single call out of an **array texture** — works on WebGL2, which is what a page
gets. It was never confirmed by the survey (`worklog/2026-09-20-factory-survey.md` §4).

**It works, with one condition**, and the condition is not obvious.

wgpu's OpenGL backend has to choose a bind target when it creates a texture, and OpenGL has no
way of asking, so it **guesses from the shape**
(`wgpu-hal-29.0.4/src/gles/mod.rs:458`, `TextureDescriptor::is_cube_compatible`):

| the texture | what wgpu binds it as |
|---|---|
| `D2`, 1 layer | `TEXTURE_2D` |
| `D2`, layers not square or not a multiple of 6 | `TEXTURE_2D_ARRAY` |
| `D2`, **square layers**, exactly 6 of them | `TEXTURE_CUBE_MAP` |
| `D2`, **square layers**, a multiple of 6 of them | `TEXTURE_CUBE_MAP_ARRAY` |

A tileset of 16 × 16 tiles has square layers by definition, and Kenney's sheet has **132** of
them — 6 × 22. So a browser bound the tileset as `TEXTURE_CUBE_MAP_ARRAY`, **which WebGL2 does
not have**, and the page drew a black screen. The same binary on a PC (Vulkan, where the flag is
only "this texture may also be viewed as a cube") drew the floor perfectly.

**A black page with no page error.** The run's own checks all said `ok` — the image loaded, it
had 132 layers of 16 × 16 px, the floor data was right — because none of that is the binding.
`pageerror` was 0 and `requestfailed` was 0. What said so was the console, which carried
`WebGL: INVALID_ENUM: bindTexture: invalid target` two hundred and fifty times and one
`wgpu-hal heuristics assumed that the view dimension will be equal to CubeArray` from wgpu's own
log. The evidence that it was fixed is a **screenshot with its pixels counted**, which is what
`docs/worklog/2026-09-18-web-black-screen.md` established as the standard here, and it is in
`worklog/2026-09-21-factory-F0.md`.

**The fix is to choose the number of tiles.** `tools/factory-tileset.py` builds the one sheet the
game loads: a vertical strip of 16 px squares, Kenney's 132 in the pack's own order and
numbering, then the four `tools/factory-belts.py` draws, then the two `tools/factory-ore.py`
draws, and **blank tiles on the end until the count is not a multiple of six** — 139 today.
The game reads the count back out of the loaded image and its checks fail if it ever becomes a
multiple of six again.

**F1 is where that paid for itself.** Adding the two ore tiles took the sheet to 138, which is
6 × 23, and the script put a blank on the end without being asked. Nobody would have noticed: the
count is not something a person carries in their head, and what goes wrong when it is a multiple
of six is a black page with no error in it.

The sheet is a strip rather than a grid because a strip is what lets the script choose the count:
`ImageArrayLayout::RowHeight { pixels: 16 }` makes one layer per 16 px of height, and a grid's
count is whatever the grid multiplies out to.

## The tiles

`docs/factory-tiles.png` is every tile of the sheet, blown up, with its number under it —
`tools/factory-tileset.py --contact-sheet docs/factory-tiles.png` rebuilds it from the sheet
itself, so the numbers on it are always the numbers the game uses. **Tiles 0–131 are Kenney's own
numbers**, row-major from the top left of `tilemap_packed.png`.

Read off the sheet on 2026-09-21. The plan (§3.6) guessed most of these from the survey; where
it was wrong is marked.

| tiles | what they are |
|---|---|
| 0, 1, 2 | the tan **floor plates** — plain with bolts, four panels, a diagonal stripe |
| 3 | plain dark **ground** |
| 9, 10, 11, 21, 33, 34, 35 | dark ground with **pipes** running through it. *The plan called 9–11 and 32–35 "dirt"; only 3 is bare, 32 is ground with rubble, and the rest carry pipe* |
| 32 | dark ground with rubble |
| 24 / 36 | a **one-tile conveyor**, pointing right, frames A and B |
| 25 26 27 / 37 38 39 | a conveyor run pointing right — left end, middle, right end — frames A and B |
| 48 / 60 and 49 50 51 / 61 62 63 | the same two, with the **orange rail** along the front |
| 4 16 28 / 5 17 29 | a conveyor pointing **up** — top end, middle, bottom end — frames A and B |
| 40 52 64 / 41 53 65 | the same, with orange rails down both sides |
| 75 76 77 | a **red machine** that straddles a belt: left (with a control panel), middle, right |
| 87 88 89 | the same in orange |
| 99 100 101 | in green |
| 111 112 113 | in blue |
| 73, 85, 97 | wooden **crates**, three sizes |
| 108, 109, 110 | three more machine fronts — grey, brown, and one of **orange blocks between grey posts on pale feet**. The pack names none of its tiles (`art/Tilesheet-tiny-factory.txt` is sizes and counts only), so what 110 is meant to be is a guess; F1 uses it as the **miner** because it is the one that reads as apparatus rather than as a cabinet |
| 114 | a **gear** |
| 6, 74, 86, 90, 91 | a bench, two tanks, and an orange head on a stand |
| 132, 133 | a conveyor **straight**, seen from above, frames A and B (`tools/factory-belts.py`) |
| 134, 135 | a conveyor **corner** — in at the left, out at the top — frames A and B |
| 136, 137 | **ore in the ground**: plenty left, and nearly gone (`tools/factory-ore.py`) |
| 138–141 | the **assembler**, two tiles by two, top row first (`tools/factory-machine.py`) |

Of Kenney's own, F1 places **110** as the miner and **85** (a wooden crate) as the chest, and F2's
`data.rb` places **109** as the furnace. The pack's conveyor tiles (24–27, 4/16/28 and the railed
ones) are in the sheet and **not used**: they are three-quarter pictures and the belts are drawn
from above.

**F2 read 75/76/77 again and the table above was wrong about them.** They are not the three parts
of one wide machine: each is a complete cabinet with its own left and right edge, and 75, 87, 99
and 111 are the same cabinet in four colours. So the pack has **no machine bigger than one tile**,
which is what `tools/factory-machine.py` is for. The blank padding tile is gone as well — 142 is
not a multiple of six, so `pad_to_a_safe_count` did not add one, which is the script deciding and
nobody editing.

**What the pack does not have**, and what `tools/factory-belts.py` is for. The survey said the
pack had no left and no down; reading the tiles pixel by pixel says otherwise:

* tile 26 (running right) is **symmetric top to bottom** apart from its chevrons, so **left is it
  mirrored horizontally** — `TileOrientation::MirrorH`, no new picture;
* tile 16 (running up) is **symmetric left to right and has no front face at all** — it is
  already drawn from straight above — so **down is it mirrored vertically**.

So the only thing genuinely missing is **the corner**, and F0a drew it twice over, in the two
styles the author was choosing between: (i) the pack's three-quarter look, where a belt running
sideways has a front face under it and a corner has to change width as it turns — four pictures —
and (ii) everything seen from straight above — one straight and one corner, turned.

**The author chose (ii)** (2026-09-21), so the belts the game draws are the two above and their
turns: `TileOrientation` makes four directions out of one straight and all eight corners out of
one corner. The pack's own belt tiles (24–27, 4/16/28 and the railed ones) are **in the sheet and
not used** — they are three-quarter pictures, and a belt from above is not one. The screenshots
of both candidates, and what reading the pack's tiles pixel by pixel found, are in
`worklog/2026-09-21-factory-F0.md` §6–§8; F1 deleted (i)'s four corners, the code that drew them,
and `src/belt_sample.rs`, which was the arrangement the screenshots are of.

**The ore is Kenney's too, in another pack's colours.** Tiny Factory has no ore, so
`tools/factory-ore.py` takes the rocks of **Kenney's Tiny Farm 1.0** (CC0, tiles 77 and 89) and
maps them from their own grey ramp onto **Tiny Factory's orange** — `#ffd896` `#fdbe53` `#e38628`
`#bd6c4a`, the colour its rails and its warning stripes are painted in. That works because the
two packs are drawn in the same palette, which is a thing reading them pixel by pixel says and
no description of either does. **The item on a belt is not Kenney's**: two of them have to sit on
a 16 px tile without touching, so it is 8 px, and a 16 px rock does not survive being halved —
the script draws it in the same four colours.

**The items are one strip of 8 px icons** (`tools/factory-items.py`,
`factory/assets/items/items.png`), cut with a `TextureAtlasLayout`, so a belt carrying ore, plates
and gears is one draw call and `icon:` in `data.rb` is an index into it. F1's nugget is the first
of them, pixel for pixel; the plate and the gear are F2's, drawn in **Tiny Factory's own greys** —
the same ramp Tiny Farm's rocks came in. The gear is tile 114 read by eye and redrawn at half the
size, because eight teeth in eight pixels is a grey square.

**The assembler is Kenney's machine, stretched.** A `machine :assembler, size: [2, 2]` wants four
pictures and the pack has one, so `tools/factory-machine.py` takes tile 99 and **nine-slices** it:
the four pixels at each edge kept as they are, the band between them blown up by exactly three.
Every pixel of the result is a pixel of Kenney's — eight colours, all of them tile 99's — so it
stands beside the one-tile furnace without looking like it came from another pack. The first
attempt repeated one middle row instead, and it turned the cabinet's screen into a flat panel and
lost the frame and the lights; the docstring says so.

## The control stage

`ruby/control.rb` on top of `ruby/control_prelude.rb`, and `factory/src/control.rs`. It is
Factorio's third stage with the same job and a tenth of the surface: **one script for the whole
factory**, five things it is told, and a goal.

```ruby
goal deliver: { gear: 60 }

on(:built)     { |what, n, x, y| log "#{n} #{what} at #{x}, #{y}" }
on(:crafted)   { |item, n, x, y| … }
on(:delivered) { |item, n, x, y| … }
on(:jammed)    { |what, n, x, y| log "the #{what} at #{x}, #{y} is full of what it made" }
```

**The five are `built`, `removed`, `crafted`, `delivered` and `jammed`**, and every one of them
carries the same four flat values: *what* (a Symbol — an item's name for the two that are about
items, a building's for the other three), *how many* of it this frame, and *where* the last of
them was.

**The grain was worked backwards from a number rubevy has.** A subscription holds sixty-four
messages before the oldest is dropped, and a script is woken once a frame, so sixty-four is what
may be published between two looks. Hence: **one message a frame per kind of thing**, with the
count in it. How many kinds there are is what `data.rb` declares — three items and six kinds of
building here — so **nothing a data file can say reaches the ceiling**, where a message per
delivered item would reach it at about sixty arms. Nothing is ever said about one item on one
belt; there are thousands of those and they are Rust.

**A jam is a machine's and not a belt's**: a machine that is holding what it made **and has the
parts for another craft**. A working factory has jammed belts in it all the time — the belt in
front of a machine slower than it *is* a jam — so publishing those would be publishing that the
factory is running. A machine that cannot get rid of what it made is a chain that has stopped, and
it stopped for a reason the player can act on: there is no arm on the other side of it. It is a
*state*, said by the factory every frame it is true, and what the script hears is the frame it
**became** true.

**The script answers by being read, not by asking.** `win!` sets `@won`; the line for the screen
is `@saying`; how much of each event it has heard is `@seen_delivered` and its four sisters; what
its own subscriptions lost is `@dropped`. The game reads them with two `ivar_get`s once a frame,
which is how the garden reads a creature's `@asleep`. A script that had to *tell* the game these
would be parked on a request every time a gear arrived, and a request costs a frame. So the only
thing the control stage does *to* the game is win: **it cannot build**, which is a decision — a
goal is a thing to say, and building is the player's hand.

**Sixty gears** is the chain `data.rb` is built round (2 miners → 4 furnaces → 1 assembler → a
gear a second) running for **one minute**, which is the unit the rest of the file is already in: a
chest holds sixty, a tile of ore holds sixty. So it is also one chestful.

**What it costs**: about **two hundred instructions a frame**, and it does not grow with the
factory — measured with and without the control stage at 340 arms (1,399 → 1,579 instructions a
frame at the median) and at 3,400 (11,983 → 11,947, which is inside the noise). **Nothing was ever
dropped** at either size. That is what one message a frame per kind buys, and the checks watch the
dropped count from both ends so that the day it stops being true is a `FAIL` rather than a silence.

**A `control.rb` that will not run leaves a factory with no goal**, never a game that stopped, and
the game says which line of it went wrong — `control.rb:3`, not `control.rb:?`. Getting that took
one line of wrapping: an inserter's file is `inserter "X" do … end`, so everything in it runs
inside the block the prelude calls and an exception is inside the prelude's own `rescue`; a
control file is written at the top level, so without help its exceptions happen while the *program
is being loaded* and there is nothing left to ask where they were. The game puts the file inside a
method of one line (`in_a_method`) and the prelude calls it from inside the `begin`. The cost is
that a syntax error that leaves a block open is reported a little past the end of the file, which
is the same thing F2 wrote down about half-written lines being reported where the parser gives up.

**Each file gets a class of its own** (`$control_class = Class.new(Control)`), which is the
garden's `creature "Beetle" do … end` with the wrapper word left out: there is one control stage,
so `on` and `goal` are written at the top of the file. Without it a `control.rb` applied over
another would inherit the first one's handlers and goal, because the VM is one VM and
`class Control` is one class in it.

## The numbers

Since F2 there are two places, and the line between them is what the data stage is for.

**The numbers of play are `ruby/data.rb`** — a file of declarations a player edits, read once
before the first frame. **There are none of them in the binary**: the game does not start without
the file, which is the rule S5b-2 set for SabiRuby Battle's match model and the same rule here.

```ruby
map :world, size: [32, 32]
item :iron_ore,   icon: 0
belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2
miner :drill, seconds_per_item: 1.0
chest :crate, capacity: 60
ore :iron_ore, per_tile: 60, patch_radius: 3.0, patches: [2, 2]
inserter :arm, seconds_per_item: 1.0
machine :furnace,   size: [1, 1], sprite: [109],                speed: 1.0
machine :assembler, size: [2, 2], sprite: [138, 139, 140, 141], speed: 1.0
recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 }, time: 2.0, made_in: :furnace
recipe :gear,       in: { iron_plate: 2 }, out: { gear: 1 },     time: 1.0, made_in: :assembler
```

**Nine words and not three.** `item`, `recipe` and `machine` are about what is *made*; `belt`,
`miner`, `chest`, `ore` and `inserter` are the fittings the world has built in, which no recipe
makes and no machine makes them in, and each has numbers of its own with names of its own; `map`
is the world itself. One `machine` shape with five optional fields would let `capacity:` on a
furnace deserialize perfectly and be refused afterwards by hand-written code; a word each means
**serde** refuses it, at the line — which is the whole reason the declarations are read through
`sabiruby_serde::declare` at all.

**`map :world, size: [w, h]` is F3a's**, and the author's sentence behind it is "I want to be able
to change the size freely" — an answer to being asked what the default should be, and the reason
the size became free before any default was settled. It is `size:` rather than a new word because
`machine … size: [2, 2]` already means exactly this — how many tiles this covers, across and up —
and one meaning gets one spelling (the same rule that gave `inserter` the miner's
`seconds_per_item:`). **The two sides need not agree**: 96 by 16 is a valley. The ore's own two
numbers came with it, because the floor under the map is worked out from them: how wide a patch is
(`patch_radius:`) and **how many there are, across and up** (`patches: [2, 2]`, which is the four
corners the game always had, said as the grid they were). `factory.settings.txt` has neither
`map_tiles` nor `ore_patch_radius` any more, and a store written by an older build is told so in
the log rather than having them ignored in silence.

**Two sizes are refused at the line they are written on**, and neither is a size anybody preferred:

```text
data.rb:45: a map 14 tiles across has no room for 2 patches of ore of radius 3 without them
            touching the wall: make it 15 tiles across, or ask for fewer patches, or a smaller patch_radius
data.rb:45: a map 2049 tiles across cannot be drawn: the floor is one texture of one texel a tile
            and 2048 is as wide as one goes
```

The floor is the ore's — a patch on the border ring is ore nobody can see or stand a miner on — and
it is asked of each side separately, `tiles > 2 × n × (radius + 0.5)`, which at two patches is F2's
own `4 × radius + 2` with the 2 that was hidden in it turned into the count. The ceiling is the
picture's: a `TilemapChunk` keeps its tiles in a texture of **one texel per tile**, so a side longer
than the device's `max_texture_dimension_2d` cannot be handed to the GPU at all. That limit differs
from machine to machine — lavapipe here says 16384 and SwiftShader 8192 — so what is refused by is
the one number every WebGL2 machine guarantees, **2048**, which is a map that draws wherever the
page goes. Memory is *not* refused by, because how much there is is a fact about the machine: the
log says what a map costs instead (113 bytes a tile on a PC, 69 in a browser, so 512 by 512 is
17 MB there and 28 here).

**The camera is given the world in `Startup`**, since nothing knows how big it is before the data
stage: the map is where the panning stops, the view at rest is `camera_half_height` **or the map's
height, whichever is less** (a sixteen-tile map showed a strip of nothing above and below it
otherwise), and the wheel's outer limit is raised — never lowered — until the whole map fits, so a
512-tile map can be zoomed out to all of it. The default map's three numbers are what they were.

**The ground is named after what comes out of it** (the author, 2026-09-21). `ore :iron_ore` says
the ground is made of iron ore and a miner standing on it brings up iron ore; the miner has no
say in the matter and no `digs:` to say it with. It was the other way round until then — the
`ore` name was a label and the miner named the item — which put "what comes out of the ground" in
two places, let `ore :coal` sit quietly next to `digs: :iron_ore`, and would have made the *miner*
the thing to edit when a second kind of ground was added. A second kind of ground is a second
`ore` line now, and a name nothing declares is refused at its own line.

**Every number is set against the belt.** A full belt carries `tiles_per_second × items_per_tile`
= 4 items a second, and:

| | rate | of a full belt |
|---|---|---|
| a miner | 1 ore a second | a quarter |
| an inserter | 1 thing a second | a quarter |
| a furnace | 0.5 plates a second | an eighth |
| an assembler | 1 gear a second, eating 2 plates | a quarter |

so one chain is **2 miners → 4 furnaces → 1 assembler → a gear a second**, and four chains fill a
belt with gears. Each line of `data.rb` says what it is a number *of*; `numbers.md` §9.3 has the
table, and the one number with no reason behind it — the belt's own speed — says so rather than
being given a plausible one.

**The numbers that move the game rather than being played by it are still
`factory.settings.txt`**, and the file does not exist until the game writes it; deleting a line
puts that number back to its default.

```text
# Factory: what the game remembers. Delete a line for the default.
camera_half_height=150
camera_snap_zoom=1
window_width=1600
window_height=900
script_budget=39000
script_frame_time_ms=8
```

**`script_budget` is the one number here that came out of an instrument.** rubevy's default is
200,000 and this game says 39,000 instead, by rubevy's own three steps: the scripts run **9,800
instructions a millisecond** (measured at three thousand inserters), a 60 Hz frame is 16.7 ms and
this game has one VM, so a quarter of it is 4 ms, and 4 ms buys 39,000. What that leaves is
twenty times what the inserters that ship use at the biggest factory the default map holds.
`script_frame_time_ms` stays at rubevy's 8 ms as the guard with room, so **the budget is what
bites** — instructions are a fact about the scripts and are the same number in a browser several
times slower. `numbers.md` §9.8 has the table the two came out of, including what happens when
the scripts' first look is *not* spread: 92,589 instructions in one frame instead of 14,040.

Two more keys are the measuring instrument's rather than the game's: `stress_items` and
`stress_arms` (also `--stress N` / `--arms N`, `FACTORY_STRESS` / `FACTORY_ARMS`, `?stress=` /
`?arms=`), and `inserter_stagger`, which is 1 in play and is set to 0 only to show what the
spreading is worth.

**There is nothing of the world's layout left here**, and the line that used to be drawn through
it is gone. F2 kept two of the ore's numbers on this side for a reason that only fitted one of
them ("the world's layout, read once"); F2a moved the other (`ore_per_tile`, so that "a tile of
ore is a chest's worth" stopped spanning two files); and what was left was one reason — the
smallest map the patches fit on is derived from the radius, and that floor was wanted in `main`,
**before there is a VM to have read any Ruby with**. **F3a removed the wanting**: the map is a
declaration too now, so `main` has no map in it at all and the world is made in `Startup`, after
the data stage, by one function (`lay_the_land`) that F5's reload can call again.

**A number that is not more than zero is refused**, said where it is written, and the game does
not start:

```text
data.rb:26: unknown field `colour`, expected `icon` (TypeError)
data.rb:6: -1 is not more than zero (TypeError)
```

A belt speed of zero is not a slow belt and a gap of zero is not a crowded tile; both are a
division. The check is in the declaration rather than inside the arithmetic, because a floor
inside the arithmetic (`1 / items_per_tile.max(0.001)`) is a number with nowhere to have come
from. Seven kinds of mistake are checked, by a test and again by the checks of a running game —
which is how the browser is covered, since a page's compiler names every program `playground.rb`
and the name has to be put back before a player sees it.

**And a gap has to divide a tile** — so `items_per_tile` is one of 1, 2, 4, 8 and 16, and the
rest are refused at their line by a sentence that names what can be written. A tile is sixteen
steps and a gap is `16 ÷ items_per_tile` of them; a gap that does not come out whole is not a
gap. This is the one place F2a narrowed what a data file may say, and what it bought is that
"a jammed tile holds one more than the gaps that fit" is exactly true at all five of them, with
every item exactly a gap behind the one in front. F1 measured what the fraction cost instead:
at `items_per_tile = 3` a jammed tile held three and not four, and at 5 it held six — the count
was decided by which way a chain of roundings fell, not by the number. The test at the bottom of
`src/belts.rs` runs all five and asserts the positions, and the one in `src/data.rs` runs the
refusals.

**Four numbers are not settings**, because moving them does not draw the same picture differently
— it draws a wrong one, or none: the pack's 16 px, how many tiles the sheet has, how big an
item's icon is, and how many icons there are. Nor are the tile numbers and the eight turns of
`src/draw.rs`. **The machines' tile numbers are not among them**: they are `sprite:` in
`data.rb`, because which picture a machine wears is a thing a data file says.

**One number F1 and F2 both could not settle is now the player's to settle**: the map is still
F0's provisional 32 by 32 and still nobody's measurement — what a *real* renderer can draw is not
known, because both renderers on this machine are software — but it is a line of `data.rb` rather
than a number in a file a page cannot write. That is F3a, and it is the author's rule in
`CLAUDE.md`: a measured number is a *default*, and a number nobody has measured is a default with
the reason written down. `numbers.md` §9.6 says what would settle it (one `?stress` run on a real
GPU) rather than inventing it.

**What a big map costs, measured** (`worklog/2026-09-21-factory-F3a.md` §7 and
`…-2026-09-22-factory-F4.md` §1.1): one step of the factory on an *empty* map was 4 µs at 32 by 32,
0.6 ms at 512 by 512 and **5.9 ms at the 2048 the drawing allows** — two walks a frame were the
length of the map rather than the length of the factory. **F4 took both of them out**, and the same
three maps now step in 1 µs each. What a big map still costs is memory (the log says how much) and
whatever the drawing costs, which is the number nobody here can measure.

**And one that is derived rather than set**: how fast the belts' two-frame animation runs. The
pack draws a chevron every 8 px and the second frame is the first with the chevrons moved half of
that, so the picture only reads as one moving belt if a frame is swapped every 4 px the belt
travels — `4 / (tiles_per_second × 16)`. Change the speed in `data.rb` and the animation follows,
which is what keeps the two from disagreeing.

## What the tiles cost

The budget, set here before F1 starts filling the sheet, the way the garden's models were
(`CREDITS.md`, "against the plan's budget of 2 MB in ten"): **64 KB, in one sheet plus the item
icons plus one licence text for each pack used.** Where it comes from: the sheet costs a
**measured 68 bytes a tile** (9,662 bytes for 145 at F0, 9,369 for 139 at F1, 9,612 for 142 at F2,
9,777 for the 143 it has now — 68.4), so even a sheet of 512 tiles — nearly four times what is in it now, and more
than Kenney's whole pack three times over — is about 35 KB. 64 KB is nearly twice that, and **27%
of the smaller of the two games already here** (sabibots 234,202 B, garden 322,924 B, `web.md`),
which is the ceiling the plan sets.

| file | bytes |
|---|---|
| `factory/assets/tiles/factory-tiles.png` | 9,777 |
| `factory/assets/items/items.png` | 414 |
| `factory/assets/tiles/LICENSE-kenney-tiny-factory.txt` | 569 |
| `factory/assets/tiles/LICENSE-kenney-tiny-farm.txt` | 566 |
| **what the page carries** | **11,326 in 4 files**, 17% of the budget |

Five 8 px pictures cost 414 bytes where one cost 180, which is **about 50 bytes a picture** after
the PNG header: at that rate the budget's remaining 53 KB is a thousand of them, and it is not
the small pictures that will run out.

Not in the page: `factory/art/` — the two packs as they came (4,452 and 5,866), their
`Tilesheet.txt` (238 each), their licences (569 and 566) and the drawn belts, ore and machine
(330, 376 and 287) and the inserter's base (306) — 13,228 bytes of source that `web/build.sh`
never copies.

## The entry page

The published site (`web/build.sh all`, which is what CI runs) has had **three** games on it
since F6: <https://sabiruby.github.io/rubevy_games/factory/>. What made it three was two lines —
the word `factory` in `GAMES_ALL` in `web/games.sh`, which is what `all` builds, and the `<li>`
in `web/index.html`, which is what links to it. From F0 to F5 both were written out and held
back, deliberately and in that pair: the array alone would have published a game nothing linked
to, the `<li>` alone would have linked to nothing. **Nothing in
`.github/workflows/pages.yml` had to change at any point**, because it names no games at all — it
runs `web/build.sh all`, and the list of games is one array in one shell file.

What the page differs by is the same block of values every game has (`docs/web.md`): the title,
three colours, the canvas id, and the keys it takes back from the browser. Factory's list of keys
is the garden's — `F1`, `F2`, `F5`, `F9`, `Tab` — because it means by `F5` and `F9` what the
garden means: write the world down and read it back, where a browser means Reload.

## The checks

`docs/verification/selftest-lines.md` has the lists — **twenty-four** lines with no window,
twenty-seven with one, and twenty-eight in a page. **Two move between them**, and both are `--` where the run
could not put the check in a position to measure anything: a headless run spawns no chunk and has
no image loader, and it has no editor either.

**They build a factory the way a player does**, which since F3 means writing a `build::Order` —
the game's own message, carrying a tile, a thing and a way round — rather than forging a mouse.
The same system that carries out a player's order carries out theirs, in a run that has no mouse.
**One tile a frame**, which is now a convenience and not a rule: an order says what was meant, so
five in one frame build five different things; it stays one a frame because then the log says
which tile went wrong. Then they wait for **the game's own numbers** rather than for a number of
seconds — a miner's dig plus four tiles of belt is 3.0 s with what `data.rb` says, measured 2.5 s;
a machine line is `swing + one swing per thing the recipe eats + time ÷ speed + swing + belt`,
which is 5.5 s for both of them, measured 5.6 and 5.4 — and they check that what came out of the
ground is either in a chest or on a belt.

**The five F3 lines are the stage said as checks.** A machine with no arm beside it takes nothing,
so the line stops on the belt; an arm in the gap joins it up and the chest fills; every arm on the
map has a script of its own; one arm given a script that raises stops **and the game knows which
line of which file**, while the other line goes on filling its chest; and an arm taken away leaves
no script behind and nothing parked on a `move` that will never be answered.

**Four more are the panel's**, in a window and a page only. They open an arm the way a player
does, type, and press the three buttons, waiting for **what each one did** — the dearest of those
being the VM holding one more program, which is three frames after the button. In a page that is
the playground's own compiler being called synchronously from inside the frame, which is the one
thing about the editor that only a page can fail.

**And they put eight broken data files through the door the real one went through**, in the game's
own VM, to prove that each is refused at the line it is broken on. That check runs in a page as
well, which is the point of it being a check and not only a test: a browser's compiler names
every program `playground.rb`.

**The five F4 lines are the control stage.** The first says the events reach the script at all.
Then the checks put their own `control.rb` over the one that ships, twice — a goal of one ore,
which the miner's line delivers in a dig and three tiles of belt, and a goal of **more ore than
there is in the whole ground**, which the same factory goes on delivering into without ever
winning (the number is asked of the world rather than written down). Then a `control.rb` that
raises, to show that what is left is a factory with no goal and that the game can still say which
line. And last, **nothing published to the control stage was dropped**, counted from both ends:
the VM's own total and the script's own `Subscription#dropped`. That is the line to watch if the
events are ever made finer, because the grain was chosen by dividing the sixty-four a subscription
holds by what a frame can publish.

**They also ask where the bare ground is** rather than knowing (F4). The checks used to build
their machine lines in the middle of the map because `Ore::laid_out` leaves the middle bare — which
is true of an even number of patches and not of an odd one, and `patches:` is a line of `data.rb`.
Both places that want bare ground now search for it, outwards from the middle, so the default map
builds where it always did and a map whose ore lands in the middle is a map the checks still work
on.

`--shot` and the checks now work together (F0 found they could not): a run that was asked for a
picture stays up for it rather than exiting the moment the checks are done.
