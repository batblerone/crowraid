//! A physics module to handle colliders, collision, and specifics therein.
//!
//! GameLayer is the primary struct that delineates which collision layer an
//! entity is within, and what it collides with within that layer.

use crate::AppSystems;
use crate::game_manager::PausableSystems;
use avian2d::prelude::*;
use bevy::prelude::*;

/// Physics constants used throughout game
///
/// Damage dealt to anything that has no [`ContactDamage`] of its own
/// Just walls for now.
pub const DEFAULT_CONTACT_DAMAGE: f32 = 5.0;
/// How fast a knockback fades. Higher = shorter and snappier, lower = longer
/// Total bounce distance is roughly `BOUNCE_SPEED / KNOCKBACK_DECAY` pixels.
pub const KNOCKBACK_DECAY: f32 = 8.0;
/// Below this threshold speed, knockback is treated as complete.
const KNOCKBACK_STOP: f32 = 5.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        tick_knockback_and_cooldowns
            .in_set(AppSystems::TickTimers)
            .in_set(PausableSystems),
    );
}

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum GameLayer {
    #[default]
    Default,
    Walls,
    Player,
    Enemy,
    PlayerBullet,
    EnemyBullet,
    Pickup,
}

impl GameLayer {
    /// The player hits walls, enemies (rammed), and enemy bullets
    pub fn player_layers() -> CollisionLayers {
        CollisionLayers::new(
            GameLayer::Player,
            [
                GameLayer::Walls,
                GameLayer::Enemy,
                GameLayer::EnemyBullet,
                GameLayer::Pickup,
            ],
        )
    }

    /// Enemies hit the player and player bullets, and pass over walls and each other.
    pub fn enemy_layers() -> CollisionLayers {
        CollisionLayers::new(
            GameLayer::Enemy,
            [GameLayer::Player, GameLayer::PlayerBullet, GameLayer::Walls],
        )
    }
    
    /// Bosses hit the player and player bullets only, they can fly over anything
    pub fn boss_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::Enemy, [GameLayer::Player,
            GameLayer::PlayerBullet])
    }

    /// Player bullets collide with enemies and walls
    pub fn player_bullet_layers() -> CollisionLayers {
        CollisionLayers::new(
            GameLayer::PlayerBullet,
            [GameLayer::Enemy, GameLayer::Walls],
        )
    }

    /// Enemy bullets collide with Player and Walls
    pub fn enemy_bullet_layers() -> CollisionLayers {
        CollisionLayers::new(
            GameLayer::EnemyBullet,
            [GameLayer::Player, GameLayer::Walls],
        )
    }

    pub fn wall_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::Walls, LayerMask::ALL) // Everything is capable of colliding with the walls
    }
    
    /// Only crows interact with pickups 
    pub fn pickup_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::Pickup, [GameLayer::Player, GameLayer::PlayerBullet])
    }
}

/// Components needed for physics operations
///
/// How much damage an entity does to anything with `Health` that
/// it bumps into.
/// Optional: anything without it deals a [`DEFAULT_CONTACT_DAMAGE`].
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct ContactDamage(pub f32);

impl Default for ContactDamage {
    fn default() -> Self {
        Self(DEFAULT_CONTACT_DAMAGE)
    }
}

/// Temporary shove (in px/s) added on top of normal movement after a bump.
/// Fades to zero on its own. `Health` requires this, so every
/// damageable entity gets one automatically.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct Knockback(pub Vec2);

/// Seconds before entities can take damage again
/// Stops one bump against a stack of wall segments for hitting
/// mutlitple times and mucking things up
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct HurtCooldown(pub f32);

/// Fades the knockbacks out and counts out cooldowns
pub fn tick_knockback_and_cooldowns(
    time: Res<Time>,
    mut query: Query<(&mut Knockback, &mut HurtCooldown)>,
) {
    let dt = time.delta_secs();
    // Exponential decay: the same feel regardless of frame rate
    let fade = (-KNOCKBACK_DECAY * dt).exp();

    for (mut knockback, mut cooldown) in &mut query {
        if knockback.0 != Vec2::ZERO {
            knockback.0 *= fade;
            if knockback.0.length() < KNOCKBACK_STOP {
                knockback.0 = Vec2::ZERO;
            }
        }
        if cooldown.0 > 0.0 {
            cooldown.0 = (cooldown.0 - dt).max(0.0);
        }
    }
}
