#!/usr/bin/env python3
"""Draws the items that ride on the belts: one 8 px icon each, in one strip.

    tools/factory-items.py          # writes factory/assets/items/items.png
    tools/factory-items.py --check  # fails if the file on disk is not what this draws

**Why they are 8 px and their own file.** Two items have to sit on one 16 px tile of belt without
touching, which leaves 8 px each (`docs/numbers.md` §9.3: it is where the default `items_per_tile`
comes from), and a sheet of 16 px squares has no room for half a tile — so the icons are not in
`factory-tiles.png` at all. The game loads this strip as one image and cuts it with a
`TextureAtlasLayout`, so a belt carrying ore, plates and gears is still one draw call.

**Why one strip and not one file each.** `item :gear, icon: 2` in `ruby/data.rb` is an index into
this strip, and a sprite batch is a run of the *same image* at the same z
(`bevy_sprite_render`): three files would be three batches.

**The colours are Kenney's.** The ore nugget is F1's, drawn in the four oranges
`tools/factory-ore.py` maps Tiny Farm's rocks onto (`#ffd896` `#fdbe53` `#e38628` `#bd6c4a`) with
the packs' own outline `#3f2631`. The plate and the gear are F2's and are drawn in **Tiny
Factory's greys** — `#ebeff8` `#c0cbdc` `#8b9bb4` `#52607c`, which are the stone ramp Tiny Farm's
rocks came in and which Tiny Factory paints its machine frames with. Nothing here invents a
colour; the plan's "足りない部品は同じパレットで自作" is a promise about the palette and this keeps
it. The gear's shape is Kenney's tile 114 read by eye and redrawn at half the size, because 114 is
16 px and a 16 px gear does not survive being halved.

F1 drew the nugget in `tools/factory-ore.py`, which also draws the ground the ore is in. It moved
here when there was more than one item, and **the pixels did not change**: the nugget below is
that one, and the old file is `git show <F1>:factory/assets/items/ore.png`.

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "factory" / "assets" / "items" / "items.png"

#: how big one icon is. It is `factory/src/draw.rs`'s `ITEM_PX` and it is where the sheet is cut.
SIZE = 8

INK = {
    ".": (0, 0, 0, 0),
    "o": (0x3F, 0x26, 0x31, 255),  # the outline, in both Kenney packs
    # Tiny Factory's orange, which `tools/factory-ore.py` maps the rocks onto
    "h": (0xFF, 0xD8, 0x96, 255),
    "l": (0xFD, 0xBE, 0x53, 255),
    "m": (0xE3, 0x86, 0x28, 255),
    "d": (0xBD, 0x6C, 0x4A, 255),
    # Tiny Factory's greys: the stone ramp, and what its machine frames are painted in
    "H": (0xEB, 0xEF, 0xF8, 255),
    "L": (0xC0, 0xCB, 0xDC, 255),
    "M": (0x8B, 0x9B, 0xB4, 255),
    "D": (0x52, 0x60, 0x7C, 255),
}

#: **one entry per icon, and the index of an entry is what `icon:` means in `ruby/data.rb`.**
ICONS: list[tuple[str, list[str]]] = [
    # 0 — the ore out of the ground. F1's nugget, pixel for pixel.
    ("iron_ore", [
        "..oooo..",
        ".ohhllo.",
        "ohllmmdo",
        "ohlmmmdo",
        "ohmmmmdo",
        ".ommmddo",
        "..oddoo.",
        "...oo...",
    ]),
    # 1 — a plate: a flat rectangle seen from above, lit from the top left, which is where every
    # tile in the pack is lit from.
    ("iron_plate", [
        "........",
        ".oooooo.",
        "oHHHLLMo",
        "oHLLLMMo",
        "oLLMMMDo",
        "oLMMMDDo",
        ".oooooo.",
        "........",
    ]),
    # 2 — a gear: Kenney's tile 114 at half the size. Four teeth rather than eight, because eight
    # teeth in eight pixels is a grey square.
    ("gear", [
        ".o.oo.o.",
        "ooLLLLoo",
        ".LLMMLL.",
        "oLM..MLo",
        "oLM..MLo",
        ".LLMMLL.",
        "ooDDDDoo",
        ".o.oo.o.",
    ]),
]


def build() -> Image.Image:
    strip = Image.new("RGBA", (SIZE * len(ICONS), SIZE), (0, 0, 0, 0))
    for i, (name, rows) in enumerate(ICONS):
        if len(rows) != SIZE or any(len(row) != SIZE for row in rows):
            sys.exit(f"{name}: every icon is {SIZE} by {SIZE}")
        for y, row in enumerate(rows):
            for x, ink in enumerate(row):
                strip.putpixel((i * SIZE + x, y), INK[ink])
    return strip


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file on disk differs")
    args = parser.parse_args()

    strip = build()
    if args.check:
        if not OUT.exists():
            sys.exit(f"{OUT} is not there")
        if Image.open(OUT).convert("RGBA").tobytes() != strip.tobytes():
            sys.exit(f"{OUT} is not what this script draws: run it without --check")
        print(f"{OUT.relative_to(ROOT)} is up to date ({len(ICONS)} icons)")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    strip.save(OUT, optimize=True)
    print(f"{OUT.relative_to(ROOT)}: {strip.width}x{strip.height} px, {len(ICONS)} icons of "
          f"{SIZE}px, {OUT.stat().st_size} bytes")
    for i, (name, _) in enumerate(ICONS):
        print(f"  icon: {i}   {name}")


if __name__ == "__main__":
    main()
