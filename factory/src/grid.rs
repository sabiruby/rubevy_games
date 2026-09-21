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

use crate::machines::Stock;
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

/// **What kinds of thing can be on a tile.** Three of them are the world's own fittings and the
/// fourth is whatever `data.rb` declares; the fifth is not a thing at all but the rest of a
/// machine that covers more than one tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// Carries items the way it faces. Takes them from any side but its own front.
    Belt,
    /// Digs the ore under it and puts what [`Rules::digs`] names into whatever it faces.
    ///
    /// [`Rules::digs`]: crate::belts::Rules::digs
    Miner,
    /// Holds what is delivered into it, up to [`Rules::chest_capacity`] things altogether.
    ///
    /// [`Rules::chest_capacity`]: crate::belts::Rules::chest_capacity
    Chest,
    /// **Takes one thing from the tile behind it and puts it in the tile in front** — and does it
    /// when a line of Ruby says so, which is what this whole game is for ([`crate::inserters`]).
    ///
    /// It is the only building here with a mind. The arm is Rust: how long a swing takes is
    /// `inserter :arm, seconds_per_item:` in `ruby/data.rb`, what it is carrying is
    /// [`Building::held`] and how far through the swing it is is [`Building::work`], exactly as a
    /// miner's dig is.
    Inserter,
    /// A machine of the kind `data.rb` declared, **on its origin tile** — the bottom-left of its
    /// footprint, which is the tile that was clicked. Everything that happens to a machine
    /// happens here ([`crate::machines`]).
    Machine(crate::data::MachineId),
    /// The rest of a machine that covers more than one tile: not a building of its own, but the
    /// index of the tile that is. A belt handing an item to one of these is handing it to the
    /// machine, which is what lets a big machine be fed from any of its sides.
    Covered { origin: u32 },
}

impl What {
    pub fn word(self) -> &'static str {
        match self {
            What::Belt => "belt",
            What::Miner => "miner",
            What::Chest => "chest",
            What::Inserter => "inserter",
            What::Machine(_) => "machine",
            What::Covered { .. } => "part of a machine",
        }
    }
}

/// One building. The state a machine keeps while it runs is in here too, because it is the grid
/// that a save file will be (F5) and a machine's half-finished work is part of the world.
///
/// **It stopped being `Copy` at F2**, when a chest stopped being a number: what a chest or a
/// machine holds is a few items of a few kinds ([`Stock`]), which is a `Vec`. A belt, which is
/// most of the tiles of a busy map, carries two empty ones and they allocate nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Building {
    pub what: What,
    /// Where it sends what it makes or carries. A chest faces nowhere in particular; it keeps the
    /// direction it was built with so that turning it is not a special case.
    pub dir: Dir,
    /// A miner: how far through the current dig it is, in seconds. A machine: how far through the
    /// craft in [`Building::making`]. An inserter: how far through the swing in
    /// [`Building::swinging`]. A belt and a chest: unused.
    pub work: f32,
    /// A chest: what is in it. A machine: the parts it has taken in and not used yet. An
    /// inserter: **what is in its hand**, which is one thing or nothing.
    pub held: Stock,
    /// A machine: what it has made and not got rid of yet. It is what stops it starting another
    /// craft, which is the same rule a miner keeps with a finished dig.
    pub made: Stock,
    /// A machine: which recipe it is part way through, if any.
    pub making: Option<crate::data::RecipeId>,
    /// **An inserter: whether its arm is on its way across.**
    ///
    /// It is a field of its own and not "is the hand full", because those are different states: a
    /// hand that is full with no swing under way is an arm that carried something over and was
    /// refused, and is holding it until it is told to try again. Nothing else uses it.
    pub swinging: bool,
}

impl Building {
    pub fn new(what: What, dir: Dir) -> Building {
        Building {
            what,
            dir,
            work: 0.0,
            held: Stock::default(),
            made: Stock::default(),
            making: None,
            swinging: false,
        }
    }
}

/// **How many tiles a map of this size has**, as a `usize` — the length of every one of the
/// per-tile `Vec`s in this game, written once so that the multiplication is not spelled out six
/// times. It cannot overflow: the data stage refuses a map with a side longer than
/// [`crate::draw::MOST_TILES_ACROSS`], and the square of that fits in a `u32`, which is the type a
/// tile index has to fit in ([`What::Covered`], [`Grid::built`]).
pub fn how_many(tiles: UVec2) -> usize {
    tiles.x as usize * tiles.y as usize
}

/// **Everything that has been built**, one slot per tile, `y * tiles.x + x` — the same order
/// `Floor` and `TilemapChunkTileData` are in, so an index is an index everywhere.
#[derive(Resource, Debug)]
pub struct Grid {
    /// How many tiles across and up. **Not square since F3a**: `map :world, size: [w, h]` in
    /// `ruby/data.rb` says, and the two numbers are read separately everywhere.
    pub tiles: UVec2,
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
    pub fn new(tiles: UVec2) -> Grid {
        Grid {
            tiles,
            cells: vec![None; how_many(tiles)],
            built: Vec::new(),
            changes: 1,
        }
    }

    pub fn index(&self, tile: UVec2) -> usize {
        (tile.y * self.tiles.x + tile.x) as usize
    }

    pub fn tile_of(&self, index: usize) -> UVec2 {
        UVec2::new(index as u32 % self.tiles.x, index as u32 / self.tiles.x)
    }

    /// Whether a tile is on the map at all — the one place the two numbers are compared, so that
    /// nothing has to remember which of `x` and `y` goes with which.
    pub fn holds(&self, tile: UVec2) -> bool {
        tile.x < self.tiles.x && tile.y < self.tiles.y
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
        let last = self.tiles.as_ivec2();
        (tile.x >= 0 && tile.y >= 0 && tile.x < last.x && tile.y < last.y)
            .then(|| (tile.y as u32 * self.tiles.x + tile.x as u32) as usize)
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
    /// **Where the patches are**: `patches.x` by `patches.y` of them, one in the middle of each
    /// cell of that grid, each a circle of `radius` tiles.
    ///
    /// There is no randomness in it on purpose — two runs of the checks have to find the ore in
    /// the same place, and a scattering would want a seed, which would be a number with nowhere
    /// to have come from. **A grid of cells is the same rule the four corners were**: `patches:
    /// [2, 2]` puts their middles at a quarter and three quarters of each side, which is what
    /// F1 wrote by hand, pixel for pixel ([`Ore::patch_middle`]).
    pub fn laid_out(tiles: UVec2, radius: f32, patches: UVec2, per_tile: u32) -> Ore {
        let mut left = vec![0u32; how_many(tiles)];
        for down in 0..patches.y {
            for across in 0..patches.x {
                let middle = Ore::patch_middle(tiles, patches, UVec2::new(across, down));
                // only the tiles the circle can reach, so that a hundred patches on a big map is
                // a hundred little circles and not a hundred walks of the whole map
                let from = (middle - radius - 0.5).ceil().max(Vec2::ZERO).as_uvec2();
                let upto = (middle + radius + 0.5).floor().max(Vec2::ZERO).as_uvec2().min(tiles);
                for y in from.y..upto.y {
                    for x in from.x..upto.x {
                        let d = (Vec2::new(x as f32, y as f32) + 0.5 - middle).length();
                        if d <= radius {
                            left[(y * tiles.x + x) as usize] = per_tile;
                        }
                    }
                }
            }
        }
        Ore { left, changed: true }
    }

    /// **The middle of one patch**, in tiles, as a point rather than a tile: the middle of its
    /// cell of the `patches` grid, which for `[2, 2]` is a quarter and three quarters of each
    /// side — `(2 × i + 1) / (2 × n)`.
    pub fn patch_middle(tiles: UVec2, patches: UVec2, which: UVec2) -> Vec2 {
        let step = tiles.as_vec2() / patches.max(UVec2::ONE).as_vec2();
        (which.as_vec2() + 0.5) * step
    }

    /// **The smallest map the patches fit on without touching the border**, in tiles **along one
    /// axis** — the floor under `map :world, size:`, derived from [`Ore::laid_out`] rather than
    /// chosen. The two axes are asked separately, because nothing in the layout ties them: a map
    /// 96 by 16 is two rows of patches on a wide floor.
    ///
    /// With `n` patches along an axis of `tiles`, the first one's middle is at `tiles / (2 × n)`,
    /// and a tile is in it when **its own middle** is within `radius` of that. The widest the
    /// circle ever reaches sideways is along the row through its middle, so no tile of it is at
    /// 0 as soon as
    ///
    /// ```text
    /// tiles / (2 × n) − radius − 0.5 > 0   ⇔   tiles > 2 × n × radius + n
    /// ```
    ///
    /// and the last patch, at `(2n − 1) × tiles / (2n)`, gives the same condition mirrored. So the
    /// answer is the smallest whole number **strictly above** `2 × n × (radius + 0.5)`. The border
    /// ring is tile 0 and tile `tiles − 1` — [`crate::draw::floor_picture`] draws them as plain
    /// ground — so a patch reaching it is ore nobody can see.
    ///
    /// **At `n = 2` this is F2's formula unchanged**: `4 × radius + 2`, which is one tile less
    /// than the `4 × (radius + 1)` the plan's note guessed (15 rather than 16 at the default
    /// radius of 3, because `radius + 1` rounds the half-tile a tile's middle sits at up to a
    /// whole one).
    ///
    /// **It is a guarantee and not always the very smallest.** The row through a circle's middle
    /// is only a row of real tiles when that middle minus a half happens to be whole; when it is
    /// not, the circle is a little narrower where the tiles actually are and it can clear the
    /// border a tile sooner. A floor that is sometimes one tile generous is a floor; one that is
    /// sometimes one tile short is ore in the wall. The test below runs both halves of that, at
    /// several counts as well as several radii.
    pub fn smallest_map(radius: f32, patches: u32) -> u32 {
        let n = patches.max(1) as f32;
        (2.0 * n * (radius + 0.5)).floor() as u32 + 1
    }

    pub fn total(&self) -> u64 {
        self.left.iter().map(|&n| n as u64).sum()
    }

    pub fn tiles_with_ore(&self) -> usize {
        self.left.iter().filter(|&&n| n > 0).count()
    }
}

/// **Whether a building pushes what it has into the tile it faces**, which is what makes the tile
/// in front of it a corner rather than a straight.
///
/// Two of the five do: a belt, and a miner, which are the two things in the game that hand
/// something on by themselves. **A machine did until F3** and does not now — nothing comes out of
/// a machine but through an inserter's hand ([`crate::inserters`]) — and an inserter never did:
/// it *drops* something onto the belt in front of it rather than joining it, so a belt with an
/// inserter beside it is the straight it was, and what is put on it appears at its entry edge.
fn feeds(building: &Building) -> bool {
    matches!(building.what, What::Belt | What::Miner)
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
        self.came_in.resize(how_many(grid.tiles), Dir::East);
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
                    .is_some_and(|b| feeds(b) && b.dir == side.back());
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

/// Where an item `along` steps through a belt tile is, in world units.
///
/// **A corner is two straight halves.** An item on a belt that turns comes in at the middle of one
/// edge and leaves at the middle of another, so the first half of its journey is along the
/// direction it arrived in and the second is along the direction it leaves in. On a straight tile
/// `came_in` is the same as `goes_out` and the two halves are one line.
///
/// **Nothing is scaled and nothing is rounded** (F2a). One step is one pixel of the art —
/// [`TILE_PX`] steps to a tile and `TILE_PX` pixels to a tile — so how far from the middle of the
/// tile an item is *is* the number of steps between it and the middle, as it stands. A tile's
/// middle is a whole number of world units ([`Map::tile_centre`]), so an item's place is one too,
/// and with the zoom rounded to whole pixels ([`crate::draw::snapped`]) it lands on a pixel of the
/// screen rather than between two. While it was a fraction of a tile, every item's place was a
/// multiply and a rounding away from the pixel it wanted.
pub fn item_at(map: &Map, tile: UVec2, came_in: Dir, goes_out: Dir, along: crate::belts::Steps) -> Vec2 {
    let centre = map.tile_centre(tile);
    let half = (TILE_PX / 2) as crate::belts::Steps;
    if along < half {
        centre - came_in.as_vec() * (half - along) as f32
    } else {
        centre + goes_out.as_vec() * (along - half) as f32
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
        let mut grid = Grid::new(UVec2::splat(8));
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
        let grid = Grid::new(UVec2::splat(4));
        assert_eq!(grid.step_from(0, Dir::East), Some(1));
        assert_eq!(grid.step_from(0, Dir::North), Some(4));
        assert_eq!(grid.step_from(0, Dir::West), None, "west of the bottom left is nowhere");
        assert_eq!(grid.step_from(0, Dir::South), None);
        // and not the other side of the map: tile 3 is the right-hand end of row 0
        assert_eq!(grid.step_from(3, Dir::East), None);
        assert_eq!(grid.step_from(3, Dir::West), Some(2));
    }

    /// **A map that is not square**, which is F3a's whole point: an index is a row and a column
    /// and the two sides are different numbers, so anything that used one of them for both is
    /// wrong here and right on a square.
    #[test]
    fn a_long_thin_map_counts_rows_and_columns_apart() {
        let grid = Grid::new(UVec2::new(6, 3));
        assert_eq!(grid.index(UVec2::new(5, 2)), 17, "the far corner is the last tile");
        assert_eq!(grid.tile_of(17), UVec2::new(5, 2));
        for t in 0..18 {
            assert_eq!(grid.index(grid.tile_of(t)), t, "tile {t} goes round");
        }
        assert!(grid.holds(UVec2::new(5, 2)));
        assert!(!grid.holds(UVec2::new(6, 2)), "six columns are 0..=5");
        assert!(!grid.holds(UVec2::new(2, 3)), "and three rows are 0..=2");
        // the edge of a row is the edge of the map and not the start of the next one
        assert_eq!(grid.step_from(5, Dir::East), None);
        assert_eq!(grid.step_from(5, Dir::North), Some(11));
        assert_eq!(grid.step_from(12, Dir::North), None, "the top row has nothing above it");
    }

    /// The patches are where the drawing and the checks both expect them, and they hold what they
    /// were asked to hold.
    ///
    /// **This is also the promise F3a made about the default world**: `patches: [2, 2]` is F1's
    /// four corners, tile for tile. The numbers written out here are the ones F1's own version of
    /// this test asserted, before the count was something a data file could say.
    #[test]
    fn four_patches_one_to_a_quarter_of_the_map() {
        let map = UVec2::splat(32);
        let ore = Ore::laid_out(map, 3.0, UVec2::splat(2), 100);
        assert_eq!(ore.left[(8 * 32 + 8) as usize], 100, "the middle of the first quarter");
        assert_eq!(ore.left[(24 * 32 + 24) as usize], 100);
        assert_eq!(ore.left[(16 * 32 + 16) as usize], 0, "the middle of the map is bare");
        assert_eq!(ore.total(), ore.tiles_with_ore() as u64 * 100);
        assert!(ore.tiles_with_ore() >= 4 * 25, "a circle of radius 3 is 25 tiles or more");
        assert_eq!(Ore::patch_middle(map, UVec2::splat(2), UVec2::ZERO), Vec2::splat(8.0));
        assert_eq!(Ore::patch_middle(map, UVec2::splat(2), UVec2::ONE), Vec2::splat(24.0));
    }

    /// **More patches, and a map that is not square** (F3a). Nine patches are nine circles, each
    /// in the middle of its ninth of the map, and a wide short map puts them where the cells are
    /// rather than where a square's quarters would be.
    #[test]
    fn as_many_patches_as_the_data_file_asks_for() {
        let map = UVec2::new(96, 16);
        let patches = UVec2::new(6, 2);
        let ore = Ore::laid_out(map, 3.0, patches, 10);
        // six across: the cells are sixteen wide, so the middles are at 8, 24, 40, …
        assert_eq!(Ore::patch_middle(map, patches, UVec2::new(0, 0)), Vec2::new(8.0, 4.0));
        assert_eq!(Ore::patch_middle(map, patches, UVec2::new(5, 1)), Vec2::new(88.0, 12.0));
        let one = Ore::laid_out(map, 3.0, UVec2::ONE, 10).tiles_with_ore();
        assert_eq!(ore.tiles_with_ore(), one * 12, "twelve patches, none of them overlapping");
        assert!(!touches_the_border(&ore, map), "and none of them on the wall");
    }

    /// Whether any of the ore is on the border ring, which is what [`Ore::smallest_map`] is
    /// about. It lives here rather than on [`Ore`] because nothing in the game asks it: the game
    /// asks the floor, and this is what makes the floor true.
    fn touches_the_border(ore: &Ore, tiles: UVec2) -> bool {
        let last = tiles - UVec2::ONE;
        let at = |x: u32, y: u32| ore.left[(y * tiles.x + x) as usize] > 0;
        (0..tiles.x).any(|x| at(x, 0) || at(x, last.y))
            || (0..tiles.y).any(|y| at(0, y) || at(last.x, y))
    }

    /// **The floor under `map :world, size:` is derived, and this is the derivation run.** At the
    /// size [`Ore::smallest_map`] gives, the patches clear the border ring; one tile smaller and
    /// they do not. Run over a spread of radii **and counts**, because a formula that is right at
    /// one value is not a formula — and F3a's count is what F2's derivation had hidden as a 2.
    #[test]
    fn the_smallest_map_is_the_smallest_map_the_patches_clear_the_border_on() {
        for patches in [1u32, 2, 3, 5, 8] {
            for radius in [0.5f32, 1.0, 1.5, 2.0, 3.0, 3.7, 5.0] {
                let smallest = Ore::smallest_map(radius, patches);
                // square, so that the same number is tried as a width and as a height
                let side = UVec2::splat(smallest);
                let fits = Ore::laid_out(side, radius, UVec2::splat(patches), 1);
                assert!(
                    !touches_the_border(&fits, side),
                    "{patches} patches of radius {radius}: {smallest} tiles should clear the border"
                );
                assert!(fits.tiles_with_ore() > 0, "radius {radius}: and there is ore on it");
            }
        }
        // **And at the radius and count the game is played at it is the smallest**: one tile
        // under it, the row through the patch's middle is a row of real tiles and the ore reaches
        // the wall. This is F2's number, unchanged by the count becoming a field.
        assert_eq!(Ore::smallest_map(3.0, 2), 15, "and not the 16 that 4 × (radius + 1) gives");
        let too_few = UVec2::splat(14);
        assert!(
            touches_the_border(&Ore::laid_out(too_few, 3.0, UVec2::splat(2), 1), too_few),
            "14 tiles is too few"
        );
        // the other half of the rustdoc's last paragraph: a radius whose middle row is not a row
        // of tiles clears sooner than the guarantee, which is why this is a floor and not an
        // equality. At radius 0.5 the guarantee is 5 and 4 already clears.
        assert_eq!(Ore::smallest_map(0.5, 2), 5);
        let four = UVec2::splat(4);
        assert!(
            !touches_the_border(&Ore::laid_out(four, 0.5, UVec2::splat(2), 1), four),
            "4 clears it too, a tile early"
        );
        // **the two axes are asked separately**: a map wide enough for six patches across and
        // tall enough for two up is a map neither number alone would have allowed
        assert_eq!(Ore::smallest_map(3.0, 6), 43);
        assert_eq!(Ore::smallest_map(3.0, 2), 15);
        let thin = UVec2::new(43, 15);
        assert!(!touches_the_border(
            &Ore::laid_out(thin, 3.0, UVec2::new(6, 2), 1),
            thin
        ));
    }
}
