# SabiRuby Battle (`sabibots`)

Two robots in a square arena. Each robot's brain is one `.rb` file, running as a task in the one
VM the game keeps, and every move it makes goes through a question the game answers.

## The boundary

The game owns the world; Ruby owns the decisions.

| Ruby asks | the game answers | what it does |
|---|---|---|
| `Rubevy.ask("me")` | `[x, y, hp, heading]` | where this robot is |
| `Rubevy.ask("scan", range)` | `[dx, dy, distance]` or `nil` | the nearest living enemy within range |
| `Rubevy.ask("thrust", dx, dy)` | `true` | push in a direction; the game caps the speed |
| `Rubevy.ask("fire", dx, dy)` | `true` / `false` | shoot, unless the gun is still cooling |
| `Rubevy.ask("arena")` | half the arena's width | so the DSL can keep off the walls |

Every one of them parks the robot's task until the answer comes back (`ScriptWorld::answer`), so
a robot waiting for a scan costs nothing and the other robot keeps running. Nothing is a callback
and nothing polls; a robot reads as ordinary sequential Ruby:

```ruby
loop do
  target = scan(40)
  aim_and_fire(target) if target
  sleep 0.05
end
```

The game answers everything in the same frame today. It does not have to — `Rubevy.ask` was built
so an answer can come frames later (a path request, an asset load), and the robot simply waits.

## The DSL

`ruby/prelude.rb` defines `Robot` (what a robot can do) and `robot "Name" do … end`, which is
`Class.new(Robot)` with the block `class_eval`'d into it. So a robot file is a class body: `def
run` is the brain, `def patrol` is a helper, and instance variables are the robot's memory
between frames.

The prelude is put in front of the robot's file and the two are compiled as one program, which is
why a robot file needs no `require`. Reading it as one program is also what makes reloading a
single file easy.

## Reloading

`rubevy-arena`'s `Watch` watches `ruby/` with `notify`. On a write the game recompiles that robot
(prelude + file) and gives the entity a new `Script`, dropping the old `ScriptTask`: the robot
starts over with the new brain, the world is untouched, and the other robot never notices. Saving
`prelude.rb` reloads every robot.

A compile error is reported in the HUD line and the log, and the robot keeps its old brain.

## Fairness, and why a bad robot cannot ruin the match

mruby-task gives each task a timeslice counted in instructions (the VM's own scheduler), and
rubevy hands the scheduler a budget per frame. A robot that never sleeps is preempted and resumed
next frame; a robot that raises has its exception become the task's result, which the game reports
and the match carries on. Neither can stop the frame.

## The HUD

One line per robot: its name, hp, a bar of what its brain spent on the last frame, the count, and
**the line it is standing on** — `scout.rb:22` in its own file, or `prelude.rb:18` where it is
inside the DSL (waiting for a scan, usually). It comes from the VM: `Vm::task_instructions` and
`Vm::task_location`, through `ScriptWorld::stats`, and both work while a task is parked as well as
while it runs.

The line numbers are the robot's own. The prelude is compiled in front of the robot's file, so
the game subtracts its length and says `prelude.rb:N` for anything above it.

The bar fills as a robot approaches a timeslice's worth of instructions (3,000) — the point where
it starts costing the other robot its turn. A robot thinking normally is tens to hundreds.

## Seeing it where there is no window

`cargo run -p sabibots -- --shot shot.png 5` opens the window, waits five seconds, writes a PNG
and exits. With `docker/run.sh` (see `docs/wsl-gpu.md`) that works on a machine with no GPU
driver, which is how the screenshots in this repository were made.

## What v0.1 does not do yet

* **An in-game editor.** The plan is: read-only code panel with the current line first, editing
  after that (Bevy has no text input of its own, so it means `bevy_egui` or a small widget).
* **More than two robots, teams, a match DSL.** `match "Training" do … end` — the second pattern
  the games are for (Ruby running the whole game, not just the agents).
* **Sound, sprites, effects.** Everything is a coloured square.

## Running

```
cargo run -p sabibots                    # window
cargo run -p sabibots -- --headless 15   # 15 seconds, result on stdout, no GPU needed
```

The headless mode runs the same systems as the window and prints each robot's hp and position at
the end. It is how the game is checked where there is no GPU.
