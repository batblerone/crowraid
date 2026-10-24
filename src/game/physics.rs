//! A physics module to handle colliders, collision, and specifics therein.
//! 
//! GameLayer is the primary struct that delineates which collision layer an
//! entity is within, and what it collides with within that layer.

use avian2d::prelude::*;

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum GameLayer {
    #[default]
    Default,
    Walls,
    Player,
    Enemy,
    PlayerBullet,
    EnemyBullet,
}

impl GameLayer {
    /// The player hits walls, enemies (rammed), and enemy bullets
    pub fn player_layers() -> CollisionLayers {
        CollisionLayers::new(
            GameLayer::Player,
            [GameLayer::Walls, GameLayer::Enemy, GameLayer::EnemyBullet],
        )
    }
    
    /// Enemies hit the player and player bullets, and pass over walls and each other.
    pub fn enemy_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::Enemy, [GameLayer::Player, GameLayer::PlayerBullet])
    }
    
    /// Player bullets collide with enemies and walls
    pub fn player_bullet_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::PlayerBullet, [GameLayer::Enemy, GameLayer::Walls])
    }
    
    /// Enemy bullets collide with Player and Walls
    pub fn enemy_bullet_layers() -> CollisionLayers {
        CollisionLayers::new(GameLayer::EnemyBullet, [GameLayer::Player, GameLayer::Walls])
    }
}