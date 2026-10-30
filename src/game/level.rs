//! The level: the scrolling, tile-drawn channel the crow flies down.
//!
//! The channels *shape* comes from `river.rs` (pure noise maths, no bevy or nuthin).
//! This module turns each generated row of water into a strip of tiles from
//! `tiles.rs`, adds colliders and pickups, scrolls the rows down the screen,
//! and reacts to the game phases: narrow the river for the boss, reopen it
//! (with the next level's shape and biome) once the level is cleared.

use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    PausableSystems,
    asset_tracking::LoadResource,
    audio::music,
    game::{
        pickup::{PickupAssets, PickupKind, PickupSpacing, pickup},
        player::{Player, PlayerAssets, SelectedPlayer, player},
        progression::{BossThresholdReached, LevelProgress},
        river::{RiverGenerator, Water},
        tiles::{
            BIOME_COUNT, COLUMNS, TILE_SIZE, TileAssets, corner_row, solid_runs, tile_collider,
            tile_mask, tile_sprite,
        },
    },
    game_manager::{GamePhase, RunSeed, Screen},
};

/// How fast the river scrolls down the screen, in pixels per second.
pub const SCROLL_SPEED: f32 = 300.0;
/// Keep rows generated up to this far above the top of the window.
const LOOKAHEAD: f32 = 400.0;
/// Rows are despawned once they are this far below the bottom of the window.
const CLEANUP_MARGIN: f32 = 2.0 * TILE_SIZE;
/// Safety cap on rows generated in one frame (e.g. right after a window resize).
const MAX_ROWS_PER_FRAME: usize = 32;
/// Where a new or respawned crow appears, in world y.
const PLAYER_START_Y: f32 = -220.0;
/// Shows through the transparent open-water tiles (mask 0).
const WATER_COLOR: Color = Color::srgb(0.2, 0.41, 0.66);

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<LevelAssets>();

    // `spawn_level` itself is registered in screens/gameplay.rs as
    // `spawn_level.after(RunSetup)` (tutorial Step 1.3).
    app.add_systems(OnEnter(Screen::Gameplay), use_water_background);
    app.add_systems(OnExit(Screen::Gameplay), restore_background);

    app.add_systems(
        Update,
        (generate_channel_segments, scroll_and_cleanup_channel)
            .chain()
            .run_if(in_state(Screen::Gameplay))
            .run_if(resource_exists::<ChannelGeneratorConfig>)
            .in_set(PausableSystems),
    );
    app.add_systems(
        Update,
        (narrow_for_boss, place_new_player)
            .run_if(in_state(Screen::Gameplay))
            .run_if(resource_exists::<ChannelGeneratorConfig>),
    );
    // OnExit, not OnEnter: `advance()` has already run, so the level is the new one.
    app.add_systems(
        OnExit(GamePhase::LevelClear),
        open_river_for_next_level.run_if(resource_exists::<ChannelGeneratorConfig>),
    );
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    music: Handle<AudioSource>,
    /// Not drawn yet: water comes from the clear colour. Kept for a scrolling backdrop.
    #[allow(dead_code)]
    background: Handle<Image>,
}

impl FromWorld for LevelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            music: assets.load("audio/music/Fluffing A Duck.ogg"),
            background: assets.load("images/background.png"),
        }
    }
}

/// Generator state for the current run. Built fresh by `spawn_level` from the
/// run seed and level, so Continue rebuilds the same river.
#[derive(Resource)]
pub struct ChannelGeneratorConfig {
    /// The river's shape: noise, level parameters, boss pinch.
    pub river: RiverGenerator,
    /// World y of the bottom of the next row to spawn.
    pub next_spawn_y: f32,
    /// Counts rows down to the next pickup.
    pub pickups: PickupSpacing,
    /// Land/water at the grid corners along the top of the last row spawned.
    /// The next row uses it as its bottom corners.
    pub prev_corners: Option<Vec<bool>>,
    /// Which 16-tile block of the sheet new rows use.
    pub biome: u32,
}

impl ChannelGeneratorConfig {
    pub fn new(seed: u32, level: u32, start_y: f32) -> Self {
        Self {
            river: RiverGenerator::new(seed, level),
            next_spawn_y: start_y,
            pickups: PickupSpacing::default(),
            prev_corners: None,
            biome: biome_for_level(level),
        }
    }
}

/// Level 1 → biome 0, level 2 → biome 1, ... wrapping around the sheet.
pub fn biome_for_level(level: u32) -> u32 {
    level.saturating_sub(1) % BIOME_COUNT
}

/// One row of river, one tile tall. Its children are the tiles, the colliders
/// and any pickup, so they all scroll and despawn together.
#[derive(Component)]
pub struct ChannelSegment {
    pub bottom_y: f32,
    pub top_y: f32,
    /// Open water in this row as (left, right) x spans. Used to place the
    /// crow on respawn, and handy for spawning enemies over water.
    pub water: Water,
}

/// Marks anything solid the crow can crash into. tiles.rs adds it to colliders.
#[derive(Component)]
pub struct Wall;

/// Remembers the app's background colour (set by the splash screen) while
/// gameplay uses the water colour instead.
#[derive(Resource)]
struct PreviousClearColor(Color);

fn use_water_background(mut commands: Commands, clear: Res<ClearColor>) {
    commands.insert_resource(PreviousClearColor(clear.0));
    commands.insert_resource(ClearColor(WATER_COLOR));
}

fn restore_background(mut commands: Commands, previous: Option<Res<PreviousClearColor>>) {
    if let Some(previous) = previous {
        commands.insert_resource(ClearColor(previous.0));
        commands.remove_resource::<PreviousClearColor>();
    }
}

/// Spawns the level: music, the crow, and enough river to fill the screen.
pub fn spawn_level(
    mut commands: Commands,
    level_assets: Res<LevelAssets>,
    player_assets: Res<PlayerAssets>,
    pickup_assets: Res<PickupAssets>,
    tile_assets: Res<TileAssets>,
    selected: Res<SelectedPlayer>,
    seed: Res<RunSeed>,
    progress: Res<LevelProgress>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    commands.spawn((
        Name::new("Level"),
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(Screen::Gameplay),
        children![(
            Name::new("Gameplay Music"),
            music(level_assets.music.clone())
        )],
    ));

    // The player is a scene, so it's spawned on its own. `place_new_player`
    // moves it into open water once the rows below exist.
    commands.spawn_scene(player(selected.0, &player_assets));

    // Start one row below the bottom of the window so there's never a gap.
    let half_height = window.height() * 0.5;
    let mut config = ChannelGeneratorConfig::new(seed.0, progress.level, -half_height - TILE_SIZE);
    fill_ahead(
        &mut commands,
        &mut config,
        &pickup_assets,
        &tile_assets,
        half_height + LOOKAHEAD,
        usize::MAX,
    );
    commands.insert_resource(config);
}

/// Spawns rows until the river reaches `limit_y`, or `max_rows` were spawned.
fn fill_ahead(
    commands: &mut Commands,
    config: &mut ChannelGeneratorConfig,
    pickup_assets: &PickupAssets,
    tile_assets: &TileAssets,
    limit_y: f32,
    max_rows: usize,
) {
    let mut spawned = 0;
    while config.next_spawn_y < limit_y && spawned < max_rows {
        spawn_single_segment(commands, config, pickup_assets, tile_assets);
        spawned += 1;
    }
}

/// Spawns one row of river: its tiles, merged colliders for the solid runs,
/// and maybe a pickup.
pub fn spawn_single_segment(
    commands: &mut Commands,
    config: &mut ChannelGeneratorConfig,
    pickup_assets: &PickupAssets,
    tile_assets: &TileAssets,
) {
    let bottom_y = config.next_spawn_y;
    let top_y = bottom_y + TILE_SIZE;

    // The very first row needs a corner row underneath it.
    if config.prev_corners.is_none() {
        let first = config.river.next_row(TILE_SIZE);
        config.prev_corners = Some(corner_row(&first));
    }
    let water = config.river.next_row(TILE_SIZE);
    let above = corner_row(&water);
    let below = config
        .prev_corners
        .replace(above.clone())
        .expect("prev_corners is seeded above");

    let biome = config.biome;
    let solid = solid_runs(&below, &above);
    let pickup_x = config.pickups.next_row(&water);

    commands
        .spawn((
            Name::new("ChannelSegment"),
            ChannelSegment {
                bottom_y,
                top_y,
                water,
            },
            Transform::from_xyz(0.0, bottom_y + TILE_SIZE * 0.5, 0.0),
            Visibility::default(),
            // On the parent, so the whole row (children included) is cleaned up.
            DespawnOnExit(Screen::Gameplay),
        ))
        .with_children(|parent| {
            for col in 0..COLUMNS {
                let mask = tile_mask(&below, &above, col);
                // Mask 0 is open water: the clear colour already draws it.
                if mask != 0 {
                    parent.spawn(tile_sprite(tile_assets, biome, mask, col));
                }
            }
            for (left, right) in solid {
                parent.spawn(tile_collider(left, right));
            }
            if let Some(x) = pickup_x {
                parent.spawn(pickup(PickupKind::random(), x, pickup_assets));
            }
        });

    config.next_spawn_y = top_y;
}

/// Keeps the river generated a fixed distance above the top of the window.
pub fn generate_channel_segments(
    mut commands: Commands,
    mut config: ResMut<ChannelGeneratorConfig>,
    pickup_assets: Res<PickupAssets>,
    tile_assets: Res<TileAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let limit_y = window.height() * 0.5 + LOOKAHEAD;
    fill_ahead(
        &mut commands,
        &mut config,
        &pickup_assets,
        &tile_assets,
        limit_y,
        MAX_ROWS_PER_FRAME,
    );
}

/// Scrolls every row down the screen and despawns rows below the window.
pub fn scroll_and_cleanup_channel(
    mut commands: Commands,
    time: Res<Time>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut config: ResMut<ChannelGeneratorConfig>,
    mut segments: Query<(Entity, &mut Transform, &mut ChannelSegment)>,
) {
    let movement = SCROLL_SPEED * time.delta_secs();
    config.next_spawn_y -= movement;

    let cleanup_y = -window.height() * 0.5 - CLEANUP_MARGIN;
    for (entity, mut transform, mut segment) in &mut segments {
        transform.translation.y -= movement;
        // Keep the bounds in world space, for spawning and player placement.
        segment.bottom_y -= movement;
        segment.top_y -= movement;

        if segment.top_y < cleanup_y {
            commands.entity(entity).despawn();
        }
    }
}

/// The bridge approach: narrow the river while the boss is coming.
fn narrow_for_boss(
    mut reached: MessageReader<BossThresholdReached>,
    mut config: ResMut<ChannelGeneratorConfig>,
) {
    if reached.read().last().is_some() {
        config.river.pinch_target = 1.0;
    }
}

/// Reopen the river with the next level's shape and biome. New rows switch
/// biome while the river is still pinched, which hides the seam.
fn open_river_for_next_level(
    mut config: ResMut<ChannelGeneratorConfig>,
    progress: Res<LevelProgress>,
) {
    config.river.pinch_target = 0.0;
    config.river.set_level(progress.level);
    config.biome = biome_for_level(progress.level);
}

/// Put every new crow (first spawn or respawn) in the widest channel near
/// the bottom of the screen.
fn place_new_player(
    mut crows: Query<&mut Transform, Added<Player>>,
    segments: Query<&ChannelSegment>,
) {
    for mut transform in &mut crows {
        let x = segments
            .iter()
            .find(|s| s.bottom_y <= PLAYER_START_Y && PLAYER_START_Y < s.top_y)
            .and_then(|s| {
                s.water
                    .iter()
                    .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))
                    .map(|&(left, right)| (left + right) * 0.5)
            })
            .unwrap_or(0.0);
        transform.translation.x = x;
        transform.translation.y = PLAYER_START_Y;
    }
}
