# shellcheck shell=bash
# **One target directory per checkout** (S5b-5). Sourced by `build.sh` and `run.sh`, each of which
# has already set `ROOT` to the checkout it was run from.
#
# Both scripts mount that checkout at `/app`, so cargo inside the container sees the same absolute
# path whichever worktree started it. With one shared target volume that had two costs, and the
# second is the worse one:
#
#   * a build begun in a second worktree was read as "nothing has changed" — the tell is a
#     `Finished` that is far too quick — because the sources cargo remembers are at the same path
#     as the ones it is being given (`docs/worklog/2026-09-20-window-check-fixes.md`);
#   * and `/target/release/garden` is one path for every worktree, so the binary `run.sh` opens a
#     window on is whichever checkout built last, with nothing on the screen to say so.
#
# The name of the checkout's directory is what tells them apart, so it is what the volume carries.
# A worktree therefore builds its dependencies once on its own; that is the price of not running
# another branch's game by accident.
#
# **The old shared volume is left where it is** rather than renamed: `docker volume rm
# rubevy-games-target` when nothing wants it any more. The cargo *registry* stays shared — a
# crate's source is the same for every worktree.
TARGET_VOLUME="rubevy-games-target-$(printf %s "$(basename "$ROOT")" | tr -c 'A-Za-z0-9_.-' '-')"
