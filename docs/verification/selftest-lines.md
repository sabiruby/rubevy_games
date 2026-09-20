# What the two games' checks have to say

Both games run a `selftest` that prints one line per thing it has proved. Until 2026-09-20 the
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
web/build.sh all && web/serve.sh                               # then …/sabibots/?selftest and …/garden/?selftest
```

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
| `--  ` / `n/a ` | **the run did not put the check in a position to measure anything.** Not a pass and not a failure. Battle writes `--`, the garden writes both (`--` for the pairings it cannot count, `n/a` for a probe something walked into) |
| `done` | the checks are finished and the page keeps running |

A line whose verdict may move between runs is marked in the lists below. There is exactly one,
and it is marked because the alternative — leaving the check out of the comparison, which is
what was done until 2026-09-20 — means never comparing it at all.

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
selftest: ok   night arrived by N s (at N s)
selftest: ok   nothing walked through anything over N frames (closest pair N of the radii, N frames under N)
selftest: ok   somebody ate within N s (first at N s)
selftest: ok   the creatures were asleep a second after night fell (N of them, newborns aside, fastest N at N s)
selftest: ok   the rules can be taken away and given back while the world runs (from N s no meter fell for N s; N did, and afterwards hunger came back)
selftest: ok   the rules in world.rb are running the world (the grass grew at N s, over N passes of `each_frame`)
selftest: ok   the starved creature's entity is gone (NvN starved at N s)
selftest: ok   what the world declares reaches a creature's memory (Beetle NvN had "wet" in its @memory at N s)
```

## Garden, a window — 44 lines

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
selftest: ok   N s paused: every creature is where it was
selftest: ok   N s paused: nobody got hungrier
selftest: ok   N s paused: the day did not turn
selftest: ok   P again gives the budget back
selftest: ok   P pauses: the scripts' budget is N
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
selftest: ok   and the garden moves again: somebody has walked
selftest: ok   and with the panel closed the same wheel in the same place zooms (egui holds the pointer: false; camera N -> N)
selftest: ok   every restarted beetle's new task has run
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   the VM panel has the creature's frames
selftest: ok   the VM panel starts closed
selftest: ok   the creatures are thinking again
selftest: ok   the day is what world.rb says it is
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

## Garden, a browser — 45 lines

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
selftest: ok   N s paused: every creature is where it was
selftest: ok   N s paused: nobody got hungrier
selftest: ok   N s paused: the day did not turn
selftest: ok   P again gives the budget back
selftest: ok   P pauses: the scripts' budget is N
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
selftest: ok   and the garden moves again: somebody has walked
selftest: ok   and with the panel closed the same wheel in the same place zooms (egui holds the pointer: false; camera N -> N)
selftest: ok   every restarted beetle's new task has run
selftest: ok   nothing ran while it was paused
selftest: ok   nothing that was sleeping woke on the resume frame: the next one is due in N ticks, as it was two seconds ago
selftest: ok   nothing was written
selftest: ok   the VM panel has the creature's frames
selftest: ok   the VM panel starts closed
selftest: ok   the creatures are thinking again
selftest: ok   the day is what world.rb says it is
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

---

## Where these lists came from

Measured on 2026-09-20 on the branch `shared-crate` after S4, one run each (three for the
garden's window, two for its page), with the two games' own logs and the Playwright console
logs in the stage's scratchpad. Every list but Battle's two is byte-identical to what S4a's runs
produce when they are put through `tools/fixedlines.sh`; Battle's window and page lists have
gained the `FN is Apply` line and Battle's headless list the `handler tasks …` line, which are
the two things S4 changed.

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
