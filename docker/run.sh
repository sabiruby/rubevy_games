#!/usr/bin/env bash
# Runs something built by docker/build.sh, on the Windows desktop through WSLg.
#   docker/run.sh                        # sabibots, release, a window
#   docker/run.sh sabibots release --headless 15
#   docker/run.sh garden release         # the other game
#   docker/run.sh --example camera       # a crate's example, release (docker/build.sh --example camera)
#
# The checks are asked for by an environment variable whose name is the game's
# (`SABIBOTS_SELFTEST`, `GARDEN_SELFTEST`, and a third game's `<GAME>_SELFTEST` with no edit
# here): `SABIBOTS_SELFTEST=1 docker/run.sh sabibots` passes that one through. Until 2026-09-20
# the name was written out here as `SABIBOTS_SELFTEST`, so the garden's window checks could not
# be asked for from this script at all and were run by writing the `docker run` out by hand.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
IMAGE=rubevy-games-build

# debug or release, if that is what comes next; anything starting with `-` is the game's own
# argument and release is meant (`docker/run.sh sabibots --headless 15` used to look for
# `/target/--headless/sabibots`)
take_mode() { if [ $# -gt 0 ] && [ "${1#-}" = "$1" ] && [ -n "$1" ]; then echo "$1"; fi; }

if [ "${1:-}" = --example ]; then
  shift
  NAME="${1:?usage: docker/run.sh --example <name> [debug|release] [args…]}"; shift
  MODE="$(take_mode "$@")"; [ -z "$MODE" ] && MODE=release || shift
  # cargo puts an example's binary under `examples/` beside the games' own
  BIN="/target/$MODE/examples/$NAME"
  PASS=()
else
  GAME="${1:-sabibots}"; shift || true
  MODE="$(take_mode "$@")"; [ -z "$MODE" ] && MODE=release || shift
  BIN="/target/$MODE/$GAME"
  # `-e NAME` with no `=` hands the variable over from this shell, which is what the caller set
  PASS=(-e "$(echo "$GAME" | tr 'a-z-' 'A-Z_')_SELFTEST")
fi

TTY=$([ -t 0 ] && echo -it || echo -t)
exec docker run --rm $TTY \
  -v "$ROOT":/app \
  -v rubevy-games-cargo:/usr/local/cargo/registry \
  -v rubevy-games-target:/target \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -v /mnt/wslg:/mnt/wslg \
  -e DISPLAY="${DISPLAY:-:0}" \
  -e WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}" \
  -e XDG_RUNTIME_DIR=/mnt/wslg/runtime-dir \
  -e PULSE_SERVER=/mnt/wslg/PulseServer \
  "${PASS[@]}" \
  -w /app "$IMAGE" \
  "$BIN" "$@"
