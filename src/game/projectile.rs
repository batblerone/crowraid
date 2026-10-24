//! Projectiles: guns, bullets, and bullets hitting things.
//!
//! Hits are detected by Avian. Bullets are sensor colliders, so they report
//! contacts without pushing anything. Which pairs can touch at all is decided by
//! [`GameLayer`] collision layers, so a player bullet only ever reports touching
//! an enemy, and an enemy bullet only ever reports touching the player.

use std::{collections::HashSet, time::Duration};

use avian2d::prelude::*;
use bevy::{color::palettes::css::*, prelude::*, window::PrimaryWindow};

use crate::{
    AppSystems,
    asset_tracking::LoadResource,
    game::{enemy::EnemyKind, physics::GameLayer, player::Player},
    game_manager::{PausableSystems, Screen},
    screens::gameplay::{Health, IsDead},
};

// ---- Player gun ----
/// Seconds between player shots while Space is held.
const PLAYER_FIRE_INTERVAL: f32 = 0.15;
const PLAYER_BULLET_SPEED: f32 = 900.0;
const PLAYER_BULLET_DAMAGE: f32 = 10.0;

// ---- Enemy guns ----
const TRI_FIRE_INTERVAL: f32 = 1.2;
const TRI_BULLET_SPEED: f32 = 450.0;
const TRI_BULLET_DAMAGE: f32 = 8.0;

const DONUT_FIRE_INTERVAL: f32 = 1.6;
const DONUT_BULLET_SPEED: f32 = 380.0;
const DONUT_BULLET_DAMAGE: f32 = 6.0;
/// Donuts fire a fan of this many bullets...
const DONUT_SHOTS: u32 = 3;
/// ...this many radians apart (~14°).
const DONUT_SPREAD: f32 = 0.25;

// ---- Bullets ----
const PLAYER_BULLET_RADIUS: f32 = 5.0;
const ENEMY_BULLET_RADIUS: f32 = 7.0;
/// Bullets start this far in front of whoever fired them.
const MUZZLE_OFFSET: f32 = 30.0;
/// How far past the window edge a bullet can travel before it's removed.
const OFFSCREEN_MARGIN: f32 = 64.0;
/// Draw bullets above enemies (z = 1) and the river (z = 0).
const BULLET_Z: f32 = 2.0;

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<ProjectileAssets>();
    app.add_systems(
        Update,
        (
            arm_new_enemies,
            player_shoot,
            enemy_shoot,
            despawn_offscreen_projectiles,
            projectile_hits,
        )
        .chain()
        .run_if(in_state(Screen::Gameplay))
        .in_set(AppSystems::Update)
        .in_set(PausableSystems),
    );
}

/// Something that can fire bullets
///
/// `Default` is only here because `bsn!` needs it. Always build
/// a real gun with `Gun::new` or `Gun::Player`.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct Gun {
    // Counts down between shots. Gun may fire when count finished.
    pub cooldown: Timer,
    pub bullet_speed: f32,
    pub damage: f32,
    // Bullets per shot, more than one makes a spread fan
    pub shots: u32,
    // Angle of fan of shots
    pub spread: f32,
}

impl Gun {
    pub fn new(interval: f32, bullet_speed: f32, damage: f32) -> Self {
        let mut cooldown = Timer::from_seconds(interval, TimerMode::Once);
        // Start loaded so first shot ain't delayed.
        cooldown.tick(Duration::from_secs_f32(interval));
        Self {
            cooldown,
            bullet_speed,
            damage,
            shots: 1,
            spread: 0.0,
        }
    }

    pub fn player() -> Self {
        Self::new(PLAYER_FIRE_INTERVAL, PLAYER_BULLET_SPEED, PLAYER_BULLET_DAMAGE)
    }

    pub fn with_spread(mut self, shots: u32, spread: f32) -> Self {
        self.shots = shots;
        self.spread = spread;
        self
    }

    // Start partway through the cooldown, so enemies don't fire in sync
    pub fn with_random_delay(mut self) -> Self  {
        self.cooldown.reset();
        let head_start = self.cooldown.duration().mul_f32(fastrand::f32());
        self.cooldown.tick(head_start);
        self
    }

    /// Advances the cooldown and reports if gun can fire
    fn ready(&mut self, delta: Duration) -> bool {
        self.cooldown.tick(delta).is_finished()
    }
}

/// A bullet in flight is moved by Avian `LinearVelocity`
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Projectile {
    pub damage: f32,
}

// But who may have fired said shot?
#[derive(Clone, Copy)]
enum Shooter {
    Player,
    Enemy,
}

/// Gives new fledgling (just spawned) enemies a gun
/// Squares don't get shit, they just ram things
pub fn arm_new_enemies(
    mut commands: Commands,
    new_enemies: Query<(Entity, &EnemyKind), Added<EnemyKind>>,
) {
    for (entity, kind) in &new_enemies {
        let gun = match kind {
            EnemyKind::Square => continue,
            EnemyKind::Triangle => { Gun::new(TRI_FIRE_INTERVAL, TRI_BULLET_SPEED, TRI_BULLET_DAMAGE) },
            EnemyKind::Donut => { Gun::new(DONUT_FIRE_INTERVAL, DONUT_BULLET_SPEED, DONUT_BULLET_DAMAGE).with_spread(DONUT_SHOTS, DONUT_SPREAD) }
        };

        commands.entity(entity).insert(gun.with_random_delay());
    }
}

// Fire straight up while `F` is held, as fast as cooldown allows.
// Dead players can't shoot
fn player_shoot(
    mut commands: Commands,
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    assets: Res<ProjectileAssets>,
    mut player: Query<(&Transform, &mut Gun), (With<Player>, Without<IsDead>)>,
) {
    let Ok((transform, mut gun)) = player.single_mut() else {
        return;
    };
    
    // Tick every frame even when not firing to keep cooldown running
    let ready = gun.ready(time.delta());
    if !ready || !input.pressed(KeyCode::KeyB) {
        return;
    };
    
    let origin = transform.translation.xy();
    fire(&mut commands, &assets, &mut gun, origin, Vec2::Y, Shooter::Player);
}

/// Every armed enemy that is on screen fires at the player whe its gun is ready.
/// once player is dead, no reason to shoot
fn enemy_shoot(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<ProjectileAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
    player: Query<&Transform, (With<Player>, Without<IsDead>)>,
    mut shooters: Query<(&Transform, &mut Gun), (Without<Player>, Without<IsDead>)>,
) {
    let Ok(player_transform) = player.single() else { return; };
    
    let target = player_transform.translation.xy();
    let top = window.height() / 2.0;
    
    for (transform, mut gun) in & mut shooters {
        let pos = transform.translation.xy();
        // enemies spawn above the window. Don't shoot or run cooldown
        // until they be visible.
        if pos.y > top {
            continue;
        }
        if !gun.ready(time.delta()) {
            continue;
        }
        
        let aim = (target - pos).try_normalize().unwrap_or(Vec2::NEG_Y);
        fire(&mut commands, &assets, &mut gun, pos, aim, Shooter::Enemy);
    }
}

/// Spawns a shots worth of bullets, then restarts cooldown
fn fire(
    commands: &mut Commands,
    assets: &ProjectileAssets,
    gun: &mut Gun,
    origin: Vec2,
    aim: Vec2,
    shooter: Shooter,
) {
    gun.cooldown.reset();
    
    let (mesh, material, radius, layers) = match shooter {
        Shooter::Player => (
            &assets.player_mesh,
            &assets.player_material,
            PLAYER_BULLET_RADIUS,
            GameLayer::player_bullet_layers(),
        ),
        Shooter::Enemy => (
            &assets.enemy_mesh,
            &assets.enemy_material,
            ENEMY_BULLET_RADIUS,
            GameLayer::enemy_bullet_layers(),
        )
    };
    
    let shots = gun.shots.max(1);
    for i in 0..shots{
        // Spread out shots evently around `aim`. For 3 shots offests are -spread, 0, +spread.
        let offset = (i as f32 - (shots - 1) as f32 / 2.0) * gun.spread;
        let direction = Vec2::from_angle(offset).rotate(aim);
        let position = origin + direction * MUZZLE_OFFSET;
        let despawn = DespawnOnExit(Screen::Gameplay);
        
        commands.spawn((
            Name::new("Projectile"),
            Projectile {damage: gun.damage},
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_translation(position.extend(BULLET_Z)),
            // This is different, it is `Dynamic` so avian always checks it against
            // kinematic enemies and the dynamic player, as a sensor, it never pushes: only reports.
            RigidBody::Dynamic,
            Collider::circle(radius),
            Sensor,
            GravityScale(0.0),
            LinearVelocity(direction * gun.bullet_speed),
            layers,
            CollisionEventsEnabled,
            despawn
        ));
    }
}

/// Removes those pesky bullets that fly off screen
fn despawn_offscreen_projectiles(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    projectiles: Query<(Entity, &Transform), With<Projectile>>,
) {
    let bounds = window.size() / 2.0 + OFFSCREEN_MARGIN;
    for (entity, transform) in &projectiles {
        let pos = transform.translation.xy();
        if pos.x.abs() > bounds.x || pos.y.abs() > bounds.y {
            commands.entity(entity).despawn();
        }
    }
}

//// Applies bullet damage from Avian's collision messages.
fn projectile_hits(
    mut commands: Commands,
    mut collisions: MessageReader<CollisionStart>,
    projectiles: Query<&Projectile>,
    mut targets: Query<&mut Health, Without<IsDead>>,
) {
    // A bullet can touch two things in one physics step. Only its first hit counts.
    let mut spent = HashSet::new();
 
    for event in collisions.read() {
        // Work out which side of the pair is the bullet.
        let (bullet, target) = if projectiles.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if projectiles.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue; // Not a bullet collision.
        };
        let Ok(projectile) = projectiles.get(bullet) else {
            continue;
        };
        if !spent.insert(bullet) {
            continue;
        }
        commands.entity(bullet).despawn();
 
        // Something with no health (like a wall, if you let bullets hit walls)
        // just absorbs the bullet. Already-dead targets are skipped by the query.
        let Ok(mut health) = targets.get_mut(target) else {
            continue;
        };
        // Already brought to zero by an earlier bullet this frame.
        if !health.is_alive() {
            continue;
        }
        health.take_damage(projectile.damage);
    }
}
 
/// Bullet meshes and colors, built in code like `EnemyAssets`.
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct ProjectileAssets {
    // Built with Assets::add, not loaded from disk, so no #[dependency].
    player_mesh: Handle<Mesh>,
    player_material: Handle<ColorMaterial>,
    enemy_mesh: Handle<Mesh>,
    enemy_material: Handle<ColorMaterial>,
}
 
impl FromWorld for ProjectileAssets {
    fn from_world(world: &mut World) -> Self {
        let (player_mesh, enemy_mesh) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Circle::new(PLAYER_BULLET_RADIUS)),
                meshes.add(Circle::new(ENEMY_BULLET_RADIUS)),
            )
        };
        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        Self {
            player_mesh,
            player_material: materials.add(ColorMaterial::from_color(WHITE)),
            enemy_mesh,
            enemy_material: materials.add(ColorMaterial::from_color(HOT_PINK)),
        }
    }
}