# Factory

The third game. **This file says what exists, not what is planned** — the plan is
[`plans/factory-plan.md`](plans/factory-plan.md), and everything this page does not mention is
not written yet.

What exists (stages **F0** and **F1**, 2026-09-21):

* a crate, `factory/`, in the workspace;
* a **floor**: one `TilemapChunk` — Bevy's own, one draw call for the whole grid — laid out of
  Kenney's Tiny Factory tiles, on a PC and **in a browser**, with **ore** in it;
* a **grid you build on**: one tile holds one building, a click puts one down and a click with
  nothing in hand takes it away;
* **conveyors** that carry, jam, merge and turn; **miners** that dig the ore they stand on;
  **chests** that hold what arrives;
* a **camera the player drives** (`games_shell::CameraPlugin`: drag, wheel, `WASD`, `Home`)
  whose zoom is **rounded to a whole number of screen pixels per pixel of the art**;
* `--headless N`, `--shot`, `--stress N`, and the checks (`FACTORY_SELFTEST`, `?selftest`).

What does not exist: the data stage, the control stage, inserters, **Ruby of any kind**, an
editor, a HUD, a guide, a save file. `factory/ruby/prelude.rb` is an empty file with a note in it.

```
cargo run -p factory                          # a window
cargo run -p factory -- --headless 10         # no window
cargo run -p factory -- --stress 4000         # a loop of belt, measured (sizes its own map)
FACTORY_SELFTEST=1 cargo run -p factory -- --headless 10
docker/build.sh factory release && docker/run.sh factory release   # a window, in the container
web/build.sh factory && web/serve.sh          # then http://localhost:8080/factory/
```

## Playing it

There is no window furniture yet — the editor, the HUD and the guide are F5 — so this is the
whole of it, and the game says it in the log when it starts:

| | |
|---|---|
| `1` `2` `3` | a belt, a miner, a chest in hand |
| `0` | nothing in hand: a click takes away what is there |
| `R` | turn what is in hand a quarter turn anticlockwise |
| click | build it, or take it away |
| drag, wheel, `WASD`, `Home` | the camera |

**A miner has to stand on ore**, which is what makes a patch worth finding; everything else goes
anywhere, and building over something replaces it. A belt carries what is on it the way it faces
and takes things from any side but its own front, so two belts pointing at each other jam rather
than passing the same item back and forth. A miner puts what it digs into whatever it faces — a
belt or a chest — and if there is nowhere to put it, **it keeps the dig and takes nothing out of
the ground** until there is.

## What one step of the factory is

`factory/src/belts.rs`, and the tests at the bottom of it run the whole of it with no `App` at
all. A belt tile is one unit long and an item on it is a number in `0..=1`: how far through that
tile it has come. A lane is the items on one tile, front first, and **every item is at least a
gap behind the one in front of it**. That invariant is the model: carrying is adding a step to
each number, jamming is a number that cannot grow, merging is two tiles handing items to one and
each finding the gap the other left.

Four passes, and their order is the model:

1. **the tails as they were** — what a belt may push into its neighbour is read from a snapshot
   taken before anything moved, so two belts merging into a third both see the same picture and
   the answer does not depend on which tile is walked first;
2. **carry** — every item forward by a step, the front one no further than the room ahead of it;
3. **hand over** — an item at the end of its tile goes to the next one **if there is still room
   when its own turn comes**, which is where a merge is decided (the lower tile index gets the
   gap) and where a chest takes an item off the belt;
4. **dig** — a miner that has finished puts an item down, or holds it.

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
| 114 | a **gear** |
| 6, 74, 86, 90, 91 | a bench, two tanks, and an orange head on a stand |
| 132, 133 | a conveyor **straight**, seen from above, frames A and B (`tools/factory-belts.py`) |
| 134, 135 | a conveyor **corner** — in at the left, out at the top — frames A and B |
| 136, 137 | **ore in the ground**: plenty left, and nearly gone (`tools/factory-ore.py`) |
| 138 | blank — the padding tile the paragraph above is about |

Of Kenney's own, F1 places **110** (the drilling rig) as the miner and **85** (a wooden crate) as
the chest. The pack's conveyor tiles (24–27, 4/16/28 and the railed ones) are in the sheet and
**not used**: they are three-quarter pictures and the belts are drawn from above.

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
the script draws it in the same four colours (`factory/assets/items/ore.png`).

## The numbers

Every number the factory is played by is a line of `factory.settings.txt`; the full table, with
where each default came from, is in [`numbers.md`](numbers.md) §9. The file does not exist until
the game writes it, and deleting a line puts that number back to its default.

```text
# Factory: what the game remembers. Delete a line for the default.
map_tiles=32
camera_half_height=150
camera_snap_zoom=1
window_width=1600
window_height=900
belt_tiles_per_second=2
items_per_tile=2
mine_seconds=1
chest_capacity=60
ore_per_tile=60
ore_patch_radius=3
```

**The six at the bottom are numbers of play, and they are only here for now**: F2's data stage is
where they belong (`ruby/data.rb`), which is the whole reason a data stage is in the plan. Four
of the six are set against each other rather than picked — a full belt carries
`belt_tiles_per_second × items_per_tile` = 4 items a second, a miner is a quarter of that so that
four of them fill one belt, a chest is a minute of one miner, a tile of ore is one chestful — and
the one with no reason behind it, the belt's speed, says so in `numbers.md` rather than being
given a plausible one.

**Two numbers are not settings**, because moving them does not draw the same picture differently
— it draws a wrong one, or none: the pack's 16 px, and how many tiles the sheet has. Nor are the
tile numbers and the eight turns of `src/draw.rs`.

**One number F1 could not settle**: `map_tiles` is still F0's provisional 32. The plan says it
comes from how many machines have to fit, and neither of the two things that would say is known
— what a *real* renderer can draw (both renderers on this machine are software, and the browser's
drew 900 belts at the same 5 frames a second with two items on them as with eighteen hundred),
and how many scripted inserters a frame holds, which is F3's measurement. `numbers.md` §9.6 says
so rather than inventing a number.

**And one that is derived rather than set**: how fast the belts' two-frame animation runs. The
pack draws a chevron every 8 px and the second frame is the first with the chevrons moved half of
that, so the picture only reads as one moving belt if a frame is swapped every 4 px the belt
travels — `4 / (belt_tiles_per_second × 16)`. Change the speed and the animation follows, which
is what keeps the two from disagreeing.

## What the tiles cost

The budget, set here before F1 starts filling the sheet, the way the garden's models were
(`CREDITS.md`, "against the plan's budget of 2 MB in ten"): **64 KB, in one sheet plus one
licence text for each pack used.** Where it comes from: the sheet costs a **measured 67 bytes a
tile** (9,662 bytes for 145 at F0, and 67.4 for the 139 it has now), so even a
sheet of 512 tiles — nearly four times what is in it now, and more than Kenney's whole pack three
times over — is about 34 KB. 64 KB is nearly twice that, and **27% of the smaller of the two games
already here** (sabibots 234,202 B, garden 322,924 B, `web.md`), which is the ceiling the plan
sets.

| file | bytes |
|---|---|
| `factory/assets/tiles/factory-tiles.png` | 9,369 |
| `factory/assets/items/ore.png` | 180 |
| `factory/assets/tiles/LICENSE-kenney-tiny-factory.txt` | 569 |
| `factory/assets/tiles/LICENSE-kenney-tiny-farm.txt` | 566 |
| **what the page carries** | **10,684 in 4 files**, 16% of the budget |

Not in the page: `factory/art/` — the two packs as they came (4,452 and 5,866), their
`Tilesheet.txt` (238 each), their licences (569 and 566) and the drawn belts and ore (330 and
376) — 12,635 bytes of source that `web/build.sh` never copies.

## The entry page

The published site (`web/build.sh all`, which is what CI runs) has **two** games on it. Factory's
page builds — `web/build.sh factory`, and `/factory/` from a local `web/serve.sh` — and its
values are in `web/games.sh`, but the word `factory` is deliberately **not** in `GAMES_ALL`, and
its `<li>` in `web/index.html` is written out and commented. Stage F6 takes both out of their
brackets at once. Nothing in `.github/workflows/pages.yml` had to change: it names no games, it
runs `web/build.sh all`.

## The checks

`docs/verification/selftest-lines.md` has the lists — eight lines with no window, eight with one,
and nine in a page. The one that moves between them is about the picture: a headless run spawns
no chunk and has no image loader, so it says `--` rather than claiming a check that never ran.

**They build a factory with clicks**, because that is the road a player takes and the only road
there is: the checks write a `WorldClick` and the same system that answers a mouse answers them.
A run with no window has no mouse, which is why the message is registered there too. Then they
wait for **the game's own numbers** rather than for a number of seconds — a miner's dig plus four
tiles of belt is 3.0 s with the defaults, the run takes 2.3 to 2.4 s of it — and they check that
what came out of the ground is either in a chest or on a belt.

`--shot` and the checks now work together (F0 found they could not): a run that was asked for a
picture stays up for it rather than exiting the moment the checks are done.
