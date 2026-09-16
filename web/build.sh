#!/usr/bin/env bash
# Builds the browser version of the games into web/dist/, ready to serve as static files.
#   web/build.sh              # both games and the entry page (what CI publishes)
#   web/build.sh garden       # one game, into web/dist/garden/
#   web/serve.sh              # then open http://localhost:8080/
#
# One site, two games (G5). Each game is a directory of its own under web/dist/ and is complete on
# its own — its page, its wasm, its assets and its own copy of the Ruby compiler module — so it can
# be served, opened and debugged without the other one existing. The copy of the compiler is the
# price of that (2.4 MB, and a visitor loads one game); sharing it would make `web/dist/garden/`
# mean nothing without its parent, which is worse for the thing this is mostly used for.
# web/dist/index.html is the entry, written only by a build of `all`.
#
# The PC version is the ordinary `cargo run -p <game>`; the two are the same code, and what
# differs is chosen by the target (see each game's src/platform.rs).
#
# Needs: rustup target wasm32-unknown-unknown; wasm-bindgen-cli of the same version as the
# wasm-bindgen crate in Cargo.lock; wasm-opt (binaryen) on PATH, optional.
# Ruby is compiled in the page by the SabiRuby playground's module: its sabiruby.wasm, sabi.js and
# browser_wasi_shim are taken from a checkout of sabiruby/sabiruby-playground (SABIRUBY_PLAYGROUND,
# default ../sabiruby-playground) after `tools/build.sh` there.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
WHAT="${1:-all}"
PLAYGROUND="${SABIRUBY_PLAYGROUND:-$ROOT/../sabiruby-playground}"
OUT="$HERE/dist"
cd "$ROOT"

case "$WHAT" in
  all) GAMES=(sabibots garden) ;;
  sabibots|garden) GAMES=("$WHAT") ;;
  *) echo "usage: web/build.sh [sabibots|garden|all]" >&2; exit 2 ;;
esac

[ -f "$PLAYGROUND/web/sabiruby.wasm" ] || { echo "no $PLAYGROUND/web/sabiruby.wasm: run tools/build.sh in sabiruby-playground (or set SABIRUBY_PLAYGROUND)" >&2; exit 1; }
want=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/version = "\(.*\)"/\1/p')
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
[ "$want" = "$have" ] || { echo "wasm-bindgen-cli $have, but Cargo.lock has $want: cargo install wasm-bindgen-cli --version $want" >&2; exit 1; }

build_game() {
  local game="$1" dir="$OUT/$1"
  cargo build -p "$game" --target wasm32-unknown-unknown --profile web

  rm -rf "$dir"
  mkdir -p "$dir/pkg" "$dir/compiler"
  wasm-bindgen --target web --no-typescript --out-dir "$dir/pkg" --out-name game \
    "target/wasm32-unknown-unknown/web/$game.wasm"
  if command -v wasm-opt >/dev/null; then
    # the features Rust enables for wasm32-unknown-unknown; --all-features would also turn on
    # encodings browsers do not read yet (compact imports)
    wasm-opt -Os --strip-debug --strip-producers --enable-bulk-memory --enable-nontrapping-float-to-int \
      --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue \
      "$dir/pkg/game_bg.wasm" -o "$dir/pkg/game_bg.wasm"
  else
    echo "wasm-opt not found: $game's module is not shrunk" >&2
  fi

  # the whole assets tree, subdirectories and all: the garden's models are
  # assets/models/*.glb with assets/models/Textures/colormap.png beside them, and a .glb names
  # its texture relative to itself. Bevy's wasm reader fetches "assets/..." as a *relative* URL
  # (bevy_asset io/wasm.rs: `window.fetch_with_str(path)`), so it resolves against the page —
  # …/rubevy_games/garden/assets/… — and a game in a subdirectory needs nothing said about it.
  cp -r "$game/assets" "$dir/assets"
  cp "$HERE/$game.html" "$dir/index.html"
  cp "$PLAYGROUND/web/sabiruby.wasm" "$PLAYGROUND/web/sabi.js" "$dir/compiler/"
  mkdir -p "$dir/compiler/vendor"
  cp -r "$PLAYGROUND/web/vendor/browser_wasi_shim" "$dir/compiler/vendor/"

  local assets
  assets=$(find "$dir/assets" -type f -exec cat {} + | wc -c)
  for f in "$dir/pkg/game_bg.wasm" "$dir/compiler/sabiruby.wasm"; do
    echo "$game/$(basename "$f"): $(wc -c < "$f") bytes ($(gzip -9 -c "$f" | wc -c) gzipped)"
  done
  echo "$game/assets: $assets bytes in $(find "$dir/assets" -type f | wc -l) files"
}

for game in "${GAMES[@]}"; do
  build_game "$game"
done

if [ "$WHAT" = all ]; then
  cp "$HERE/index.html" "$OUT/index.html"
fi
echo "built $OUT"
