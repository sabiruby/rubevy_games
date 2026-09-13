# rubevy_games

Games whose brains are Ruby. They run on [rubevy](https://github.com/kishima/rubevy) — the
[SabiRuby](https://github.com/kishima/sabiruby) VM (mruby 4.1 bytecode, written in Rust) inside
[Bevy](https://bevyengine.org) — and the Ruby in them is read from `.rb` files as the game runs,
so editing a file changes what happens on screen without a rebuild.

| game | what it is | state |
|---|---|---|
| [`sabibots`](sabibots) | **SabiRuby Battle** — robots that fight; each robot's brain is one Ruby file | playable v0.1 |
| `factory` | machines on a line, each with its own script; queues are the conveyors | planned |
| `cards` | a card game whose rules are a Ruby DSL | planned |

```
cargo run -p sabibots                 # a window, 2D, 900x900
cargo run -p sabibots -- --headless 15  # no window: 15 seconds, the result on stdout
```

Then open `sabibots/ruby/robots/scout.rb`, change how it fights, and save: that robot starts
again with the new brain while the game keeps running.

## Why Ruby here

Three things the VM gives a game, which is what these are built to show:

* **A script cannot hang the game.** Each brain is a task with a timeslice counted in
  instructions, so `loop { }` in someone's robot costs it its turn, not your frame rate.
* **Waiting is free.** `sleep 0.05`, or asking the game something (`scan`) parks the task until
  there is an answer. Nothing polls, nothing spins, and the code stays sequential.
* **Ruby is a good DSL.** `robot "Scout" do … end` is a class built at load time; what a robot can
  do is a method on it. No parser, no configuration format, no rebuild.

## Layout

```
crates/rubevy-arena/   the 2D floor: camera, HUD, watching the Ruby directory for edits
sabibots/             SabiRuby Battle
  src/main.rs         the game: arena, robots, bullets, and the answers to what Ruby asks
  ruby/prelude.rb     the DSL every robot is written in
  ruby/robots/*.rb    the robots
docs/                 how it is put together, and what is next
```

Bevy is pinned once, in the workspace's `[workspace.dependencies]`: a binary can have only one
Bevy, and keeping the version in one place makes bumping it a single edit for every game here.

## Requirements

A GPU for the windowed mode (Bevy's usual Linux dependencies apply). `--headless` needs none and
is what the games are tested with. The Ruby is compiled in process by the reference mruby
compiler (`sabiruby-compiler`), so nothing has to be built ahead of time.

## License

MIT (`LICENSE`). The Ruby in `*/ruby/` is part of the games and under the same terms.
