//! Spawn the main level.

use avian2d::prelude::*;
use bevy::{color::palettes::css::*, prelude::*};
use noise::{NoiseFn, Perlin};

use crate::{
    PausableSystems,
    asset_tracking::LoadResource,
    audio::music,
    game::player::{PlayerAssets, player},
    game_manager::Screen,
};

pub const SEGMENT_HEIGHT: f32 = 10.0;
pub const SCROLL_SPEED: f32 = 300.0;
pub const SCREEN_HALF_WIDTH: f32 = 600.0;
pub const CHANNEL_WIDTH: f32 = 800.0;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ChannelGeneratorConfig>()
        .load_resource::<LevelAssets>()
        .add_systems(
            Update,
            (generate_channel_segments, scroll_and_cleanup_channel)
                .chain()
                .run_if(in_state(Screen::Gameplay))
                .in_set(PausableSystems),
        );
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    music: Handle<AudioSource>,
    background: Handle<Image>,
}

#[derive(Resource)]
pub struct ChannelGeneratorConfig {
    pub perlin: Perlin,
    pub next_spawn_y: f32,
    pub seed_offset: f64,
}

impl Default for ChannelGeneratorConfig {
    fn default() -> Self {
        Self {
            perlin: Perlin::new(42),
            next_spawn_y: -400.0, // A lookahead point for spawn abouve screen.
            seed_offset: 0.0,
        }
    }
}

#[derive(Component)]
pub struct ChannelSegment {
    pub bottom_y: f32,
    pub top_y: f32,
}

#[derive(Component)]
pub struct Wall;

impl FromWorld for LevelAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            music: assets.load("audio/music/Fluffing A Duck.ogg"),
            background: assets.load("images/background.png"),
        }
    }
}

/// A system that spawns the main level.
pub fn spawn_level(
    mut commands: Commands,
    level_assets: Res<LevelAssets>,
    player_assets: Res<PlayerAssets>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut config: ResMut<ChannelGeneratorConfig>,
) {
    config.next_spawn_y = -400.0; // Reset height pointer whenever level entered.

    commands.spawn((
        Name::new("Level"),
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(Screen::Gameplay),
        children![
            player(400.0, &player_assets, &mut texture_atlas_layouts),
            (
                Name::new("Gameplay Music"),
                music(level_assets.music.clone())
            ),
        ],
    ));

    // Spawn some stuff immediately so you don't get blank screen on load
    let initial_segments = ((800.0 - (-400.0)) / SEGMENT_HEIGHT).ceil() as i32;
    for _ in 0..initial_segments {
        spawn_single_segment(&mut commands, &mut config);
    }
}

/// Spawns a single vertical channel with segments left and right using Avian for colliders
pub fn spawn_single_segment(commands: &mut Commands, config: &mut ChannelGeneratorConfig) {
    let bottom_y = config.next_spawn_y;
    let top_y = bottom_y + SEGMENT_HEIGHT;
    let mid_y = bottom_y + (SEGMENT_HEIGHT * 0.5);

    // A sampling of perlin good sir for those sweet sweet smooth winding centers
    let noise_sample = config
        .perlin
        .get([0.0, (bottom_y as f64 * 0.003) + config.seed_offset]);
    let center_offset = (noise_sample * 150.0) as f32;

    // Find the inner boundaries
    let channel_left = center_offset - (CHANNEL_WIDTH * 0.5);
    let channel_right = center_offset + (CHANNEL_WIDTH * 0.5);

    // Calc the widths and centers for a solid landmass
    let left_width = channel_left + SCREEN_HALF_WIDTH;
    let left_center = -SCREEN_HALF_WIDTH + (left_width * 0.5);

    let right_width = SCREEN_HALF_WIDTH - channel_right;
    let right_center = channel_right + (right_width * 0.5);

    // Sometimes, life splits you in half, randomly with a second perlin check
    let split_noise = config.perlin.get([50.0, bottom_y as f64 * 0.005]);
    let is_split = split_noise > 0.65;

    // Spawn something to track all this now
    let mut segment_commands = commands.spawn((
        Name::new("ChannelSegment"),
        ChannelSegment { bottom_y, top_y },
        Transform::from_xyz(0.0, mid_y, 0.0),
        Visibility::default(),
    ));

    // Give some collision up in that bish
    segment_commands.with_children(|parent| {
        // left wall
        parent.spawn((
            Wall,
            Sprite {
                color: Color::from(FOREST_GREEN),
                custom_size: Some(Vec2::new(left_width, SEGMENT_HEIGHT)),
                ..default()
            },
            RigidBody::Static,
            Collider::rectangle(left_width, SEGMENT_HEIGHT),
            Transform::from_xyz(left_center, 0.0, 0.0),
            CollisionEventsEnabled,
            // State management
            DespawnOnExit(Screen::Gameplay),
        ));

        // Right Wall
        parent.spawn((
            Wall,
            Sprite {
                color: Color::from(FOREST_GREEN),
                custom_size: Some(Vec2::new(right_width, SEGMENT_HEIGHT)),
                ..default()
            },
            RigidBody::Static,
            Collider::rectangle(right_width, SEGMENT_HEIGHT),
            Transform::from_xyz(right_center, 0.0, 0.0),
            CollisionEventsEnabled,
            // State management
            DespawnOnExit(Screen::Gameplay),
        ));

        // For a split
        if is_split {
            let island_width = 50.0;
            parent.spawn((
                Wall,
                Sprite {
                    color: Color::from(FOREST_GREEN),
                    custom_size: Some(Vec2::new(island_width, SEGMENT_HEIGHT)),
                    ..default()
                },
                RigidBody::Static,
                Collider::rectangle(island_width, SEGMENT_HEIGHT),
                Transform::from_xyz(center_offset, 0.0, 0.0),
                CollisionEventsEnabled,
                // State management
                DespawnOnExit(Screen::Gameplay),
            ));
        }
    });

    config.next_spawn_y = top_y;
}

/// Continuout generaton of wall segments
pub fn generate_channel_segments(
    mut commands: Commands,
    mut config: ResMut<ChannelGeneratorConfig>,
    query: Query<&ChannelSegment>,
) {
    // Gotta have that buffer for them segments ahead of player view
    let highest_y = query.iter().map(|s| s.top_y).fold(-500.0, f32::max);

    let mut spawned_count = 0;
    while config.next_spawn_y < highest_y + 1200.0 && spawned_count < 10 {
        spawn_single_segment(&mut commands, &mut config);
        spawned_count += 1;
    }
}
/// Scrolls the channel segments down the screen
pub fn scroll_and_cleanup_channel(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut ChannelSegment), With<ChannelSegment>>,
    mut config: ResMut<ChannelGeneratorConfig>,
) {
    let movement = SCROLL_SPEED * time.delta_secs();
    config.next_spawn_y -= movement;

    for (entity, mut transform, mut segment) in &mut query {
        // Shift it down
        transform.translation.y -= movement;

        // update component boundaries for the sake of tracker accuracy
        segment.bottom_y -= movement;
        segment.top_y -= movement;

        // Despawn when off screen
        if segment.top_y - (SCROLL_SPEED * time.delta_secs()) < -800.0 {
            commands.entity(entity).despawn();
        }
    }
}
