# rubevy_games

Games whose brains are Ruby. They run on [rubevy](https://github.com/sabiruby/rubevy) — the
[SabiRuby](https://github.com/sabiruby/sabiruby) VM (mruby 4.1 bytecode, written in Rust) inside
[Bevy](https://bevyengine.org) — and the Ruby in them is read from `.rb` files as the game runs,
so editing a file changes what happens on screen without a rebuild.

| game | what it is | state |
|---|---|---|
| [`sabibots`](sabibots) | **SabiRuby Battle** — robots that fight; each robot's brain is one Ruby file | playable v0.1 |
| [`garden`](garden) | **Garden** — a 3D world whose creatures read and write their own ECS components from Ruby by name, and breed by mixing a `Genome` that is a Rust struct and a Ruby class at once | the world, the minds and the genome (G0–G2); save/load through serde is next |
| `factory` | machines on a line, each with its own script; queues are the conveyors | planned |
| `cards` | a card game whose rules are a Ruby DSL | planned |

**Play it in a browser:** <https://sabiruby.github.io/rubevy_games/> — the same game, built for
the web. Or on a PC:

```
cargo run -p sabibots                   # a window, 2D, 16:9
cargo run -p sabibots -- --headless 15  # no window: 15 seconds, the result on stdout
cargo run -p garden                     # the other one, 3D
cargo run -p garden -- --headless 90    # no window: 90 seconds
web/build.sh && web/serve.sh            # the browser build, at http://localhost:8080/
```

The PC build and the browser build are the same code; which one you get is chosen by the target
(`sabibots/src/platform.rs`, and [`docs/web.md`](docs/web.md) for how the browser one is put
together). In the browser, **Save** keeps the brain in the browser's local storage instead of a
file, and there is no file to edit from outside.

Then change how a robot fights in the editor on the right of the game window: **Apply** (F5)
runs the edited brain in that robot straight away, in memory, without touching the file; **Save
to file** (Ctrl+S) keeps it. Editing `sabibots/ruby/robots/scout.rb` in any other editor works
too — saving it restarts the robots on that file. The buttons along the top of the editor pick
the robot. `sabibots/ruby/matches/training.rb` is the
match itself — who is on the field, when the walls close in, what ends it — and it is Ruby too.

## Why Ruby here

Three things the VM gives a game, which is what these are built to show:

* **A script cannot hang the game.** Each brain is a task with a timeslice counted in
  instructions, so `loop { }` in someone's robot costs it its turn, not your frame rate.
* **Waiting is free.** `sleep 0.05`, or asking the game something (`radar`) parks the task until
  there is an answer. Nothing polls, nothing spins, and the code stays sequential.
* **Ruby is a good DSL.** `robot "Scout" do … end` is a class built at load time; what a robot can
  do is a method on it. No parser, no configuration format, no rebuild.

## Layout

```
crates/rubevy-arena/   the 2D floor: camera, HUD, watching the Ruby directory for edits
sabibots/             SabiRuby Battle
  src/main.rs         the game: arena, robots, bullets, and the answers to what Ruby asks
  src/platform.rs     what differs between the PC build and the browser build
  ruby/prelude.rb     the DSL every robot is written in
  ruby/robots/*.rb    the robots
garden/               Garden: the 3D world, and the components its creatures will read by name
  src/main.rs         the rules, and the components that are the Ruby API
  src/platform.rs     the same split as sabibots'
web/                  the browser build: build.sh, serve.sh, index.html (dist/ is the output)
docs/                 how it is put together, and what is next
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
The art is Kenney's *Top-down Tanks Remastered* (CC0); see [`CREDITS.md`](CREDITS.md).
