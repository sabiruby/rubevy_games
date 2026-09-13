#!/usr/bin/env bash
# Builds the browser version of a game into web/dist/, ready to serve as static files.
#   web/build.sh              # sabibots
#   web/serve.sh              # then open http://localhost:8080/
#
# The PC version is the ordinary `cargo run -p sabibots`; the two are the same code, and what
# differs is chosen by the target (see sabibots/src/platform.rs).
#
# Needs: rustup target wasm32-unknown-unknown; wasm-bindgen-cli of the same version as the
# wasm-bindgen crate in Cargo.lock; wasm-opt (binaryen) on PATH, optional.
# Ruby is compiled in the page by the SabiRuby playground's module: its sabiruby.wasm, sabi.js and
# browser_wasi_shim are taken from a checkout of kishima/sabiruby-playground (SABIRUBY_PLAYGROUND,
# default ../sabiruby-playground) after `tools/build.sh` there.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
GAME="${1:-sabibots}"
PLAYGROUND="${SABIRUBY_PLAYGROUND:-$ROOT/../sabiruby-playground}"
OUT="$HERE/dist"
cd "$ROOT"

[ -f "$PLAYGROUND/web/sabiruby.wasm" ] || { echo "no $PLAYGROUND/web/sabiruby.wasm: run tools/build.sh in sabiruby-playground (or set SABIRUBY_PLAYGROUND)" >&2; exit 1; }
want=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/version = "\(.*\)"/\1/p')
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')
[ "$want" = "$have" ] || { echo "wasm-bindgen-cli $have, but Cargo.lock has $want: cargo install wasm-bindgen-cli --version $want" >&2; exit 1; }

cargo build -p "$GAME" --target wasm32-unknown-unknown --profile web

rm -rf "$OUT"
mkdir -p "$OUT/pkg" "$OUT/compiler"
wasm-bindgen --target web --no-typescript --out-dir "$OUT/pkg" --out-name game \
  "target/wasm32-unknown-unknown/web/$GAME.wasm"
if command -v wasm-opt >/dev/null; then
  # the features Rust enables for wasm32-unknown-unknown; --all-features would also turn on
  # encodings browsers do not read yet (compact imports)
  wasm-opt -Os --strip-debug --strip-producers --enable-bulk-memory --enable-nontrapping-float-to-int \
    --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$OUT/pkg/game_bg.wasm" -o "$OUT/pkg/game_bg.wasm"
else
  echo "wasm-opt not found: the game's module is not shrunk" >&2
fi

cp -r "$GAME/assets" "$OUT/assets"
cp "$HERE/index.html" "$OUT/index.html"
cp "$PLAYGROUND/web/sabiruby.wasm" "$PLAYGROUND/web/sabi.js" "$OUT/compiler/"
mkdir -p "$OUT/compiler/vendor"
cp -r "$PLAYGROUND/web/vendor/browser_wasi_shim" "$OUT/compiler/vendor/"

for f in "$OUT/pkg/game_bg.wasm" "$OUT/compiler/sabiruby.wasm"; do
  echo "$(basename "$f"): $(wc -c < "$f") bytes ($(gzip -9 -c "$f" | wc -c) gzipped)"
done
echo "built $OUT"
