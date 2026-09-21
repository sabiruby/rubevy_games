#!/usr/bin/env bash
# **What a run's checks decided, in a shape two runs can be compared in.**
#
#   tools/fixedlines.sh run.log
#   diff <(tools/fixedlines.sh before.log) <(tools/fixedlines.sh after.log)
#
# Takes a PC run's output or a Playwright console log (either, and several at once) and prints
# one line per check: the verdict and the sentence, every number blanked, sorted. What comes out
# is the run's **list of checks**, which is what `docs/verification/selftest-lines.md` records as
# the standard for each game — "43 lines" and "31" said nothing about *which* lines, so a check
# that quietly stopped running and another that quietly started would have cancelled out.
#
# Three kinds of line are left out, and all three are left out because a run has as many of them
# as it happens to have:
#
#   * the **stage directions** — `selftest: first meal at 0.55 s`, `selftest: the rules were taken
#     away at 20.03 s`. They carry no verdict; they are what the checks saw, and the checks that
#     read them are in the list.
#   * Battle's **one or two lines a hit**: `… within 0.3 s of the hit at 4.16 s`. How many hits a
#     match has swings by a factor of three between runs of the same length (S1 saw 20 and 9).
#     The three summing lines (`a handler ran within 0.3 s of the hit (29/29)`) stay.
#   * the garden's **one line a pairing**: a pair of a species with no `on(:mate)`, and a pairing
#     that measured nothing. The check they belong to prints its own line at the end.
#
# Numbers are blanked because almost every sentence carries one (a time, a count, a distance) and
# none of them is the same twice. A number that *is* the point of a check is checked by the check,
# which is what the verdict in front of the sentence says.
#
# **The carriage return goes first** (S9). `docker/run.sh` runs a window's game on a TTY (`-t`), so
# every line of that run ends in CR, and a CR is part of the line `grep -o` cuts out: the sorted
# list that comes out then differs from a PC run's on every single line, and a `diff` of the two
# says so — thirty-two lines changed, none of them changed. Until S9 the answer was a sentence in
# `docs/verification/selftest-lines.md` telling the reader to pipe the log through `tr -d '\r'`
# first, which is a tool asking to be mended: the comparison this script exists to make is
# precisely the one between a window's run and a PC's. A log that never had a CR in it is not
# touched by this, so what the script prints for every run compared before S9 is unchanged.
set -euo pipefail
cat "$@" \
  | tr -d '\r' \
  | grep -o 'selftest: .*' \
  | sed 's/ color: [a-z#].*$//' \
  | grep -E 'selftest: (ok  |FAIL|--  |done)' \
  | grep -v 'of the hit at' \
  | grep -v 'the rules paired two of a species' \
  | grep -v 'measured nothing' \
  | sed 's/[0-9][0-9.]*/N/g' \
  | sort
