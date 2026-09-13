# SabiRuby Battle (`sabibots`)

Two robots in a square arena. Each robot's brain is one `.rb` file, running as a task in the one
VM the game keeps, and every move it makes goes through a question the game answers.

## Two kinds of script

A **robot** is one file with a brain in it. A **match** is one file that runs the whole game: it
puts the robots on the field, watches what happens to them, and decides when it is over. Both are
tasks in the same VM, written in the same way — ask the game something, wait for the answer — and
the game does only what it is told.

```ruby
# ruby/matches/training.rb
match "Training" do
  team :red,  robots: %w[scout hunter]
  team :blue, robots: %w[scout scout]

  rule(:sudden_death, after: 20) { |m| m.shrink 8.0 }   # the walls close in
  rule(:closer, after: 35) { |m| m.shrink 6.0 }

  on_destroyed { |file, team| Rubevy.log("match: #{team}'s #{file} is down") }
end
```

The match asks for `spawn` (a robot file, a team, a place), `board` (every robot with its team,
hp and position), `events` (what has happened since it last looked), `clock`, `shrink` and `win`.
Nothing about the rules is in the Rust: the number of teams, when the walls move, what counts as
a victory — all of it is in that file, and saving it starts the match again.

`ruby/match_prelude.rb` is the DSL, the same way `ruby/prelude.rb` is the robots'.

## The boundary

The game owns the world; Ruby owns the decisions.

| Ruby asks | the game answers | what it does |
|---|---|---|
| `Rubevy.ask("me")` | `[x, y, hp, heading]` | where this robot is |
| `Rubevy.ask("scan", range)` | `[dx, dy, distance]` or `nil` | the nearest living enemy within range |
| `Rubevy.ask("thrust", dx, dy)` | `true` | push in a direction; the game caps the speed |
| `Rubevy.ask("fire", dx, dy)` | `true` / `false` | shoot, unless the gun is still cooling |
| `Rubevy.ask("arena")` | half the arena's width | so the DSL can keep off the walls |

And what only the match asks for:

| the match asks | the game answers |
|---|---|
| `ask("spawn", file, team, x, y)` | the new robot's id; it comes with its own brain |
| `ask("board")` | `[[id, team, hp, x, y], …]` |
| `ask("events")` | `[[kind, id, other], …]` since the last call — `0` is "down" |
| `ask("clock")` | seconds since the match started |
| `ask("shrink", by)` | the arena's new half-width; the wall of crates moves |
| `ask("win", team)` | the result, for the HUD |

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

## The code panel

The right-hand side lists the watched robot's own file with the line it is standing on marked:

```
scout.rb:22
   20      end
   21    end
>  22    sleep 0.05
   23  end
```

`1`, `2`, … pick a robot, `Tab` moves on, `C` hides the panel.

The marked line is the innermost frame **in the robot's own file**, which is not the innermost
frame: a robot waiting for a scan is standing inside `prelude.rb`, three frames down. The VM
answers the whole stack (`Vm::task_frames`), so the panel can show the line of the author's file
that is waiting, and the title says where it actually is (`scout.rb  (in prelude.rb:15)`).

It is read-only for now. Editing in place is the next step, and the lines are kept as a
`Vec<String>` for that reason.

## Seeing it where there is no window

`cargo run -p sabibots -- --shot shot.png 5` opens the window, waits five seconds, writes a PNG
and exits. With `docker/run.sh` (see `docs/wsl-gpu.md`) that works on a machine with no GPU
driver, which is how the screenshots in this repository were made.

## The picture

Kenney's *Top-down Tanks Remastered* (CC0), a dozen files of it in `sabibots/assets/sprites/`:
tank hulls for the robots, their own coloured shots, sand tiles for the floor and metal crates
for the wall. The hull turns to where the robot is moving or last fired
(`heading`), which makes the Ruby's decisions legible at a glance — a robot circling its enemy
looks like it is circling.

Bevy looks for assets next to the executable, which is not where a workspace puts them, so the
game points `AssetPlugin` at its own `assets/` directory.

## What v0.1 does not do yet

* **An in-game editor.** The plan is: read-only code panel with the current line first, editing
  after that (Bevy has no text input of its own, so it means `bevy_egui` or a small widget).
* **Sound, sprites, effects.** Everything is a coloured square.

## Running

```
cargo run -p sabibots                    # window
cargo run -p sabibots -- --headless 15   # 15 seconds, result on stdout, no GPU needed
```

The headless mode runs the same systems as the window and prints each robot's hp and position at
the end. It is how the game is checked where there is no GPU.
