#!/usr/bin/env python3
"""Draws the 8 px pictures the game draws as sprites: the items, and F3's two marks.

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

**F3 put two things that are not items on the end of the strip**: the inserter's hand, which is
drawn travelling across an inserter with whatever it is carrying, and the mark that sits over an
inserter whose script has stopped. They are here rather than in a file of their own for the reason
above — one image is one batch — and they are **after** the items so that `icon:` in `ruby/data.rb`
still means "the nth item" and a data file cannot reach them (`ITEMS` below is the bound the game
checks `icon:` against, `MARKS` is the rest).

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

#: **one entry per item, and the index of an entry is what `icon:` means in `ruby/data.rb`.**
#: `factory/src/draw.rs`'s `ITEM_ICONS` is how many there are, and a data file whose `icon:` is
#: past the end of this list is refused by a test.
ITEMS: list[tuple[str, list[str]]] = [
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

#: **The pictures that are not items** (F3), after them in the strip. The game names each of these
#: by its own `const` in `factory/src/draw.rs`; nothing in `ruby/data.rb` can reach them.
MARKS: list[tuple[str, list[str]]] = [
    # the inserter's hand: a two-pronged grab seen from above, in the pack's greys. It is drawn
    # travelling from the tile behind an inserter to the tile in front of it, with whatever it is
    # carrying under it, which is the whole of what makes a swing visible.
    ("hand", [
        "o......o",
        "oM....Mo",
        "oML..LMo",
        ".oMLLMo.",
        "..oMMo..",
        "..oMMo..",
        "...oo...",
        "........",
    ]),
    # the mark over an inserter whose script has stopped — an exception, an overrun, a file that
    # would not compile. Orange, because that is the pack's colour for a thing that wants looking
    # at, and it is the one mark the game draws over a building.
    ("stopped", [
        "..oooo..",
        ".ollllo.",
        "olloollo",
        "olloollo",
        "ollllllo",
        "olloollo",
        ".ollllo.",
        "..oooo..",
    ]),
]

#: everything in the strip, in the order it is drawn in
ICONS = ITEMS + MARKS


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
        print(f"{OUT.relative_to(ROOT)} is up to date ({len(ITEMS)} items, {len(MARKS)} marks)")
        return

    OUT.parent.mkdir(parents=True, exist_ok=True)
    strip.save(OUT, optimize=True)
    print(f"{OUT.relative_to(ROOT)}: {strip.width}x{strip.height} px, {len(ICONS)} pictures of "
          f"{SIZE}px ({len(ITEMS)} items, {len(MARKS)} marks), {OUT.stat().st_size} bytes")
    for i, (name, _) in enumerate(ICONS):
        print(f"  {i}   {name}{'' if i < len(ITEMS) else '   (not an item: `icon:` cannot reach it)'}")


if __name__ == "__main__":
    main()
