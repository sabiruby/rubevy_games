//! **The grid the factory is built on**: what is on each tile, which way it faces, and where the
//! ore is.
//!
//! None of this is drawn and none of it is Bevy's: a headless run has the whole factory in it and
//! a window is a picture of it (`src/draw.rs`). That split is F0's — the floor was a `Vec<u16>`
//! before it was a `TilemapChunk` — and F1 is the stage it was made for, because the conveyors
//! have to run in a test with no renderer.
//!
//! **One tile holds one building.** The plan allows machines that cover several tiles and F1 has
//! none: a miner and a chest are one tile each, which is what the two of them need to be for a
//! belt to reach them from any side.

use bevy::prelude::*;

use crate::{Map, TILE_PX};

/// **Which way a building faces**, and the only four there are. `East` is the direction tile
/// (x + 1) is in, which is the direction the pack's own conveyor tile runs in before it is turned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    East,
    North,
    West,
    South,
}

impl Dir {
    /// In the order a turn goes through them, which is why [`Dir::left`] is `(i + 1) % 4`.
    pub const ALL: [Dir; 4] = [Dir::East, Dir::North, Dir::West, Dir::South];

    /// One tile that way. `y` counts upwards, as [`Map`] does.
    pub fn step(self) -> IVec2 {
        match self {
            Dir::East => IVec2::new(1, 0),
            Dir::North => IVec2::new(0, 1),
            Dir::West => IVec2::new(-1, 0),
            Dir::South => IVec2::new(0, -1),
        }
    }

    /// The same as a unit vector, for the arithmetic that puts an item on a belt.
    pub fn as_vec(self) -> Vec2 {
        self.step().as_vec2()
    }

    /// A quarter turn anticlockwise — `East` becomes `North`.
    pub fn left(self) -> Dir {
        Dir::ALL[(self.number() + 1) % 4]
    }

    /// A quarter turn clockwise.
    pub fn right(self) -> Dir {
        Dir::ALL[(self.number() + 3) % 4]
    }

    pub fn back(self) -> Dir {
        Dir::ALL[(self.number() + 2) % 4]
    }

    pub fn number(self) -> usize {
        match self {
            Dir::East => 0,
            Dir::North => 1,
            Dir::West => 2,
            Dir::South => 3,
        }
    }

    /// For the log and the checks, which are the only things that name a direction in F1.
    pub fn word(self) -> &'static str {
        match self {
            Dir::East => "east",
            Dir::North => "north",
            Dir::West => "west",
            Dir::South => "south",
        }
    }
}

/// **What kinds of thing can be on a tile.** Three, which is what F1's line needs: something that
/// digs, something that carries, something that holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// Carries items the way it faces. Takes them from any side but its own front.
    Belt,
    /// Digs the ore under it and puts it into whatever it faces.
    Miner,
    /// Holds what a belt or a miner delivers into it, up to [`Rules::chest_capacity`].
    ///
    /// [`Rules::chest_capacity`]: crate::belts::Rules::chest_capacity
    Chest,
}

impl What {
    pub fn word(self) -> &'static str {
        match self {
            What::Belt => "belt",
            What::Miner => "miner",
            What::Chest => "chest",
        }
    }
}

/// One building. The state a machine keeps while it runs is in here too, because it is the grid
/// that a save file will be (F5) and a machine's half-finished work is part of the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Building {
    pub what: What,
    /// Where it sends what it makes or carries. A chest faces nowhere in particular; it keeps the
    /// direction it was built with so that turning it is not a special case.
    pub dir: Dir,
    /// A miner: how far through the current dig it is, in seconds. A chest: unused.
    pub work: f32,
    /// A chest: how many items are in it. A miner: unused.
    pub held: u32,
}

impl Building {
    pub fn new(what: What, dir: Dir) -> Building {
        Building { what, dir, work: 0.0, held: 0 }
    }
}

/// **Everything that has been built**, one slot per tile, `y * tiles + x` — the same order
/// `Floor` and `TilemapChunkTileData` are in, so an index is an index everywhere.
#[derive(Resource, Debug)]
pub struct Grid {
    pub tiles: u32,
    cells: Vec<Option<Building>>,
    /// The tiles that have something on them, so that a frame does not walk the whole map to find
    /// four belts. Kept in index order, which is also the order the conveyors are stepped in.
    built: Vec<u32>,
    /// **How many times anything has been built or taken away.** A counter rather than a flag
    /// because two things follow the grid — the picture and [`Flow`] — and a flag one of them
    /// clears is a flag the other one misses. Each keeps the number it last caught up with.
    pub changes: u64,
}

impl Grid {
    pub fn new(tiles: u32) -> Grid {
        Grid {
            tiles,
            cells: vec![None; (tiles * tiles) as usize],
            built: Vec::new(),
            changes: 1,
        }
    }

    pub fn index(&self, tile: UVec2) -> usize {
        (tile.y * self.tiles + tile.x) as usize
    }

    pub fn tile_of(&self, index: usize) -> UVec2 {
        UVec2::new(index as u32 % self.tiles, index as u32 / self.tiles)
    }

    pub fn at(&self, index: usize) -> Option<&Building> {
        self.cells.get(index).and_then(|c| c.as_ref())
    }

    pub fn at_mut(&mut self, index: usize) -> Option<&mut Building> {
        self.cells.get_mut(index).and_then(|c| c.as_mut())
    }

    /// Every tile with something on it, in index order.
    pub fn built(&self) -> &[u32] {
        &self.built
    }

    /// The tile one step `dir` from `index`, or `None` at the edge of the map.
    pub fn step_from(&self, index: usize, dir: Dir) -> Option<usize> {
        let tile = self.tile_of(index).as_ivec2() + dir.step();
        let last = self.tiles as i32;
        (tile.x >= 0 && tile.y >= 0 && tile.x < last && tile.y < last)
            .then(|| (tile.y as u32 * self.tiles + tile.x as u32) as usize)
    }

    /// **Build.** Replaces whatever was there, which is what a player expects of a belt laid over
    /// a belt; the caller is what decides that a miner may only stand on ore.
    pub fn place(&mut self, index: usize, building: Building) {
        if index >= self.cells.len() {
            return;
        }
        if self.cells[index].is_none() {
            let at = self.built.partition_point(|&i| (i as usize) < index);
            self.built.insert(at, index as u32);
        }
        self.cells[index] = Some(building);
        self.changes += 1;
    }

    /// **Take away.** Returns what was there, so that the caller can say what it removed.
    pub fn remove(&mut self, index: usize) -> Option<Building> {
        let gone = self.cells.get_mut(index).and_then(|c| c.take());
        if gone.is_some() {
            self.built.retain(|&i| i as usize != index);
            self.changes += 1;
        }
        gone
    }
}

/// **The ore in the ground**, one number per tile: how many items are left to dig out of it.
///
/// It is the floor rather than a building — a miner stands *on* it — so it is its own resource,
/// and the picture of the floor is redrawn when a patch runs out.
#[derive(Resource, Debug)]
pub struct Ore {
    pub left: Vec<u32>,
    pub changed: bool,
}

impl Ore {
    /// **Where the patches are**: one in the middle of each quarter of the map, a circle of
    /// `radius` tiles. There is no randomness in it on purpose — two runs of the checks have to
    /// find the ore in the same place, and F1 has no seed to keep in a save file.
    pub fn laid_out(tiles: u32, radius: f32, per_tile: u32) -> Ore {
        let mut left = vec![0u32; (tiles * tiles) as usize];
        let quarter = tiles as f32 / 4.0;
        for cx in [quarter, quarter * 3.0] {
            for cy in [quarter, quarter * 3.0] {
                for y in 0..tiles {
                    for x in 0..tiles {
                        let d = Vec2::new(x as f32 + 0.5 - cx, y as f32 + 0.5 - cy).length();
                        if d <= radius {
                            left[(y * tiles + x) as usize] = per_tile;
                        }
                    }
                }
            }
        }
        Ore { left, changed: true }
    }

    /// **The smallest map the patches fit on without touching the border**, in tiles — the floor
    /// under the `map_tiles` setting, derived from [`Ore::laid_out`] rather than chosen.
    ///
    /// The first patch's middle is at `tiles / 4`, and a tile is in it when **its own middle** is
    /// within `radius` of that. The widest the circle ever reaches sideways is along the row
    /// through its middle, so no tile of it has `x ≤ 0` as soon as
    ///
    /// ```text
    /// tiles / 4 − radius − 0.5 > 0   ⇔   tiles > 4 × radius + 2
    /// ```
    ///
    /// and the far patch, at `3 × tiles / 4`, gives the same condition mirrored. So the answer is
    /// the smallest whole number **strictly above** `4 × radius + 2`. The border ring is tile 0
    /// and tile `tiles − 1` — [`crate::draw::floor_picture`] draws them as plain ground — so a
    /// patch reaching it is ore nobody can see.
    ///
    /// **This is one tile less than the `4 × (radius + 1)` the plan's note guessed** (15 rather
    /// than 16 at the default radius of 3): `radius + 1` rounds the half-tile a tile's middle
    /// sits at up to a whole one.
    ///
    /// **It is a guarantee and not always the very smallest.** The row through the circle's
    /// middle is only a row of real tiles when `tiles / 4 − 0.5` happens to be whole; when it is
    /// not, the circle is a little narrower where the tiles actually are and it can clear the
    /// border a tile sooner. A floor that is sometimes one tile generous is a floor; one that is
    /// sometimes one tile short is ore in the wall. The test below runs both halves of that.
    pub fn smallest_map(radius: f32) -> u32 {
        (4.0 * radius + 2.0).floor() as u32 + 1
    }

    pub fn total(&self) -> u64 {
        self.left.iter().map(|&n| n as u64).sum()
    }

    pub fn tiles_with_ore(&self) -> usize {
        self.left.iter().filter(|&&n| n > 0).count()
    }
}

/// **Which way an item arrives at each tile**, which is not the same as which way the tile faces:
/// a belt is a corner when what feeds it comes in from the side.
///
/// It is worked out from the grid rather than kept in it, because it is a fact *about the
/// neighbours* and a belt laid next door changes it. Both the picture (a corner tile, turned) and
/// the item's path across the tile (two straight halves) read it, so it is worked out once when
/// something is built and not twice a frame.
#[derive(Resource, Debug, Default)]
pub struct Flow {
    pub came_in: Vec<Dir>,
    /// Which [`Grid::changes`] this was worked out from.
    pub seen: u64,
}

impl Flow {
    /// The rule, in the order it is asked:
    ///
    /// 1. something behind it feeding it straight on — a straight;
    /// 2. exactly one thing feeding it from a side — a corner, turning from that side;
    /// 3. anything else (nothing feeding it, or two sides at once) — a straight, because a tile
    ///    that two belts merge into is drawn as the line it is part of and the joining belt's own
    ///    tile is where the turn is seen.
    pub fn refresh(&mut self, grid: &Grid) {
        self.seen = grid.changes;
        self.came_in.clear();
        self.came_in.resize((grid.tiles * grid.tiles) as usize, Dir::East);
        for &t in grid.built() {
            let t = t as usize;
            let Some(building) = grid.at(t) else { continue };
            let out = building.dir;
            if building.what != What::Belt {
                self.came_in[t] = out;
                continue;
            }
            let mut sides = Vec::new();
            let mut straight = false;
            for side in Dir::ALL {
                let Some(n) = grid.step_from(t, side) else { continue };
                let feeding = grid
                    .at(n)
                    .is_some_and(|b| matches!(b.what, What::Belt | What::Miner) && b.dir == side.back());
                if !feeding {
                    continue;
                }
                if side.back() == out {
                    straight = true;
                } else if side.back() != out.back() {
                    sides.push(side.back());
                }
            }
            self.came_in[t] = match (straight, sides.len()) {
                (false, 1) => sides[0],
                _ => out,
            };
        }
    }
}

/// Where an item sitting `along` of the way through a belt tile is, in world units.
///
/// **A corner is two straight halves.** An item on a belt that turns comes in at the middle of one
/// edge and leaves at the middle of another, so the first half of its journey is along the
/// direction it arrived in and the second is along the direction it leaves in. On a straight tile
/// `came_in` is the same as `goes_out` and the two halves are one line.
pub fn item_at(map: &Map, tile: UVec2, came_in: Dir, goes_out: Dir, along: f32) -> Vec2 {
    let centre = map.tile_centre(tile);
    let half = TILE_PX as f32 / 2.0;
    if along < 0.5 {
        centre - came_in.as_vec() * (0.5 - along) * 2.0 * half
    } else {
        centre + goes_out.as_vec() * (along - 0.5) * 2.0 * half
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_goes_round_in_fours() {
        for dir in Dir::ALL {
            assert_eq!(dir.left().left().left().left(), dir);
            assert_eq!(dir.left().right(), dir);
            assert_eq!(dir.back().back(), dir);
            assert_eq!(dir.left().left(), dir.back());
            // and the step matches the turn: east is +x, and a left turn from it is +y
            assert_eq!(dir.left().step(), IVec2::new(-dir.step().y, dir.step().x));
        }
    }

    #[test]
    fn the_list_of_what_is_built_stays_in_index_order() {
        let mut grid = Grid::new(8);
        for tile in [UVec2::new(4, 4), UVec2::new(1, 0), UVec2::new(7, 7), UVec2::new(0, 3)] {
            let at = grid.index(tile);
            grid.place(at, Building::new(What::Belt, Dir::East));
        }
        assert_eq!(grid.built(), &[1, 24, 36, 63]);
        // placing over something already there does not put it in the list twice
        grid.place(24, Building::new(What::Belt, Dir::North));
        assert_eq!(grid.built(), &[1, 24, 36, 63]);
        assert_eq!(grid.at(24).unwrap().dir, Dir::North);
        assert!(grid.remove(24).is_some());
        assert_eq!(grid.built(), &[1, 36, 63]);
        assert!(grid.remove(24).is_none(), "taking away nothing is not an event");
    }

    #[test]
    fn a_step_off_the_edge_is_off_the_map() {
        let grid = Grid::new(4);
        assert_eq!(grid.step_from(0, Dir::East), Some(1));
        assert_eq!(grid.step_from(0, Dir::North), Some(4));
        assert_eq!(grid.step_from(0, Dir::West), None, "west of the bottom left is nowhere");
        assert_eq!(grid.step_from(0, Dir::South), None);
        // and not the other side of the map: tile 3 is the right-hand end of row 0
        assert_eq!(grid.step_from(3, Dir::East), None);
        assert_eq!(grid.step_from(3, Dir::West), Some(2));
    }

    /// The patches are where the drawing and the checks both expect them, and they hold what they
    /// were asked to hold.
    #[test]
    fn four_patches_one_to_a_quarter_of_the_map() {
        let ore = Ore::laid_out(32, 3.0, 100);
        assert_eq!(ore.left[(8 * 32 + 8) as usize], 100, "the middle of the first quarter");
        assert_eq!(ore.left[(24 * 32 + 24) as usize], 100);
        assert_eq!(ore.left[(16 * 32 + 16) as usize], 0, "the middle of the map is bare");
        assert_eq!(ore.total(), ore.tiles_with_ore() as u64 * 100);
        assert!(ore.tiles_with_ore() >= 4 * 25, "a circle of radius 3 is 25 tiles or more");
    }

    /// Whether any of the ore is on the border ring, which is what [`Ore::smallest_map`] is
    /// about. It lives here rather than on [`Ore`] because nothing in the game asks it: the game
    /// asks the floor, and this is what makes the floor true.
    fn touches_the_border(ore: &Ore, tiles: u32) -> bool {
        let last = tiles - 1;
        (0..tiles).any(|i| {
            [(i, 0), (i, last), (0, i), (last, i)]
                .iter()
                .any(|&(x, y)| ore.left[(y * tiles + x) as usize] > 0)
        })
    }

    /// **The floor under `map_tiles` is derived, and this is the derivation run.** At the size
    /// [`Ore::smallest_map`] gives, the four patches clear the border ring; one tile smaller and
    /// they do not. Run over a spread of radii, because a formula that is right at one value is
    /// not a formula.
    #[test]
    fn the_smallest_map_is_the_smallest_map_the_patches_clear_the_border_on() {
        for radius in [0.5f32, 1.0, 1.5, 2.0, 3.0, 3.7, 5.0] {
            let smallest = Ore::smallest_map(radius);
            let fits = Ore::laid_out(smallest, radius, 1);
            assert!(
                !touches_the_border(&fits, smallest),
                "radius {radius}: {smallest} tiles should clear the border"
            );
            assert!(fits.tiles_with_ore() > 0, "radius {radius}: and there is ore on it");
        }
        // **And at the radius the game is played at it is the smallest**: one tile under it, the
        // row through the patch's middle is a row of real tiles and the ore reaches the wall.
        assert_eq!(Ore::smallest_map(3.0), 15, "and not the 16 that 4 × (radius + 1) gives");
        assert!(touches_the_border(&Ore::laid_out(14, 3.0, 1), 14), "14 tiles is too few");
        // the other half of the rustdoc's last paragraph: a radius whose middle row is not a row
        // of tiles clears sooner than the guarantee, which is why this is a floor and not an
        // equality. At radius 0.5 the guarantee is 5 and 4 already clears.
        assert_eq!(Ore::smallest_map(0.5), 5);
        assert!(!touches_the_border(&Ore::laid_out(4, 0.5, 1), 4), "4 clears it too, a tile early");
    }
}
