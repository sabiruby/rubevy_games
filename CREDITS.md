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

## Font

The text is Bevy's built-in default font (Fira Mono, SIL Open Font License 1.1), which ships with
the engine; nothing is added to this repository for it.

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
