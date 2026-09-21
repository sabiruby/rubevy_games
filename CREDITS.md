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
strip of the same 132 tiles, in the pack's own order and numbering, with blank tiles on the end
(the script's own comment says why: a browser will not bind an array texture whose square layers
number a multiple of six). The strip is `factory/assets/tiles/factory-tiles.png`, with a copy of
the licence text beside it, because that is the file that redistributes the art.

| tile of the pack | used as |
|---|---|
| 0, 1, 2 | the factory floor plates |
| 3 | the plain ground, which F0 lays as a border |
| 26 and 16, and their mirrors | the conveyors running right, left, up and down in F0a's mock-up |
| 75–77, 87–89, 99–101, 111–113 | the four machines standing beside it |
| the rest | in the sheet, not used yet: the crates, the pipes and the gear that F1 onwards will place. `docs/factory.md` has the numbered table |

Tiles **132–143** of the same sheet are not Kenney's: they are the conveyor corners the pack has
none of, drawn by `tools/factory-belts.py` in the pack's own palette and cross-section, **MIT,
the same as the rest of this repository** (`LICENSE`). They exist so that the author can choose
between two ways of drawing a belt (`docs/factory.md`); F1 keeps one of them.

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
| `crates/games-shell/assets/fonts/NotoSansJP-Guide.subset.ttf` | 65,904 bytes: Noto Sans JP pinned to `wght=400` and subset to the 334 characters the guides use (G7 re-cut it) |
| `crates/games-shell/assets/fonts/OFL.txt` | its licence |

The source font is 9,589,900 bytes; what is in each binary is 0.65% of that. `tools/subset-font.sh`
is the recipe — it reads the characters out of `garden/src/guide_text.rs`,
`sabibots/src/guide_text.rs` and `crates/games-shell/src/guide.rs` and cuts the font again, which
has to be done whenever the Japanese is edited. The OFL allows the font to be modified and bundled
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
