#!/usr/bin/env python3
"""Draws the conveyor tiles Kenney's Tiny Factory does not have.

    tools/factory-belts.py           # writes factory/art/belts.png and prints the table
    tools/factory-belts.py --check   # fails if the file on disk is not what this draws

**What the pack has, and what it is actually missing.** Tiny Factory draws a conveyor running
*right* (24–27 / 36–39, two frames) and one running *up* (4 16 28 / 5 17 29). It has **no corner
at all**, and the survey said it had no left and no down either. Reading the tiles pixel by pixel
on 2026-09-21 says otherwise, and it changes the size of this job:

  * the right-running tile is symmetric top to bottom apart from its chevrons, so **left is the
    right one mirrored horizontally**;
  * the up-running tile is symmetric left to right and **has no front face at all** — it is
    already drawn from straight above — so **down is the up one mirrored vertically**, and a
    rotation of it is a perfectly good belt running sideways seen from above.

`TileData::orientation` does all of that without a new picture. So the only thing genuinely
missing is **the corner**, in whichever of the two styles the author picks:

  (i)  the pack's three-quarter look kept. A belt running right is eight pixels of surface with
       **six pixels of front face under it**; a belt running up is twelve pixels wide with a rail
       down each side and no face. A corner between them has to get from one to the other, so it
       **changes width as it turns** and has to be drawn once for turning up and again for
       turning down — a face that is always at the bottom is not something a rotation makes.
  (ii) everything seen from straight above. One straight and one corner, twelve pixels wide with
       the same rails whichever way they point, and every direction and every corner comes out of
       them by rotating and mirroring.

Both are drawn with **the pack's own colours**, counted out of its belt tiles rather than typed
in, and both use **the pack's own cross-section**, measured across tile 16.

The corners are a first draft: what the author is being asked is which of the two to go on with,
not whether these particular pixels are right. F1 keeps one of them.

Needs: python3 with Pillow (12.3 was used).
"""

import argparse
import math
import pathlib
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
ART = ROOT / "factory" / "art"
PACK = ART / "kenney_tiny-factory_tilemap_packed.png"
OUT = ART / "belts.png"

TILE = 16
PACK_COLUMNS = 12

# The pack's belt palette, counted out of tiles 25–27, 4/16/28 and 49–51 on 2026-09-21, with the
# name each colour earns in the pack's own tiles.
GAP = (0x3F, 0x26, 0x31, 255)         # the dark ground a belt sits in
RAIL_LIGHT = (0xC0, 0xCB, 0xDC, 255)  # the lit edge of the frame, and the chevrons
SURFACE = (0x8B, 0x9B, 0xB4, 255)     # the belt's running surface, and the shaded edge
FACE = (0x5A, 0x69, 0x88, 255)        # the front of a belt seen in three-quarter view
FACE_DARK = (0x3E, 0x4E, 0x6E, 255)   # the rollers' shadow across that front

# **Measured across the pack's tile 16**, which is one belt's width in 16 px: 0–1 gap, 2 light
# rail, 3 rail, 4–11 surface, 12 rail, 13 light rail, 14–15 gap. In half-widths from the middle
# of the band (which is at 8.0): the band ends at 6, the light rail is the outermost pixel of it
# and the shaded rail the one inside that.
HALF_WIDTH_FROM_ABOVE = 6.0
# **And across the pack's tile 26**: rows 0–1 gap, 2–9 surface, 10–15 front face. Half of eight
# is four, and the face is six deep.
HALF_WIDTH_THREE_QUARTER = 4.0
FACE_DEPTH = 6

#: how often a chevron repeats along the belt, in pixels — two to a tile, as the pack draws them
CHEVRON_EVERY = 8.0
#: how thick a chevron's stroke is
CHEVRON_THICK = 2.0
#: how many points the centre line is sampled at when a pixel looks for the nearest one. 200 over
#: a quarter turn of at most 16 px is a step of well under a tenth of a pixel.
SAMPLES = 200


def pack_tile(index: int) -> Image.Image:
    sheet = Image.open(PACK).convert("RGBA")
    c, r = index % PACK_COLUMNS, index // PACK_COLUMNS
    return sheet.crop((c * TILE, r * TILE, c * TILE + TILE, r * TILE + TILE))


def colour_at(along: float, across: float, half: float, phase: float, backwards: bool = False):
    """The belt's own colour at a point, given where it is along the belt and across it.

    `across` is signed, and the sign is which side of the middle the point is on. Outside the
    band this returns `None` — what is there is the caller's business, because in three-quarter
    view one side of the band has a front face on it and the other has the ground.
    """
    d = abs(across)
    if d > half:
        return None
    if d > half - 1.0:
        return RAIL_LIGHT
    if d > half - 2.0:
        return SURFACE
    # A chevron points the way the belt runs, so its apex — the middle of the band — is the part
    # furthest along: the stroke is where `along + |across|` is constant, a `>` on its side.
    front = (-along if backwards else along) + d
    return RAIL_LIGHT if (front - phase) % CHEVRON_EVERY < CHEVRON_THICK else SURFACE


def centre_line(kind: str, y_in: float, half_in: float, half_out: float):
    """The middle of the belt through the tile, sampled: point, distance travelled, half-width,
    and how deep the front face is there.

    `up` turns a belt arriving at the left edge into one leaving at the top, and `down` into one
    leaving at the bottom. Both are quarter ellipses rather than circles, because a belt arriving
    in three-quarter view is centred six pixels down and one leaving upwards is centred eight
    pixels across, and those are not the same distance from the corner they turn about.

    **The face runs out as the belt turns.** A front face belongs to a belt seen from the side,
    and by the end of the turn the belt is pointing away from the viewer and has none — which is
    the whole of why (i) needs two corner pictures and (ii) needs one.
    """
    points = []
    travelled = 0.0
    previous = None
    for i in range(SAMPLES + 1):
        t = (math.pi / 2) * i / SAMPLES
        half = half_in + (half_out - half_in) * (t / (math.pi / 2))
        # in at the left edge, centred `y_in` down, and moving right before it turns
        if kind == "up":
            point = (8.0 * math.sin(t), y_in * math.cos(t))
        else:  # "down": out at the bottom edge instead
            point = (8.0 * math.sin(t), y_in + (TILE - y_in) * (1.0 - math.cos(t)))
        if previous is not None:
            travelled += math.dist(previous, point)
        previous = point
        points.append((point, travelled, half, FACE_DEPTH * math.cos(t)))
    return points


def draw_corner(kind: str, phase: float, three_quarter: bool, backwards: bool = False) -> Image.Image:
    """A corner tile: the band swept along the centre line, and in (i) a front face under it.

    **The two styles differ in the line's two ends**, which is the whole of what the author is
    being shown. In (i) a belt arriving from the left is eight pixels of surface centred six
    pixels down, with its face below that, and one leaving upwards is twelve pixels wide — so the
    band has to widen as it turns. In (ii) both ends are the same twelve pixels and it does not.
    """
    if three_quarter:
        line = centre_line(kind, 6.0, HALF_WIDTH_THREE_QUARTER, HALF_WIDTH_FROM_ABOVE)
    else:
        line = centre_line(kind, TILE / 2, HALF_WIDTH_FROM_ABOVE, HALF_WIDTH_FROM_ABOVE)
    out = Image.new("RGBA", (TILE, TILE), GAP)
    face = pack_tile(26)
    for y in range(TILE):
        for x in range(TILE):
            px, py = x + 0.5, y + 0.5
            (cx, cy), along, half, face_deep = min(line, key=lambda s: math.dist(s[0], (px, py)))
            d = math.dist((cx, cy), (px, py))
            # which side of the middle: below the centre line is the side the face is on
            side = 1.0 if py > cy else -1.0
            colour = colour_at(along, side * d, half, phase, backwards)
            if colour is not None:
                out.putpixel((x, y), colour)
            elif three_quarter and side > 0 and d <= half + face_deep:
                # the front face, taken row for row from the pack's own right-running tile so
                # that a corner set beside a straight one is the same picture
                out.putpixel((x, y), face.getpixel((x, 10 + min(int(d - half), FACE_DEPTH - 1))))
    return out


def draw_straight_from_above(phase: float) -> Image.Image:
    """(ii): a belt running right, seen from straight above. Symmetric top to bottom, so one
    picture turns into all four directions."""
    out = Image.new("RGBA", (TILE, TILE), GAP)
    for y in range(TILE):
        for x in range(TILE):
            colour = colour_at(x + 0.5, (y + 0.5) - TILE / 2, HALF_WIDTH_FROM_ABOVE, phase)
            if colour is not None:
                out.putpixel((x, y), colour)
    return out


#: what the sheet holds, in order. The index here is the tile's number within the sheet, and
#: `tools/factory-tileset.py` puts the sheet after the pack's 132.
# **(i) needs four corner pictures and (ii) needs one**, and that is the number the author is
# really choosing between. A corner joins two of the tile's edges and is travelled one way or the
# other, which is eight cases. Mirroring left to right keeps a front face at the bottom of the
# tile where it belongs, so it halves (i)'s eight to four; rotating does not, because a rotated
# face ends up on the side or the top. (ii) has no face, so all eight of its cases are one
# picture under `TileData::orientation`.
TILES = [
    ("(i)  corner: left in, top out, frame A", lambda: draw_corner("up", 0.0, True)),
    ("(i)  corner: left in, top out, frame B", lambda: draw_corner("up", 4.0, True)),
    ("(i)  corner: top in, left out, frame A", lambda: draw_corner("up", 0.0, True, True)),
    ("(i)  corner: top in, left out, frame B", lambda: draw_corner("up", 4.0, True, True)),
    ("(i)  corner: left in, bottom out, frame A", lambda: draw_corner("down", 0.0, True)),
    ("(i)  corner: left in, bottom out, frame B", lambda: draw_corner("down", 4.0, True)),
    ("(i)  corner: bottom in, left out, frame A", lambda: draw_corner("down", 0.0, True, True)),
    ("(i)  corner: bottom in, left out, frame B", lambda: draw_corner("down", 4.0, True, True)),
    ("(ii) straight: running right, frame A", lambda: draw_straight_from_above(0.0)),
    ("(ii) straight: running right, frame B", lambda: draw_straight_from_above(4.0)),
    ("(ii) corner: left in, top out, frame A", lambda: draw_corner("up", 0.0, False)),
    ("(ii) corner: left in, top out, frame B", lambda: draw_corner("up", 4.0, False)),
]


def build() -> Image.Image:
    sheet = Image.new("RGBA", (TILE * len(TILES), TILE), (0, 0, 0, 0))
    for i, (_, draw) in enumerate(TILES):
        sheet.paste(draw(), (i * TILE, 0))
    return sheet


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the file on disk differs")
    args = parser.parse_args()
    sheet = build()
    if args.check:
        if not OUT.exists():
            sys.exit(f"{OUT} is not there")
        if Image.open(OUT).convert("RGBA").tobytes() != sheet.tobytes():
            sys.exit(f"{OUT} is not what this script draws: run it without --check")
        print(f"{OUT.relative_to(ROOT)} is up to date")
        return
    OUT.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(OUT, optimize=True)
    print(f"{OUT.relative_to(ROOT)}: {sheet.width}x{sheet.height} px, {len(TILES)} tiles, "
          f"{OUT.stat().st_size} bytes")
    for i, (what, _) in enumerate(TILES):
        print(f"  {132 + i:>3}  (sheet {i})  {what}")


if __name__ == "__main__":
    main()
