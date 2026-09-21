# What the games' checks have to say

Every game here runs a `selftest` that prints one line per thing it has proved. Until 2026-09-20 the
standard those runs were held to was a **number** — "the garden: ok 43", "Battle: 31 fixed lines
plus two a hit" — which is not a standard. A number does not say *which* lines, so a check that
quietly stopped running and another that quietly started cancel out, and a check whose sentence
changed looks like two checks changing at once. This file is the list itself.

## How two runs are compared

`tools/fixedlines.sh` turns a run's output — a PC run's stdout or a Playwright console log —
into one line per check: the verdict, the sentence, every number blanked, sorted.

```
tools/fixedlines.sh run.log
diff <(tools/fixedlines.sh before.log) <(tools/fixedlines.sh after.log)
grep -c 'selftest: FAIL' run.log            # and this must be 0
```

It leaves out the lines a run has as many of as it happens to have — the stage directions, and
Battle's one or two lines a hit, and the garden's one line a pairing. The script's own head says
which and why. Numbers are blanked because nearly every sentence carries a time or a count and
none of them is the same twice; a number that *is* the point of a check is judged by the check,
which is what the verdict in front of the sentence says.

So a stage of work is checked by running both games before and after and diffing the lists. A
stage that means to add a check adds exactly one line; a stage that means to change nothing
diffs empty. **Both halves matter**: the diff being empty is not enough on its own if the run
printed a `FAIL`, and no `FAIL` is not enough if a check silently went missing.

## How each run is asked for

```
SABIBOTS_SELFTEST=1 cargo run -p sabibots -- --headless 25     # Battle, no window
SABIBOTS_SELFTEST=1 docker/run.sh sabibots release             # Battle, a window (WSLg, lavapipe)
GARDEN_SELFTEST=1   cargo run -p garden   -- --headless 90     # the garden, no window
GARDEN_SELFTEST=1   docker/run.sh garden   release             # the garden, a window
FACTORY_SELFTEST=1  cargo run -p factory  -- --headless 20     # Factory, no window
FACTORY_SELFTEST=1  docker/run.sh factory  release             # Factory, a window
web/build.sh all && web/serve.sh                               # then …/sabibots/?selftest and …/garden/?selftest
web/build.sh factory                                           # then …/factory/?selftest
```

**Factory is not in `web/build.sh all`** until F6 (`web/games.sh`, `docs/factory.md`), so its page
is built by name. Its run is a `docker/run.sh` like the other two, with no edit to that script:
the environment it hands over is found by the game's own name in capitals.

A run of a `docker/run.sh` is on a TTY, so **its lines end in CR**. Until S9 the reader was told
here to put such a log through `tr -d '\r'` before comparing, or every line looked changed —
a note standing in for a mend, in the one comparison this file exists to make. `tools/fixedlines.sh`
drops the CR itself now, and a log that never had one comes out of it exactly as it did before.

A window needs a GPU, which this machine does not have outside the container
(`docs/wsl-gpu.md`). A browser run is driven with playwright-core out of the neighbouring
sabiruby-playground checkout and the Chromium in `~/.cache/ms-playwright`, by a script that
stays in the scratchpad — see "Why the browser script is not in here" at the end.

The three runs of a game are **three different lists**, not one list seen three ways: a headless
run never opens the editor, and a page prints a `done` line a PC does not (there is nothing for
a page to exit to, `games_shell::checks::CHECKS_EXIT_WHEN_DONE`).

## Verdicts

| | |
|---|---|
| `ok  ` | measured, and it is what it should be |
| `FAIL` | measured, and it is not |
| `--  ` | **the run did not put the check in a position to measure anything.** Not a pass and not a failure — the pairing that could not be counted, the probe something walked into. **Both games write `--`** since S5b-5; the garden used to write `n/a` in one of its two places, which was two spellings of one verdict in one game |
| `done` | the checks are finished and the page keeps running |

A line whose verdict may move between runs is marked in the lists below, because the alternative
— leaving the check out of the comparison, which is what was done until 2026-09-20 — means never
comparing it at all. **There are three**, and S5b-5 added the third:

* Battle's `the handler tasks of every robot that went down ended` is `ok` in a run long enough
  for a robot with a handler to be destroyed and to have been down half a second, and `--` in one
  that is not. It is marked again where the list itself is, below.
* the garden's `nothing that was sleeping woke on the resume frame` is `ok` when something was
  asleep with a deadline when the pause began and `--` when nothing was.
* the garden's `and with the panel closed the same wheel in the same place zooms` is `--` when
  the game opened the editor again before the wheel was turned — `choose_watched` does that when
  the creature being watched dies, and a creature starving in that second is the garden's own
  business. The run measured nothing about the wheel, which is neither a pass nor a failure.

---

## SabiRuby Battle, no window — 4 lines

`SABIBOTS_SELFTEST=1 cargo run -p sabibots -- --headless 25`. No editor, so none of the window's
checks; what is left is the end-of-match summary of the handler checks. The per-hit lines the
first three sum up are the ones `fixedlines.sh` leaves out.

```
selftest: ok   N hits on a robot with a handler were checked
selftest: ok   a handler ran within N s of the hit (N/N)
selftest: ok   the handler tasks of every robot that went down ended (N/N)
selftest: ok   the heading changed within N s of the hit (N/N)
```

**`the handler tasks of every robot that went down ended` is the one line whose verdict moves.**
It is `ok` in a run long enough for a robot with a handler to be destroyed and to have been down
half a second, and `--   (none was down long enough to check)` in one that is not — 25 seconds
was enough on 2026-09-20 and 3 seconds was not. Saying `ok` there would be claiming a check
passed that never ran. Until S4 the two cases printed two different sentences and the line was
dropped from the comparison altogether.

## SabiRuby Battle, a window — 32 lines

`SABIBOTS_SELFTEST=1 docker/run.sh sabibots release`. The editor's checks, driven with real keys
and the editor's own actions. The match does not end inside the run, so none of the four lines
above appears.

```
selftest: ok   Apply does not touch the file
selftest: ok   Apply gives robot N the edited behaviour
selftest: ok   Apply leaves robot N (same file) alone
selftest: ok   Apply to all leaves robot N (another file) alone
selftest: ok   Apply to all reaches robot N (same file)
selftest: ok   FN is Apply: the key alone applied the text
selftest: ok   FN opens it
selftest: ok   N s paused: every robot is where it was
selftest: ok   N s paused: the match's clock did not move
selftest: ok   P again gives the budget back
selftest: ok   P pauses: the scripts' budget is N
selftest: ok   Revert is for the shown robot only: robot N keeps its behaviour
selftest: ok   Revert puts robot N back on its file
selftest: ok   Revert shows the file again
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(N))
selftest: ok   a syntax error is refused on the author's own line N: not applied: scout.rb: scout.rb:N:N: syntax error, unexpected '<'; expected an expression after the operator
selftest: ok   after Apply the text is what the robot runs
selftest: ok   after a restart there are four robots again, not eight
selftest: ok   and the match moves again: somebody has driven
selftest: ok   every robot starts with full health
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   robot N comes back on its file
selftest: ok   robot N comes back with its applied behaviour
selftest: ok   the VM panel has the watched robot's frames
selftest: ok   the VM panel starts closed
selftest: ok   the behaviours are running again
selftest: ok   the match's clock runs again
selftest: ok   the panel has the heap counters
selftest: ok   the wall ends exactly on its corners
selftest: ok   typing marks the text edited
```

`FN is Apply` is `F5`, blanked: the checks forge the key rather than writing `Editor::action`
down, so that a wrong `apply_key` cannot pass (S4; before that nothing in these checks went
through the editor's keyboard at all). `FN opens it` is `F2`, the VM panel.

## SabiRuby Battle, a browser — 33 lines

`…/sabibots/?selftest`. The window's list, plus the `done` line, and with one sentence different:
the page's compiler cannot be told a file's name, so a syntax error is reported in
`playground.rb` where a PC says `scout.rb`. The line numbers are the author's either way, which
is what that check is for (`docs/web.md`).

```
selftest: done — the match keeps running (a page has nothing to exit to)
selftest: ok   Apply does not touch the file
selftest: ok   Apply gives robot N the edited behaviour
selftest: ok   Apply leaves robot N (same file) alone
selftest: ok   Apply to all leaves robot N (another file) alone
selftest: ok   Apply to all reaches robot N (same file)
selftest: ok   FN is Apply: the key alone applied the text
selftest: ok   FN opens it
selftest: ok   N s paused: every robot is where it was
selftest: ok   N s paused: the match's clock did not move
selftest: ok   P again gives the budget back
selftest: ok   P pauses: the scripts' budget is N
selftest: ok   Revert is for the shown robot only: robot N keeps its behaviour
selftest: ok   Revert puts robot N back on its file
selftest: ok   Revert shows the file again
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(N))
selftest: ok   a syntax error is refused on the author's own line N: not applied: scout.rb: playground.rb:N:N: syntax error, unexpected '<'; expected an expression after the operator
selftest: ok   after Apply the text is what the robot runs
selftest: ok   after a restart there are four robots again, not eight
selftest: ok   and the match moves again: somebody has driven
selftest: ok   every robot starts with full health
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   robot N comes back on its file
selftest: ok   robot N comes back with its applied behaviour
selftest: ok   the VM panel has the watched robot's frames
selftest: ok   the VM panel starts closed
selftest: ok   the behaviours are running again
selftest: ok   the match's clock runs again
selftest: ok   the panel has the heap counters
selftest: ok   the wall ends exactly on its corners
selftest: ok   typing marks the text edited
```

## Garden, no window — 13 lines

`GARDEN_SELFTEST=1 cargo run -p garden -- --headless 90`. The world's own rules, the creatures,
the save. The `--` lines about pairings that could come to nothing are per-pairing and left out;
the check they belong to is the `a child was born …` line, which carries the tally.

```
selftest: ok   a beetle touched by a rabbit changed heading within N s (N/N)
selftest: ok   a child was born whose genome is its parents' mixed and mutated (at N s: speed N vs N/N, mean N; sight N vs N/N, mean N; appetite N vs N/N, mean N (mutated off both parents)) [N pairings, N children]
selftest: ok   a hungry creature with a plant in sight reached it (from N away, at N s)
selftest: ok   a save with the wrong version is refused (/tmp/garden-from-another-version.json: saved with version N, this garden reads N)
selftest: ok   a spawn Hash with a gene missing names the gene (missing field `sight` (TypeError))
selftest: ok   night arrived within a day of N s (at N s)
selftest: ok   nothing walked through anything over N frames (closest pair N of the radii, N frames under N)
selftest: ok   somebody ate within N s (first at N s)
selftest: ok   the creatures were asleep a second after night fell (N of them, newborns aside, fastest N at N s)
selftest: ok   the rules can be taken away and given back while the world runs (from N s no meter fell for N s; N did, and afterwards hunger came back)
selftest: ok   the rules in world.rb are running the world (the grass grew at N s, over N frames they ran in)
selftest: ok   the starved creature's entity is gone (NvN starved at N s)
selftest: ok   what the world declares reaches a creature's memory (Beetle NvN had "wet" in its @memory at N s)
```

## Garden, a window — 47 lines

`GARDEN_SELFTEST=1 docker/run.sh garden release`. The editor, the VM panel, the wheel, `P`, and
`F3`'s third file — the rules of the world.

```
selftest: ok   Apply does not touch the file
selftest: ok   Apply does not touch world.rb
selftest: ok   Apply restarts every beetle on the edited text
selftest: ok   Ctrl+Enter: the garden is running the edited rules, without stopping
selftest: ok   FN hides the VM panel
selftest: ok   FN opens it
selftest: ok   FN opens the rules of the world
selftest: ok   FN shows it again
selftest: ok   FN writes a garden where `save_file` says (N bytes, version N)
selftest: ok   N s paused: every creature is where it was
selftest: ok   N s paused: nobody got hungrier
selftest: ok   N s paused: the day did not turn
selftest: ok   N s paused: the same creatures are there
selftest: ok   P again gives both budgets back
selftest: ok   P pauses: both VMs' budgets are N
selftest: ok   Revert puts every beetle back on the file
selftest: ok   Revert puts the file's rules back
selftest: ok   Revert shows the file again
selftest: ok   Revert shows world.rb again
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(N))
selftest: ok   a beetle born from now on is born running it
selftest: ok   a beetle born in the very frame of Apply is restarted too
selftest: ok   after Apply the text is what the beetles run
selftest: ok   after Apply the text is what the world runs
selftest: ok   and nothing is running a text of its own
selftest: ok   and the game says so rather than reporting trouble (saved N creatures, N plants (N bytes))
selftest: ok   and the garden moves again: somebody has walked
selftest: ok   and with the panel closed the same wheel in the same place zooms (egui holds the pointer: false; camera N -> N)
selftest: ok   every restarted beetle's new task has run
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   the VM panel has the creature's frames
selftest: ok   the VM panel starts closed
selftest: ok   the creatures are thinking again
selftest: ok   the day is what world.rb says it is (N s)
selftest: ok   the day turns again
selftest: ok   the editor has a third file, and it is not a creature
selftest: ok   the editor shows the file of the creature that was clicked
selftest: ok   the meters move again
selftest: ok   the panel has the heap counters
selftest: ok   the rabbits are left alone
selftest: ok   the rules the editor applied are the ones in memory
selftest: ok   the wheel over the editor scrolls the editor and not the garden (egui holds the pointer: true; camera N -> N)
selftest: ok   the whole VM is still running afterwards
selftest: ok   typing in the rules marks them edited
selftest: ok   typing marks the text edited
```

**Four of these used to flake, and S7 mended what made them.** `Apply restarts every beetle on
the edited text`, `Revert puts every beetle back on the file`, `every restarted beetle's new task
has run` and `the meters move again` each FAILed now and then with nothing changed. S6 counted
them and found two causes, and S7 (`worklog/2026-09-20-window-check-fixes.md`) mended both: a
creature born in the frame Apply was pressed was missed by the hand-over for good, and the checks
waited a number of **seconds** for something that is counted in the VM's **frames**. They wait for
the thing itself now, up to a derived number of frames. **A FAIL on any line in this list is
evidence of something**; there is no line here that is re-run and excused.

`a beetle born in the very frame of Apply is restarted too` is S7's, and it is the only check in
either game that arranges the world rather than watching it: the birth it is about is a race the
garden would otherwise win about once in a hundred runs, so the run forces one into that frame
(`window::birth_in_the_apply_frame`, registered only by the checks).

## Garden, a browser — 48 lines

`…/garden/?selftest`. The window's list plus the `done` line.

```
selftest: done — the garden keeps running (a page has nothing to exit to)
selftest: ok   Apply does not touch the file
selftest: ok   Apply does not touch world.rb
selftest: ok   Apply restarts every beetle on the edited text
selftest: ok   Ctrl+Enter: the garden is running the edited rules, without stopping
selftest: ok   FN hides the VM panel
selftest: ok   FN opens it
selftest: ok   FN opens the rules of the world
selftest: ok   FN shows it again
selftest: ok   FN writes a garden where `save_file` says (N bytes, version N)
selftest: ok   N s paused: every creature is where it was
selftest: ok   N s paused: nobody got hungrier
selftest: ok   N s paused: the day did not turn
selftest: ok   N s paused: the same creatures are there
selftest: ok   P again gives both budgets back
selftest: ok   P pauses: both VMs' budgets are N
selftest: ok   Revert puts every beetle back on the file
selftest: ok   Revert puts the file's rules back
selftest: ok   Revert shows the file again
selftest: ok   Revert shows world.rb again
selftest: ok   `def` in the listing is painted in the keyword colour (kind Some(N))
selftest: ok   a beetle born from now on is born running it
selftest: ok   a beetle born in the very frame of Apply is restarted too
selftest: ok   after Apply the text is what the beetles run
selftest: ok   after Apply the text is what the world runs
selftest: ok   and nothing is running a text of its own
selftest: ok   and the game says so rather than reporting trouble (saved N creatures, N plants (N bytes))
selftest: ok   and the garden moves again: somebody has walked
selftest: ok   and with the panel closed the same wheel in the same place zooms (egui holds the pointer: false; camera N -> N)
selftest: ok   every restarted beetle's new task has run
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   the VM panel has the creature's frames
selftest: ok   the VM panel starts closed
selftest: ok   the creatures are thinking again
selftest: ok   the day is what world.rb says it is (N s)
selftest: ok   the day turns again
selftest: ok   the editor has a third file, and it is not a creature
selftest: ok   the editor shows the file of the creature that was clicked
selftest: ok   the meters move again
selftest: ok   the panel has the heap counters
selftest: ok   the rabbits are left alone
selftest: ok   the rules the editor applied are the ones in memory
selftest: ok   the wheel over the editor scrolls the editor and not the garden (egui holds the pointer: true; camera N -> N)
selftest: ok   the whole VM is still running afterwards
selftest: ok   typing in the rules marks them edited
selftest: ok   typing marks the text edited
```

## Factory, no window — 18 lines

`FACTORY_SELFTEST=1 cargo run -p factory -- --headless 30`. F1's eight, F2's five and F3's five:
the world, the arithmetic a click goes through, the picture (where there is one), **a factory
built with orders and watched until it delivers**, the data stage — what it read, what it
refuses, and what a script can read back out of it — and **the inserters**, which are the machines
with a mind.

```
selftest: --   no floor was drawn (this run has no renderer)
selftest: ok   a click built each of the N things the line needs (miner, belt, belt, belt, chest)
selftest: ok   a click with nothing in hand takes the belt at N, N away, and what was on it goes with it
selftest: ok   a miner cannot be built where there is no ore (N, N)
selftest: ok   a script reads the tables back: recipe_of(:iron_plate)[:made_in] is furnace and item_of(:gear)[:icon] is N
selftest: ok   a world point and the tile it is in agree at the corners and the middle (N/N), and a point off the map is off it
selftest: ok   a wrong data.rb is refused with the line it is wrong on (N/N)
selftest: ok   an inserter taken away leaves no script behind: N scripts where there were N, and none of them is waiting on an arm that is gone (N)
selftest: ok   an inserter whose script raises stops, and the game knows where: inserter.rb:N (after N s)
selftest: ok   every inserter on the map has a script of its own: N arms, N scripts
selftest: ok   every item is either in a chest or on a belt: N dug, N held, N carried
selftest: ok   the assembler turned N iron_plate into N gear after N s (the numbers say N s)
selftest: ok   the data stage was done before Update N: N items, N recipes, N machines, a belt of N a second
selftest: ok   the furnace turned N iron_ore into N iron_plate after N s (the numbers say N s)
selftest: ok   the map is N by N tiles with N of them holding N of ore
selftest: ok   the miner dug, the belts carried and the chest holds N after N s (the numbers say N s)
selftest: ok   the rest of the factory is untouched: N of N inserters stopped, and the assembler line has N in its chest
selftest: ok   with no inserter in the gap nothing reaches the machine: N/N lines are jammed on the belt with the machine empty
```

**`no floor was drawn` is this game's `--` line, and it does not move**: a headless run has no
renderer at all, so it spawns no `TilemapChunk` and there is no image loader to fetch the tileset
with. The verdict says the run did not put the check in a position to measure anything, which is
neither a pass nor a failure.

**The orders are forged** — the checks write a `build::Order`, which is the message a mouse click
turns into and carries the tile, the thing and the way round — so the building, the refusal to put
a miner off the ore, and the taking away are all measured through the same system a mouse goes
through, in a run that has no mouse. **The checks are ordered before the system that carries
orders out**, because a check that gives an order on one frame and looks at what it did on the
next has to be on the same side of it in every frame. F2 needed that line for a stronger reason
and paid for the lack of it with a window run that built a chest where it meant a belt
(`worklog/2026-09-21-factory-F2.md` §8.4a); F3 moved the seam to where what was meant is written
down, and what is left is a property of *checking*.

**The waits are on the game's own numbers**, and there are five of them: F1's line is
`mine_seconds + 4 ÷ belt_tiles_per_second`; the jam is two tiles of belt; each machine line is
`swing (the script's first look) + one swing per thing the recipe eats + time ÷ speed + swing +
1 ÷ belt`, which is 5.5 s for both of them; the broken arm is a swing. All of them print both
numbers, so a run that is slow says so rather than only passing. Change a line of `data.rb` and
the bounds follow; there is no number of seconds in any of them.

**`a wrong data.rb is refused with the line it is wrong on`** puts seven broken data files through
the same door the real one went through, in the game's own VM, and checks that each is refused at
the line it is broken on: an unknown field, a `time` of zero, an item nothing declares, a machine
no tiles wide, a belt that runs backwards, a gap that does not divide a tile (F2a), and Ruby that
will not parse. It runs in a page as well as on a PC, which is the point — a browser's compiler
names every program `playground.rb` and the name has to be put back before a player sees it.

**The five F3 lines are the stage in order.** A machine with no arm beside it takes nothing, so
the line stops on the belt; an arm in the gap joins it up and the chest fills; every arm on the
map has a script; one arm given a script that raises stops **and the game knows which line of
which file**, while the other line goes on filling its chest; and an arm taken away leaves no
script and nothing parked on a `move` that will never be answered.

## Factory, a window — 18 lines

`FACTORY_SELFTEST=1 docker/run.sh factory release`. The same seventeen, and the `--` line replaced
by the check it stood in for.

```
selftest: ok   a click built each of the N things the line needs (miner, belt, belt, belt, chest)
selftest: ok   a click with nothing in hand takes the belt at N, N away, and what was on it goes with it
selftest: ok   a miner cannot be built where there is no ore (N, N)
selftest: ok   a script reads the tables back: recipe_of(:iron_plate)[:made_in] is furnace and item_of(:gear)[:icon] is N
selftest: ok   a world point and the tile it is in agree at the corners and the middle (N/N), and a point off the map is off it
selftest: ok   a wrong data.rb is refused with the line it is wrong on (N/N)
selftest: ok   an inserter taken away leaves no script behind: N scripts where there were N, and none of them is waiting on an arm that is gone (N)
selftest: ok   an inserter whose script raises stops, and the game knows where: inserter.rb:N (after N s)
selftest: ok   every inserter on the map has a script of its own: N arms, N scripts
selftest: ok   every item is either in a chest or on a belt: N dug, N held, N carried
selftest: ok   the assembler turned N iron_plate into N gear after N s (the numbers say N s)
selftest: ok   the data stage was done before Update N: N items, N recipes, N machines, a belt of N a second
selftest: ok   the furnace turned N iron_ore into N iron_plate after N s (the numbers say N s)
selftest: ok   the map is N by N tiles with N of them holding N of ore
selftest: ok   the miner dug, the belts carried and the chest holds N after N s (the numbers say N s)
selftest: ok   the rest of the factory is untouched: N of N inserters stopped, and the assembler line has N in its chest
selftest: ok   the tileset arrived as an array of N layers of N by N px (frame N)
selftest: ok   with no inserter in the gap nothing reaches the machine: N/N lines are jammed on the belt with the machine empty
```

The tileset line checks the **number** of layers as well as their size, because a count that is a
multiple of six is what a browser draws a black page over (`docs/factory.md`). It is not a check
that anything was *drawn* — nothing in a log can be — and the evidence for that is a screenshot
with its pixels counted, in `worklog/2026-09-21-factory-F0.md` for the floor and
`worklog/2026-09-21-factory-F1.md` §5.4, `…-F2.md` and `…-F3.md` for the factory built on it.

## Factory, a browser — 19 lines

`…/factory/?selftest`, after `web/build.sh factory`. The window's list plus the `done` line.

```
selftest: done — the factory keeps running (a page has nothing to exit to)
selftest: ok   a click built each of the N things the line needs (miner, belt, belt, belt, chest)
selftest: ok   a click with nothing in hand takes the belt at N, N away, and what was on it goes with it
selftest: ok   a miner cannot be built where there is no ore (N, N)
selftest: ok   a script reads the tables back: recipe_of(:iron_plate)[:made_in] is furnace and item_of(:gear)[:icon] is N
selftest: ok   a world point and the tile it is in agree at the corners and the middle (N/N), and a point off the map is off it
selftest: ok   a wrong data.rb is refused with the line it is wrong on (N/N)
selftest: ok   an inserter taken away leaves no script behind: N scripts where there were N, and none of them is waiting on an arm that is gone (N)
selftest: ok   an inserter whose script raises stops, and the game knows where: inserter.rb:N (after N s)
selftest: ok   every inserter on the map has a script of its own: N arms, N scripts
selftest: ok   every item is either in a chest or on a belt: N dug, N held, N carried
selftest: ok   the assembler turned N iron_plate into N gear after N s (the numbers say N s)
selftest: ok   the data stage was done before Update N: N items, N recipes, N machines, a belt of N a second
selftest: ok   the furnace turned N iron_ore into N iron_plate after N s (the numbers say N s)
selftest: ok   the map is N by N tiles with N of them holding N of ore
selftest: ok   the miner dug, the belts carried and the chest holds N after N s (the numbers say N s)
selftest: ok   the rest of the factory is untouched: N of N inserters stopped, and the assembler line has N in its chest
selftest: ok   the tileset arrived as an array of N layers of N by N px (frame N)
selftest: ok   with no inserter in the gap nothing reaches the machine: N/N lines are jammed on the belt with the machine empty
```

---

## Where these lists came from

Measured on 2026-09-20 on the branch `shared-crate` after S4, one run each (three for the
garden's window, two for its page), with the two games' own logs and the Playwright console
logs in the stage's scratchpad. Every list but Battle's two is byte-identical to what S4a's runs
produce when they are put through `tools/fixedlines.sh`; Battle's window and page lists have
gained the `FN is Apply` line and Battle's headless list the `handler tasks …` line, which are
the two things S4 changed.

**Factory's three lists are F3's**, measured on 2026-09-21 on the branch `factory` (headless once,
the window twice, the page once). **F3 added five to each** and moved two of the others. The five
are the stage: with no inserter in the gap nothing reaches the machine, every inserter has a
script of its own, one whose script raises stops and the game knows where, the rest of the factory
is untouched, and an inserter taken away leaves no script behind. The two that moved are the
machine lines, which now say what their arms take as well as what their recipe does and are
watched for 5.5 s rather than 3.5 — the same sentence with the numbers the arms put in it.
Thirteen became eighteen, and the page's fourteen became nineteen. **The other two games' six
lists diffed empty over the whole stage**, twice: once for the VM the workspace rides moving to
`8d0fea2` and once at the end.

**Before that they were F0's, then F1's, then F2's**, measured on 2026-09-21 on the same branch,
one run each. **F2a changed none of them** — it changed what a position on a
belt *is* (a whole number of the sixteen steps a tile is long) and moved `ore_per_tile` into
`data.rb`, and the three lists diffed empty over the whole of it, with the garden's and Battle's
headless lists diffing empty as well. The one thing it added is inside a line that was already
there: `a wrong data.rb is refused with the line it is wrong on` now puts **seven** broken files
through the door rather than six, the new one being a gap that does not divide a tile. The count
is a number in the sentence, so the list does not move. **F2 added five lines to each of the three** and moved none of the
others: the data stage was done before the first `Update`, wrong data files are each refused
at the line they are wrong on, a script reads the tables back, and the furnace and the assembler
each turned what a recipe says into what it says in the time it says. Eight became thirteen, and
the page's nine became fourteen. The six lists of the other two games diffed empty over the whole
stage — twice, because F2 also moved the VM the workspace rides
(`worklog/2026-09-21-factory-F2.md` §1 and §6).

**Factory's lists were F0's and then F1's**, measured on 2026-09-21 on the branch
`factory`, one run each, with the logs in that stage's scratchpad. F1 moved every one of them and
the commit says why: F0's four checks were about a floor that F1 replaced, so *the floor is N by N
tiles laid with a tile of the sheet* became *the map is N by N tiles with N of them holding N of
ore*, *a click in the middle of a tile is read as that tile* became the four lines that build a
factory with clicks and watch it deliver, and the two that did not move (the corners, the tileset)
are word for word what they were. Four lines became eight. They are a change to nobody else's: the
six above were re-run at the end of the stage and all six diffed empty.

**S9 moved two**, on 2026-09-21 on the branch `s9`: the garden's window and page lists gained
`FN writes a garden where \`save_file\` says` and `and the game says so rather than reporting
trouble`, which are the garden's Save pressed at a path of the checks' own — the one setting in
`docs/numbers.md` that no run had ever exercised, because a check that saved where the game really
saves would write into this repository (`worklog/2026-09-21-s9.md` §7). The garden's window went
from 45 lines to 47 and its page from 46 to 48; the other four lists diffed empty.

**The garden's window list was an inference when S9 wrote it, and it has been measured since.**
`docker info` segfaulted on this machine during S9 and Docker Desktop is the author's to start, so
the two lines were added to the list from the page's run. Docker came back the same afternoon, and
on the merge of S9 into main the three windows were run: the garden's 47 lines, the Battle's 32 and
Factory's 8, FAIL 0, each `diff` empty against the list here. `GARDEN_SELFTEST=1 docker/run.sh
garden release --shot FILE 30` — the run S9's `checks_end_the_run()` is for — printed the same 47,
kept running after `done`, and wrote its picture at 30 s (`worklog/2026-09-21-s9.md` §10).

**S7 moved one line**, on 2026-09-20 on the same branch: the garden's window and page lists gained
`a beetle born in the very frame of Apply is restarted too`, and nothing else in any of the six
lists moved (`diff` empty on the other four). The garden's window went from 43 lines to 44 and its
page from 44 to 45.

A list is not a thing to be edited by hand when a run disagrees with it. Either the run found
something or the change meant to add or remove a check — and then the list is re-made from a
run and the commit that moves it says which line moved and why.

## Why the browser script is not in here

`docs/web.md` calls the Playwright script "a throwaway" and says it lives in the scratchpad, and
gives no further reason; the reasons are in the script rather than the prose. It reaches into
two things this repository does not own and cannot pin: `playwright-core` out of a checkout of
**sabiruby-playground** next door (this repository has no `package.json`, no `node_modules` and
no reason to grow a JavaScript toolchain), and a Chromium in `~/.cache/ms-playwright` whose
directory carries its build number (`chromium-1243`). Both are absolute paths to one person's
machine. A copy in `tools/` would be a file that cannot be run by anyone who has not already set
up the playground, and that goes stale silently the next time Playwright's Chromium is updated.

What is worth keeping is what the script has to get right, and that is written down rather than
committed: launch with `--use-angle=swiftshader --enable-unsafe-swiftshader`, and
`page.goto(url, { waitUntil: 'commit' })` — the module script ends in `await game.default()`,
which never returns while the game runs, so `'load'` times out on a page that is working
perfectly (`docs/web.md`, "Checked in a browser"). Count `pageerror` and `requestfailed`; both
have to be 0. Then put the console log through `tools/fixedlines.sh`, which strips Chromium's
`%c` style arguments itself.

`tools/fixedlines.sh` **is** in here, and that is the part that was worth keeping: it is the
comparison, it has no dependencies, and the lists above mean nothing without it.
