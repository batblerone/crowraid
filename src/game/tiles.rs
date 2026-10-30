//! Tile-set rendering for the river: dual-grid marching squares over the
//! water spans that river.rs produces.

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
pub const BIOME_COUNT: u32 = 3;
const TILES_PER_BIOME: usize = 16;
/// Size of one tile in the image file.
const TILE_PIXELS: u32 = 16;

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

pub fn atlas_index(biome: u32, mask: u8) -> usize {
    biome as usize * TILES_PER_BIOME + mask as usize
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

/// One visible tile, positioned within its row's parent entity.
pub fn tile_sprite(assets: &TileAssets, biome: u32, mask: u8, col: usize) -> impl Bundle {
    let mut sprite = Sprite::from_atlas_image(
        assets.sheet.clone(),
        TextureAtlas {
            layout: assets.layout.clone(),
            index: atlas_index(biome, mask),
        },
    );
    sprite.custom_size = Some(Vec2::splat(TILE_SIZE));
    if mask == 15 {
        // Free variety on solid land: random mirroring.
        sprite.flip_x = fastrand::bool();
        sprite.flip_y = fastrand::bool();
    }
    let x = GRID_LEFT + (col as f32 + 0.5) * TILE_SIZE;
    (Name::new("Tile"), sprite, Transform::from_xyz(x, 0.0, 0.0))
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
    #[dependency]
    pub sheet: Handle<Image>,
    /// Built in code, so no #[dependency].
    pub layout: Handle<TextureAtlasLayout>,
}

impl FromWorld for TileAssets {
    fn from_world(world: &mut World) -> Self {
        let layout =
            world
                .resource_mut::<Assets<TextureAtlasLayout>>()
                .add(TextureAtlasLayout::from_grid(
                    UVec2::splat(TILE_PIXELS),
                    4,
                    4 * BIOME_COUNT,
                    None,
                    None,
                ));
        let sheet = world
            .resource::<AssetServer>()
            .load_builder()
            .with_settings(|settings: &mut ImageLoaderSettings| {
                settings.sampler = ImageSampler::nearest();
            })
            .load("images/tiles/river_tiles.png");
        Self { sheet, layout }
    }
}
