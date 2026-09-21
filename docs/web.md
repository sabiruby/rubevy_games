# The browser build

All three games run in a browser as well as in a window on a PC, on one GitHub Pages site:

| | |
|---|---|
| <https://sabiruby.github.io/rubevy_games/> | the entry page: what the three games are, and a link to each |
| <https://sabiruby.github.io/rubevy_games/sabibots/> | SabiRuby Battle |
| <https://sabiruby.github.io/rubevy_games/garden/> | Garden |
| <https://sabiruby.github.io/rubevy_games/factory/> | Factory (F6, 2026-09-22) |

**SabiRuby Battle's address changed.** It used to be the top one; when the garden arrived (G5) the
top became the entry page and Battle moved down a directory. Anything outside this repository that
links `…/rubevy_games/` as the Battle needs the `sabibots/` on the end.

A game is the same game in both places — the same systems, the same Ruby, the same editor — and
what differs is chosen by the target at compile time.

```
cargo run -p sabibots               # PC
cargo run -p garden                 # the other one
cargo run -p factory                # the third one
web/build.sh && web/serve.sh        # all three, in a browser, at http://localhost:8080/
web/build.sh garden                 # just one, at http://localhost:8080/garden/
```

## One site, three games

`web/build.sh <game>` writes `web/dist/<game>/`, and `web/build.sh all` (the default, and what CI
runs) writes all of them and the entry page above them:

```
web/index.html      →  dist/index.html      the entry page: three links and a sentence each
web/page.html.in  ┐
web/games.sh      ┘ →  dist/<game>/index.html    the game's page, one template + that game's values
                       dist/<game>/pkg/          the game (wasm-bindgen's output)
                       dist/<game>/assets/       the game's assets, copied whole
                       dist/<game>/compiler/     the Ruby compiler module
```

### One template, and a game's values

There were two hand-written pages, `web/garden.html` and `web/sabibots.html`, and they differed
in fifteen lines out of seventy — a third game would have been a third copy of the same file. As
of 2026-09-20 there is `web/page.html.in` with eight `{{…}}` in it, and `web/games.sh`, which is
one block of shell a game:

| the value | what it fills |
|---|---|
| the game's word (`GAMES_ALL`) | the canvas id (`<canvas id="garden">`), and the prefix of the two functions the page hangs on `window` — `gardenCompile`, `gardenHighlight` |
| `TITLE` | the `<title>` and the word on the loading screen |
| `BG`, `FG`, `DIM` | the page behind the canvas, so that the moment before the game draws is the game's own colour rather than white |
| `KEYS` | the keys this game takes back from the browser (below) — the garden wants `F9` and Battle does not |
| `COMPILE_NOTE`, `KEYS_NOTE` | the two comments, which are prose about *this* game and are what somebody who opens `view-source` reads |

The word is not a value of its own on purpose: it is the crate's name, and the canvas id, the
two `window.<game>…` names and the `localStorage` prefix are all already spelled that way, in
`src/platform.rs` and in the game's `WindowPlugin`. Changing it would orphan every script a
player has saved.

`web/build.sh` fills the template with plain parameter expansion rather than `sed`, because the
values are prose with slashes, ampersands, apostrophes, backticks and newlines in them and none
of that has to be escaped in `${page//'{{X}}'/$value}`; it refuses to write a page that still
has a `{{` in it. **A third game is one word in `GAMES_ALL` and one block in `games.sh`** — plus
its own `<li>` in `web/index.html`, the entry page, which is a name and a sentence about the game
and not a value. The two pages it writes for the older games are byte-identical to the two files
it replaced, except that Battle's page used to point at `web/garden.html` for the other key list
and now points at the block beside it.

**That claim was paid out at F6 and it held.** Factory's block had been in `games.sh` since F0,
with the word deliberately kept out of `GAMES_ALL` so that `web/build.sh factory` could build a
page for a browser check while `all` — which is what CI publishes — would not. Publishing it was
the word, the `<li>`, and **nothing in `.github/workflows/pages.yml`**, which names no games at
all. Two of the values had gone stale in the six stages between, and that is the cost of a block
of prose written before the game existed: the key list had no `F9` (F5 gave Factory a save file,
and it means by `F5`/`F9` what the garden means), and both notes still said *nothing calls the
compiler yet* and *none of those panels is in this game yet*. Neither is a thing a build can
catch — they are prose for whoever opens `view-source`.

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
| Factory's save (F5) | `factory.save.json`, a file | `localStorage`, key `factory:factory.save.json` — and the three Ruby files Ctrl+S writes are `factory:ruby/data.rb`, `factory:ruby/control.rb` and `factory:ruby/inserter.rb`, so **a `data.rb` saved in this browser is what the next reload reads**, which is what makes the map's size a thing a page can be given |
| the garden's checks | `GARDEN_SELFTEST=1` | `?selftest` in the page's address (G5) |
| Factory's checks and knobs | `FACTORY_SELFTEST=1`, `--stress N`, `--arms N` | `?selftest`, `?stress=N`, `?arms=N` — the knobs are `games_shell::checks::asked_number` (S5b-5) and **`?stress`/`?arms` are how a measurement is taken on a machine nobody here has** (`verification/factory-on-a-real-gpu.md`) |
| the Battle's checks | `SABIBOTS_SELFTEST=1` | `?selftest` in the page's address (2026-09-18) — the same query string, the same `CHECKS_EXIT_WHEN_DONE`, and a page that said nothing before it now runs the editor's whole sequence. What each run has to print, line by line, is [`verification/selftest-lines.md`](verification/selftest-lines.md) |

And outside that module:

* each game's `Cargo.toml` adds per target what Bevy needs: `multi_threaded` and `x11` on a PC,
  `web` and `webgl2` in a browser (the workspace's Bevy has only what both share). The PC build
  also takes `sabiruby-compiler`; the browser build takes `wasm-bindgen`, `js-sys`, `web-sys`.
* `rubevy-egui`'s `Watch` has a browser version whose `new` answers `None` (there is no directory
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
      │ window.gardenCompile(source)   → RITE bytes   (sabi_compile, sabi_take_binary)
      │ window.gardenHighlight(source) → kind bytes   (sabi_highlight, sabi_take_highlight)
  pkg/game_bg.wasm        (wasm32-unknown-unknown: Bevy, egui, rubevy, the SabiRuby VM)
```

The compiler module is loaded first and the call is synchronous, so to the game `compile` is an
ordinary function that returns bytes or an error message — the same shape as on a PC. Two things
have been added to the playground for this: `sabi_take_binary`, the compiled bytes, and — for the
editor's colours (2026-09-18) — `sabi_highlight` / `sabi_take_highlight`, one kind byte per source
byte from the same Prism that is already in there. The module is ~2.4 MB (0.8 MB gzipped) and
carries a VM of its own that the game does not use; a compiler-only module would be smaller, and
is not worth a second C build yet.

**The colour bridge is allowed to be missing.** A browser holds pages in its cache, and a game can
be opened from an `index.html` older than the build it loads; `platform::highlight` catches the
`ReferenceError` and answers zeroes, which is the listing as it was before there was any colour
(`garden/src/platform.rs`). The page's own side never throws either. A black screen has been paid
for once here already (`docs/worklog/2026-09-18-web-black-screen.md`) and the editor's colours are
not worth a second one.

**What the page's compiler cannot pass on is the file's name.** On a PC `platform::compile` hands
`sabiruby_compiler::Options { filename: … }` the creature's own file, and the debug info in the
bytecode says `beetle.rb:102`. The playground's bridge takes a source string and nothing else, so
every script compiled in a page is called `playground.rb`, and that is the name the HUD's "the
line it is waiting on" column and the VM panel's frames show. The *line numbers* are right, and
everything that uses them is right with them — the garden finds a creature's own line by counting
past the prelude's length, not by the name (`watch_minds`) — so this is a label and nothing more.
Giving the bridge a name is a change to sabiruby-playground.

In CI (`.github/workflows/pages.yml`) the compiler module is built from sabiruby-playground at a
pinned commit (`PLAYGROUND_REF`), against the SabiRuby `Cargo.lock` names for the games, so the
bytecode the compiler writes and the VM that reads it come from the same source. A bridge the game
calls has to exist in the pinned playground — `sabi_take_binary` was why the pin moved once and
`sabi_highlight` why it moved again — and the playground is built here against the games' own
SabiRuby, so a `sabiruby_compiler` function that arrived after the lock's commit would not compile
in CI even though it compiles at home.

### Three pins, and they are one pin

There is not one pin here but **three, and they all name the same SabiRuby**:

| | where | what it says | how it is read |
|---|---|---|---|
| 1 | this repository's `Cargo.lock` | which SabiRuby the games' **VM** is — today `sabiruby 0.6.1`, a published version; until 2026-09-22 a `[patch.crates-io]` git rev | `pages.yml`'s first step greps for the rev, then falls back to `v` + the version, and **fails rather than checking out an empty ref** (an empty one is "the default branch", which would silently publish against the VM's `main`) |
| 2 | `pages.yml`'s `PLAYGROUND_REF` | which **playground** the compiler module is built from | checked out by commit |
| 3 | that playground's own `SABIRUBY_REF` | which SabiRuby **it** is built against for its own site | not read here at all — it is how you tell whether the playground commit you are pinning was ever built against pin 1 |

**A release that removes a name has to move all three, and 2026-09-22 is what it looks like when
one is left behind.** SabiRuby 0.6.0 removed `Vm::current_line` (renamed `next_line`). Pins 1 and
3 moved to 0.6.1; `PLAYGROUND_REF` stayed at `d72e000`, whose stepper still called the old name.
The games' own build was green — **the games do not call it** — and the failure was in CI's *other*
checkout, so two pages runs went red, nothing deployed, and the published site stayed as it was
with no sign on it. The pin moved to `5337b2f`, whose stepper asks `next_line` and whose own
`SABIRUBY_REF` is `v0.6.1`.

**Moving one of them, in order.** Raise the VM (pin 1) → find or make a playground commit whose
`SABIRUBY_REF` is that same version (pin 3), because a playground that does not build against it
will not build here either → put that commit in `PLAYGROUND_REF` (pin 2). Going the other way —
wanting a newer bridge — is the same three in the same order, because the playground commit you
want is only usable if its own SabiRuby is one the lock can name.

**Run the step locally before pushing**, which is cheap and catches exactly this. It is the one CI
step that builds something this repository never builds:

```
git clone <playground> pg && git -C pg checkout $PLAYGROUND_REF
git clone <sabiruby> sabiruby && git -C sabiruby checkout v$(the lock's version)   # beside pg
cd pg && tools/build.sh      # needs WASI_SDK_PATH and wasm-opt; ~20 s warm
```

The two checkouts have to be **siblings**, because the playground's `wasm/Cargo.toml` names
`../../sabiruby` by path — the same layout `pages.yml` makes. Done for F6 on 2026-09-22 with
`5337b2f` and `v0.6.1`: it builds, and the module is 2,448,533 bytes (840,475 gzipped).

## Keys

A browser wants some of a game's keys for itself: **F5 reloads**, Ctrl+S opens a save dialog, F1
opens Help, Tab moves the focus. Each page takes them first — `keydown` in the capture phase,
`preventDefault` — and the game still sees them, so the keys are the same two builds:

| | SabiRuby Battle | Garden | Factory |
|---|---|---|---|
| F5 | Apply the edited behaviour | **save the whole garden** | **save the whole factory** |
| F9 | — | read it back | read it back |
| Ctrl+S | Save the behaviour (in the browser: `localStorage`) | Save the creature's file | Save the file open in the editor — one of three |
| Ctrl+Enter | — | Apply to every creature of that species | Apply |
| Tab | the next robot | the next creature | — |
| F1 / F2 / P | the editor / the VM panel / pause the scripts | the same | the same, and `P` stops **the world** as well as the scripts (F5) |
| H, ? | the in-game guide (G6) | the same | the same |
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
the version CI installs. **Taken again on 2026-09-22 at F6**, when the site became three games,
all four columns of all three on one machine within the same minutes (one `web/build.sh all`, and
a second `wasm-bindgen` run over the same three `target/…/web/*.wasm` for the two raw columns,
because the script optimises the module in place):

| | bytes | gzip -9 | with `wasm-opt -Os` | its gzip |
|---|---|---|---|---|
| `sabibots/pkg/game_bg.wasm` | 39,265,829 | 10,082,522 | 35,264,746 | 10,793,564 |
| `factory/pkg/game_bg.wasm` | 39,693,676 | 10,235,529 | **35,668,110** | 10,942,946 |
| `garden/pkg/game_bg.wasm` | 40,099,872 | 10,305,560 | 36,038,000 | 11,013,138 |
| `compiler/sabiruby.wasm` (each game's copy) | 2,448,533 | 840,475 | — | — |
| `sabibots/assets/` (16 files) | 234,202 | | | |
| `garden/assets/` (10 files) | 322,924 | | | |
| `factory/assets/` (4 files) | **11,326** | | | |

**Factory lands between the other two**, 1.1% above Battle and 1.0% below the garden, which is
where a 2D game with one more VM-facing stage and no 3D belongs: it pays Battle's sprite renderer
and not the garden's `bevy_pbr`, `bevy_gltf` and `bevy_animation`, and what it adds over Battle is
its own eleven modules — the data stage's serde, the control stage, the save file, three editors —
at four hundred kilobytes. **Its assets are 3.5% of the garden's** (one 143-tile sheet, one strip
of five 8 px pictures and two licence texts, `docs/factory.md`), which is the whole of the
difference between a pack of glTF models and a tile sheet.

Against the 2026-09-21 reading (S5b) the two older games moved **+53,179 raw / +48,140 optimised
for Battle (+0.14%)** and **+63,971 / +57,971 for the garden (+0.16%)**. What is in that sixth of
a percent is S11 — `Settings::positive`/`counted` and `checks::Errands` in the shared crate, which
both games link — and F5's re-cut guide font, which is in `games-shell` and therefore in all three
(81,480 B against 64,616; `CREDITS.md`).

**The compiler module moved too**, +3,024 raw and +207 gzipped, and it is the one row here that is
not this repository's doing: the playground pin went `d72e000` → `5337b2f` and its SabiRuby with
it, 0.5.2 → 0.6.1 ("Three pins" above).

The rows before these were measured at the sabiruby 0.5.1 build, before G6's Japanese font and
everything after it, so the difference between the two tables is not any one change's — the
two deltas worth naming are measured against their own build and are further down. The nearest
one is S4a's split of the shared crate into `rubevy-egui` and `games-shell`, which cost
**+6,952 bytes for Battle and +4,551 for the garden** (0.02%) against the same tree unsplit; S4's
two new lines in Battle's checks cost 92 bytes. (The garden's assets gzip a kilobyte smaller than
last time because the two readings concatenated the ten files in different orders; the files are
the same files.)

The compiler module's row is the playground at `5337b2f` (2026-09-22), which is what
`PLAYGROUND_REF` pins; it was 2,445,509 / 840,268 at `d72e000` and 2,435,191 / 835,234 at
`3f47c9d`, and **+10,318 raw / +5,034 gzipped** of the older of those two steps is
`sabi_highlight` and the SabiRuby the playground was rebuilt against. Of it, +1,736 / +493 is the
export itself (measured in sabiruby-playground's `docs/worklog/2026-09-18-highlight.md`, the same
tree built with and without it).

### What the editor's colours cost (2026-09-18)

The game module, `wasm-opt -Os`, both games built from the same tree with and without the change —
the only difference being the commit the code came from, so the VM and the toolchain are held
still:

| | without the colours | with them | delta |
|---|---|---|---|
| `sabibots/pkg/game_bg.wasm` | 34,934,405 | **34,937,939** | **+3,534** (+0.010%) |
| its `gzip -9` | 10,675,858 | **10,677,505** | **+1,647** |
| `garden/pkg/game_bg.wasm` | 35,642,569 | **35,647,958** | **+5,389** (+0.015%) |
| its `gzip -9` | 10,881,862 | **10,885,349** | **+3,487** |

A few kilobytes, because the browser build adds no lexer — the lexer is in the compiler module,
which the page has loaded anyway. What is in the game module is the nine colours, the run loop and
one more `wasm_bindgen` import. On a PC the reference compiler was already linked in and
`highlight()` is a second entry point into it, so the cost there is smaller still.

(These two rows are bigger than the `wasm-opt` column above them because everything else moved in
between — G6's font, G9, the leftovers, and SabiRuby 0.5.0 → 0.5.2. The table above was not
re-measured; the pair here was, on one machine, minutes apart, and only the pair is a delta.)

The garden's assets are the Kenney models: `animal-crab.glb` 150,768, `animal-bunny.glb` 131,568,
`grass.glb` 11,496, `Textures/colormap.png` 10,915, `tree_default.glb` 9,428, `plant_bush.glb`
4,396, `rock_smallA.glb` 3,044, and two licence files.

Two things in that table are worth saying out loud.

**`wasm-opt -Os` makes the file smaller and the download bigger.** It takes about 10% off the raw
module and puts about 7% back on the gzipped one (sabibots 10.03 → 10.74 MB gzipped, the garden
10.24 → 10.95 MB; −10.2% and −10.1% raw against +7.1% and +6.9% gzipped, on the 2026-09-20
table), which is what the wire actually carries — Pages serves gzip. The optimizer's
output is smaller but less repetitive, and gzip lives on repetition. CI has `wasm-opt` and still
runs it, because the raw size is what the browser must decode and keep, but **the case for it is
not the download**, and this is the first time the two were measured side by side here. Kept
(author's decision 2026-09-17): what the browser decodes and holds matters more here than the
0.7 MB of transfer.

### What the in-game guide cost (G6)

The guide (`H`) is written in English and Japanese — one of the two at a time from G6b, switched
by a button — and egui's default fonts have no CJK at all, so a subset of Noto Sans JP,
62,780 bytes at G6, 66,796 after G6b re-cut it, 65,904 after G7 renamed the words in it
(four characters in, seven out) and **64,616** after G9 changed the `F2` and `P` rows
(one character in — `く`, from 「続く」 — and six out), is
`include_bytes!`d into `games-shell` and added to egui as a fallback family. **The author accepted the size increase** (2026-09-17), on the
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

**The garden is 2.1% bigger than Battle** (40.10 vs 39.27 MB raw, 2026-09-22), which is the whole
of 3D: `bevy_pbr`, `bevy_gltf`, `bevy_animation` and the glTF loader against Battle's sprites.
**Factory, 2D like Battle, is 1.1% above it** — so a whole third game, with three Ruby stages, a
save file and a three-file editor, costs less than the garden's renderer does. The plan
guessed 35–40 MB against "Battle's 30 MB"; all three are at the top of that. The figure recorded
here at the time of the Battle's own build was 26.7 MB — the module has grown by 8 MB since, on
an unchanged `web` profile, and what grew it (Bevy 0.19, `bevy_egui` 0.42, `rubevy-egui` and `games-shell`) has
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

### Factory in a page (F6, 2026-09-22)

Driven the same way, at 1280×720, from a `web/build.sh all` of the same commit. `?selftest`
prints its **forty lines with no `FAIL`**, no page error and no failed request, its list is the one
in `verification/selftest-lines.md` with the diff empty, and the screenshot has **no pure black
pixel in it at all** (577 distinct colours — the floor's two oranges and the panels), which is the
check F0 had to invent when a tileset whose square layers numbered a multiple of six drew a black
canvas and said nothing.

**What start-up costs in a page**, which is the number this game has more of than the other two
because it compiles three files instead of one. Each compile now says what it took, and over three
runs:

| | run 1 | run 2 | run 3 |
|---|---|---|---|
| `data.rb` (read *and* run, before the first frame) | 24.3 ms | 18.1 ms | 8.8 ms |
| `control.rb` (392 lines with its prelude) | 7.4 | 11.9 | 3.8 |
| `inserter.rb` (244 lines, when the first arm is built) | 0.7 | 0.6 | 0.7 |
| **all three** | **32.4** | **30.6** | **13.3** |
| from `page.goto` to the last of them | 1.07 s | 0.80 s | 0.94 s |

The spread is the compiler module warming up, not the files: `data.rb` is the first thing compiled
and is dearest every time, `inserter.rb` is the longest program and is the cheapest because it is
third. Against F2's reading of about 7 ms for `data.rb` alone, what grew is the file (F2a's
`ore`, F3a's `map`) and what was added is two more programs. **All of it is inside one second of
a page that has just fetched a 35 MB module**, which is where the second the visitor actually
waits is.

**What a frame costs is all drawing, and this machine cannot measure it.** `?stress=4000` — a
loop of belt on a 48×48 map with four thousand items on it — reports

```
stress: map 48x48 items=4000 belts=2116 arms=0 frame ms p50 361.70 p95 710.80 (about 3 fps) | step us p50 100 p95 200 | ANGLE (… SwiftShader driver) (Gl, Cpu)
```

**100 µs of factory inside a 360 ms frame.** The same page with `?arms=1` on the shipped 32×32 map
is 306 ms a frame. Two orders of magnitude between the simulation and the picture, and the
adapter line says why: `device_type: Cpu`. F1 found the same thing from the other end — two items
and eighteen hundred items were both 5 fps — and wrote that the browser could not be measured
here. It still cannot. The map's default size is the one number in this game nobody has been able
to derive, and what it waits on is one reading from a real GPU:
`verification/factory-on-a-real-gpu.md` is five lines that take it.

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
