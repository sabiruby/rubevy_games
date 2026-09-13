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

## Telling the robots apart

Every robot has a number — the order the match put it on the field — and it is the same number
everywhere: over its head (`3 scout  hp 86`, in its team's colour), at the start of its HUD row
(`3 blue/scout`), in the editor's title, and on the key that picks it (`3`). The robot the editor
is showing is marked over its head: `> 1 scout  hp 100 <` in yellow. The text has a dark copy
behind it so it reads on the sand, and it follows the robot rather than being its child, so it
does not turn with the hull.

While the editor is open the camera slides so the arena sits in the part of the window the editor
does not cover; hide the editor (`F1`) and the arena comes back to the middle.

## The editor

The right-hand window is an editor (egui, through `bevy_egui` 0.42 — the release built for Bevy
0.19). It shows the watched robot's own file and puts a band behind **the line of that file its
brain is standing on**:

```
red/scout  scout.rb  (in prelude.rb:15)
   9        target = scan(40)        ← banded
```

Type in it and press **Ctrl+S** (or the Save button): the file is written, the same file watcher
that notices an edit made in any other editor notices this one, and that robot starts again with
the new brain while the match carries on. `● unsaved` shows while the text differs from the file.

Along the top of the editor is a button per robot (`1 scout`, `2 hunter`, … in team colours, faded when the robot is down); click one to show its brain. `1`–`8` do the same from the keyboard, `Tab` moves on, `F1` hides the editor. Switching to another file keeps what was typed into the one being left: unsaved edits are held per file and come back when you return to it, and the editor lists which files have them. Keys typed into the editor stay the
editor's: a `2` in the code does not switch robots (`EguiWantsInput`).

The banded line is the innermost frame **in the robot's own file**, which is not the innermost
frame: a robot waiting for a scan stands three frames deep inside `prelude.rb`. The VM answers the
whole stack (`Vm::task_frames`), the editor picks the frame in the file it shows, and the title
says where the brain really is.

Two details that make it a listing rather than a text box: it never wraps (a custom layouter sets
the wrap width to infinity), so each row of the gutter is one line of the file; and the band is a
background on that line's text section, so it moves with the text as the file is edited.

The editor lives in `rubevy-arena` (`Editor`, `EditorPlugin`) so the other games get it for free.
The older read-only panel (`CodePanel`, Bevy UI only) is still there for a game that does not want
egui.

## Blasts, and the view following the arena

A hit leaves a small puff and a robot going down a large one (Kenney's `explosion1` / `explosion3`),
growing and fading over a quarter of a second or 0.7 s.

The window is 16:9 and the arena square: the camera keeps the arena's height (plus a little
floor past the wall) in view, and the floor reaches past the wall sideways. When the match closes
the walls in, `ArenaSize` changes, the wall of crates is rebuilt at the new edge, and the camera
zooms to it — the fight fills the window as the field gets smaller.

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

* **Sound, sprites, effects.** Everything is a coloured square.

## Running

```
cargo run -p sabibots                    # window
cargo run -p sabibots -- --headless 15   # 15 seconds, result on stdout, no GPU needed
```

The headless mode runs the same systems as the window and prints each robot's hp and position at
the end. It is how the game is checked where there is no GPU.
