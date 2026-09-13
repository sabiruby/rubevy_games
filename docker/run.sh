#!/usr/bin/env bash
# Runs a game built by docker/build.sh, on the Windows desktop through WSLg.
#   docker/run.sh                        # sabibots, release, a window
#   docker/run.sh sabibots release --headless 15
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
GAME="${1:-sabibots}"; shift || true
MODE="${1:-release}"; shift || true
IMAGE=rubevy-games-build

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
  -w /app "$IMAGE" \
  "/target/$MODE/$GAME" "$@"
