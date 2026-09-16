#!/usr/bin/env bash
# Re-cuts the Japanese font the in-game guide is drawn with (G6).
#
#   tools/subset-font.sh                      # downloads Noto Sans JP and re-cuts
#   tools/subset-font.sh /path/to/'NotoSansJP[wght].ttf'
#
# **Run this after editing the Japanese of the guide.** The font that goes in the binary carries
# only the characters the guide's strings use — 63 KB instead of 9.6 MB — so a word with a
# character that was not there before is drawn as a blank box (tofu) until this is run again.
# The three files it reads are the three that hold guide text:
#
#   garden/src/guide_text.rs              the garden's words
#   sabibots/src/guide_text.rs            SabiRuby Battle's words
#   crates/rubevy-arena/src/guide.rs      the shared bits: the HUD's "H: help" hint, the footer
#
# It writes crates/rubevy-arena/assets/fonts/NotoSansJP-Guide.subset.ttf, which is `include_bytes!`d
# by `rubevy_arena::guide` — so `cargo build` after this, and `web/build.sh` for the pages. The
# size is recorded in `docs/web.md` and the licence in `CREDITS.md`.
#
# Needs: python3 with fonttools (`python3 -m venv .venv && .venv/bin/pip install fonttools brotli`,
# then run this with PATH=.venv/bin:$PATH). fonttools 4.65 was used.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
OUT="$ROOT/crates/rubevy-arena/assets/fonts/NotoSansJP-Guide.subset.ttf"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

SRC="${1:-}"
if [ -z "$SRC" ]; then
  SRC="$WORK/NotoSansJP.ttf"
  echo "fetching Noto Sans JP from Google Fonts (SIL OFL 1.1; see CREDITS.md)"
  curl -sfL -o "$SRC" "https://github.com/google/fonts/raw/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf"
  curl -sfL -o "$ROOT/crates/rubevy-arena/assets/fonts/OFL.txt" \
    "https://github.com/google/fonts/raw/main/ofl/notosansjp/OFL.txt"
fi
[ -f "$SRC" ] || { echo "no such font: $SRC" >&2; exit 1; }

# every non-ASCII character the guide text uses, plus the whole of printable ASCII: a Japanese
# sentence with `me[:Hunger]` in the middle of it should not change font half way through a word
python3 - "$ROOT" > "$WORK/chars.txt" <<'PY'
import sys, pathlib
root = pathlib.Path(sys.argv[1])
chars = {chr(c) for c in range(0x20, 0x7f)}
for f in ["garden/src/guide_text.rs", "sabibots/src/guide_text.rs", "crates/rubevy-arena/src/guide.rs"]:
    chars |= {c for c in (root / f).read_text(encoding="utf-8") if ord(c) >= 0xa0}
sys.stdout.write("".join(sorted(chars)))
sys.stderr.write(f"{len(chars)} characters in the guide\n")
PY

# one weight, not the whole axis: egui asks a font for one face, and `wght` is 3.8 MB of the file
python3 -m fontTools.varLib.instancer "$SRC" wght=400 -o "$WORK/instance.ttf" >/dev/null
python3 -m fontTools.subset "$WORK/instance.ttf" --text-file="$WORK/chars.txt" --output-file="$OUT" \
  --layout-features='' --no-hinting --drop-tables+=DSIG --name-IDs='0,1,2,3,4,5,6,13,14' --notdef-outline
# the instancer leaves the variable font's own family name behind; say what this actually is
python3 - "$OUT" <<'PY'
import sys
from fontTools.ttLib import TTFont
f = TTFont(sys.argv[1])
n = f["name"]
n.setName("Noto Sans JP Subset", 1, 3, 1, 0x409)
n.setName("Regular", 2, 3, 1, 0x409)
n.setName("Noto Sans JP Subset Regular", 4, 3, 1, 0x409)
n.setName("NotoSansJPSubset-Regular", 6, 3, 1, 0x409)
f.save(sys.argv[1])
print(f"{sys.argv[1]}: {f['maxp'].numGlyphs} glyphs")
PY
ls -l "$OUT"
echo "now: cargo build --release -p garden -p sabibots   (and web/build.sh all for the pages)"
