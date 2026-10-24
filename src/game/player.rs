//! Player-specific behavior.

/// Atlas constants
const ATLAS_HEIGHT: u32 = 64;
const ATLAS_WIDTH: u32 = 64;

use avian2d::prelude::*;
use bevy::{
    image::{ImageLoaderSettings, ImageSampler, TextureAtlasTemplate},
    prelude::*,
};

use crate::{
    AppSystems, PausableSystems,
    asset_tracking::LoadResource,
    game::{
        animation::PlayerAnimation,
        movement::{MovementController, ScreenWrap},
        physics::*,
        projectile::*,
    },
    game_manager::Screen,
    screens::gameplay::Health,
};

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<PlayerAssets>();
    app.init_resource::<SelectedPlayer>();
    
    // Record directional input as movement controls.
    app.add_systems(
        Update,
        record_player_directional_input
            .in_set(AppSystems::RecordInput)
            .in_set(PausableSystems),
    );
}

// The marker for Player
#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct Player;

/// The stats an attribute for each kind of player.
pub struct PlayerStats {
    pub health: f32,
    pub speed: f32,
    pub scale: f32,
    pub collider_rad: f32,
}

/// Where character sprite info is stored
pub struct PlayerSpriteSheet {
    pub path: &'static str,
    pub frame_size: UVec2,
    pub columns: u32,
    pub rows: u32,
    // gap in pixels between frames in image
    pub padding: u32,
}

// From here we define our player kinds, for now it is just one enum of a player, but if we have unlockable skins or players we can add others.
#[derive(Component, Reflect, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[reflect(Component)]
pub enum PlayerKind {
    #[default]
    OrdinaryCrow,
}

/// Which character the next run spawns, useful for a character select screen.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct SelectedPlayer(pub PlayerKind);

impl PlayerKind {
    /// For every kind, in the same order as the enum. Asset loading walks this list.
    pub const ALL: [PlayerKind; 1] = [PlayerKind::OrdinaryCrow];
    
    pub fn player_sheet(self) -> PlayerSpriteSheet {
        match self {
            PlayerKind::OrdinaryCrow => PlayerSpriteSheet {
                path: "images/crow_animated.png",
                frame_size: UVec2::new(ATLAS_HEIGHT, ATLAS_WIDTH),
                columns: 1,
                rows: 5,
                padding: 1,
            },
        }
    }
    
    pub fn player_stats(self) -> PlayerStats {
        match self {
            PlayerKind::OrdinaryCrow => PlayerStats {
                health: 100.0,
                speed: 400.0,
                scale: 1.0,
                collider_rad: 64.0,
            },
        }
    }
    
    /// This kinds position within `ALL`, and in `PlayerAssets` lists.
    fn index(self) -> usize {
        self as usize
    }
}


/// Everything that the player is: identity, stats, movement, physics, lifetime, etc.
pub fn player_base_scene(kind: PlayerKind) -> impl Scene {
    let stats = kind.player_stats();
    let health = Health::new(stats.health);
    let controller = MovementController {
        max_speed: stats.speed,
        ..default()
    };
    let collider = Collider::circle(stats.collider_rad);
    let layers = GameLayer::player_layers();
    let despawn = DespawnOnExit(Screen::Gameplay);
    
    bsn! {
        Name::new("Player")
        Player
        kind
        health
        controller
        ScreenWrap
        RigidBody::Dynamic
        collider
        LockedAxes::ROTATION_LOCKED
        GravityScale(0.0)
        CollisionEventsEnabled
        layers
        despawn
    }
}

/// The actual player character, ready to fly into the world anew
pub fn player(kind: PlayerKind, assets: &PlayerAssets) -> impl Scene  {
    let animation = PlayerAnimation::new();
    let sheet = assets.sheet(kind);
    let layout = assets.layout(kind);
    let index = animation.get_atlas_index();
    let transform = Transform::from_scale(Vec3::splat(kind.player_stats().scale));
    let gun = Gun::player();
    bsn! {
        @player_base_scene(kind)
        // Sprite is it's own thing in bsn!. So using the sprite function of bsn
        // we can define the sprite for our player. Where image is the sheet
        // that we have loaded in our asset loader down below.
        // 
        // The texture_atlas is different. This is an option from our asset loader that
        // we want to pass off into our bsn! call. To do this we have to use the template
        // where then we have to `.into()` our layout to get the `HandleTemplate` needed for
        // `TextureAtlasTemplate`.
        Sprite {
            image: sheet,
            texture_atlas: { 
                Some(TextureAtlasTemplate{
                    layout: layout.into(),
                    index: index
                })
            }
        }
        transform
        animation
        gun
    }
}

fn record_player_directional_input(
    input: Res<ButtonInput<KeyCode>>,
    mut controller_query: Query<&mut MovementController, With<Player>>,
) {
    // Collect directional input.
    let mut intent = Vec2::ZERO;
    if input.pressed(KeyCode::KeyW) || input.pressed(KeyCode::ArrowUp) {
        intent.y += 1.0;
    }
    if input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::ArrowDown) {
        intent.y -= 1.0;
    }
    if input.pressed(KeyCode::KeyA) || input.pressed(KeyCode::ArrowLeft) {
        intent.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) || input.pressed(KeyCode::ArrowRight) {
        intent.x += 1.0;
    }

    // Normalize intent so that diagonal movement is the same speed as horizontal / vertical.
    // This should be omitted if the input comes from an analog stick instead.
    let intent = intent.normalize_or_zero();

    // Apply movement intent to controllers.
    for mut controller in &mut controller_query {
        controller.intent = intent;
    }
}

/// Defining and implementing the assets for the player
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct PlayerAssets {
    /// One sprite sheet per player kind, in `PlayerKind::ALL` order. Loaded from disk,
    /// so the loading screen will wait for every sprite sheet.
    #[dependency]
    sheets: Vec<Handle<Image>>,
    /// One atlas layout per kind, in the same order. Built in code
    /// with `Assets::add`, not loaded so no #[dependency]
    layouts: Vec<Handle<TextureAtlasLayout>>,
    /// The steps used in animation, this is legacy from bevy_template_2d, but may find use elsewhere.
    #[dependency]
    pub steps: Vec<Handle<AudioSource>>,
}

impl PlayerAssets {
    pub fn sheet(&self, kind: PlayerKind) -> Handle<Image> {
        self.sheets[kind.index()].clone()
    }
    
    pub fn layout(&self, kind: PlayerKind) -> Handle<TextureAtlasLayout> {
        self.layouts[kind.index()].clone()
    }
}

impl FromWorld for PlayerAssets {
    fn from_world(world: &mut World) -> Self {
        // The lists below are looked up by `kind as usize`, so `ALL` must match
        // the enums's order. this catches a mismatch in dev builds.
        for (i, kind) in PlayerKind::ALL.iter().enumerate() {
            debug_assert_eq!(
                kind.index(),
                i,
                "PlayerKind::ALL must list veriants in declaration order."
            );
        }
        
        // A texture atlas is a fancy way to split a single image into a grid of related images.
        // an example is found here: https://github.com/bevyengine/bevy/blob/latest/examples/2d/texture_atlas.rs
        let layouts = {
            let mut layouts = world.resource_mut::<Assets<TextureAtlasLayout>>();
            PlayerKind::ALL
                .iter()
                .map(|kind| {
                    let sheet = kind.player_sheet();
                    layouts.add(TextureAtlasLayout::from_grid(
                        sheet.frame_size,
                        sheet.columns,
                        sheet.rows,
                        Some(UVec2::splat(sheet.padding)),
                        None,
                    ))
                })
                .collect()
        };
        
        let assets = world.resource::<AssetServer>();
        
        let sheets = PlayerKind::ALL
            .iter()
            .map(|kind| {
                assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        // Use nearest image sampling to preserve pixel art style.
                        settings.sampler = ImageSampler::nearest();
                    })
                    .load(kind.player_sheet().path)
            })
            .collect();
        
        Self {
            sheets,
            layouts,
            steps: vec![
                assets.load("audio/sound_effects/step1.ogg"),
                assets.load("audio/sound_effects/step2.ogg"),
                assets.load("audio/sound_effects/step3.ogg"),
                assets.load("audio/sound_effects/step4.ogg"),
            ],
        }
    }
}
