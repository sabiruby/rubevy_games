//! **The picture of the factory**, and nothing that decides anything.
//!
//! Everything in here reads the grid, the ore and the lanes and writes tiles and sprites; a run
//! with no window adds none of it and the factory runs just the same (`src/belts.rs`). Two
//! chunks and a sprite each:
//!
//! * **the floor** — one `TilemapChunk`, the plates and the ore, redrawn when a patch runs down;
//! * **the buildings** — a second chunk over it, `AlphaMode2d::Blend` so that the floor shows
//!   through the corners of a belt, redrawn when something is built and when the belts' two-frame
//!   animation turns over;
//! * **the items** — 8 px sprites, one per item on a belt.
//!
//! **Which picture a belt gets is worked out from its neighbours** ([`Flow`]): a belt fed from the
//! side is a corner, and a corner is the one drawn tile turned. That is what the author's choice
//! of belts-seen-from-above bought, and `belt_picture` below is the whole of the saving — eight
//! turns and four directions out of two pictures.

use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::prelude::*;
use bevy::sprite_render::{AlphaMode2d, TileData, TileOrientation, TilemapChunk, TilemapChunkTileData};
use games_shell::camera::CameraView;

use crate::belts::{Lanes, Rules};
use crate::grid::{Dir, Flow, Grid, Ore, What};
use crate::items::{OnBelt, Pool};
use crate::{Map, TILE_PX};

// ---------------------------------------------------------------------------------------------
// Which tile of the sheet is what
// ---------------------------------------------------------------------------------------------
//
// Read off `docs/factory-tiles.png`, which `tools/factory-tileset.py --contact-sheet` draws from
// the sheet itself. These are not settings for the same reason `TILE_PX` is not: another number
// here does not draw the factory differently, it draws the wrong picture.

/// The tan floor plates, laid in diagonal stripes so that a screenshot shows tiles.
const FLOOR_PLATES: [u16; 3] = [0, 1, 2];
/// The plain dark ground: the border of the map, and what is under a patch that has run out.
const GROUND: u16 = 3;
/// Ore in the ground: plenty left, and nearly gone (`tools/factory-ore.py`).
const ORE_RICH: u16 = 136;
const ORE_POOR: u16 = 137;
/// A conveyor running east, its two animation frames.
const BELT_STRAIGHT: [u16; 2] = [132, 133];
/// A conveyor turning: in at the left, out at the top, its two frames.
const BELT_CORNER: [u16; 2] = [134, 135];
/// The drilling rig Kenney's pack has, used as the miner.
const MINER: u16 = 110;
/// A wooden crate, used as the chest.
const CHEST: u16 = 85;

/// **A quarter turn anticlockwise per direction**, for a picture drawn running east.
/// `Rotate90` turns the pack's east-running belt into a north-running one, which is what F0a's
/// mock-up showed the author.
const FACING: [TileOrientation; 4] = [
    TileOrientation::Default,
    TileOrientation::Rotate90,
    TileOrientation::Rotate180,
    TileOrientation::Rotate270,
];

/// **The four left turns**, indexed by the direction the item arrives in. The drawn corner is
/// "in from the left, out at the top", which is an item arriving heading east and leaving heading
/// north — a left turn — so the turn for an item arriving heading east is the picture as drawn.
const TURN_LEFT: [TileOrientation; 4] = FACING;

/// **The four right turns.** Mirroring the drawn corner left to right turns "east in, north out"
/// into "west in, north out", which is a right turn; the other three are that mirror turned.
const TURN_RIGHT: [TileOrientation; 4] = [
    TileOrientation::MirrorHRotate180, // east in, south out
    TileOrientation::MirrorHRotate270, // north in, east out
    TileOrientation::MirrorH,          // west in, north out
    TileOrientation::MirrorHRotate90,  // south in, west out
];

/// Which picture a belt gets, given how an item arrives and which way the belt faces.
pub fn belt_picture(came_in: Dir, goes_out: Dir, frame: usize) -> (u16, TileOrientation) {
    if came_in == goes_out {
        (BELT_STRAIGHT[frame], FACING[goes_out.number()])
    } else if goes_out == came_in.left() {
        (BELT_CORNER[frame], TURN_LEFT[came_in.number()])
    } else if goes_out == came_in.right() {
        (BELT_CORNER[frame], TURN_RIGHT[came_in.number()])
    } else {
        // arriving from straight ahead is not a thing a belt does (`Flow::refresh`); if it ever
        // is, a straight is the picture that cannot be wrong in an interesting way
        (BELT_STRAIGHT[frame], FACING[goes_out.number()])
    }
}

/// Which tile of the sheet the floor has at a place: the border, a patch of ore, or a plate.
pub fn floor_picture(map: &Map, ore: &Ore, rules: &Rules, tile: UVec2) -> u16 {
    let last = map.tiles - 1;
    let at = (tile.y * map.tiles + tile.x) as usize;
    if tile.x == 0 || tile.y == 0 || tile.x == last || tile.y == last {
        GROUND
    } else if ore.left[at] > 0 {
        // half a patch left is where it starts looking dug out
        if ore.left[at] * 2 > rules.ore_per_tile { ORE_RICH } else { ORE_POOR }
    } else {
        FLOOR_PLATES[((tile.x + tile.y) % FLOOR_PLATES.len() as u32) as usize]
    }
}

// ---------------------------------------------------------------------------------------------
// The two chunks
// ---------------------------------------------------------------------------------------------

#[derive(Resource, Debug)]
pub struct Chunks {
    pub floor: Entity,
    pub buildings: Entity,
}

/// The item's picture, loaded once. A [`Sprite`] per item is what both ways of holding an item
/// end up drawing, so the handle is shared rather than loaded per item.
#[derive(Resource, Debug)]
pub struct Icons {
    pub ore: Handle<Image>,
}

/// Which frame of the belts' two-frame animation is showing, and when it last turned over.
#[derive(Resource, Debug, Default)]
pub struct Animation {
    pub frame: usize,
    pub since: f32,
}

pub fn start_drawing(mut commands: Commands, assets: Res<AssetServer>, map: Res<Map>) {
    // The tileset is one image and the chunk wants an array texture with one layer per tile, so
    // the cut is asked for through the loader's settings — it cannot be asked for in a `.meta`
    // file beside the image, because `AssetMetaCheck::Never` is what keeps a page from being
    // handed a 404 for it (plan §5, and F0's whole first day).
    let tileset = assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.array_layout = Some(ImageArrayLayout::RowHeight { pixels: TILE_PX });
        })
        .load("tiles/factory-tiles.png");
    let empty = vec![None; (map.tiles * map.tiles) as usize];
    let floor = commands
        .spawn((
            TilemapChunk {
                chunk_size: UVec2::splat(map.tiles),
                tile_display_size: UVec2::splat(TILE_PX),
                tileset: tileset.clone(),
                ..default()
            },
            TilemapChunkTileData(empty.clone()),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    let buildings = commands
        .spawn((
            TilemapChunk {
                chunk_size: UVec2::splat(map.tiles),
                tile_display_size: UVec2::splat(TILE_PX),
                tileset,
                // **over the floor**: a belt's corners and a crate's edges are transparent, and
                // the default is opaque, which would draw the floor's colour through them
                alpha_mode: AlphaMode2d::Blend,
            },
            TilemapChunkTileData(empty),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ))
        .id();
    commands.insert_resource(Chunks { floor, buildings });
    commands.insert_resource(Icons { ore: assets.load("items/ore.png") });
}

/// The floor, when a patch of ore has changed — which is rare, and is why this is not every frame.
pub fn draw_floor(
    map: Res<Map>,
    rules: Res<Rules>,
    mut ore: ResMut<Ore>,
    chunks: Res<Chunks>,
    mut tiles: Query<&mut TilemapChunkTileData>,
) {
    if !ore.changed {
        return;
    }
    let Ok(mut data) = tiles.get_mut(chunks.floor) else { return };
    for y in 0..map.tiles {
        for x in 0..map.tiles {
            let at = (y * map.tiles + x) as usize;
            data.0[at] = Some(TileData::from_tileset_index(floor_picture(
                &map,
                &ore,
                &rules,
                UVec2::new(x, y),
            )));
        }
    }
    ore.changed = false;
}

/// The buildings, when something is built or taken away, and when the belts' animation turns over.
pub fn draw_buildings(
    time: Res<Time>,
    rules: Res<Rules>,
    grid: Res<Grid>,
    flow: Res<Flow>,
    mut animation: ResMut<Animation>,
    chunks: Res<Chunks>,
    mut tiles: Query<&mut TilemapChunkTileData>,
    mut drawn: Local<Vec<u32>>,
    mut seen: Local<u64>,
) {
    let turned = time.elapsed_secs() - animation.since >= rules.belt_frame_seconds();
    if turned {
        animation.frame = (animation.frame + 1) % BELT_STRAIGHT.len();
        animation.since = time.elapsed_secs();
    }
    if !turned && *seen == grid.changes {
        return;
    }
    let Ok(mut data) = tiles.get_mut(chunks.buildings) else { return };
    if *seen != grid.changes {
        // everything that had a picture and may not have one now
        for &at in drawn.iter() {
            data.0[at as usize] = None;
        }
        drawn.clear();
        drawn.extend_from_slice(grid.built());
        *seen = grid.changes;
    }
    for &at in drawn.iter() {
        let at = at as usize;
        let Some(building) = grid.at(at) else { continue };
        data.0[at] = Some(match building.what {
            What::Belt => {
                let (index, orientation) =
                    belt_picture(flow.came_in[at], building.dir, animation.frame);
                TileData { tileset_index: index, orientation, ..TileData::from_tileset_index(index) }
            }
            What::Miner => TileData::from_tileset_index(MINER),
            What::Chest => TileData::from_tileset_index(CHEST),
        });
    }
}

// ---------------------------------------------------------------------------------------------
// The items
// ---------------------------------------------------------------------------------------------

/// **The lanes' items, drawn out of a pool of sprites.** There is no entity per item here: the
/// pool is as long as the most items that have ever been on screen at once, and the rest are
/// hidden rather than despawned, so a busy factory does not spawn and despawn thousands of
/// entities a second.
pub fn draw_items_from_lanes(
    map: Res<Map>,
    grid: Res<Grid>,
    flow: Res<Flow>,
    lanes: Res<Lanes>,
    icons: Res<Icons>,
    mut pool: ResMut<Pool>,
    mut commands: Commands,
    mut sprites: Query<(&mut Transform, &mut Visibility)>,
    mut places: Local<Vec<Vec2>>,
) {
    crate::items::places(&map, &grid, &flow, &lanes, &mut places);
    for (i, at) in places.iter().enumerate() {
        match pool.sprites.get(i) {
            Some(&entity) => {
                if let Ok((mut transform, mut visible)) = sprites.get_mut(entity) {
                    transform.translation = at.extend(2.0);
                    *visible = Visibility::Inherited;
                }
            }
            // a sprite spawned now cannot be written until the next frame, which is when this
            // item will be drawn; one frame late for one item is not a thing anybody sees
            None => pool.sprites.push(commands.spawn(item_sprite(&icons, *at)).id()),
        }
    }
    for &entity in pool.sprites.iter().skip(places.len()) {
        if let Ok((_, mut visible)) = sprites.get_mut(entity) {
            *visible = Visibility::Hidden;
        }
    }
}

/// **The items that are entities, drawn where they say they are.** Each one gets its picture the
/// first frame it is seen, which is also the only place a sprite is put on an item: a run with no
/// window never does this and its items are three numbers each.
pub fn draw_items_as_entities(
    map: Res<Map>,
    grid: Res<Grid>,
    flow: Res<Flow>,
    icons: Res<Icons>,
    mut items: Query<(Entity, &OnBelt, Option<&mut Transform>)>,
    mut commands: Commands,
) {
    for (entity, on, transform) in &mut items {
        let at = on.tile as usize;
        let Some(building) = grid.at(at) else { continue };
        let place = crate::grid::item_at(&map, grid.tile_of(at), flow.came_in[at], building.dir, on.along);
        match transform {
            Some(mut transform) => transform.translation = place.extend(2.0),
            None => {
                commands.entity(entity).insert(item_sprite(&icons, place));
            }
        }
    }
}

fn item_sprite(icons: &Icons, at: Vec2) -> impl Bundle {
    (
        Sprite {
            image: icons.ore.clone(),
            // one world unit is one pixel of the art, and the icon is 8 px: drawn at its own
            // size, which is the only size pixel art is drawn at
            custom_size: Some(Vec2::splat(8.0)),
            ..default()
        },
        Transform::from_translation(at.extend(2.0)),
    )
}

// ---------------------------------------------------------------------------------------------
// The zoom
// ---------------------------------------------------------------------------------------------

/// Whether the camera's zoom is rounded to a whole number of screen pixels per pixel of the art.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SnapZoom(pub bool);

/// **Pixel art is drawn at a whole-number zoom or it shimmers.**
///
/// F0 measured the cost of not doing this: the same floor drawn by the same code on a PC and in a
/// browser differed in 887 pixels of 1,440,000, all of them on the seams between tiles, because
/// 900 px over 256 world units is 3.515625 screen pixels per pixel of the art and *which* texel a
/// screen pixel lands in is then a rounding (`worklog/2026-09-21-factory-F0.md` §2.4). Rounding
/// the zoom to a whole number makes every screen pixel a whole texel and the seams disappear.
///
/// It is written as a system over [`CameraView`] rather than a change to the shared camera,
/// because "how much world the window holds" is the camera's and "the world is pixel art" is this
/// game's. The wheel still zooms by the ratio the crate says; this catches the value afterwards.
pub fn snap_zoom(snap: Res<SnapZoom>, windows: Query<&Window>, mut view: ResMut<CameraView>) {
    if !snap.0 {
        return;
    }
    let Some(height) = windows.iter().next().map(|w| w.height()) else { return };
    let wanted = snapped(height, view.half_height);
    if (wanted - view.half_height).abs() > 1e-4 {
        view.half_height = wanted;
    }
}

/// The nearest half-view to `half_height` that shows a whole number of screen pixels per pixel of
/// the art — or, once one pixel of the art is smaller than one screen pixel, a whole number of
/// art pixels per screen pixel, so that zooming out past 1:1 keeps working.
pub fn snapped(window_height: f32, half_height: f32) -> f32 {
    let scale = window_height / (2.0 * half_height.max(0.001));
    let whole = if scale >= 1.0 { scale.round().max(1.0) } else { 1.0 / (1.0 / scale).round() };
    window_height / (2.0 * whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every direction and every turn comes out of the two drawn belts, and no two of the eight
    /// turns are the same picture — which is the property the author's choice rests on.
    #[test]
    fn eight_turns_and_four_directions_out_of_two_pictures() {
        let mut straights = Vec::new();
        let mut turns = Vec::new();
        for came_in in Dir::ALL {
            straights.push(belt_picture(came_in, came_in, 0));
            turns.push(belt_picture(came_in, came_in.left(), 0));
            turns.push(belt_picture(came_in, came_in.right(), 0));
        }
        assert!(straights.iter().all(|&(index, _)| index == BELT_STRAIGHT[0]));
        assert!(turns.iter().all(|&(index, _)| index == BELT_CORNER[0]));
        for list in [&straights, &turns] {
            let mut seen: Vec<TileOrientation> = list.iter().map(|&(_, o)| o).collect();
            seen.sort_by_key(|o| *o as u8);
            seen.dedup();
            assert_eq!(seen.len(), list.len(), "two of these are the same picture: {list:?}");
        }
        // the second frame is the same arrangement in the other picture
        assert_eq!(belt_picture(Dir::East, Dir::East, 1).0, BELT_STRAIGHT[1]);
        assert_eq!(belt_picture(Dir::East, Dir::North, 1).0, BELT_CORNER[1]);
    }

    /// The zoom lands on whole numbers and stays inside the range it was given.
    #[test]
    fn the_zoom_rounds_to_whole_pixels() {
        // F0's provisional view in the default window was 3.515625 screen pixels to one of the
        // art, and the nearest whole number to that is four
        assert_eq!(snapped(900.0, 128.0), 112.5, "which is four");
        // the default this stage chose is three, and three stays three
        assert_eq!(snapped(900.0, 150.0), 150.0);
        assert_eq!(snapped(900.0, 100.0), 90.0, "four and a half rounds to five");
        assert_eq!(snapped(720.0, 150.0), 180.0, "a browser's shorter canvas: two");
        // far out, where one pixel of the art is smaller than a screen pixel
        assert_eq!(snapped(900.0, 1000.0), 900.0, "half of the window is one art pixel per two");
        for window in [720.0f32, 900.0, 1080.0] {
            for half in [12.0f32, 45.0, 128.0, 150.0, 333.0, 900.0] {
                let scale = window / (2.0 * snapped(window, half));
                let whole = if scale >= 1.0 { scale } else { 1.0 / scale };
                assert!((whole - whole.round()).abs() < 1e-4, "{window} {half} -> {scale}");
            }
        }
    }
}
