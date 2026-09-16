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
`crates/rubevy-arena/assets/fonts/OFL.txt`, beside the file, exactly as it comes from
[google/fonts `ofl/notosansjp`](https://github.com/google/fonts/tree/main/ofl/notosansjp)
(downloaded 2026-09-17).

| file | what it is |
|---|---|
| `crates/rubevy-arena/assets/fonts/NotoSansJP-Guide.subset.ttf` | 62,780 bytes: Noto Sans JP pinned to `wght=400` and subset to the 327 characters the guides use |
| `crates/rubevy-arena/assets/fonts/OFL.txt` | its licence |

The source font is 9,589,900 bytes; what is in each binary is 0.65% of that. `tools/subset-font.sh`
is the recipe — it reads the characters out of `garden/src/guide_text.rs`,
`sabibots/src/guide_text.rs` and `crates/rubevy-arena/src/guide.rs` and cuts the font again, which
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
