#!/usr/bin/env python3
"""Draws the assembler: Kenney's green machine, **stretched** to cover two tiles by two.

    tools/factory-machine.py          # writes factory/art/machine.png (32 x 32 px = 2 x 2 tiles)
    tools/factory-machine.py --check  # fails if the file on disk is not what this draws

**Why there is anything to draw.** Kenney's Tiny Factory has machines, and every one of them is
**one tile**: 75, 87, 99 and 111 are the same cabinet in four colours, each complete with its own
left and right edge (read tile by tile on 2026-09-21; `docs/factory.md` had them down as the three
parts of a wide machine, which they are not). A `machine :assembler, size: [2, 2]` in
`ruby/data.rb` wants four pictures, and the pack has one.

**Why stretching rather than drawing.** The plan's rule for what the pack has not got is "自作、
同じパレットで" — draw it, in the same palette. Stretching keeps a stronger promise than the
palette: **every pixel of the result is a pixel of Kenney's**. It is a nine-slice — the four
pixels at each edge kept as they are and the band between them blown up by a whole number
([`repeated`]) — so the frame, the screen and the two lights all survive at the size they were
drawn, and what grows is the panel. No new colour and no new shape, and a 2 by 2 machine that
stands beside the 1 by 1 furnace without looking like it came out of a different pack.

`SOURCE` is the green machine (99). Green because the furnace is Kenney's copper arch (109) and
the miner is 110, and three machines side by side want three colours.

The output is a 2 by 2 picture — a picture of the machine, so that a person can look at it — and
`tools/factory-tileset.py` cuts it **row by row from the top**, which is the order
`machine :assembler, sprite: [...]` lists its tiles in. (The map counts rows upwards; a list of
pictures is read the way a picture is.)

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
ART = ROOT / "factory" / "art"
KENNEY = ART / "kenney_tiny-factory_tilemap_packed.png"
OUT = ART / "machine.png"

TILE = 16
COLUMNS = 12
#: Kenney's green machine. 75 is the red one, 87 the orange, 111 the blue; 109 is the copper arch
#: the furnace uses and 110 the miner.
SOURCE = 99
#: how many tiles the stretched machine covers, across and down
SIZE = (2, 2)


def repeated(length: int, wanted: int) -> list[int]:
    """Which source row (or column) each row of the result comes from.

    A **nine-slice**, which is how a frame is stretched anywhere: the first and the last `keep`
    are the source's own, edge for edge, and the band between them is blown up to fill the rest.
    Repeating one middle row instead — which was the first thing tried — turns the cabinet's
    screen into a flat panel and loses the frame and the lights inside it.

    `keep` is a quarter of the source, which is 4 px of a 16 px tile: the border is two and the
    highlight inside it is one, so four holds the frame with a pixel to spare. It also makes the
    middle band of a 2 by 2 machine **exactly three times** its own size (8 px to 24), and pixel
    art blown up by a whole number is still pixel art.
    """
    keep = length // 4
    band = list(range(keep, length - keep))
    stretch = wanted - 2 * keep
    out = list(range(keep))
    out += [band[i * len(band) // stretch] for i in range(stretch)]
    out += list(range(length - keep, length))
    if len(out) != wanted:
        sys.exit(f"{len(out)} rows for {wanted}")
    return out


def build() -> Image.Image:
    sheet = Image.open(KENNEY).convert("RGBA")
    c, r = SOURCE % COLUMNS, SOURCE // COLUMNS
    source = sheet.crop((c * TILE, r * TILE, c * TILE + TILE, r * TILE + TILE))
    wide, tall = SIZE[0] * TILE, SIZE[1] * TILE
    columns, rows = repeated(TILE, wide), repeated(TILE, tall)
    out = Image.new("RGBA", (wide, tall), (0, 0, 0, 0))
    for y, sy in enumerate(rows):
        for x, sx in enumerate(columns):
            out.putpixel((x, y), source.getpixel((sx, sy)))
    return out


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file on disk differs")
    args = parser.parse_args()

    image = build()
    if args.check:
        if not OUT.exists():
            sys.exit(f"{OUT} is not there")
        if Image.open(OUT).convert("RGBA").tobytes() != image.tobytes():
            sys.exit(f"{OUT} is not what this script draws: run it without --check")
        print(f"{OUT.relative_to(ROOT)} is up to date")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    image.save(OUT, optimize=True)
    colours = set(image.convert('RGBA').getcolors(maxcolors=4096) or [])
    print(f"{OUT.relative_to(ROOT)}: {image.width}x{image.height} px = {SIZE[0]}x{SIZE[1]} tiles, "
          f"{OUT.stat().st_size} bytes — Kenney's tile {SOURCE}, stretched")
    print(f"  {len(colours)} colours, all of them tile {SOURCE}'s")


if __name__ == "__main__":
    main()
