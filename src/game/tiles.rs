//! Tile-set rendering for the river: dual-grid marching squares over the
//! water spans that river.rs produces.
//!
//! The art is Kenney's Roguelike RPG Pack (CC0), used straight from
//! `roguelikeSheet_transparent.png`. Kenney draws each ground type as a 3×3
//! "island" plus a 2×2 block of inner corners, with the shore along the tile
//! edges. The dual grid wants the shore through the *middle* of a tile, so
//! every tile is assembled from four 8×8 px quarters, one per grid corner,
//! each cut from whichever Kenney tile has the right edge or corner shape.

use avian2d::prelude::*;
use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
};

use crate::{
    asset_tracking::LoadResource,
    game::{level::Wall, physics::GameLayer},
};

pub const TILE_SIZE: f32 = 32.0;
/// 38 × 32 = 1216 px, just wider than the 1200 px playfield.
pub const COLUMNS: usize = 38;
pub const GRID_LEFT: f32 = -(COLUMNS as f32) * TILE_SIZE * 0.5;
/// Size of one tile in the image file.
const TILE_PIXELS: u32 = 16;
/// Kenney's sheet leaves 1 px of transparent spacing between tiles.
const SHEET_STRIDE: u32 = TILE_PIXELS + 1;
const QUARTER_PIXELS: u32 = TILE_PIXELS / 2;

/// One ground type on Kenney's sheet, as (column, row) tile coordinates.
#[derive(Clone, Copy)]
pub struct Terrain {
    /// Top-left tile of the 2×2 block whose tiles meet in a notch of water.
    inner: UVec2,
    /// Top-left tile of the 3×3 rounded island.
    outer: UVec2,
}

impl Terrain {
    const fn new(inner: (u32, u32), outer: (u32, u32)) -> Self {
        Self {
            inner: UVec2::new(inner.0, inner.1),
            outer: UVec2::new(outer.0, outer.1),
        }
    }
}

pub const GRASS: Terrain = Terrain::new((0, 15), (2, 15));
pub const STONE: Terrain = Terrain::new((5, 15), (7, 15));
pub const SAND: Terrain = Terrain::new((5, 21), (7, 21));
// Spares, ready to drop into BIOMES.
#[allow(dead_code)]
pub const AUTUMN: Terrain = Terrain::new((0, 18), (2, 18));
#[allow(dead_code)]
pub const PURPLE: Terrain = Terrain::new((0, 21), (2, 21));
#[allow(dead_code)]
pub const DIRT: Terrain = Terrain::new((5, 9), (7, 9));

/// Level 1 uses the first biome, level 2 the second, ... then it wraps.
/// Reorder or add any of the terrains above.
pub const BIOMES: [Terrain; 3] = [GRASS, SAND, STONE];
pub const BIOME_COUNT: u32 = BIOMES.len() as u32;

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<TileAssets>();
}

/// Is each grid corner along one row land? COLUMNS + 1 corners.
pub fn corner_row(water: &[(f32, f32)]) -> Vec<bool> {
    (0..=COLUMNS)
        .map(|i| {
            let x = GRID_LEFT + i as f32 * TILE_SIZE;
            !water.iter().any(|&(l, r)| x > l && x < r)
        })
        .collect()
}

/// The 4-bit corner mask of tile `col`, between two corner rows.
pub fn tile_mask(below: &[bool], above: &[bool], col: usize) -> u8 {
    (below[col] as u8)
        | ((below[col + 1] as u8) << 1)
        | ((above[col] as u8) << 2)
        | ((above[col + 1] as u8) << 3)
}

/// Runs of tiles that are mostly land (3+ land corners) become colliders.
/// Shore tiles with 1-2 land corners stay open, so diagonal banks are
/// forgiving rather than snagging on invisible corners.
pub fn solid_runs(below: &[bool], above: &[bool]) -> Vec<(f32, f32)> {
    let x = |col: usize| GRID_LEFT + col as f32 * TILE_SIZE;
    let mut runs = Vec::new();
    let mut start = None;
    for col in 0..COLUMNS {
        let solid = tile_mask(below, above, col).count_ones() >= 3;
        match (solid, start) {
            (true, None) => start = Some(col),
            (false, Some(s)) => {
                runs.push((x(s), x(col)));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push((x(s), x(COLUMNS)));
    }
    runs
}

/// The sprites for one tile, positioned within its row's parent entity.
/// Solid land is a single sprite; shore tiles are up to four quarter sprites
/// (water quarters are left out so the clear colour shows through).
pub fn tile_sprites(
    assets: &TileAssets,
    biome: u32,
    mask: u8,
    col: usize,
) -> impl Iterator<Item = impl Bundle> {
    let terrain = BIOMES[biome as usize % BIOMES.len()];
    let center_x = GRID_LEFT + (col as f32 + 0.5) * TILE_SIZE;
    let sprite = |rect: Rect, size: f32| Sprite {
        rect: Some(rect),
        custom_size: Some(Vec2::splat(size)),
        ..Sprite::from_image(assets.sheet.clone())
    };

    let mut sprites = Vec::with_capacity(4);
    if mask == 15 {
        let mut full = sprite(tile_rect(terrain.outer + UVec2::ONE), TILE_SIZE);
        // Free variety on solid land: random mirroring.
        full.flip_x = fastrand::bool();
        full.flip_y = fastrand::bool();
        sprites.push((
            Name::new("Tile"),
            full,
            Transform::from_xyz(center_x, 0.0, 0.0),
        ));
    } else {
        let half = TILE_SIZE * 0.5;
        for quarter in [
            UVec2::new(0, 0),
            UVec2::new(1, 0),
            UVec2::new(0, 1),
            UVec2::new(1, 1),
        ] {
            let Some(rect) = quarter_source(terrain, mask, quarter) else {
                continue;
            };
            // Sheet y runs down, world y runs up.
            let x = center_x + (quarter.x as f32 - 0.5) * half;
            let y = (0.5 - quarter.y as f32) * half;
            sprites.push((
                Name::new("Tile Quarter"),
                sprite(rect, half),
                Transform::from_xyz(x, y, 0.0),
            ));
        }
    }
    sprites.into_iter()
}

/// Which 8×8 piece of the sheet draws `quarter` of a tile with this corner
/// mask, or `None` if that quarter is water. `quarter` is (0|1, 0|1) with
/// (0, 0) the top-left, matching the sheet's orientation.
fn quarter_source(terrain: Terrain, mask: u8, quarter: UVec2) -> Option<Rect> {
    // tile_mask bits: 1 bottom-left, 2 bottom-right, 4 top-left, 8 top-right.
    let land = |x: u32, y: u32| mask & (1 << (x + if y == 0 { 2 } else { 0 })) != 0;
    let (qx, qy) = (quarter.x, quarter.y);
    if !land(qx, qy) {
        return None;
    }
    let beside = land(1 - qx, qy);
    let above_or_below = land(qx, 1 - qy);
    let diagonal = land(1 - qx, 1 - qy);
    // The quarter facing this one's position: where the island's edge pieces are.
    let facing = UVec2::ONE - quarter;

    let (tile, piece) = match (beside, above_or_below) {
        // Solid land.
        (true, true) if diagonal => (terrain.outer + UVec2::ONE, quarter),
        // Land all round except a notch of water at the tile's centre.
        (true, true) => (terrain.inner + quarter, facing),
        // A horizontal shore: top or bottom edge of the island.
        (true, false) => (
            terrain.outer + UVec2::new(1, facing.y * 2),
            UVec2::new(qx, facing.y),
        ),
        // A vertical shore: left or right edge of the island.
        (false, true) => (
            terrain.outer + UVec2::new(facing.x * 2, 1),
            UVec2::new(facing.x, qy),
        ),
        // An outer corner of the island.
        (false, false) => (terrain.outer + facing * 2, facing),
    };
    Some(quarter_rect(tile, piece))
}

/// Pixel rect of a whole tile on the sheet.
fn tile_rect(tile: UVec2) -> Rect {
    let min = (tile * SHEET_STRIDE).as_vec2();
    Rect::from_corners(min, min + Vec2::splat(TILE_PIXELS as f32))
}

/// Pixel rect of one 8×8 quarter of a tile on the sheet.
fn quarter_rect(tile: UVec2, quarter: UVec2) -> Rect {
    let min = (tile * SHEET_STRIDE + quarter * QUARTER_PIXELS).as_vec2();
    Rect::from_corners(min, min + Vec2::splat(QUARTER_PIXELS as f32))
}

/// An invisible collider covering one run of solid tiles.
pub fn tile_collider(left: f32, right: f32) -> impl Bundle {
    let width = right - left;
    (
        Wall,
        RigidBody::Static,
        Collider::rectangle(width, TILE_SIZE),
        Transform::from_xyz((left + right) * 0.5, 0.0, 0.0),
        GameLayer::wall_layers(),
        CollisionEventsEnabled,
    )
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct TileAssets {
    /// Kenney's sheet, used whole: sprites pick their pieces with `Sprite::rect`.
    #[dependency]
    pub sheet: Handle<Image>,
}

impl FromWorld for TileAssets {
    fn from_world(world: &mut World) -> Self {
        let sheet = world
            .resource::<AssetServer>()
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load("images/tiles/roguelikeSheet_transparent.png");
        Self { sheet }
    }
}