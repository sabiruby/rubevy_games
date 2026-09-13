#!/usr/bin/env bash
# Builds the image and then a game inside it. The cargo registry and the target directory live in
# named volumes, so a second build is fast and the repository stays clean.
#   docker/build.sh              # sabibots, release
#   docker/build.sh sabibots debug
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
GAME="${1:-sabibots}"
MODE="${2:-release}"
IMAGE=rubevy-games-build

docker build -t "$IMAGE" "$HERE"
docker run --rm -t \
  -v "$ROOT":/app \
  -v rubevy-games-cargo:/usr/local/cargo/registry \
  -v rubevy-games-target:/target \
  -w /app "$IMAGE" \
  cargo build $([ "$MODE" = release ] && echo --release) -p "$GAME"
