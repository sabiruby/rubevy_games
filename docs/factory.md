# Factory

The third game. **This file says what exists, not what is planned** — the plan is
[`plans/factory-plan.md`](plans/factory-plan.md), and everything this page does not mention is
not written yet.

What exists (stages **F0**, **F1** and **F2**, 2026-09-21):

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
* `--headless N`, `--shot`, `--stress N`, and the checks (`FACTORY_SELFTEST`, `?selftest`).

What does not exist: the control stage, inserters, an editor, a HUD, a guide, a save file.
`factory/ruby/prelude.rb` is still an empty file with a note in it — the data stage needed
nothing in it, and why is written there.

```
cargo run -p factory                          # a window
cargo run -p factory -- --headless 20         # no window
cargo run -p factory -- --stress 4000         # a loop of belt, measured (sizes its own map)
FACTORY_SELFTEST=1 cargo run -p factory -- --headless 20
docker/build.sh factory release && docker/run.sh factory release   # a window, in the container
web/build.sh factory && web/serve.sh          # then http://localhost:8080/factory/
```

## Playing it

There is no window furniture yet — the editor, the HUD and the guide are F5 — so this is the
whole of it, and the game says it in the log when it starts:

| | |
|---|---|
| `1` `2` `3` | a belt, a miner, a chest in hand |
| `4` onwards | the machines `data.rb` declares, in the order it declares them (`4` the furnace, `5` the assembler) — the game prints the list when it starts |
| `0` | nothing in hand: a click takes away what is there |
| `R` | turn what is in hand a quarter turn anticlockwise |
| click | build it, or take it away |
| drag, wheel, `WASD`, `Home` | the camera |

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

Four passes, and their order is the model:

1. **the tails as they were** — what a belt may push into its neighbour is read from a snapshot
   taken before anything moved, so two belts merging into a third both see the same picture and
   the answer does not depend on which tile is walked first;
2. **carry** — every item forward by a step, the front one no further than the room ahead of it;
3. **hand over** — an item at the end of its tile goes to the next one **if there is still room
   when its own turn comes**, which is where a merge is decided (the lower tile index gets the
   gap) and where a chest takes an item off the belt;
4. **dig** — a miner that has finished puts an item down, or holds it;
5. **deliver** — a machine pushes what it has made into what it faces, one item at a time;
6. **craft** — a machine with nothing waiting to go out takes a recipe it has the parts for,
   consumes them, and works at it for `time / speed`.

Deliver is before craft so that a machine that finished last frame is empty again when it is asked
whether it can start; a machine still holding what it made does not start another, which is
exactly the rule a blocked miner keeps with its finished dig. A machine holds **one craft's worth
in and one out** — the smallest buffer that lets it run without a gap, which is why it is not a
number anybody chose.

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

## The numbers

Since F2 there are two places, and the line between them is what the data stage is for.

**The numbers of play are `ruby/data.rb`** — a file of declarations a player edits, read once
before the first frame. **There are none of them in the binary**: the game does not start without
the file, which is the rule S5b-2 set for SabiRuby Battle's match model and the same rule here.

```ruby
item :iron_ore,   icon: 0
belt :conveyor, tiles_per_second: 2.0, items_per_tile: 2
miner :drill, seconds_per_item: 1.0, digs: :iron_ore
chest :crate, capacity: 60
ore :patch, per_tile: 60
machine :furnace,   size: [1, 1], sprite: [109],                speed: 1.0
machine :assembler, size: [2, 2], sprite: [138, 139, 140, 141], speed: 1.0
recipe :iron_plate, in: { iron_ore: 1 }, out: { iron_plate: 1 }, time: 2.0, made_in: :furnace
recipe :gear,       in: { iron_plate: 2 }, out: { gear: 1 },     time: 1.0, made_in: :assembler
```

**Seven words and not three.** `item`, `recipe` and `machine` are about what is *made*; `belt`,
`miner`, `chest` and `ore` are the fittings the world has built in, which no recipe makes and no
machine makes them in, and each has numbers of its own with names of its own. One `machine` shape
with five optional fields would let `capacity:` on a furnace deserialize perfectly and be refused
afterwards by hand-written code; a word each means **serde** refuses it, at the line — which is
the whole reason the declarations are read through `sabiruby_serde::declare` at all.

**Every number is set against the belt.** A full belt carries `tiles_per_second × items_per_tile`
= 4 items a second, and:

| | rate | of a full belt |
|---|---|---|
| a miner | 1 ore a second | a quarter |
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
map_tiles=32
camera_half_height=150
camera_snap_zoom=1
window_width=1600
window_height=900
ore_patch_radius=3
```

**Why `ore_patch_radius` did not go into `data.rb` with the rest.** The smallest map the four
patches fit on without touching the border is derived from it, and that floor is wanted in
`main`, **before there is a VM to have read any Ruby with**. That is where the line is drawn, and
F2a is where it got drawn in the right place: F2 kept `ore_per_tile` on this side of it too, for
a reason that only fitted the radius ("the world's layout, read once"), and the cost was that
"a tile of ore is a chest's worth" spanned two files, so moving the chest moved nothing. It is
`ore :patch, per_tile: 60` in `data.rb` now, next to the chest it is a chest's worth of.

**A number that is not more than zero is refused**, said where it is written, and the game does
not start:

```text
data.rb:26: unknown field `colour`, expected `icon` (TypeError)
data.rb:6: -1 is not more than zero (TypeError)
```

A belt speed of zero is not a slow belt and a gap of zero is not a crowded tile; both are a
division. The check is in the declaration rather than inside the arithmetic, because a floor
inside the arithmetic (`1 / items_per_tile.max(0.001)`) is a number with nowhere to have come
from. Six kinds of mistake are checked, by a test and again by the checks of a running game —
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

**One number F1 and F2 both could not settle**: `map_tiles` is still F0's provisional 32. The plan
says it comes from how many machines have to fit, and neither of the two things that would say is
known — what a *real* renderer can draw (both renderers on this machine are software), and how
many scripted inserters a frame holds, which is F3's measurement. F2 added one piece of it: a
chain is 7 machines and 4 chains fill a belt, so the question is now "how many chains".
`numbers.md` §9.6 says so rather than inventing a number.

**And one that is derived rather than set**: how fast the belts' two-frame animation runs. The
pack draws a chevron every 8 px and the second frame is the first with the chevrons moved half of
that, so the picture only reads as one moving belt if a frame is swapped every 4 px the belt
travels — `4 / (tiles_per_second × 16)`. Change the speed in `data.rb` and the animation follows,
which is what keeps the two from disagreeing.

## What the tiles cost

The budget, set here before F1 starts filling the sheet, the way the garden's models were
(`CREDITS.md`, "against the plan's budget of 2 MB in ten"): **64 KB, in one sheet plus the item
icons plus one licence text for each pack used.** Where it comes from: the sheet costs a
**measured 67 bytes a tile** (9,662 bytes for 145 at F0, 9,369 for 139 at F1, 9,612 for the 142
it has now — 67.7), so even a sheet of 512 tiles — nearly four times what is in it now, and more
than Kenney's whole pack three times over — is about 35 KB. 64 KB is nearly twice that, and **27%
of the smaller of the two games already here** (sabibots 234,202 B, garden 322,924 B, `web.md`),
which is the ceiling the plan sets.

| file | bytes |
|---|---|
| `factory/assets/tiles/factory-tiles.png` | 9,612 |
| `factory/assets/items/items.png` | 317 |
| `factory/assets/tiles/LICENSE-kenney-tiny-factory.txt` | 569 |
| `factory/assets/tiles/LICENSE-kenney-tiny-farm.txt` | 566 |
| **what the page carries** | **11,064 in 4 files**, 17% of the budget |

Three items cost 317 bytes where one cost 180, which is **46 bytes an icon** after the PNG
header: at that rate the budget's remaining 53 KB is a thousand items, and it is not the icons
that will run out.

Not in the page: `factory/art/` — the two packs as they came (4,452 and 5,866), their
`Tilesheet.txt` (238 each), their licences (569 and 566) and the drawn belts, ore and machine
(330, 376 and 287) — 12,922 bytes of source that `web/build.sh` never copies.

## The entry page

The published site (`web/build.sh all`, which is what CI runs) has **two** games on it. Factory's
page builds — `web/build.sh factory`, and `/factory/` from a local `web/serve.sh` — and its
values are in `web/games.sh`, but the word `factory` is deliberately **not** in `GAMES_ALL`, and
its `<li>` in `web/index.html` is written out and commented. Stage F6 takes both out of their
brackets at once. Nothing in `.github/workflows/pages.yml` had to change: it names no games, it
runs `web/build.sh all`.

## The checks

`docs/verification/selftest-lines.md` has the lists — thirteen lines with no window, thirteen
with one, and fourteen in a page. The one that moves between them is about the picture: a
headless run spawns no chunk and has no image loader, so it says `--` rather than claiming a
check that never ran.

**They build a factory with clicks**, because that is the road a player takes and the only road
there is: the checks write a `WorldClick` and the same system that answers a mouse answers them.
A run with no window has no mouse, which is why the message is registered there too. **One tile a
frame**, because the system that answers a click reads what is in hand when it runs and not when
the click was written. Then they wait for **the game's own numbers** rather than for a number of
seconds — a miner's dig plus four tiles of belt is 3.0 s with what `data.rb` says, measured 2.5 s;
a furnace's belt and craft and belt is 3.5 s, measured 3.5 — and they check that what came out of
the ground is either in a chest or on a belt.

**And they put six broken data files through the door the real one went through**, in the game's
own VM, to prove that each is refused at the line it is broken on. That check runs in a page as
well, which is the point of it being a check and not only a test: a browser's compiler names
every program `playground.rb`.

`--shot` and the checks now work together (F0 found they could not): a run that was asked for a
picture stays up for it rather than exiting the moment the checks are done.
