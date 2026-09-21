#!/usr/bin/env python3
"""Builds the one tile sheet Factory draws with.

    tools/factory-tileset.py            # writes the sheet and prints the index table
    tools/factory-tileset.py --check    # builds it in memory and fails if the file differs
    tools/factory-tileset.py --contact-sheet docs/factory-tiles.png   # every tile, blown up,
                                                                      # with its number under it

It reads what is under `factory/art/` and writes `factory/assets/tiles/factory-tiles.png`,
which is the only picture the game loads. What goes in:

  0..131   Kenney's Tiny Factory sheet (CC0), row-major, **in the pack's own numbering**, so a
           number read off the pack or off `docs/factory.md` is the number the game uses.
  132..    ours, in the order `OWN` lists them below. Empty at F0; F0a's conveyors are the first.
  the end  blank tiles, if the count needs pushing off a bad number — see below.

**Why a vertical strip and not the pack's 12 by 11 grid.** Bevy's `TilemapChunk` wants an array
texture: one layer per tile. wgpu's OpenGL backend has to choose a bind target when it creates a
texture, and it guesses from the shape (`wgpu-hal-29.0.4/src/gles/mod.rs:458`,
`TextureDescriptor::is_cube_compatible`): a `D2` texture whose layers are **square** and whose
layer count is **a multiple of 6** is assumed to be a *cube map array*. Kenney's sheet is 12 x 11
= 132 square tiles, 132 = 6 x 22 — so in a browser wgpu bound it as `TEXTURE_CUBE_MAP_ARRAY`,
which WebGL2 does not have, and the page drew a black screen with
`INVALID_ENUM: bindTexture: invalid target` in the console and no page error at all. On a PC
(Vulkan) the same code drew the floor perfectly. Measured 2026-09-21; the run is in
`docs/worklog/2026-09-21-factory-F0.md`.

A strip is what lets this script *choose* the layer count, and [`pad_to_a_safe_count`] pushes it
off 6 and off every multiple of 6. The pack's numbering survives because the strip is built in
the pack's own order.

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
ART = ROOT / "factory" / "art"
OUT = ROOT / "factory" / "assets" / "tiles" / "factory-tiles.png"

# The pack's own tile size, and how its sheet is cut (`factory/art/Tilesheet-tiny-factory.txt`).
TILE = 16
KENNEY_SHEET = ART / "kenney_tiny-factory_tilemap_packed.png"
KENNEY_COLUMNS = 12
KENNEY_ROWS = 11

# Ours, appended after the pack's in this order. Each entry is (file, how many tiles across, how
# many down); the file is read as a grid of `TILE` squares in the same row-major order.
# F0a's conveyor sheet is the first thing in it: the straight and the corner Kenney's pack has
# none of, seen from straight above — the style the author chose on 2026-09-21
# (`tools/factory-belts.py`).
#
# `ore.png` is F1's: the ground with ore in it, Kenney's Tiny Farm rocks in Tiny Factory's orange
# (`tools/factory-ore.py`). The item that comes out of it is **not** here — it is 8 px and a sheet
# of 16 px squares has no room for half a tile; it is its own file, drawn as a sprite.
OWN: list[tuple[str, int, int]] = [("belts.png", 4, 1), ("ore.png", 2, 1)]


def tiles_of(path: pathlib.Path, columns: int, rows: int) -> list[Image.Image]:
    """A sheet cut into `TILE` squares, row-major."""
    sheet = Image.open(path).convert("RGBA")
    want = (columns * TILE, rows * TILE)
    if sheet.size != want:
        sys.exit(f"{path}: expected {want[0]}x{want[1]} px, found {sheet.size[0]}x{sheet.size[1]}")
    return [
        sheet.crop((c * TILE, r * TILE, c * TILE + TILE, r * TILE + TILE))
        for r in range(rows)
        for c in range(columns)
    ]


def pad_to_a_safe_count(tiles: list[Image.Image]) -> list[Image.Image]:
    """Blank tiles on the end until the layer count is one a browser can bind.

    `is_cube_compatible` is "square layers and a multiple of 6 of them", and 6 exactly is read as
    a plain cube map. Anything else is a `TEXTURE_2D_ARRAY`, which is what is wanted. One blank
    tile is always enough to step off a multiple of 6, so this adds at most one.
    """
    blank = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    while len(tiles) % 6 == 0:
        tiles.append(blank.copy())
    return tiles


def build() -> tuple[Image.Image, list[str]]:
    tiles = tiles_of(KENNEY_SHEET, KENNEY_COLUMNS, KENNEY_ROWS)
    table = [f"    0 - {len(tiles) - 1:>3}   Kenney Tiny Factory 1.0 (CC0), the pack's own numbers"]
    for name, columns, rows in OWN:
        first = len(tiles)
        tiles += tiles_of(ART / name, columns, rows)
        table.append(f"  {first:>3} - {len(tiles) - 1:>3}   {name}")
    before = len(tiles)
    tiles = pad_to_a_safe_count(tiles)
    if len(tiles) != before:
        table.append(
            f"  {before:>3} - {len(tiles) - 1:>3}   blank, so that the layer count is not a multiple of 6"
        )

    strip = Image.new("RGBA", (TILE, TILE * len(tiles)), (0, 0, 0, 0))
    for i, tile in enumerate(tiles):
        strip.paste(tile, (0, i * TILE))
    return strip, table


def contact_sheet(strip: Image.Image, path: pathlib.Path) -> None:
    """Every layer of the sheet, blown up, with its number under it.

    This is the picture a person picks tiles out of — `docs/factory.md` is written from it — and
    it is made from the strip rather than from any of the sources, so the numbers on it are
    always the numbers the game uses. Twelve across, because that is how Kenney's own sheet reads
    and the first 132 numbers are the pack's.
    """
    from PIL import ImageDraw

    layers = strip.height // TILE
    scale, gutter, across = 6, 14, KENNEY_COLUMNS
    cell = TILE * scale
    down = (layers + across - 1) // across
    out = Image.new("RGBA", (across * (cell + gutter), down * (cell + gutter)), (24, 26, 32, 255))
    pen = ImageDraw.Draw(out)
    for i in range(layers):
        x, y = (i % across) * (cell + gutter), (i // across) * (cell + gutter)
        tile = strip.crop((0, i * TILE, TILE, i * TILE + TILE)).resize((cell, cell), Image.NEAREST)
        pen.rectangle([x - 1, y - 1, x + cell, y + cell], outline=(70, 76, 90, 255))
        out.alpha_composite(tile, (x, y))
        pen.text((x + 2, y + cell + 1), str(i), fill=(200, 205, 215, 255))
    out.save(path, optimize=True)
    print(f"{path}: {out.width}x{out.height} px, {layers} tiles, {path.stat().st_size} bytes")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file on disk differs")
    parser.add_argument("--contact-sheet", metavar="PATH", help="write the numbered blow-up here")
    args = parser.parse_args()

    strip, table = build()
    layers = strip.height // TILE
    if args.contact_sheet:
        contact_sheet(strip, pathlib.Path(args.contact_sheet))
        if not args.check:
            return
    if args.check:
        if not OUT.exists():
            sys.exit(f"{OUT} is not there")
        if Image.open(OUT).convert("RGBA").tobytes() != strip.tobytes():
            sys.exit(f"{OUT} is not what this script builds: run it without --check")
        print(f"{OUT.relative_to(ROOT)} is up to date ({layers} layers)")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    strip.save(OUT, optimize=True)
    print(f"{OUT.relative_to(ROOT)}: {strip.width}x{strip.height} px, {layers} layers of {TILE}px, "
          f"{OUT.stat().st_size} bytes")
    print(f"  layers % 6 = {layers % 6} (0 would be a cube map array in WebGL2 — see the head of this file)")
    print("\n".join(table))


if __name__ == "__main__":
    main()
