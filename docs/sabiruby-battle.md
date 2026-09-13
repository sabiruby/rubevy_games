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
match "Training", noise: 0.3 do
  team :red,  robots: %w[scout hunter]
  team :blue, robots: %w[scout scout]

  # from 20 s on, the walls close in one crate every 2 seconds
  rule(:sudden_death, after: 20, every: 2.0) { |m| m.shrink 1 }

  on_destroyed { |file, team| Rubevy.log("match: #{team}'s #{file} is down") }
end
```

The match asks for `spawn` (a robot file, a team, a place), `board` (every robot with its team,
hp and position), `events` (what has happened since it last looked), `clock`, `shrink` and `win`.
Nothing about the rules is in the Rust: the number of teams, when the walls move, what counts as
a victory — all of it is in that file, and saving it starts the match again.

`ruby/match_prelude.rb` is the DSL, the same way `ruby/prelude.rb` is the robots'.

## The boundary

The game owns the world; Ruby owns the decisions. What a robot gets is deliberately raw — a tank's
controls and noisy readings — so there is room to write a brain that is better than another one.

| Ruby asks | the game answers | what it does |
|---|---|---|
| `ask("status")` | `[x, y, hp, team, heading, speed, turret, energy, cooldown, arena, time]` | the robot itself |
| `ask("radar", range)` | the status row, then `[id, team, hp, x, y, vx, vy, heading, turret, distance, bearing]` per robot | every other robot still running within range, friends included, through the match's noise |
| `ask("incoming", range)` | `[[x, y, vx, vy, distance], …]`, nearest first | shots from other teams on their way |
| `ask("act", throttle, turn, aim, power)` | `[fired, energy, cooldown]` | the controls; any of them `-999` to leave as it is |
| `ask("seed")` | a number | a robot's own dice, rolled from the match's, for `srand` |

And what only the match asks for:

| the match asks | the game answers |
|---|---|
| `ask("rules", noise, seed)` | how noisy radars and guns are; the dice (`-1`: the clock) |
| `ask("spawn", file, team, x, y)` | the new robot's id; it comes with its own brain |
| `ask("board")` | `[[id, team, hp, x, y], …]` |
| `ask("events")` | `[[kind, id, other], …]` since the last call — `0` is "down" |
| `ask("clock")` | seconds since the match started |
| `ask("shrink", crates)` | every wall moves in by that many crates (down to 7 a side); the new half-width |
| `ask("win", team)` | the result, for the HUD |

Every one of them parks the robot's task until the answer comes back (`ScriptWorld::answer`), so
a robot waiting for its radar costs nothing and the other robots keep running. Nothing is a
callback and nothing polls; a robot reads as ordinary sequential Ruby:

```ruby
loop do
  target = nearest_enemy(60)
  if target
    angle = lead(target, 0.9)
    act throttle: 0.5, turn: steer_to(target.bearing), aim: angle,
        fire: aimed?(angle) ? 0.9 : nil
  end
  sleep 0.08
end
```

A question costs about a frame: the task asks, the game answers in its next pass, the scheduler
resumes the task. That is why `act` sets all four controls at once and `radar` brings the robot's
own status back with it — a brain that asked one question per control would react a few frames
late. The game answers everything in the frame it is asked today; `Rubevy.ask` also allows an
answer that comes frames later (a path request, an asset load), and the robot simply waits.

## A tank, not a cursor

The first version had `thrust(dx, dy)` (move in any direction, at once) and `fire(dx, dy)` (shoot
in any direction, at once) and `scan` (the nearest enemy, exactly). Every brain written with them
came out the same, because there was nothing to be good at: pointing at the enemy was already the
best aim, and moving sideways was already the best dodge.

Now a robot is a tank:

* **The hull turns at a limited rate** (`turn`, -1..1 of 2.6 rad/s) and the robot moves only along
  it (`throttle`, -1..1; reverse is slower). Speed takes about a fifth of a second to build and
  to die away. Getting out of the way of a shot means turning first.
* **The turret turns on its own**, towards the angle it was given, at 4 rad/s. A robot can drive
  one way and shoot another, but a turret that has to swing round is a turret that is not
  firing.
* **A shot's power** (0.2..1) trades damage (4..16) for speed (55..30), reload (0.3..0.8 s) and
  energy. A light shot is hard to dodge and does little; a heavy one hurts and can be seen coming.
* **Energy** (100) comes back at 12/s. Full throttle costs 9/s, a shot 16 × (0.25 + power). A robot
  that fires everything it has cannot also run, and one with none left crawls at a third of its
  speed and cannot fire at all.
* **Shots fly**, so a moving target has to be led: aim where it will be, not where it is.
* Tanks push each other apart instead of driving through each other, and a team's shots pass over
  its own robots.

## Randomness

`match "Training", noise: 0.3, seed: 7 do … end`:

* `noise` (0..1) blurs the radar — positions by up to `noise × distance × 5%`, velocities too — and
  the shots from `incoming`, and spreads every shot by up to `noise × 0.1` rad on top of a small
  spread every gun has. A robot far away is a guess; a robot close is a fact.
* `seed` fixes the game's dice. Each robot's `rand` is `srand`ed from them when it starts, so a
  robot that flips coins (the scout's circling direction, the hunter's wandering) flips the same
  coins in a seeded match. Leave it out and every match is different. A replay with the same seed
  is alike rather than identical: frame times still vary, and so do the moments the questions are
  answered.

## The DSL

`ruby/prelude.rb` has two halves.

The first is **what the game offers**, as thin as it can be: `status`, `me` (the last status seen,
without asking), `radar(range)` → `Contact`s, `incoming(range)` → `Shot`s, and
`act(throttle:, turn:, aim:, fire:)` with `drive`, `aim`, `fire`, `stop` as one-control shorthands.
`Status`, `Contact` and `Shot` are plain classes over the rows the game answers.

The second is **a library written on top of it in plain Ruby**, which a robot can use, copy and
change, or ignore:

| helper | what it works out |
|---|---|
| `angle_diff(a, b)`, `angle_to(x, y)`, `distance_to`, `angle_to_center` | geometry from where the robot is |
| `steer_to(angle)` | a `turn` value that brings the hull round, gently as it gets close |
| `enemies(range)`, `nearest_enemy(range)` | the radar, filtered |
| `lead(target, power)` | where to aim so a shot of that power meets a target that keeps its course |
| `aimed?(angle, tolerance)` | the gun is ready and the turret is close enough |
| `near_wall?(margin)` | time to turn back in |
| `on_collision?(shot)`, `dodge_angle(shot)` | whether a shot will hit if nothing changes, and which way is across its path |
| `wander_turn` | a turn that keeps a direction for a while, then picks another at random |

None of the helpers asks the game anything except through `radar`: they compute a number, and the
robot's `act` sends them all at once.

`robot "Name" do … end` is `Class.new(Robot)` with the block `class_eval`'d into it. So a robot
file is a class body: `def run` is the brain, other `def`s are its own helpers, and instance
variables are the robot's memory between frames.

The two robots that come with the game:

* **scout** circles its nearest enemy (the side flips now and then), dodges a shot that is on
  course to hit it, and fires light, fast shots (power 0.3) while it has energy to spare.
* **hunter** drives at its enemy until it is 18 away, backs off slowly from there, and fires heavy
  shots (0.9) when its turret is on the lead angle; with nothing on the radar it wanders with its
  turret sweeping.

## Next: reflexes

A brain is one loop, so a robot that is sleeping between decisions notices a hit only on its next
pass. The next step is `reflex(:hit) { … }`: a block that runs as a task of its own when the game
reports a hit, beside the main loop — mruby-task's scheduler already runs many tasks, and this
is where a robot would start using more than one.

## The DSL

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

## The scoreboard

The panel at the top left has one row per robot, in words and bars:

| column | what it is |
|---|---|
| robot | its number and brain (`1 scout`, `*` if it runs an applied brain), in its team's colour; click it to show it in the editor |
| health | a bar, green, yellow below half, red below a quarter; `down` when it is out |
| energy | a bar out of 100: driving and firing spend it, time brings it back |
| thinking | the instructions its Ruby runs per frame, averaged over about a second; a timeslice's worth (3,000) fills the bar |

Each robot also has a small health bar over it, under its name.

The first version showed `prelude.rb:15` and a per-frame instruction count. Both were true and
neither was readable: a brain passes through a dozen lines a frame and sleeps most frames, so the
"current line" jumps and the count flickers between 0 and a hundred. A `mostly doing` column (the
line it had spent the most time on lately) replaced it and went the same way — two lines that
take about the same time trade places many times a second — and so did the editor's
`(in prelude.rb:82)`. What is left is what can be read: the editor's shading, which fades over
about a second, and the smoothed instruction count.

The scoreboard and the editor are windows with title bars: drag them anywhere, and fold the
scoreboard away with its triangle. The scoreboard starts at the top left, the editor at the top
right. The scoreboard's button starts the match over (below).

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
1 red/scout  scout.rb
   7      target = nearest_enemy(45)        ← shaded
```

What is typed stays **in memory** until you say otherwise. Trying something in a running match
should not rewrite the project on disk — and a build whose files cannot be written (a packaged
game, a browser) gets the same editor. Four buttons decide what happens to the text:

| button | key | what it does |
|---|---|---|
| **Apply** | F5 / Ctrl+Enter | compiles the text and restarts **this robot only** with it; the file is untouched |
| **Apply to all *file*** | | the same, for every robot whose brain is that file |
| **Save to file** | Ctrl+S | writes the text to the file; robots on the file pick it up |
| **Revert** | | forgets the edits and puts this robot back on its file |

A robot running an applied brain is marked `*` — `3 scout*` on its button and over its head, and
`scout.rb*` in the editor's title. The file watcher leaves such robots alone: an edit made in
another editor reaches the robots on the file, not the one you are experimenting with. A text that
does not compile is not applied; the editor says why and the robot keeps the brain it has.

Closing the game forgets applied brains that were not saved. Edits typed and not applied are kept
per robot while the game runs, and the editor lists the robots that have some.

The editor itself does no file I/O (`EditorAction` is what it hands the game), so the rules above
live in the game (`do_editor_actions`), and another game can choose different ones.

`SABIBOTS_SELFTEST=1 docker/run.sh` drives these buttons the way a click would and checks the
outcome: Apply reaches only the shown robot and writes nothing, Apply to all reaches the robots on
the same file and no others, Revert puts the shown robot back on its file. Save is left out of it
on purpose, since it writes to the repository.

Along the top of the editor is a button per robot (`1 scout`, `2 hunter`, … in team colours, faded when the robot is down); click one to show its brain. `1`–`8` do the same from the keyboard, `Tab` moves on, `F1` hides the editor. Switching to another file keeps what was typed into the one being left: unsaved edits are held per file and come back when you return to it, and the editor lists which files have them. Keys typed into the editor stay the
editor's: a `2` in the code does not switch robots (`EguiWantsInput`).

The line that is shaded is the innermost frame **in the robot's own file**, which is not the
innermost frame: a robot waiting for its radar stands a few frames deep inside `prelude.rb`. The
VM answers the whole stack (`Vm::task_frames`) and the game picks the frame in the file it shows,
accumulating it into the shading every frame.

Two details that make it a listing rather than a text box: it never wraps (a custom layouter sets
the wrap width to infinity), so each row of the gutter is one line of the file; and the shading is a
background on that line's text section, so it moves with the text as the file is edited.

The editor lives in `rubevy-arena` (`Editor`, `EditorPlugin`) so the other games get it for free.
The older read-only panel (`CodePanel`, Bevy UI only) is still there for a game that does not want
egui.

## Starting again

`Restart (R)` on the scoreboard — `Play again (R)`, once there is a winner — starts the match over
without restarting the game. Everything on the field is despawned (robots, and with them their
barrels, names and bars; shots; blasts; the match's own entity), the arena goes back to its size,
and the match script is started again, so it reads `training.rb` afresh and spawns the robots as
it did the first time.

Despawning is also what stops the old scripts: rubevy terminates a task when its entity goes (or
its `ScriptTask` is removed). Until that was added a reload left the old brain running beside
the new one, asking for the same body — invisible, since nothing showed it any more. A question
asked just before a restart is answered with `nil` and nobody is waiting for it.

A brain applied in the editor and not saved comes back with the robot's number: number 3 is still
the third robot the match spawns. Edits typed and not applied are dropped.

A robot that goes down has its task stopped the same way, so a wreck no longer spends
instructions on a body that cannot move.

`SABIBOTS_SELFTEST=1` ends by pressing Restart and checking that there are four robots again (not
eight), all at full health, that an applied brain came back and a reverted one did not, and that
the wall ends on its corners.

## Blasts, and the view following the arena

A robot that is down turns grey — its hull swapped for Kenney's dark one, its name and its
editor button greyed — so what is still in the fight is what has colour. A hit leaves a small puff
and a robot going down a large one (Kenney's `explosion1` / `explosion3`),
growing and fading over a quarter of a second or 0.7 s.

The window is 16:9 and the arena square: the camera keeps the arena's height (plus a little
floor past the wall) in view, and the floor reaches past the wall sideways. When the match closes the walls in, `ArenaSize` changes and the wall of crates is rebuilt at the new edge. The crates are spaced about 2.6 apart with the step stretched so each side is a whole number of them, so every side ends exactly on a corner (a fixed step used to leave the top and right sides a crate past the corner). The walls close in a crate at a time: `shrink(1)` takes one crate off each end of every side, the crate size stays the same, and the arena gets smaller in steps the eye can follow — 25 crates a side at the start, 23, 21, … down to 7, where it stops. A rule with `every:` repeats, which is how the match does it gradually (the first version shrank by 8 and then 6 units at once, which looked like a jump rather than walls moving). The camera stays where it was framed, so the walls are seen moving in; zooming to follow them made the shrink hard to notice, and it is off by default (`ArenaPlugin::follow_shrink`).

## Seeing it where there is no window

`cargo run -p sabibots -- --shot shot.png 5` opens the window, waits five seconds, writes a PNG
and exits. With `docker/run.sh` (see `docs/wsl-gpu.md`) that works on a machine with no GPU
driver, which is how the screenshots in this repository were made.

## The picture

Kenney's *Top-down Tanks Remastered* (CC0), a dozen files of it in `sabibots/assets/sprites/`:
tank hulls for the robots, their own coloured shots, sand tiles for the floor and metal crates
for the wall, and a barrel on each hull that turns on its own (Kenney has red and blue barrels;
the other teams get a tinted blue one). The hull points where the tank is heading and the barrel
where the turret is, which makes the Ruby's decisions legible at a glance — a scout circling its
enemy with its gun turned inwards looks like exactly that.

Bevy looks for assets next to the executable, which is not where a workspace puts them, so the
game points `AssetPlugin` at its own `assets/` directory.

## What it does not do yet

* **Sound.**
* **Reflexes** (see above): a brain notices a hit on its next pass, not when it happens.
* **Keeping edits that were typed and not applied across a restart.** Applied brains are kept.

## Running

```
cargo run -p sabibots                    # window
cargo run -p sabibots -- --headless 15   # 15 seconds, result on stdout, no GPU needed
web/build.sh && web/serve.sh             # in a browser (docs/web.md)
```

The headless mode runs the same systems as the window and prints each robot's hp and position at
the end. It is how the game is checked where there is no GPU.
