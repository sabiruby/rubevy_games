# The browser build

Both games run in a browser as well as in a window on a PC, on one GitHub Pages site:

| | |
|---|---|
| <https://sabiruby.github.io/rubevy_games/> | the entry page: what the two games are, and a link to each |
| <https://sabiruby.github.io/rubevy_games/sabibots/> | SabiRuby Battle |
| <https://sabiruby.github.io/rubevy_games/garden/> | Garden |

**SabiRuby Battle's address changed.** It used to be the top one; when the garden arrived (G5) the
top became the entry page and Battle moved down a directory. Anything outside this repository that
links `…/rubevy_games/` as the Battle needs the `sabibots/` on the end.

A game is the same game in both places — the same systems, the same Ruby, the same editor — and
what differs is chosen by the target at compile time.

```
cargo run -p sabibots               # PC
cargo run -p garden                 # the other one
web/build.sh && web/serve.sh        # both, in a browser, at http://localhost:8080/
web/build.sh garden                 # just one, at http://localhost:8080/garden/
```

## One site, two games

`web/build.sh <game>` writes `web/dist/<game>/`, and `web/build.sh all` (the default, and what CI
runs) writes both and the entry page above them:

```
web/index.html      →  dist/index.html      the entry page: two links and a sentence each
web/sabibots.html   →  dist/sabibots/index.html
web/garden.html     →  dist/garden/index.html
                       dist/<game>/pkg/          the game (wasm-bindgen's output)
                       dist/<game>/assets/       the game's assets, copied whole
                       dist/<game>/compiler/     the Ruby compiler module
```

**Each game's directory is complete on its own**, down to its own copy of the 2.4 MB compiler
module. That is the one deliberate duplication here, and it is for the thing the script is mostly
used for: `web/build.sh garden` gives a directory that can be served, opened and debugged with no
other game in the picture, and a visitor loads one game, not both. Sharing one copy at the top
would have made `dist/garden/` mean nothing without its parent.

The garden's assets are `models/*.glb` with `models/Textures/colormap.png` beside them, and a
`.glb` names its texture relative to itself, so the whole tree is copied and nothing has to be
said about the subdirectory. Nothing has to be said about the game being one directory *down*
from the site root either: Bevy's wasm reader fetches `assets/…` as a **relative** URL
(`bevy_asset`'s `io/wasm.rs` — `window.fetch_with_str(path)`), so it resolves against the page and
lands on `…/rubevy_games/garden/assets/…`. The one thing that does have to be said is
`AssetMetaCheck::Never`, or Bevy asks for a `.meta` beside every asset and every one of those is
a 404.

## What differs, and where

Everything that differs is in each game's `src/platform.rs`, one module per target behind
`cfg(target_arch = "wasm32")`, with the same functions in both:

| | PC | browser |
|---|---|---|
| `ruby_dir`, `read` | the files under `<game>/ruby/` | the same files, built into the binary (`build.rs` → `include_str!`); a script saved in this browser wins over the built-in one |
| `write` (Save) | the file | `localStorage`, key `sabibots:ruby/robots/scout.rb` — **the prefix is the game's**, so the two pages on one site cannot overwrite each other |
| `compile` | `sabiruby-compiler`, linked in | `window.sabibotsCompile` / `window.gardenCompile`, a second wasm module (below) — again one name per game, because each page defines its own |
| `clock_seed` | `SystemTime` | `Date.now()` (`SystemTime::now` panics on `wasm32-unknown-unknown`) |
| `assets_dir` | `<game>/assets` found from the crate | `assets/` next to the page |
| `SAVE_LABEL` | Save to file | Save in browser |
| the garden's save (G3) | `garden.save.json`, a file | `localStorage`, key `garden:garden.save.json` |
| the garden's checks | `GARDEN_SELFTEST=1` | `?selftest` in the page's address (G5) |

And outside that module:

* each game's `Cargo.toml` adds per target what Bevy needs: `multi_threaded` and `x11` on a PC,
  `web` and `webgl2` in a browser (the workspace's Bevy has only what both share). The PC build
  also takes `sabiruby-compiler`; the browser build takes `wasm-bindgen`, `js-sys`, `web-sys`.
* `rubevy-arena`'s `Watch` has a browser version whose `new` answers `None` (there is no directory
  to watch), so a game's reload system is the same code and simply never reloads. The garden says
  so out loud on start-up: `could not watch "ruby": saving a creature's file will not reload it`.
* The window is given a canvas (`#sabibots` / `#garden`, `fit_canvas_to_parent`), which a PC
  ignores.
* `--headless`, `--shot`, `--save` and `--load` compile in both but mean nothing in a page: there
  is no command line to put them on.

## Two wasm modules

The game's VM is Rust and builds for any target. The compiler is the reference mruby compiler,
which is C with `setjmp`/`longjmp`, and `wasm32-unknown-unknown` — what Bevy and wasm-bindgen use
in a browser — has no C library for it. The SabiRuby playground already builds exactly that
compiler for the browser, as a `wasm32-wasip1` module with wasi-sdk. So each page loads both:

```
garden/index.html
  compiler/sabiruby.wasm  (wasm32-wasip1: the compiler, and a VM the game does not use)
      ▲ sabi.js + browser_wasi_shim
      │ window.gardenCompile(source) → RITE bytes  (sabi_compile, sabi_take_binary)
  pkg/game_bg.wasm        (wasm32-unknown-unknown: Bevy, egui, rubevy, the SabiRuby VM)
```

The compiler module is loaded first and the call is synchronous, so to the game `compile` is an
ordinary function that returns bytes or an error message — the same shape as on a PC. The only
thing added to the playground for this was `sabi_take_binary`, the compiled bytes. The module is
~2.4 MB (0.8 MB gzipped) and carries a VM of its own that the game does not use; a compiler-only
module would be smaller, and is not worth a second C build yet.

**What the page's compiler cannot pass on is the file's name.** On a PC `platform::compile` hands
`sabiruby_compiler::Options { filename: … }` the creature's own file, and the debug info in the
bytecode says `beetle.rb:102`. The playground's bridge takes a source string and nothing else, so
every script compiled in a page is called `playground.rb`, and that is the name the HUD's "the
line it is waiting on" column and the VM panel's frames show. The *line numbers* are right, and
everything that uses them is right with them — the garden finds a creature's own line by counting
past the prelude's length, not by the name (`watch_minds`) — so this is a label and nothing more.
Giving the bridge a name is a change to sabiruby-playground.

In CI (`.github/workflows/pages.yml`) the compiler module is built from sabiruby-playground at a
pinned commit, against the SabiRuby commit `Cargo.lock` names for the games, so the bytecode the
compiler writes and the VM that reads it come from the same source.

## Keys

A browser wants some of a game's keys for itself: **F5 reloads**, Ctrl+S opens a save dialog, F1
opens Help, Tab moves the focus. Each page takes them first — `keydown` in the capture phase,
`preventDefault` — and the game still sees them, so the keys are the same two builds:

| | SabiRuby Battle | Garden |
|---|---|---|
| F5 | Apply the edited behaviour | **save the whole garden** |
| F9 | — | read it back |
| Ctrl+S | Save the behaviour (in the browser: `localStorage`) | Save the creature's file |
| Ctrl+Enter | — | Apply to every creature of that species |
| Tab | the next robot | the next creature |
| F1 / F2 / P | the editor / the VM panel / pause the scripts | the same |
| H, ? | the in-game guide (G6) | the same |
| wheel | — | zoom: a tenth of the distance per notch, and the browser's `deltaY` is read as 100 pixels to the notch (G6; `docs/garden.md`) |
| right-drag, Shift+drag, WASD, Home | — | slide the view, and put it back (G6) |

F5 is the one that mattered for the garden, because there the key *saves* and the browser's own
meaning for it is Reload — the exact opposite of what the player pressed it for. Two things were
done about it rather than one:

* the page takes the key back, which was already proved for Battle's F5 and is proved again here:
  driven in a headless browser, the document reports `F5 prevented=true`, the game logs
  `saved 11 creatures, 47 plants at 23.6 s to garden.save.json (12387 bytes)`, and the page has
  still been loaded exactly once. Pressed again at 45, 65 and 85 seconds it saves every time.
* **the HUD has the two buttons** — *Save the garden (F5)* and *Load it back (F9)* — and they are
  in both builds. A key that works only because a page remembered to intercept it is a thin thing
  to hang a garden on, and nobody who opens a link has been told which key saves. The buttons
  leave a flag that the same system the keys feed reads on the next frame, so there is one path
  and not two, and the note beside them (`saved 11 creatures, 58 plants (13429 bytes) · at 42 s`,
  amber when nothing was written or read) is the same note the refused save version writes. Both
  were clicked in the headless browser and both did what they say.

They are drawn **above** the list of creatures, and that is the browser's doing. The first version
put them at the foot of the panel, beside the line that lists the keys, which reads better — and
at 1280×800 with fourteen creatures in the garden that row is past the bottom of the panel and
underneath the VM window, which is the one place a player cannot reach. Six clicks along it in the
headless browser hit nothing. What is above the list cannot be pushed anywhere by the list.

`Ctrl+L` was considered for the web build and **is not usable**: it is one of the shortcuts
Chrome keeps for itself (the address bar), and a page's `preventDefault` does not reach it —
unlike Ctrl+S, Ctrl+F or F5, which are the page's to take. That last sentence is received wisdom
and was **not** measured here, because a headless browser has no address bar to watch; it is the
reason the pair chosen was "the keys the PC build already has, plus buttons on the screen" rather
than a new Ctrl pair that would have had to be tested in a browser with a window.

One guard was taken *off* the two keys while G5 was being measured. They briefly stood behind
`EguiWantsInput::wants_keyboard_input()`, the way `P` and `F2` do, so that a key pressed with the
caret in the editor belonged to the editor. `P` is a letter and that is right for it; F5 and F9
are keys egui never wants, and guarding them makes a save key that stops working for the rest of
the session after the first edit. (Removing it did not fix what was actually wrong that day —
see below — but it was wrong on its own.)

## The save has a version (G5)

The garden's save carries `version: 1` as its first field, and a file that says anything else —
or nothing — is **not loaded**: the log and the HUD say `saved with version 99, this garden reads
1` and the garden starts fresh. It is a browser feature before it is a file feature. A file on a
PC is something a player can see and delete; a save in `localStorage` is a string that outlives
every future build of the page, so the first garden a new build meets there is very often one an
older build wrote. `docs/garden.md` has the rest.

## Size

Measured with `--profile web` (release, `opt-level = "s"`, thin LTO, stripped) and binaryen 132 —
the version CI installs. The garden's two `wasm-opt` columns are from the sabiruby 0.5.1 build (the
restart queue gone, `GARDEN_RELOAD_AT` and `CHECKS_EXIT_WHEN_DONE` in); it is 10 KB smaller than
the 0.5.0 one and 5 KB smaller gzipped, which is nothing, and the unoptimized columns were not
re-measured:

| | bytes | gzip -9 | with `wasm-opt -Os` | its gzip |
|---|---|---|---|---|
| `sabibots/pkg/game_bg.wasm` | 38,772,420 | 9,877,837 | 34,794,885 | 10,593,843 |
| `garden/pkg/game_bg.wasm` | 39,250,403 | 10,011,507 | 35,216,938 | 10,715,122 |
| `compiler/sabiruby.wasm` (each game's copy) | 2,435,191 | 835,234 | — | — |
| `sabibots/assets/` (16 files) | 234,202 | | | |
| `garden/assets/` (10 files) | 322,924 | 64,744 | | |

The garden's assets are the Kenney models: `animal-crab.glb` 150,768, `animal-bunny.glb` 131,568,
`grass.glb` 11,496, `Textures/colormap.png` 10,915, `tree_default.glb` 9,428, `plant_bush.glb`
4,396, `rock_smallA.glb` 3,044, and two licence files.

Two things in that table are worth saying out loud.

**`wasm-opt -Os` makes the file smaller and the download bigger.** It takes about 10% off the raw
module and puts about 7% back on the gzipped one (sabibots 9.88 → 10.59 MB gzipped, the garden
10.01 → 10.72 MB), which is what the wire actually carries — Pages serves gzip. The optimizer's
output is smaller but less repetitive, and gzip lives on repetition. CI has `wasm-opt` and still
runs it, because the raw size is what the browser must decode and keep, but **the case for it is
not the download**, and this is the first time the two were measured side by side here. Kept
(author's decision 2026-09-17): what the browser decodes and holds matters more here than the
0.7 MB of transfer.

### What the in-game guide cost (G6)

The guide (`H`) is written in English and Japanese — one of the two at a time from G6b, switched
by a button — and egui's default fonts have no CJK at all, so a subset of Noto Sans JP,
62,780 bytes at G6, 66,796 after G6b re-cut it and **65,904** after G7 renamed the words in it
(four characters in, seven out), is
`include_bytes!`d into `rubevy-arena` and added to egui as a fallback family. **The author accepted the size increase** (2026-09-17), on the
condition that the font be subset rather than shipped whole. Measured on the same machine and the
same binaryen, all four numbers after `wasm-opt -Os`:

| | before G6 | with the camera (G6.2) | with the guide and the font (G6.3) | the guide's delta |
|---|---|---|---|---|
| `garden/pkg/game_bg.wasm` | 35,216,938 | 35,217,417 | **35,306,969** | **+89,552** (+0.25%) |
| its `gzip -9` | 10,715,122 | 10,716,945 | **10,772,842** | **+55,897** (+0.52%) |
| `sabibots/pkg/game_bg.wasm` | 34,794,885 | — | **34,884,167** | **+89,282** (+0.26%) |
| its `gzip -9` | 10,593,843 | — | **10,649,110** | **+55,267** (+0.52%) |

**About 90 KB per game, of which 63 KB is the font**; the other 27 KB is the panel, the two games'
strings and what egui's font machinery pulls in with a second family. Gzipped — which is what the
wire carries, since Pages serves gzip — it is 55 KB, because a subsetted `glyf` table is already
close to incompressible. A quarter of a per cent on a 35 MB module, for the difference between a
page that explains itself and one that does not.

The font was 9,589,900 bytes as it comes from Google Fonts. Shipping it whole would have been a
27% increase on the module, which is the decision the subsetting avoided; `tools/subset-font.sh`
is what has to be run again when the Japanese is edited (`docs/garden.md`, `CREDITS.md`).

**The garden is 1.2% bigger than Battle** (39.25 vs 38.77 MB raw), which is the whole of 3D:
`bevy_pbr`, `bevy_gltf`, `bevy_animation` and the glTF loader against Battle's sprites. The plan
guessed 35–40 MB against "Battle's 30 MB"; both games are at the top of that. The figure recorded
here at the time of the Battle's own build was 26.7 MB — the module has grown by 8 MB since, on
an unchanged `web` profile, and what grew it (Bevy 0.19, `bevy_egui` 0.42, `rubevy-arena`) has
not been taken apart. Most of what is in there is Bevy's renderer.

## Checked in a browser

Headless Chromium 153 with software WebGL (SwiftShader), driving each page as a player would
(playwright-core, a throwaway script). The garden's `?selftest` is what makes this possible at
all: the lines that say a creature ate are written by the checks, and until G5 there was no way to
ask a page for them.

Two things about the tooling, because both cost time:

* **`page.goto` must not wait for `load`.** The page's module script ends in `await
  game.default()`, which does not return while the game is running, and the `load` event waits for
  the module — so `waitUntil: 'load'` times out on a page that is working perfectly. `'commit'` is
  the right one.
* **egui wants to know where the pointer is before it is pressed.** A click with no movement
  before it presses a button egui does not think is hovered, and nothing happens. Move, let a
  frame go by, then press.

The page is not slow. Counting `requestAnimationFrame` callbacks: 7531 in 150 seconds, which is
about 50 a second once it is running. (An earlier reading of 221 in 40 seconds was mostly the
35 MB module being fetched and compiled, and it is the reason the first draft of this section said
"about 5 frames a second". Battle's note that a key pressed and released inside one frame is lost
does not reproduce here: three `keyboard.press` taps, three saves.)

What was seen, over a 150-second run of `garden/?selftest`, a 45-second one of `garden/`, and
several shorter ones:

* no panics, no page errors, **no request failed and nothing 404'd** — the models are fetched from
  `garden/assets/models/` and the creatures wear them (`Beetle walks, idles and eats from its
  model`);
* the checks' own lines: `selftest: first meal at 1.63 s`, `selftest: the hungry beetle reached
  its plant at 2.45 s`, `Beetle 398v0 starved at 2.0 s`, `night at 25.3 s`, `a Beetle was born at
  4.2 s`, and a creature's remembered Hash printed out of the VM;
* **the version check works in the place it was written for**: `?selftest` writes a save that says
  99 into `localStorage` and hands it to the same arm `--load` uses, and the page answers
  `garden.from-another-version.json: saved with version 99, this garden reads 1` and builds a new
  garden. The sentence is in the HUD too, in amber;
* the save round trip through `localStorage`: F5 writes 11,918 characters under
  `garden:garden.save.json` (`version 1`, 11 creatures, 44 plants, tick 43.6), the page is
  reloaded, the save is still there, and F9 brings it back — `loaded 11 creatures, 44 plants,
  7 trees, 9 rocks at 43.6 s (69 entities made way)`. The two HUD buttons do the same two things
  when clicked;
* F5 and F9 do not reload the page — the document has been loaded exactly twice at the end of
  that, and the second one is the reload the test asked for;
* the editor, the VM panel and the HUD all draw at 1280×800 and are usable, though between them
  they cover most of the window and the VM panel's bottom rows run past the edge at that height.
  They are egui windows and can be dragged and collapsed. This is what moved the save buttons
  above the creature list.

### Three things it found, and where they went

**A restart that replaces every creature at once left their new tasks made but never run — the
VM's doing, and fixed in sabiruby 0.5.1.** In the browser it showed in two places. The window
checks that `?selftest` also runs passed 21 of 22, and the one that failed was *every restarted
beetle's new task has run*: 0.6 s after Apply had handed the beetles over one at a time, not one
of them had run an instruction and the VM panel said `Created, 0 insn total, no frames` for each.
Worse was **F9 into a running garden**, which despawns everything and builds eleven creatures in
one frame: `11 creatures never started; the garden is running anyway`, and every creature came
back with an empty memory while the world itself was exactly right.

Both were the same bug, and it was not "the scheduler stops": ending a creature's `ScriptTask`
wakes its six handler tasks with `Rubevy::Unsubscribed`, their empty `rescue` makes the block's
value `nil`, and `Vm::task_run_limited` read that `nil` as "nothing ready" and ended the host's
whole frame — six frames a creature, and a browser has fewer frames a second to spare than a PC
does. sabiruby 0.5.1 tells the two apart. Driven again on the same machine and the same Chromium,
with the game's own workaround queue deleted: **21 of 21**, *every restarted beetle's new task has
run* among them, and F9 into a garden that has been running for three quarters of a minute gives
`loaded 10 creatures, 46 plants, 7 trees, 9 rocks at 45.3 s (78 entities made way)` with no
`never started` line and all ten memories back — a re-save from the page a moment later has
`meals: 10` in it, not `meals: 1`.

**With the checks turned on, the page stopped answering the mouse and the keyboard — and it was
the checks ending the app.** After `?selftest`'s window checks had finished driving the editor,
neither F5, F9 nor P reached the game. G5 recorded it unexplained, having cleared the DOM (the
canvas keeps focus and the `keydown` arrives at `#garden`) and the `EguiWantsInput` guard that F5
and F9 briefly had. The answer was in the last line of `window_selftest`: `exit.write(AppExit::
Success)`. On a PC that is right — the checks were asked for on a command line and the shell wants
its prompt back. **In a browser there is nothing to exit to.** winit's wasm event loop stops being
pumped, every system stops running, and the last frame drawn stays on the canvas, so the page goes
on *looking* like a garden while nothing in it will ever move again. That is also why the earlier
reading that "the world keeps living" was wrong: the log stops dead at the last check.

`platform::CHECKS_EXIT_WHEN_DONE` is `true` on a PC and `false` in a page, where the checks say
`selftest: done — the garden keeps running (a page has nothing to exit to)` instead. Measured
after the change, on the same 150-second run: F5 gives `saved 15 creatures, 30 plants at 150.2 s`,
F9 gives `loaded 15 creatures … (62 entities made way)` and the rabbits say what trees they
remember, and `P` — which prints nothing of its own — is measured by what stops: `[script]` lines
in four seconds, **2 running, 0 paused, 3 resumed**. The world goes on living too (`night at
145.4 s`, long after the checks ended).

**The page's compiler cannot pass a file's name on**, so the HUD's "the line it is waiting on"
column reads `playground.rb:102` in a browser where a PC says `beetle.rb:102`. The line numbers
are right and everything computed from them is right. See "Two wasm modules" above.
