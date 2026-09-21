#!/usr/bin/env python3
"""Draws the inserter's base — the one 16 px tile F3 adds to the sheet.

    tools/factory-inserter.py          # writes factory/art/inserter.png
    tools/factory-inserter.py --check  # fails if the file on disk is not what this draws

**Why one tile and not five.** The inserter is the machine the player writes Ruby for, so what
has to be visible is the *arm*: an item is picked up behind it, carried across it and put down in
front. An arm that moves cannot be a tile — a `TilemapChunk` tile is one of eight orientations and
nothing in between — so the arm and what it is holding are **sprites** drawn over the tilemap
(`factory/src/draw.rs`), and what is left for the sheet is the thing the arm turns on. One tile.

**It faces east, and the other three directions are that tile turned** (`TileOrientation`), which
is the same saving the belts take: the base is drawn with its output — the orange mouth — on the
right, so a quarter turn anticlockwise per direction is the whole of it. That is why the base is
*not* symmetric left to right, and it is the only thing about the drawing that is load-bearing.

**The colours are Kenney's.** The frame is Tiny Factory's greys (`#3e4e6e` `#5a6988` `#8b9bb4`
`#c0cbdc` `#ebeff8`) and the mouth is the pack's own orange rail (`#fdbe53` `#e38628`), which is
what the pack paints the things that move with. The outline is `#3f2631`, both packs'. Nothing
here invents a colour.

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "factory" / "art" / "inserter.png"

#: the pack's tile size (`factory/art/Tilesheet-tiny-factory.txt`)
TILE = 16

INK = {
    ".": (0, 0, 0, 0),
    "o": (0x3F, 0x26, 0x31, 255),  # the outline, in both Kenney packs
    "D": (0x3E, 0x4E, 0x6E, 255),
    "S": (0x5A, 0x69, 0x88, 255),
    "M": (0x8B, 0x9B, 0xB4, 255),
    "L": (0xC0, 0xCB, 0xDC, 255),
    "H": (0xEB, 0xEF, 0xF8, 255),
    "m": (0xE3, 0x86, 0x28, 255),  # the pack's orange rail, shaded
    "l": (0xFD, 0xBE, 0x53, 255),
}

#: **The base, drawn with its output to the right.** The light square in the middle is the pivot
#: the arm stands on; the orange wedge is the side things are put down on.
BASE = [
    "................",
    "..oooooooooooo..",
    ".oDDDDDDDDDDDDo.",
    ".oDSSSSSSSSSSDo.",
    ".oDSMMMMMmmMSDo.",
    ".oDSMLLLLmllmDo.",
    ".oDSMLHHLmlllmo.",
    ".oDSMLHoHLmlllo.",
    ".oDSMLHoHLmlllo.",
    ".oDSMLHHLmlllmo.",
    ".oDSMLLLLmllmDo.",
    ".oDSMMMMMmmMSDo.",
    ".oDSSSSSSSSSSDo.",
    ".oDDDDDDDDDDDDo.",
    "..oooooooooooo..",
    "................",
]


def build() -> Image.Image:
    if len(BASE) != TILE or any(len(row) != TILE for row in BASE):
        sys.exit(f"the base is {TILE} by {TILE} pixels")
    tile = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    for y, row in enumerate(BASE):
        for x, ink in enumerate(row):
            tile.putpixel((x, y), INK[ink])
    # **it has to be asymmetric left to right**, or turning it would draw the same picture four
    # times and an inserter would never say which way it faces
    if tile.tobytes() != tile.transpose(Image.FLIP_LEFT_RIGHT).tobytes():
        return tile
    sys.exit("the base is the same mirrored: nothing in it says which way it faces")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file on disk differs")
    args = parser.parse_args()

    tile = build()
    if args.check:
        if not OUT.exists():
            sys.exit(f"{OUT} is not there")
        if Image.open(OUT).convert("RGBA").tobytes() != tile.tobytes():
            sys.exit(f"{OUT} is not what this script draws: run it without --check")
        print(f"{OUT.relative_to(ROOT)} is up to date")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    tile.save(OUT, optimize=True)
    print(f"{OUT.relative_to(ROOT)}: {tile.width}x{tile.height} px, {OUT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
