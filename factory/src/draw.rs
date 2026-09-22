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

use crate::belts::Lanes;
use crate::data::Data;
use crate::grid::{Dir, Flow, Grid, Ore, What};
use crate::items::Pool;
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
/// Kenney's tile 110 — orange blocks between grey posts. The pack names nothing, so what it
/// was drawn as is a guess; it is the one that reads as apparatus, so it is the miner.
const MINER: u16 = 110;
/// A wooden crate, used as the chest.
const CHEST: u16 = 85;
/// **The inserter's base** (F3), drawn with its output to the right, so the other three
/// directions are [`FACING`] applied to it (`tools/factory-inserter.py`). The arm itself is not a
/// tile: it moves, and a tile has eight orientations and nothing in between.
const INSERTER: u16 = 142;
/// **How big an item's icon is.** 8 px, and it is not a setting for the same reason `TILE_PX` is
/// not: it is where the sheet is cut. Two of them fit across a 16 px tile without touching, which
/// is where the default `items_per_tile` in `data.rb` comes from — the number follows the art,
/// and a player who wants a denser belt moves the number and lets them overlap.
pub const ITEM_PX: u32 = 8;
/// **How many of the strip's pictures are items.** `icon:` in `ruby/data.rb` is an index into
/// them and a test refuses a data file that points past the end. `tools/factory-items.py` draws
/// the strip.
pub const ITEM_ICONS: u32 = 3;

/// **The two pictures after the items**, which are not items and which no data file can name
/// (F3): the inserter's hand, drawn travelling with whatever it is carrying, and the mark over an
/// inserter whose script has stopped.
pub const HAND: usize = ITEM_ICONS as usize;
pub const STOPPED: usize = ITEM_ICONS as usize + 1;
/// Everything in the strip, which is what the atlas is cut into.
pub const ICONS: u32 = ITEM_ICONS + 2;

/// **What is drawn over what**, in world units of z. The floor is 0 and the buildings are 1;
/// these three are the sprite pool's, and they are three rather than one because a hand and the
/// thing in it are in the same place and one of them has to be on top. Three numbers of z is
/// three sprite batches of the one image, which is two more draw calls than F2 had and is the
/// whole of what the arm's picture costs.
const Z_ITEM: f32 = 2.0;
const Z_CARRIED: f32 = 2.1;
const Z_MARK: f32 = 2.2;
/// **The ghost is over everything** (F7): it is what is *about to be* there, and a picture of
/// that behind the things that are there would be worse than nothing.
const Z_GHOST: f32 = 3.0;

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
///
/// `per_tile` is how much a full tile of ore holds — `data.rb`'s since F2a — because what the
/// picture says is *how much is left of it*, which is a fraction and not an amount.
pub fn floor_picture(map: &Map, ore: &Ore, per_tile: u32, tile: UVec2) -> u16 {
    let last = map.tiles - UVec2::ONE;
    let at = (tile.y * map.tiles.x + tile.x) as usize;
    if tile.x == 0 || tile.y == 0 || tile.x == last.x || tile.y == last.y {
        GROUND
    } else if ore.left[at] > 0 {
        // half a patch left is where it starts looking dug out
        if ore.left[at] * 2 > per_tile { ORE_RICH } else { ORE_POOR }
    } else {
        FLOOR_PLATES[((tile.x + tile.y) % FLOOR_PLATES.len() as u32) as usize]
    }
}

// ---------------------------------------------------------------------------------------------
// The two chunks
// ---------------------------------------------------------------------------------------------

/// **How long a side of the map may be, in tiles** — the one ceiling `map :world, size:` has, and
/// it is the drawing's rather than anything this game preferred (F3a).
///
/// **What breaks.** A [`TilemapChunk`] keeps its tiles in a texture of **one texel per tile**
/// (`Rgba16Uint`, `bevy_sprite_render::tilemap_chunk::make_chunk_tile_data_image`), so a map whose
/// side is longer than the device's `max_texture_dimension_2d` cannot be handed to the GPU at all:
/// wgpu refuses the texture and the floor is never drawn. Nothing else in the game has a smaller
/// limit — a tile index is a `u32` ([`crate::grid::how_many`]) and 2048² is 4.2 million, which
/// fits with three bits to spare.
///
/// **Where the number comes from.** That limit is the *device's*, and Bevy asks the adapter for
/// its own rather than for a fixed set (`bevy_render::renderer`, `WgpuSettingsPriority::
/// Functionality`), so it differs from machine to machine — which is exactly what a game that is
/// published as a page cannot depend on. What does not differ is the floor under it: **WebGL2
/// guarantees 2048** (`wgpu-types-29.0.4/src/limits.rs:498`, `downlevel_defaults`, which is what
/// `downlevel_webgl2_defaults` inherits and what the WebGL2 specification requires of
/// `MAX_TEXTURE_SIZE`). A map inside this is a map that draws on any machine the page reaches;
/// one outside it might draw here and be a black page for somebody else, which is the bug F0
/// spent a day on in another form. Measured on the two renderers this was written with —
/// lavapipe in the container and SwiftShader in the browser both report more than this, and
/// [`start_drawing`] logs what the machine actually said so that the guarantee can be compared
/// with the fact.
///
/// **It is not what runs out first, and that is on purpose.** A map this big is 4.2 million tiles
/// and about half a gigabyte of grid, lanes and ore; what a machine can hold is a fact about the
/// machine, so it is logged (`crate::lay_the_land` says how many megabytes a map costs) and not
/// made into a refusal. The ceiling refuses only what is certainly broken everywhere.
///
/// **Why the map is not cut into several chunks**, which would lift this: the split is real work
/// — the floor and the buildings become grids of chunks, every write to a tile has to find its
/// chunk, and the two `TilemapChunkTileData` walks in this file become nested ones — and what it
/// would buy is maps larger than the memory of the machine drawing them. If a real map ever wants
/// to be 3,000 tiles across, this is the line to come back to.
pub const MOST_TILES_ACROSS: u32 = 2048;

#[derive(Resource, Debug)]
pub struct Chunks {
    pub floor: Entity,
    pub buildings: Entity,
    /// **The ghost** (F7): a third chunk, as big as the biggest machine and no bigger, moved to
    /// whichever tile the mouse is over. See [`draw_ghost`].
    pub ghost: Entity,
    /// How many tiles across and up the ghost's chunk is, which is the biggest footprint
    /// `data.rb` declares. A `TilemapChunk`'s size is immutable, so this is settled when the
    /// chunks are spawned and a new `data.rb` spawns them again.
    pub ghost_tiles: UVec2,
    /// **The sheet itself**, which the palette cuts its pictures out of (`crate::palette`). It is
    /// kept here because this is where it is loaded; two loads of one path with different
    /// settings is a thing Bevy warns about, and the settings are this module's.
    pub tileset: Handle<Image>,
}

/// **Which tile of the sheet a thing wears**, for anything that wants its picture without a grid
/// to read it out of: the palette's rows (F7) and the ghost.
///
/// A belt's is the straight one, which is what a belt with nothing next to it is; the corner is
/// worked out from the neighbours and only exists once it is on the map ([`belt_picture`]). A
/// machine's is the top left of its footprint, which is the first of its `sprite:` list. Nothing
/// in hand has no picture at all, which is what the wrecking ball is.
pub fn picture_of(what: Option<What>, data: &Data) -> Option<u16> {
    match what? {
        What::Belt => Some(BELT_STRAIGHT[0]),
        What::Miner => Some(MINER),
        What::Chest => Some(CHEST),
        What::Inserter => Some(INSERTER),
        What::Machine(kind) => data.machines.get(kind as usize)?.sprite.first().copied(),
        What::Covered { .. } => None,
    }
}

/// **The items' pictures**: one sheet of [`ITEM_PX`] squares and the layout that cuts it, loaded
/// once and shared by every sprite in the pool.
///
/// One image and one layout rather than one image per item, because a sprite batch is the run of
/// the same image at the same z (`bevy_sprite_render`): a belt carrying ore, plates and gears is
/// one draw call this way and three the other.
#[derive(Resource, Debug)]
pub struct Icons {
    pub sheet: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// Which frame of the belts' two-frame animation is showing, and when it last turned over.
#[derive(Resource, Debug, Default)]
pub struct Animation {
    pub frame: usize,
    pub since: f32,
}

pub fn start_drawing(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    map: Res<Map>,
    data: Res<Data>,
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
) {
    // **What this machine's own limit turned out to be**, beside the one the data stage refuses
    // by ([`MOST_TILES_ACROSS`], which is the guarantee every WebGL2 machine gives rather than
    // what any one of them has). It is logged and not checked, because a run that got here has a
    // map the guarantee already allows; what it is for is that the guarantee can be read against
    // the fact, in a container and in a page, without taking anybody's word for it.
    if let Some(device) = device {
        let most = device.limits().max_texture_dimension_2d;
        info!(
            "the floor is one texture of {} by {} texels; this renderer allows {most} (the size a data file may ask for is capped at {})",
            map.tiles.x, map.tiles.y, MOST_TILES_ACROSS
        );
        if map.tiles.max_element() > most {
            error!(
                "this renderer cannot draw a map {} by {}: {most} is as far as it goes",
                map.tiles.x, map.tiles.y
            );
        }
    }
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
    let empty = vec![None; crate::grid::how_many(map.tiles)];
    let floor = commands
        .spawn((
            TilemapChunk {
                chunk_size: map.tiles,
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
                chunk_size: map.tiles,
                tile_display_size: UVec2::splat(TILE_PX),
                tileset: tileset.clone(),
                // **over the floor**: a belt's corners and a crate's edges are transparent, and
                // the default is opaque, which would draw the floor's colour through them
                alpha_mode: AlphaMode2d::Blend,
            },
            TilemapChunkTileData(empty),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ))
        .id();
    // **the ghost's chunk** (F7): as many tiles as the biggest machine covers, so that a 2×2
    // assembler can be shown whole and a map of four million tiles does not pay for a third
    // texture of its own size. A `TilemapChunk` cannot be resized — the component is immutable —
    // so the size is settled here, where a new `data.rb` comes past again.
    let ghost_tiles = data
        .machines
        .iter()
        .fold(UVec2::ONE, |most, machine| most.max(machine.size));
    let ghost = commands
        .spawn((
            TilemapChunk {
                chunk_size: ghost_tiles,
                tile_display_size: UVec2::splat(TILE_PX),
                tileset: tileset.clone(),
                // it is a translucent picture of something that is not there yet
                alpha_mode: AlphaMode2d::Blend,
            },
            TilemapChunkTileData(vec![None; crate::grid::how_many(ghost_tiles)]),
            Transform::from_xyz(0.0, 0.0, Z_GHOST),
        ))
        .id();
    commands.insert_resource(Chunks { floor, buildings, ghost, ghost_tiles, tileset });
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(ITEM_PX),
        ICONS,
        1,
        None,
        None,
    ));
    commands.insert_resource(Icons { sheet: assets.load("items/items.png"), layout });
}

/// The floor, when a patch of ore has changed — which is rare, and is why this is not every frame.
pub fn draw_floor(
    map: Res<Map>,
    rules: Res<crate::belts::Rules>,
    mut ore: ResMut<Ore>,
    chunks: Res<Chunks>,
    mut tiles: Query<&mut TilemapChunkTileData>,
) {
    if !ore.changed {
        return;
    }
    let Ok(mut picture) = tiles.get_mut(chunks.floor) else { return };
    for y in 0..map.tiles.y {
        for x in 0..map.tiles.x {
            let at = (y * map.tiles.x + x) as usize;
            picture.0[at] = Some(TileData::from_tileset_index(floor_picture(
                &map,
                &ore,
                rules.ore_per_tile,
                UVec2::new(x, y),
            )));
        }
    }
    ore.changed = false;
}

/// The buildings, when something is built or taken away, and when the belts' animation turns over.
pub fn draw_buildings(
    time: Res<Time>,
    rules: Res<crate::belts::Rules>,
    data: Res<Data>,
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
    let Ok(mut picture) = tiles.get_mut(chunks.buildings) else { return };
    if *seen != grid.changes {
        // everything that had a picture and may not have one now
        for &at in drawn.iter() {
            picture.0[at as usize] = None;
        }
        drawn.clear();
        drawn.extend_from_slice(grid.built());
        *seen = grid.changes;
    }
    for &at in drawn.iter() {
        let at = at as usize;
        let Some(building) = grid.at(at) else { continue };
        picture.0[at] = Some(match building.what {
            What::Belt => {
                let (index, orientation) =
                    belt_picture(flow.came_in[at], building.dir, animation.frame);
                TileData { tileset_index: index, orientation, ..TileData::from_tileset_index(index) }
            }
            What::Miner => TileData::from_tileset_index(MINER),
            What::Chest => TileData::from_tileset_index(CHEST),
            What::Inserter => TileData {
                orientation: FACING[building.dir.number()],
                ..TileData::from_tileset_index(INSERTER)
            },
            // **a machine is as many pictures as it covers tiles**, and which one a tile gets is
            // where that tile is inside the footprint — row by row from the bottom left, which is
            // the order `machine :name, sprite: […]` lists them in (`crate::data`)
            What::Machine(kind) => TileData::from_tileset_index(machine_picture(
                &data,
                kind,
                grid.tile_of(at),
                grid.tile_of(at),
            )),
            What::Covered { origin } => {
                let origin = origin as usize;
                let Some(What::Machine(kind)) = grid.at(origin).map(|b| b.what) else { continue };
                TileData::from_tileset_index(machine_picture(
                    &data,
                    kind,
                    grid.tile_of(origin),
                    grid.tile_of(at),
                ))
            }
        });
    }
}

/// Which of a machine's pictures the tile at `here` gets, given that the machine was built at
/// `origin`. A tile outside the footprint — which should not happen — gets the first, because a
/// picture that is in the wrong place is easier to see than none at all.
fn machine_picture(data: &Data, kind: crate::data::MachineId, origin: UVec2, here: UVec2) -> u16 {
    let Some(machine) = data.machines.get(kind as usize) else { return 0 };
    // **row by row from the top**, which is the order a `sprite:` list is written in and the
    // order `tools/factory-tileset.py` cuts a sheet in. The map counts rows upwards, so the
    // topmost row of the footprint is the one with the *largest* `y`.
    let offset = here.as_ivec2() - origin.as_ivec2();
    let from_the_top = (machine.size.y as i32 - 1 - offset.y).max(0) as u32;
    let at = from_the_top * machine.size.x + offset.x.max(0) as u32;
    *machine.sprite.get(at as usize).or_else(|| machine.sprite.first()).unwrap_or(&0)
}

// ---------------------------------------------------------------------------------------------
// The items
// ---------------------------------------------------------------------------------------------

/// **The items, drawn out of a pool of sprites.** There is no entity per item: the pool is as
/// long as the most items that have ever been on screen at once, and the rest are hidden rather
/// than despawned, so a busy factory does not spawn and despawn thousands of entities a second.
///
/// Which is also the reason the items are not entities at all (`src/items.rs`).
#[allow(clippy::too_many_arguments)]
pub fn draw_items(
    map: Res<Map>,
    grid: Res<Grid>,
    flow: Res<Flow>,
    lanes: Res<Lanes>,
    rules: Res<crate::belts::Rules>,
    data: Res<Data>,
    arms: Res<crate::inserters::Arms>,
    icons: Res<Icons>,
    mut pool: ResMut<Pool>,
    mut commands: Commands,
    mut sprites: Query<(&mut Transform, &mut Visibility, &mut Sprite)>,
    mut on_belts: Local<Vec<(Vec2, crate::data::ItemId)>>,
    mut drawn: Local<Vec<(Vec3, usize)>>,
) {
    crate::items::places(&map, &grid, &flow, &lanes, &mut on_belts);
    drawn.clear();
    for &(at, item) in on_belts.iter() {
        // which of the sheet's icons this item wears is `data.rb`'s `icon:`, and an item whose
        // number is off the end of the sheet wears the first rather than nothing
        let icon = data.items.get(item as usize).map(|i| i.icon).unwrap_or(0) as usize;
        drawn.push((at.extend(Z_ITEM), icon));
    }
    // **the arms, and what they are carrying** (F3). The hand and the item are the same place, so
    // the item is a hair nearer the eye; the mark over a stopped inserter is nearer still.
    for &t in grid.built() {
        let t = t as usize;
        let Some(building) = grid.at(t) else { continue };
        if building.what != What::Inserter {
            continue;
        }
        let here = map.tile_centre(grid.tile_of(t));
        if building.swinging || !building.held.is_empty() {
            let at = hand_at(map.tile_centre(grid.tile_of(t)), building, &rules);
            drawn.push((at.extend(Z_ITEM), HAND));
            if let Some(item) = building.held.first() {
                let icon = data.items.get(item as usize).map(|i| i.icon).unwrap_or(0) as usize;
                drawn.push((at.extend(Z_CARRIED), icon));
            }
        }
        if arms.has_stopped(t) {
            drawn.push((here.extend(Z_MARK), STOPPED));
        }
    }
    for (i, &(at, icon)) in drawn.iter().enumerate() {
        match pool.sprites.get(i) {
            Some(&entity) => {
                if let Ok((mut transform, mut visible, mut sprite)) = sprites.get_mut(entity) {
                    transform.translation = at;
                    *visible = Visibility::Inherited;
                    if let Some(atlas) = sprite.texture_atlas.as_mut() {
                        atlas.index = icon;
                    }
                }
            }
            // a sprite spawned now cannot be written until the next frame, which is when this
            // item will be drawn; one frame late for one item is not a thing anybody sees
            None => pool.sprites.push(commands.spawn(item_sprite(&icons, at, icon)).id()),
        }
    }
    for &entity in pool.sprites.iter().skip(drawn.len()) {
        if let Ok((_, mut visible, _)) = sprites.get_mut(entity) {
            *visible = Visibility::Hidden;
        }
    }
}

/// **Where an inserter's hand is**, in world units: straight across from the middle of the tile
/// behind it to the middle of the tile in front, over the length of a swing.
///
/// An arm that is not swinging is over its own tile — which is where an arm that carried
/// something across and was refused stands, holding it, until its script tries again. A straight
/// sweep rather than an arc because the sweep is what says "this came from there and went there",
/// which is the thing a player has to be able to read off the screen.
fn hand_at(here: Vec2, arm: &crate::grid::Building, rules: &crate::belts::Rules) -> Vec2 {
    if !arm.swinging {
        return here;
    }
    let part = (arm.work / rules.swing_seconds).clamp(0.0, 1.0);
    let reach = arm.dir.as_vec() * TILE_PX as f32;
    (here - reach).lerp(here + reach, part)
}

fn item_sprite(icons: &Icons, at: Vec3, icon: usize) -> impl Bundle {
    (
        Sprite {
            image: icons.sheet.clone(),
            texture_atlas: Some(TextureAtlas { layout: icons.layout.clone(), index: icon }),
            // one world unit is one pixel of the art, and the icon is `ITEM_PX`: drawn at its own
            // size, which is the only size pixel art is drawn at
            custom_size: Some(Vec2::splat(ITEM_PX as f32)),
            ..default()
        },
        Transform::from_translation(at),
    )
}

// ---------------------------------------------------------------------------------------------
// The ghost
// ---------------------------------------------------------------------------------------------

/// **What is about to be built, on the tile the mouse is over** (F7, the author's third point:
/// "I cannot tell which way round it will go").
///
/// It is the real picture and not an outline — the belt's corner worked out from its neighbours
/// as if it were already laid, the inserter turned the way its arm will swing, a machine's whole
/// footprint — drawn at [`crate::palette::PaletteStyle::ghost_alpha`], and tinted red where the
/// tile would refuse it (`crate::build::would_refuse`, which is the same rule the click obeys).
///
/// **One entity, reused.** It is a third `TilemapChunk` the size of the biggest machine, moved
/// to the tile under the cursor; nothing is spawned or despawned as the mouse travels. What it
/// costs is one small texture and one draw call, and only while a window is open at all.
///
/// **A wrecking ball has no picture**, so nothing in hand shows the tile itself picked out —
/// which is what the wrecking ball is: not a thing to put there, but a thing to take away.
#[allow(clippy::too_many_arguments)]
pub fn draw_ghost(
    map: Res<Map>,
    grid: Res<Grid>,
    ore: Res<crate::grid::Ore>,
    data: Res<Data>,
    hand: Res<crate::build::Hand>,
    style: Res<crate::palette::PaletteStyle>,
    animation: Res<Animation>,
    chunks: Res<Chunks>,
    view: Res<CameraView>,
    insets: Res<games_shell::camera::ViewInsets>,
    pointer: Option<Res<bevy_egui::input::EguiWantsInput>>,
    windows: Query<&Window>,
    mut tiles: Query<(&mut TilemapChunkTileData, &mut Transform)>,
    mut visible: Query<&mut Visibility>,
) {
    let Ok((mut picture, mut place)) = tiles.get_mut(chunks.ghost) else { return };
    let mut show = |on: bool| {
        if let Ok(mut visible) = visible.get_mut(chunks.ghost) {
            *visible = if on { Visibility::Inherited } else { Visibility::Hidden };
        }
    };
    // **egui first**, the same question the camera asks of a wheel: a mouse over the palette or
    // the editor is not pointing at a tile at all
    if pointer.is_some_and(|p| p.wants_pointer_input() || p.is_pointer_over_area()) {
        show(false);
        return;
    }
    let Some(window) = windows.iter().next() else {
        show(false);
        return;
    };
    let size = Vec2::new(window.width(), window.height());
    // **where the hand is aiming, with no pointer at all.** A container's window and a page
    // nobody has moved a mouse over have no cursor position, and a ghost that is nowhere says
    // nothing about what is in hand — so it stands in the middle of what can be seen, which is
    // the tile a click with no mouse would be aimed at. It is also how a `--shot` shows it.
    let at = window.cursor_position().unwrap_or(size * 0.5);

    let point = view.lens(size, &insets).world_at(at);
    let Some(tile) = map.tile_at(point) else {
        show(false);
        return;
    };
    let refused = crate::build::would_refuse(&grid, &ore, &data, tile, hand.what);
    if hand.what.is_none() && refused == Some(crate::build::Refusal::NothingThere) {
        // nothing in hand over an empty tile: there is nothing to say
        show(false);
        return;
    }
    show(true);
    let colour = style.ghost_colour(refused.is_some());
    for slot in picture.0.iter_mut() {
        *slot = None;
    }
    // the chunk's own transform puts *its* middle somewhere; what has to land on the tile under
    // the cursor is its tile (0, 0), which is half a chunk away from that middle
    let step = TILE_PX as f32;
    let offset = Vec2::new(
        step * (chunks.ghost_tiles.x as f32 - 1.0) / 2.0,
        step * (chunks.ghost_tiles.y as f32 - 1.0) / 2.0,
    );
    let middle = map.tile_centre(tile) + offset;
    place.translation = middle.extend(Z_GHOST);
    let index = |dx: u32, dy: u32| (dy * chunks.ghost_tiles.x + dx) as usize;
    match hand.what {
        // the wrecking ball: the tile itself, picked out
        None => {
            if let Some(slot) = picture.0.get_mut(index(0, 0)) {
                *slot = Some(TileData {
                    color: colour,
                    ..TileData::from_tileset_index(GROUND)
                });
            }
        }
        Some(What::Machine(kind)) => {
            let Some(machine) = data.machines.get(kind as usize) else { return };
            for dy in 0..machine.size.y.min(chunks.ghost_tiles.y) {
                for dx in 0..machine.size.x.min(chunks.ghost_tiles.x) {
                    let here = tile + UVec2::new(dx, dy);
                    if let Some(slot) = picture.0.get_mut(index(dx, dy)) {
                        *slot = Some(TileData {
                            color: colour,
                            ..TileData::from_tileset_index(machine_picture(&data, kind, tile, here))
                        });
                    }
                }
            }
        }
        Some(What::Belt) => {
            // **the corner it would be**, worked out from the neighbours exactly as if it had
            // been laid (`Flow`'s own rule, which is why that rule is a function)
            let at = grid.index(tile);
            let came_in = crate::grid::Flow::where_it_comes_in(&grid, at, hand.dir);
            let (tileset_index, orientation) = belt_picture(came_in, hand.dir, animation.frame);
            if let Some(slot) = picture.0.get_mut(index(0, 0)) {
                *slot = Some(TileData { tileset_index, color: colour, orientation, visible: true });
            }
        }
        Some(what) => {
            let Some(tileset_index) = picture_of(Some(what), &data) else { return };
            // a miner and a chest look the same whichever way they are turned; an inserter does
            // not, and the way it is turned is the way its arm will swing
            let orientation = match what {
                What::Inserter => FACING[hand.dir.number()],
                _ => TileOrientation::Default,
            };
            if let Some(slot) = picture.0.get_mut(index(0, 0)) {
                *slot = Some(TileData { tileset_index, color: colour, orientation, visible: true });
            }
        }
    }
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
///
/// `half_height` is more than zero: the setting it starts from is refused if it is not, and the
/// wheel keeps it inside a range either side of that (`games_shell::camera::zoom_by`). There is
/// no floor here for the same reason there is none in [`Rules::spacing`] — a floor would be a
/// number with nowhere to come from.
///
/// [`Rules::spacing`]: crate::belts::Rules::spacing
pub fn snapped(window_height: f32, half_height: f32) -> f32 {
    let scale = window_height / (2.0 * half_height);
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
