//! **F0a's two belt mock-ups, and nothing else. F1 deletes this file.**
//!
//! Kenney's Tiny Factory draws a conveyor running right and one running up, and no corner at all.
//! Which way to make up the difference is the author's to decide, and the two candidates are:
//!
//! * **(i) the pack's three-quarter look kept.** A belt running right is eight pixels of surface
//!   with six pixels of *front face* under it — the side of the machine — and a belt running up
//!   is twelve pixels wide with no face. Straights are the pack's own (left is the right one
//!   mirrored, down is the up one mirrored), and the corners are drawn: **four pictures**,
//!   because a front face belongs at the bottom of the tile whichever way the belt goes, so
//!   mirroring left to right keeps it there and rotating does not.
//! * **(ii) everything seen from straight above.** One straight and one corner are drawn, twelve
//!   pixels wide with the same rails whichever way they point, and every direction and every
//!   corner comes out of them through [`TileOrientation`] — **two pictures**.
//!
//! Both are laid out **the same way**: one loop of belt that uses all four directions and all
//! four corners, with four of the pack's machines below it, so that the two screenshots differ in
//! the art and in nothing else.
//!
//! ```text
//! cargo run -p factory -- --belts i --shot i.png 3
//! cargo run -p factory -- --belts ii
//! ```

use bevy::prelude::*;
use bevy::sprite_render::{TileData, TileOrientation};

use crate::{Map, TILE_PX};

/// Which mock-up. The word after `--belts`.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub enum Belts {
    /// (i) the pack's three-quarter look, corners drawn four times
    ThreeQuarter,
    /// (ii) straight from above, one corner turned eight ways
    FromAbove,
}

impl Belts {
    pub fn from_word(word: &str) -> Option<Belts> {
        match word {
            "i" | "1" | "quarter" => Some(Belts::ThreeQuarter),
            "ii" | "2" | "above" => Some(Belts::FromAbove),
            _ => None,
        }
    }
}

/// How wide the mock-up's map is, in tiles. The picture below is sixteen characters across, and
/// the map is square because [`Map`] is.
pub const TILES: u32 = 16;

/// Half of how much world the mock-up's window holds. Twelve tiles from top to bottom, which is
/// the eleven rows of the picture and a tile of air: at the default 900 px window that is about
/// 4.7 screen pixels per pixel of the art, which is large enough to see a chevron.
pub fn half_height() -> f32 {
    6.0 * TILE_PX as f32
}

/// **The arrangement**, first line at the top. One loop of belt running anticlockwise — left
/// along the top, down the left side, right along the bottom, up the right side — so that all
/// four directions and all four corners are on the screen at once, and four of the pack's
/// machines underneath it for the art to be judged against.
///
/// ```text
///   1 2 3 4   the four corners, in the order the belt meets them
///   < > ^ v   a straight running that way
///   R O G B   a machine (three tiles wide), red / orange / green / blue
///   .         the floor
/// ```
const PICTURE: [&str; 11] = [
    "................",
    "..2<<<<<<<<<<1..",
    "..v..........^..",
    "..v..........^..",
    "..v..........^..",
    "..3>>>>>>>>>>4..",
    "................",
    "..RRR....GGG....",
    "................",
    "..OOO....BBB....",
    "................",
];

/// Where the pack's own belt tiles are (`docs/factory.md`): the right-running middle and the
/// up-running middle, first frame.
const PACK_RIGHT: u16 = 26;
const PACK_UP: u16 = 16;
/// The pack's four machines, left, middle and right of each.
const MACHINES: [[u16; 3]; 4] = [[75, 76, 77], [87, 88, 89], [99, 100, 101], [111, 112, 113]];

/// Where `tools/factory-belts.py`'s tiles land in the sheet — the script prints these numbers,
/// and `docs/factory.md` lists them.
const OWN_FIRST: u16 = 132;
const I_LEFT_TOP: u16 = OWN_FIRST;
const I_TOP_LEFT: u16 = OWN_FIRST + 2;
const I_LEFT_BOTTOM: u16 = OWN_FIRST + 4;
const I_BOTTOM_LEFT: u16 = OWN_FIRST + 6;
const II_STRAIGHT: u16 = OWN_FIRST + 8;
const II_CORNER: u16 = OWN_FIRST + 10;

fn tile(index: u16, orientation: TileOrientation) -> TileData {
    TileData { tileset_index: index, orientation, ..TileData::from_tileset_index(index) }
}

/// Mirrored left to right: the one turn that keeps a front face at the bottom of the tile.
const MIRROR_H: TileOrientation = TileOrientation::MirrorH;
/// Mirrored top to bottom.
const MIRROR_V: TileOrientation = TileOrientation::MirrorHRotate180;

fn piece(what: char, belts: Belts) -> Option<TileData> {
    let plain = TileOrientation::Default;
    Some(match (belts, what) {
        // ---- (i): the pack's straights, and four drawn corners -------------------------------
        (Belts::ThreeQuarter, '>') => tile(PACK_RIGHT, plain),
        (Belts::ThreeQuarter, '<') => tile(PACK_RIGHT, MIRROR_H),
        (Belts::ThreeQuarter, '^') => tile(PACK_UP, plain),
        (Belts::ThreeQuarter, 'v') => tile(PACK_UP, MIRROR_V),
        (Belts::ThreeQuarter, '1') => tile(I_BOTTOM_LEFT, plain),
        (Belts::ThreeQuarter, '2') => tile(I_LEFT_BOTTOM, MIRROR_H),
        (Belts::ThreeQuarter, '3') => tile(I_TOP_LEFT, MIRROR_H),
        (Belts::ThreeQuarter, '4') => tile(I_LEFT_TOP, plain),
        // ---- (ii): one straight and one corner, turned ---------------------------------------
        (Belts::FromAbove, '>') => tile(II_STRAIGHT, plain),
        (Belts::FromAbove, '<') => tile(II_STRAIGHT, TileOrientation::Rotate180),
        (Belts::FromAbove, '^') => tile(II_STRAIGHT, TileOrientation::Rotate90),
        (Belts::FromAbove, 'v') => tile(II_STRAIGHT, TileOrientation::Rotate270),
        (Belts::FromAbove, '1') => tile(II_CORNER, TileOrientation::Rotate90),
        (Belts::FromAbove, '2') => tile(II_CORNER, TileOrientation::Rotate180),
        (Belts::FromAbove, '3') => tile(II_CORNER, TileOrientation::Rotate270),
        (Belts::FromAbove, '4') => tile(II_CORNER, plain),
        // ---- the machines, which are the pack's in both -------------------------------------
        (_, 'R' | 'O' | 'G' | 'B') => return None, // laid by the loop below, which knows the run
        _ => return None,
    })
}

/// The mock-up, as the tiles to put over the floor.
pub fn over_the_floor(map: &Map, belts: Belts) -> Vec<(UVec2, TileData)> {
    let mut out = Vec::new();
    let rows = PICTURE.len() as u32;
    // the picture is written top line first, and tile (0, 0) is the bottom left
    let bottom = (map.tiles.saturating_sub(rows)) / 2;
    for (line_number, line) in PICTURE.iter().enumerate() {
        let y = bottom + (rows - 1 - line_number as u32);
        let characters: Vec<char> = line.chars().collect();
        let mut x = 0u32;
        while (x as usize) < characters.len() {
            let what = characters[x as usize];
            if let Some(machine) = "ROGB".find(what) {
                // three tiles wide: left, middle, right of the same machine
                for (offset, index) in MACHINES[machine].iter().enumerate() {
                    out.push((UVec2::new(x + offset as u32, y), tile(*index, TileOrientation::Default)));
                }
                x += 3;
                continue;
            }
            if let Some(data) = piece(what, belts) {
                out.push((UVec2::new(x, y), data));
            }
            x += 1;
        }
    }
    out
}
