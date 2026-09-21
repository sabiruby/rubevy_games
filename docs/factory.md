# Factory

The third game. **This file says what exists, not what is planned** — the plan is
[`plans/factory-plan.md`](plans/factory-plan.md), and everything this page does not mention is
not written yet.

What exists (stage **F0**, 2026-09-21):

* a crate, `factory/`, in the workspace;
* a **floor**: one `TilemapChunk` — Bevy's own, one draw call for the whole grid — laid out of
  Kenney's Tiny Factory tiles, on a PC and **in a browser**;
* a **camera the player drives**, which is `games_shell::CameraPlugin` and nothing of this
  game's: drag, wheel, `WASD`, `Home`;
* a **click turned into the tile it landed on**, which is the arithmetic F1 builds placing and
  removing on;
* `--headless N`, `--shot`, and the checks (`FACTORY_SELFTEST`, `?selftest`);
* a page, built by `web/build.sh factory`, which is **not on the published site** — see "The
  entry page" below.

What does not exist: conveyors, items, machines, the data stage, the control stage, inserters,
Ruby of any kind, an editor, a guide, a save file. `factory/ruby/prelude.rb` is an empty file
with a note in it.

```
cargo run -p factory                          # a window
cargo run -p factory -- --headless 10         # no window
FACTORY_SELFTEST=1 cargo run -p factory -- --headless 10
docker/build.sh factory release && docker/run.sh factory release   # a window, in the container
web/build.sh factory && web/serve.sh          # then http://localhost:8080/factory/
```

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
numbering, then the four `tools/factory-belts.py` draws, and **blank tiles on the end until
the count is not a multiple of six** — 136 today, which needs none of them.
The game reads the count back out of the loaded image and its checks fail if it ever becomes a
multiple of six again, which is the trap this would otherwise be for whatever a later stage adds
to the sheet.

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

## The numbers

Every number this stage has is a default that `factory.settings.txt` can move; the full table,
with where each default came from, is in [`numbers.md`](numbers.md) §9. The file does not exist
until the game writes it, and deleting a line puts that number back to its default.

```text
# Factory: what the game remembers. Delete a line for the default.
map_tiles=32
camera_half_height=128
window_width=1600
window_height=900
```

Two numbers are **not** settings, because moving them does not draw the same picture differently
— it draws a wrong one, or none: the pack's 16 px, and how many tiles the sheet has.

## What the tiles cost

The budget, set here before F1 starts filling the sheet, the way the garden's models were
(`CREDITS.md`, "against the plan's budget of 2 MB in ten"): **64 KB, in one sheet plus one
licence text for each pack used.** Where it comes from: the sheet costs a **measured 67 bytes a
tile** (9,662 bytes for 145 at F0, and 66.6 for the 136 it has now that (i) is gone), so even a
sheet of 512 tiles — nearly four times what is in it now, and more than Kenney's whole pack three
times over — is about 34 KB. 64 KB is nearly twice that, and **27% of the smaller of the two games
already here** (sabibots 234,202 B, garden 322,924 B, `web.md`), which is the ceiling the plan
sets.

| file | bytes |
|---|---|
| `factory/assets/tiles/factory-tiles.png` | 9,063 |
| `factory/assets/tiles/LICENSE-kenney-tiny-factory.txt` | 569 |
| **what the page carries** | **9,632 in 2 files**, 15% of the budget |

Not in the page: `factory/art/` — the pack as it came (4,452), its `Tilesheet.txt` (238), its
licence (569) and the drawn belts (330) — 5,589 bytes of source that `web/build.sh` never copies.

## The entry page

The published site (`web/build.sh all`, which is what CI runs) has **two** games on it. Factory's
page builds — `web/build.sh factory`, and `/factory/` from a local `web/serve.sh` — and its
values are in `web/games.sh`, but the word `factory` is deliberately **not** in `GAMES_ALL`, and
its `<li>` in `web/index.html` is written out and commented. Stage F6 takes both out of their
brackets at once. Nothing in `.github/workflows/pages.yml` had to change: it names no games, it
runs `web/build.sh all`.

## The checks

`docs/verification/selftest-lines.md` has the lists — four lines with no window, four with one,
and five in a page. The one that moves between them is about the picture: a headless run spawns
no chunk and has no image loader, so it says `--` rather than claiming a check that never ran.
