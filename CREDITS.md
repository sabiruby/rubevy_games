# Credits

## Art

**Top-down Tanks Remastered** by Kenney (<https://kenney.nl>), **CC0 1.0 Universal (public
domain)**. The files used here are in `sabibots/assets/sprites/`, with the pack's own licence
text beside them as `LICENSE-kenney.txt`:

| file | used as |
|---|---|
| `tankBody_blue_outline.png`, `tankBody_red_outline.png` | the robots |
| `tankBody_dark_outline.png` | a robot that is down |
| `bulletBlue1_outline.png`, `bulletRed1_outline.png` | their shots |
| `tileSand1.png`, `tileSand2.png`, `tileGrass1.png` | the floor |
| `crateMetal.png` | the arena wall |
| `treeBrown_large.png`, `explosion1-3.png` | not used yet (cover, hits) |

CC0 asks for nothing, but saying where the art came from is the decent thing to do, and it is
how anyone who wants more of it finds the pack.

## Tiles (Factory)

**Tiny Factory 1.0** by Kenney (<https://kenney.nl/assets/tiny-factory>), **CC0 1.0 Universal
(public domain)**, downloaded 2026-09-21 from the pack's own zip. 16 × 16 tiles; the pack's
`Tilemap/tilemap_packed.png` is 12 × 11 = 132 of them.

The pack as it came is in `factory/art/`, with its own licence text beside it as
`LICENSE-kenney-tiny-factory.txt` and its `Tilesheet.txt` as `Tilesheet-tiny-factory.txt`.
**What the game loads is one file built from it** by `tools/factory-tileset.py` — a vertical
strip of the same 132 tiles, in the pack's own order and numbering, and the drawn ones after
them, with blank tiles on the end **when the count needs them** (the script's own comment says
why: a browser will not bind an array texture whose square layers number a multiple of six; at
143 layers it needs none). The strip is `factory/assets/tiles/factory-tiles.png`, with a copy of
the licence text beside it, because that is the file that redistributes the art.

| tile of the pack | used as |
|---|---|
| 0, 1, 2 | the factory floor plates |
| 3 | the plain ground, which F0 lays as a border |
| 110 | a machine front of orange blocks between grey posts, which F1 places as **the miner** (the pack names none of its tiles) |
| 85 | a wooden crate, which F1 places as **the chest** |
| 109 | a copper arch, which F2's `ruby/data.rb` names as **the furnace**'s picture |
| 99 | the green cabinet, **stretched** to two tiles by two for the assembler (below) |
| the rest | in the sheet, not used yet: the other machine fronts, the pipes and the gear that later stages will place. `docs/factory.md` has the numbered table |

Tiles **132–135** of the same sheet are not Kenney's: they are the conveyor straight and corner,
seen from straight above, drawn by `tools/factory-belts.py` in the pack's own palette and
cross-section, **MIT, the same as the rest of this repository** (`LICENSE`). The pack has no
corner at all, and the author chose on 2026-09-21 to have the belts drawn from above rather than
in the pack's three-quarter view, which is what makes four pictures enough for every direction
and every turn (`docs/factory.md`).

**Tiny Farm 1.0** by Kenney (<https://kenney.nl/assets/tiny-farm>), **CC0 1.0 Universal (public
domain)**, downloaded 2026-09-21 from the pack's own zip, for one thing: **the ore**. Tiny Factory
has none, and this pack's rocks — tiles **77 and 89** of its `Tilemap/tilemap_packed.png` — are
drawn in the same palette as Tiny Factory, so `tools/factory-ore.py` recolours them from their
grey ramp onto Tiny Factory's orange and sets them on its ground tile. They are tiles **136 and
137** of the sheet the game loads: plenty of ore left, and nearly gone. The pack as it came is in
`factory/art/kenney_tiny-farm_tilemap_packed.png` with `LICENSE-kenney-tiny-farm.txt` and
`Tilesheet-tiny-farm.txt` beside it, and the licence text is beside the sheet in
`factory/assets/tiles/` as well, because that is the file that redistributes the art.

Tiles **138–141** are the **assembler**, two tiles by two. The pack has no machine bigger than one
tile — 75, 87, 99 and 111 are the same cabinet in four colours, each complete with its own edges —
so `tools/factory-machine.py` **nine-slices** tile 99: the four pixels at each edge kept as they
are and the band between them blown up by exactly three. **Every pixel of it is a pixel of
Kenney's** and it uses no colour the tile did not, which is a stronger promise than "the same
palette"; the arrangement is this repository's, **MIT** (`LICENSE`).

Tile **142** is the **inserter's base**, drawn by `tools/factory-inserter.py` in the pack's own
greys and its own orange rail: a plinth with a light pivot in the middle and an orange mouth on
the side things are put down on. The pack has nothing that reads as an arm on a post, and the
base has to be **asymmetric left to right** or turning it would draw the same picture four times
(the script refuses to write one that is not). The arm itself is not a tile at all — a tile has
eight orientations and nothing in between, and an arm moves — so it is a sprite. **MIT, the same
as the rest of this repository.**

**The 8 px pictures** (`factory/assets/items/items.png`, five of them in one strip) are not
Kenney's either. Three are the items: two of those have to sit on a 16 px tile without touching,
and a 16 px rock does not survive being halved, so `tools/factory-items.py` draws them pixel by
pixel — the ore nugget in the four oranges the ground ore was recoloured into (F1 drew it and it
has not changed), the plate and the gear in Tiny Factory's own greys, the gear being the pack's
tile 114 read by eye and redrawn at half the size. The other two are F3's and are **not items**:
the inserter's hand, drawn travelling with whatever it is carrying, and the orange mark that sits
over an inserter whose script has stopped. **MIT, the same as the rest of this repository.**

CC0 asks for nothing, but saying where the art came from is the decent thing to do, and it is how
anyone who wants more of it finds the pack.

## Models (the Garden)

Two more packs by **Kenney** (<https://kenney.nl>), both **CC0 1.0 Universal (public domain)**.
Each pack's own licence text is in `garden/assets/models/` beside the files, as
`LICENSE-kenney-nature-kit.txt` and `LICENSE-kenney-cube-pets.txt`.

**Nature Kit 2.1** (<https://kenney.nl/assets/nature-kit>), downloaded 2026-09-17, glTF-binary
from the pack's own `Models/GLTF format/`:

| file | used as |
|---|---|
| `grass.glb` | a tuft of grass — the `Plant` a creature eats |
| `plant_bush.glb` | the rounder sort of the same |
| `tree_default.glb` | a `Tree`: an obstacle, not food |
| `rock_smallA.glb` | a `Rock` |

**Cube Pets 2.0** (<https://kenney.nl/assets/cube-pets>), downloaded 2026-09-17, from
`Models/GLB format/`. These are node-animated (no skeleton) and carry eight clips each — `static`,
`idle`, `walk`, `run`, `eat`, `dance` and two gestures — of which the garden plays three:

| file | used as |
|---|---|
| `animal-bunny.glb` | the Rabbit |
| `animal-crab.glb` | the Beetle — the pack has no beetle, and a crab is the nearest thing in it: a shell, six legs and a scuttle. It is the only model here that is not what it is called |
| `Textures/colormap.png` | the one palette both of them index into, referenced from inside the `.glb` by that relative path |

Sizes and triangle counts are in `docs/garden.md`; the whole set is 314 KiB in seven files,
against the plan's budget of 2 MB in ten.

## Fonts

Bevy's own text is its built-in default font (Fira Mono, SIL Open Font License 1.1), which ships
with the engine, and the panels drawn over egui use egui's defaults (Ubuntu-Light and two Noto
symbol faces, likewise shipped with it). Nothing is added to this repository for either.

One font is. The in-game guide (`H`) is written in English **and Japanese**, and none of the fonts
above has a single CJK character in it:

**Noto Sans JP**, by Google, **SIL Open Font License 1.1**. The licence text is
`crates/games-shell/assets/fonts/OFL.txt`, beside the file, exactly as it comes from
[google/fonts `ofl/notosansjp`](https://github.com/google/fonts/tree/main/ofl/notosansjp)
(downloaded 2026-09-17).

| file | what it is |
|---|---|
| `crates/games-shell/assets/fonts/NotoSansJP-Guide.subset.ttf` | 81,480 bytes: Noto Sans JP pinned to `wght=400` and subset to the characters the three guides use — 396 glyphs, re-cut for Factory's guide (F5) |
| `crates/games-shell/assets/fonts/OFL.txt` | its licence |

The source font is 9,589,900 bytes; what is in each binary is 0.85% of that. `tools/subset-font.sh`
is the recipe — it reads the characters out of `garden/src/guide_text.rs`,
`sabibots/src/guide_text.rs`, `factory/src/guide_text.rs` and `crates/games-shell/src/guide.rs`
and cuts the font again, which has to be done whenever the Japanese is edited. The OFL allows the font to be modified and bundled
(this is a subset, not a renamed font: the name records say "Noto Sans JP Subset" and the copyright
and licence records are Noto's own, unchanged). What the size costs the browser build is measured
in `docs/web.md`.

## In the browser build

The page (`web/dist/`, built by `web/build.sh`) also carries two things from
[sabiruby-playground](https://github.com/sabiruby/sabiruby-playground), copied at build time and
not kept in this repository:

* `compiler/sabiruby.wasm` and `compiler/sabi.js` — the SabiRuby playground's module, with the
  reference mruby compiler (MIT, see `LICENSE-mruby` of SabiRuby) and Prism (MIT) inside.
* `compiler/vendor/browser_wasi_shim/` — [browser_wasi_shim](https://github.com/bjorn3/browser_wasi_shim),
  MIT OR Apache-2.0; its licence files are copied with it.

## Code

MIT (`LICENSE`), © 2026 Kishima Craft Works. The VM is
[SabiRuby](https://github.com/sabiruby/sabiruby) and the plugin is
[rubevy](https://github.com/sabiruby/rubevy), both MIT and by the same author.
