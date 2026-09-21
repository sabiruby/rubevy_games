#!/usr/bin/env python3
"""Draws the ore in the ground.

    tools/factory-ore.py           # writes factory/art/ore.png
    tools/factory-ore.py --check   # fails if the file on disk is not what this draws

**The ground.** Kenney's Tiny Factory has no ore in it, and the plan (§3.6) names the stand-in:
the rocks of **Kenney's Tiny Farm 1.0** (CC0, <https://kenney.nl/assets/tiny-farm>), tiles 77 and
89 of its packed sheet, *with colour*. Reading them on 2026-09-21 says why that works so cheaply:
the two packs are drawn in **the same palette**. The rocks use `#3f2631` for the outline,
`#8b9bb4`, `#c0cbdc`, `#ebeff8` for the stone and `#52607c` for its shadow, and every one of those
is in Tiny Factory's own sheet as well. So making them ore is a **map from one ramp to another**,
and the ramp they are mapped onto is Tiny Factory's orange — `#ffd896` `#fdbe53` `#e38628`
`#bd6c4a` — which is the colour its rails and its warning stripes are painted in. Nothing here
invents a colour.

Two tiles come out, because **a patch that is being dug should look like it**: the three-rock tile
89 while there is plenty left, the two-pebble tile 77 when it is nearly out.

**The item that comes out of it is not here.** F1 drew the 8 px nugget in this file, because
there was one item in the world; F2 declares as many as `ruby/data.rb` likes, so all of them are
drawn by `tools/factory-items.py` into one strip. The nugget moved there unchanged, in the four
colours the ramp below maps onto.

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
ART = ROOT / "factory" / "art"
FARM = ART / "kenney_tiny-farm_tilemap_packed.png"
FACTORY = ART / "kenney_tiny-factory_tilemap_packed.png"
ORE_OUT = ART / "ore.png"

TILE = 16
COLUMNS = 12

#: the rocks, in the order they are used as a patch runs down: plenty, then nearly out
ROCKS = [89, 77]
#: Tiny Factory's plain dark ground, which the rocks are set on so that a patch is one tile and
#: not a rock floating over whatever the floor happens to be
GROUND_TILE = 3

#: **the map from the rock's greys to Tiny Factory's orange**, both read off the packs themselves
ORE_RAMP = {
    (0xEB, 0xEF, 0xF8): (0xFF, 0xD8, 0x96),  # the stone's highlight
    (0xC0, 0xCB, 0xDC): (0xFD, 0xBE, 0x53),  # its lit face
    (0x8B, 0x9B, 0xB4): (0xE3, 0x86, 0x28),  # its body
    (0x52, 0x60, 0x7C): (0xBD, 0x6C, 0x4A),  # its shadow
    # `#3f2631` is the outline in both packs and stays as it is
}


def tile_of(sheet: pathlib.Path, index: int) -> Image.Image:
    image = Image.open(sheet).convert("RGBA")
    c, r = index % COLUMNS, index // COLUMNS
    return image.crop((c * TILE, r * TILE, c * TILE + TILE, r * TILE + TILE))


def as_ore(rock: Image.Image) -> Image.Image:
    """The rock, painted in the ore ramp. A colour the ramp does not name is left alone."""
    out = rock.copy()
    for y in range(out.height):
        for x in range(out.width):
            r, g, b, a = out.getpixel((x, y))
            if a == 0:
                continue
            if (r, g, b) in ORE_RAMP:
                out.putpixel((x, y), (*ORE_RAMP[(r, g, b)], a))
    return out


def ground_with(rock: Image.Image) -> Image.Image:
    out = tile_of(FACTORY, GROUND_TILE)
    out.alpha_composite(as_ore(rock))
    return out


def build_ore() -> Image.Image:
    sheet = Image.new("RGBA", (TILE * len(ROCKS), TILE), (0, 0, 0, 0))
    for i, rock in enumerate(ROCKS):
        sheet.paste(ground_with(tile_of(FARM, rock)), (i * TILE, 0))
    return sheet


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if either file on disk differs")
    args = parser.parse_args()

    for path, image, what in [
        (ORE_OUT, build_ore(), f"{len(ROCKS)} ground tiles, Tiny Farm's rocks {ROCKS} in ore colours"),
    ]:
        if args.check:
            if not path.exists():
                sys.exit(f"{path} is not there")
            if Image.open(path).convert("RGBA").tobytes() != image.tobytes():
                sys.exit(f"{path} is not what this script draws: run it without --check")
            print(f"{path.relative_to(ROOT)} is up to date")
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        image.save(path, optimize=True)
        print(f"{path.relative_to(ROOT)}: {image.width}x{image.height} px, "
              f"{path.stat().st_size} bytes — {what}")


if __name__ == "__main__":
    main()
