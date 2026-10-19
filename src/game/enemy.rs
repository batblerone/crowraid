//! The enemies the player must fight. Three things that seek to end
//! the players run.
//!
//! Three distinct enemies that all vary how they attack the player. The
//! Square (Placeholder) is slow and deliberate, only going down the river.
//! in a line looking to collide with the player. They have little health,
//! and will despawn after they collide with anything, or do damage to the
//! player.
//! The Triangle (Placeholder) will fly towards the player. Shooting at the
//! player as they fly. Slightly less health than the player, a bit sturdier
//! than the squarees.
//! The Dounut (Placeholder) flys in a more erratic pattern. Still going down
//! screen but in wild loops while shooting at the player. This will have the most
//! health and be a challenge for the player.

use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::{color::palettes::css::*, prelude::*};

use crate::asset_tracking::LoadResource;
use crate::screens::gameplay::Health;

/// New enemies will appear this far above the screen.
const SPAWN_Y: f32 = 1088.0;
/// A fall back despawn distance for enemies off screen
const DESPAWN_Y: f32 = -1000.0;
/// The longest gap between an enemy spawn, the real gap is random
const MAX_ENEMY_GAP: f32 = 10.0; // Seconds

/// Constants for the squarees
const SQUARE_SPEED: f32 = 100.0;
const SQUARE_HEALTH: f32 = 10.0;
const SQUARE_SCALE: f32 = 3.0;

/// Constants for the triangles
const TRI_SPEED: f32 = 300.0;
const TRI_HEALTH: f32 = 50.0;
const TRI_SCALE: f32 = 3.0;

/// Constants for the donuts
const DONUT_SPEED: f32 = 500.0;
const DONUT_HEALTH: f32 = 80.0;
const DONUT_SCALE: f32 = 3.0;

pub fn plugin(app: &mut App) {
    app.load_resource::<EnemyAssets>();
}

// The marker for enemies
#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct Enemy;

// A match struct for enemy stats to build within scenes
pub struct EnemyStats {
    pub health: f32,
    pub speed: f32,
    pub score: u32,
    pub scale: f32,
}

/// The scene definition of an enemy for our game, shared between all enemies within the game
fn enemy_base(kind: EnemyKind) -> impl Scene {
    let health = kind.enemy_stats().health;
    bsn! {
        Enemy
        template_value(kind)
        Health { current: health, max: health }
    }
}

/// One of any given kind of enemies
pub fn enemy(kind: EnemyKind, position: Vec2, assets: &EnemyAssets) -> impl Scene {
    let EnemyVisual { mesh, material } = assets.enemy_visuals(kind).clone();
    let translation = position.extend(0.0);
    let scale = Vec3::splat(kind.enemy_stats().scale);
    bsn! {
        enemy_base(kind)
        Mesh2d(mesh)
        MeshMaterial2d::<ColorMaterial>(material)
        Transform {translation: translation, scale: scale }
    }
}

#[derive(Resource)]
pub struct EnemeyTimer(Timer);

fn random_enemy_gap() -> Timer {
    Timer::from_seconds(0.4 + fastrand::f32() * (MAX_ENEMY_GAP - 0.5), TimerMode::Once)
}

fn reset_enemy_timer(mut commands: Commands) {
    commands.insert_resource(EnemyTimer(random_gap()));
}

#[derive(Component, Reflect, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[reflect(Component)]
pub enum EnemyKind {
    #[default]
    Square,
    Triangle,
    Donut,
}

impl EnemyKind {
    pub fn enemy_stats(self) -> EnemyStats {
        match self {
            EnemyKind::Square => EnemyStats {
                health: SQUARE_HEALTH, speed: SQUARE_SPEED, score: 0 , scale: SQUARE_SCALE
            },
            EnemyKind::Triangle => EnemyStats {
                health: TRI_HEALTH, speed: TRI_SPEED, score: 0, scale: TRI_SCALE
            },
            EnemyKind::Donut => EnemyStats {
                health: DONUT_HEALTH, speed: DONUT_SPEED, score: 0, scale: DONUT_SCALE
            },
        }
    }
}

#[derive(Clone, Reflect)]
pub struct EnemyVisual {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ColorMaterial>,
}


/// Defining and implementing the assets for the enemies
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct EnemyAssets {
    // These will be built in code with Assets::add, not loaded into AssetServer.
    // Do NOT mark #[dependency]. Only mark handles that come from disk
    pub square: EnemyVisual,
    pub triangle: EnemyVisual,
    pub donut: EnemyVisual,
    // For later, when sprite sheets are solid
    // pub sheet: Handle<Image>,
    // pub layout: Handle<TextureAtlasLayout>, // Again, built in code, no #[dependency].
}

impl EnemyAssets {
    pub fn enemy_visuals(&self, kind: EnemyKind) -> &EnemyVisual {
        match kind {
            EnemyKind::Square => &self.square,
            EnemyKind::Triangle => &self.triangle,
            EnemyKind::Donut => &self.donut,
        }
    }
}

impl FromWorld for EnemyAssets {
    fn from_world(world: &mut World) -> Self {
        let (square_mesh, triangle_mesh, donut_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Rectangle::new(32.0, 32.0)),
                meshes.add(Triangle2d::new(
                    Vec2::new(0.0, 20.0),
                    Vec2::new(-18.0, -14.0),
                    Vec2::new(18.0, -14.0),
                )),
                meshes.add(Annulus::new(10.0, 10.0)),
            )
        };

        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        Self {
            square: EnemyVisual {
                mesh: square_mesh,
                material: materials.add(ColorMaterial::from_color(BLUE)),
            },
            triangle: EnemyVisual {
                mesh: triangle_mesh,
                material: materials.add(ColorMaterial::from_color(RED)),
            },
            donut: EnemyVisual {
                mesh: donut_mesh,
                material: materials.add(ColorMaterial::from_color(ORANGE)),
            },
        }
    }
}
