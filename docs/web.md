# The browser build

SabiRuby Battle runs in a browser as well as in a window on a PC:
**https://kishima.github.io/rubevy_games/**. The two are the same game — the same systems, the same
Ruby, the same editor — and what differs is chosen by the target at compile time.

```
cargo run -p sabibots          # PC
web/build.sh && web/serve.sh   # browser, at http://localhost:8080/
```

## What differs, and where

Everything that differs is in `sabibots/src/platform.rs`, one module per target behind
`cfg(target_arch = "wasm32")`, with the same functions in both:

| | PC | browser |
|---|---|---|
| `ruby_dir`, `read` | the files under `sabibots/ruby/` | the same files, built into the binary (`build.rs` → `include_str!`); a brain saved in this browser wins over the built-in one |
| `write` (Save) | the file | `localStorage`, key `sabibots:ruby/robots/scout.rb` |
| `compile` | `sabiruby-compiler`, linked in | `window.sabibotsCompile`, a second wasm module (below) |
| `clock_seed` | `SystemTime` | `Date.now()` (`SystemTime::now` panics on `wasm32-unknown-unknown`) |
| `assets_dir` | `sabibots/assets` found from the crate | `assets/` next to the page |
| `SAVE_LABEL` | Save to file | Save in browser |

And outside that module:

* `sabibots/Cargo.toml` adds per target what Bevy needs: `multi_threaded` and `x11` on a PC,
  `web` and `webgl2` in a browser (the workspace's Bevy has only what both share). The PC build
  also takes `sabiruby-compiler`; the browser build takes `wasm-bindgen`, `js-sys`, `web-sys`.
* `rubevy-arena`'s `Watch` has a browser version whose `new` answers `None` (there is no directory
  to watch), so the game's reload system is the same code and simply never reloads.
* The window is given a canvas (`#sabibots`, `fit_canvas_to_parent`), which a PC ignores.
* `--headless`, `--shot` and `SABIBOTS_SELFTEST` compile in both but mean nothing in a page.

## Two wasm modules

The game's VM is Rust and builds for any target. The compiler is the reference mruby compiler,
which is C with `setjmp`/`longjmp`, and `wasm32-unknown-unknown` — what Bevy and wasm-bindgen use
in a browser — has no C library for it. The SabiRuby playground already builds exactly that
compiler for the browser, as a `wasm32-wasip1` module with wasi-sdk. So the page loads both:

```
index.html
  compiler/sabiruby.wasm  (wasm32-wasip1: the compiler, and a VM the game does not use)
      ▲ sabi.js + browser_wasi_shim
      │ window.sabibotsCompile(source) → RITE bytes  (sabi_compile, sabi_take_binary)
  pkg/game_bg.wasm        (wasm32-unknown-unknown: Bevy, egui, rubevy, the SabiRuby VM)
```

The compiler module is loaded first and the call is synchronous, so to the game `compile` is an
ordinary function that returns bytes or an error message — the same shape as on a PC. The only
thing added to the playground for this was `sabi_take_binary`, the compiled bytes. The compiler
module is ~2.4 MB (0.8 MB gzipped) and carries a VM of its own that the game does not use; a
compiler-only module would be smaller, and is not worth a second C build yet.

In CI (`.github/workflows/pages.yml`) the compiler module is built from sabiruby-playground at a
pinned commit, against the SabiRuby commit `Cargo.lock` names for the game, so the bytecode the
compiler writes and the VM that reads it come from the same source.

## Keys

A browser wants some of the game's keys for itself: F5 reloads, Ctrl+S opens a save dialog, Tab
moves the focus. The page takes them first (`keydown` in the capture phase, `preventDefault`) and
the game still sees them, so Apply is F5 and Save is Ctrl+S in both builds.

## Size

| file | bytes | gzipped |
|---|---|---|
| `pkg/game_bg.wasm` | 26.7 MB | 8.1 MB |
| `compiler/sabiruby.wasm` | 2.4 MB | 0.8 MB |

The game is built with the `web` profile (release, `opt-level = "s"`, thin LTO, stripped) and
`wasm-opt -Os`. Without stripping it was 60 MB, almost half of it the name section. Most of what is
left is Bevy's renderer; trimming its features (the game draws sprites and egui, nothing 3D) is
the next thing to try.

`wasm-opt` is given the features Rust turns on for the target. `--all-features` also enables the
compact import encoding, which Chromium rejects (`Invalid import kind 127`).

## Checked in a browser

Headless Chromium with software WebGL (SwiftShader), driving the page as a player would:

* the four robots are compiled in the page and come online; no request 404s (Bevy looks for a
  `.meta` beside every asset unless told not to: `AssetMetaCheck::Never`);
* typing into the editor and F5 applies the edited brain (a robot comes back online) and does not
  reload the page;
* Ctrl+S puts the brain in `localStorage`, and after a reload it is still there.

At software rendering speed (about 7 frames a second) a key pressed and released within one frame
was missed by the test; held for 400 ms it is not. A real GPU runs the page at the display's rate.
