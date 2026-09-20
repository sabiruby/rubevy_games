#!/usr/bin/env bash
# Builds the image and then a game inside it. The cargo registry and the target directory live in
# named volumes, so a second build is fast and the repository stays clean.
#   docker/build.sh              # sabibots, release
#   docker/build.sh sabibots debug
#   docker/build.sh --example camera    # a crate's example, so docker/run.sh can open its window
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
IMAGE=rubevy-games-build

if [ "${1:-}" = --example ]; then
  NAME="${2:?usage: docker/build.sh --example <name> [debug|release]}"
  MODE="${3:-release}"
  WHAT=(--example "$NAME")
else
  WHAT=(-p "${1:-sabibots}")
  MODE="${2:-release}"
fi

docker build -t "$IMAGE" "$HERE"
docker run --rm -t \
  -v "$ROOT":/app \
  -v rubevy-games-cargo:/usr/local/cargo/registry \
  -v rubevy-games-target:/target \
  -w /app "$IMAGE" \
  cargo build $([ "$MODE" = release ] && echo --release) "${WHAT[@]}"
