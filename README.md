# rubevy_games

Games whose behaviours are Ruby. They run on [rubevy](https://github.com/sabiruby/rubevy) — the
[SabiRuby](https://github.com/sabiruby/sabiruby) VM (mruby 4.1 bytecode, written in Rust) inside
[Bevy](https://bevyengine.org) — and the Ruby in them is read from `.rb` files as the game runs,
so editing a file changes what happens on screen without a rebuild.

| game | what it is | state |
|---|---|---|
| [`sabibots`](sabibots) | **SabiRuby Battle** — robots that fight; each robot's behaviour is one Ruby file | playable v0.1 |
| [`garden`](garden) | **Garden** — a 3D world whose creatures read and write their own ECS components from Ruby by name, breed by mixing a `Genome` that is a Rust struct and a Ruby class at once, and are saved to JSON along with what each of them remembers | playable v0.1: the world, the behaviours, the genome, the save file, the window and the browser build (G0–G5) |
| [`factory`](factory) | **Factory** — a small top-down factory: `ruby/data.rb` declares the items, the recipes, the machines and how big the world is; `ruby/control.rb` hears what the factory does and says what it is for; and each **inserter** runs a script of its own that decides when to move a thing from the tile behind it to the tile in front. All three can be edited and applied while the factory runs | playable v0.1: the floor, the grid, belts, miners and chests, the three Ruby files, the editor, the save file, the HUD, the VM panel and the guide (F0–F6) |
| `cards` | a card game whose rules are a Ruby DSL | planned |

**Play them in a browser:** <https://sabiruby.github.io/rubevy_games/> — all three, built for
the web, from the same source as the PC build:

* SabiRuby Battle: <https://sabiruby.github.io/rubevy_games/sabibots/>
* Garden: <https://sabiruby.github.io/rubevy_games/garden/>
* Factory: <https://sabiruby.github.io/rubevy_games/factory/>

(Battle used to be at the top address itself; it moved down one when the garden arrived, and the
top address is now the page that links to all three.) Or on a PC:

```
cargo run -p sabibots                   # a window, 2D, 16:9
cargo run -p sabibots -- --headless 15  # no window: 15 seconds, the result on stdout
cargo run -p garden                     # the other one, 3D: click a creature, edit its file, Ctrl+Enter
                                        #   (F2 the VM panel, P pause, F5 saves the garden, F9 brings it back)
cargo run -p garden -- --headless 90    # no window: 90 seconds
cargo run -p garden -- --headless 30 --save garden.save.json   # ... and write it down at the end
cargo run -p factory                    # the third one: 1-4 and 5.. build, click an inserter to
                                        #   write its Ruby, F1 the editor's three files, H the guide
cargo run -p factory -- --headless 66   # no window: 66 seconds
cargo run -p factory -- --stress 4000   # a loop of belt, measured (it sizes its own map)
web/build.sh && web/serve.sh            # all three browser builds, at http://localhost:8080/
web/build.sh garden                     # just one, at http://localhost:8080/garden/
```

The PC build and the browser build are the same code; which one you get is chosen by the target
(each game's `src/platform.rs`, and [`docs/web.md`](docs/web.md) for how the browser one is put
together). In the browser, **Save** keeps the behaviour — or the whole garden, or the whole
factory — in the browser's local storage instead of a file, and there is no file to edit from
outside.

Then change how a robot fights in the editor on the right of the game window: **Apply** (F5)
runs the edited behaviour in that robot straight away, in memory, without touching the file; **Save
to file** (Ctrl+S) keeps it. Editing `sabibots/ruby/robots/scout.rb` in any other editor works
too — saving it restarts the robots on that file. The buttons along the top of the editor pick
the robot. `sabibots/ruby/matches/training.rb` is the
match itself — who is on the field, when the walls close in, what ends it — and it is Ruby too.

## Why Ruby here

Three things the VM gives a game, which is what these are built to show:

* **A script cannot hang the game.** Each behaviour is a task with a timeslice counted in
  instructions, so `loop { }` in someone's robot costs it its turn, not your frame rate.
* **Waiting is free.** `sleep 0.05`, or asking the game something (`radar`) parks the task until
  there is an answer. Nothing polls, nothing spins, and the code stays sequential.
* **Ruby is a good DSL.** `robot "Scout" do … end` is a class built at load time; what a robot can
  do is a method on it. No parser, no configuration format, no rebuild.

## Layout

```
crates/rubevy-egui/   the panels, for any rubevy game: the code editor, the VM inspector,
                      watching the Ruby directory for edits, and what they cover of the window
crates/games-shell/   what the games here stand on: the PC/browser split, the checks, the
                      flags, the in-game guide, the settings, the HUD and the two cameras
sabibots/             SabiRuby Battle
  src/main.rs         the game: arena, robots, bullets, and the answers to what Ruby asks
  src/platform.rs     what differs between the PC build and the browser build
  ruby/prelude.rb     the DSL every robot is written in
  ruby/robots/*.rb    the robots
garden/               Garden: the 3D world, and the components its creatures read by name
  src/main.rs         the rules, and the components that are the Ruby API
  src/window.rs       the editor, the VM panel and the HUD — the game's half of rubevy-egui's
  src/platform.rs     the same split as sabibots'
  ruby/prelude.rb     the DSL the creatures are written in
  ruby/creatures/*.rb one file per species
factory/              Factory: the grid, the belts, the machines and the arms
  src/belts.rs        what one step of the factory is — no Bevy in it, so it unit-tests alone
  src/data.rs         the data stage: ruby/data.rb read into tables before the first frame
  src/control.rs      the control stage: the five events, the goal, and winning
  src/inserters.rs    the one machine with a mind, and the scripts the player writes
  ruby/data.rb        the items, the recipes, the machines, the map and every number of play
  ruby/control.rb     what the factory is for
  ruby/inserter.rb    when an arm moves a thing
web/                  the browser build: build.sh, serve.sh, index.html (the entry page), and
                      one page.html.in with one block of values a game in games.sh, which
                      build.sh fills in (dist/ is the output)
tools/                fixedlines.sh, which turns a run of the checks into a list two runs can
                      be diffed by, subset-font.sh for the guide's Japanese, and the factory-*.py
                      that build Factory's tile sheet out of Kenney's packs
docs/                 how it is put together, what the checks have to say, and what is next
```

Bevy is pinned once, in the workspace's `[workspace.dependencies]`: a binary can have only one
Bevy, and keeping the version in one place makes bumping it a single edit for every game here.

## Requirements

A GPU for the windowed mode (Bevy's usual Linux dependencies apply). `--headless` needs none and
is what the games are tested with. The Ruby is compiled in process by the reference mruby
compiler (`sabiruby-compiler`), so nothing has to be built ahead of time.

The browser build needs the `wasm32-unknown-unknown` target, `wasm-bindgen-cli` of the version
in `Cargo.lock`, optionally `wasm-opt`, and a checkout of
[sabiruby-playground](https://github.com/sabiruby/sabiruby-playground) next to this one with its
`tools/build.sh` run (that is where the compiler module comes from). See `docs/web.md`.

## License

MIT (`LICENSE`). The Ruby in `*/ruby/` is part of the games and under the same terms.
The art is Kenney's (CC0) — *Top-down Tanks Remastered*, *Nature Kit* and *Cube Pets*, *Tiny
Factory* and *Tiny Farm* — with a few sprites drawn to the same palettes, and one subset of Noto
Sans JP (OFL) for the guide's Japanese; see [`CREDITS.md`](CREDITS.md).
