//! Spawn the main level.

use bevy::prelude::*;

use crate::{
    asset_tracking::LoadResource,
    audio::music,
    game::player::{PlayerAssets, player},
    screens::*,
};

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<LevelAssets>();
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct LevelAssets {
    #[dependency]
    music: Handle<AudioSource>,
    background: Handle<Image>,
}

#[derive(Component)]
pub struct ScrollingBackground;

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
) {
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

    for i in 0..2 {
        commands.spawn((
            Sprite {
                image: level_assets.background.clone(),
                ..default()
            },
            Transform::from_xyz(0.0, i as f32 * BACKGROUND_TILE_HEIGHT, -1.0),
            ScrollingBackground,
        ));
    }
}

pub fn scroll_background(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<ScrollingBackground>>,
) {
    // use time.delta_secs() for frame independent movemnent
    let movement = SCROLL_SPEED * time.delta_secs();

    for mut transform in &mut query {
        // move the tile downwards (player flys forward)
        transform.translation.y -= movement;

        // The tile position is reset once scrolls off the bottom
        if transform.translation.y <= -BACKGROUND_TILE_HEIGHT {
            transform.translation.y += BACKGROUND_TILE_HEIGHT * 2.0;
        }
    }
}
