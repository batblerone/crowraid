//! There is one boss per level, summoned when the score reaches a threshold
//! 
//! Beating a boss clears the level. Clearing the level initiates a checkpoint and saves 
//! progress.

use std::f32::consts::TAU;

use avian2d::prelude::*;
use bevy::{color::palettes::css::*, prelude::*, window::PrimaryWindow};

use crate::{
    AppSystems, PausableSystems,
    asset_tracking::LoadResource,
    game::{
        physics::{ContactDamage, GameLayer},
        progression::{BossThresholdReached, LevelProgress},
        projectile::Gun,
        score::PointValue,
    },
    game_manager::{BossDefeated, Screen},
    screens::gameplay::{Health, IsDead, handle_deaths, mark_dead},
};

const BOSS_SIZE: Vec2 = Vec2::new(180.0, 70.0);
const BOSS_BASE_HEALTH: f32 = 400.0;
const BOSS_HEALTH_PER_LEVEL: f32 = 200.0;
const BOSS_BASE_POINTS: u32 = 250;
const BOSS_CONTACT_DAMAGE: f32 = 40.0;
/// Where the boss settles, measured from the screen centre.
const BOSS_HOLD_Y: f32 = 200.0;
const BOSS_ENTRY_SPEED: f32 = 120.0;
const BOSS_SWAY_AMPLITUDE: f32 = 180.0;
const BOSS_SWAY_PERIOD: f32 = 4.0;
const BOSS_FIRE_INTERVAL: f32 = 1.0;
const BOSS_BULLET_SPEED: f32 = 350.0;
const BOSS_BULLET_DAMAGE: f32 = 10.0;
const BOSS_SPREAD: f32 = 0.22;

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<BossAssets>();
    app.add_systems(
        Update,
        (spawn_boss, move_boss)
            .chain()
            .run_if(in_state(Screen::Gameplay))
            .in_set(AppSystems::Update)
            .in_set(PausableSystems),
    );
    // Between "health hit zero" and "entity despawned", so Added<IsDead> is visible.
    app.add_systems(
        PostUpdate,
        report_boss_death
            .after(mark_dead)
            .before(handle_deaths)
            .run_if(in_state(Screen::Gameplay)),
    );
}

#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct Boss;

#[derive(Component, Debug, Clone, Copy)]
enum BossMotion {
    /// Gliding down from above the screen to BOSS_HOLD_Y.
    Entering,
    /// Swaying side to side; `time` drives the sine wave.
    Swaying { time: f32 },
}

fn spawn_boss(
    mut commands: Commands,
    mut reached: MessageReader<BossThresholdReached>,
    assets: Res<BossAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    for event in reached.read() {
        let level = event.level.max(1);
        let health = BOSS_BASE_HEALTH + BOSS_HEALTH_PER_LEVEL * (level - 1) as f32;
        // 3 bullets per volley at level 1, then 5, 7, capped at 9.
        let shots = 1 + 2 * level.min(4);
        let start = Vec3::new(0.0, window.height() * 0.5 + BOSS_SIZE.y, 1.0);

        commands.spawn((
            Name::new(format!("Boss (level {level})")),
            Boss,
            BossMotion::Entering,
            Health::new(health),
            PointValue(BOSS_BASE_POINTS * level),
            ContactDamage(BOSS_CONTACT_DAMAGE),
            // enemy_shoot already fires any Gun that isn't the player's.
            Gun::new(BOSS_FIRE_INTERVAL, BOSS_BULLET_SPEED, BOSS_BULLET_DAMAGE)
                .with_spread(shots, BOSS_SPREAD),
            Mesh2d(assets.mesh.clone()),
            MeshMaterial2d(assets.material.clone()),
            Transform::from_translation(start),
            RigidBody::Kinematic,
            Collider::rectangle(BOSS_SIZE.x, BOSS_SIZE.y),
            GameLayer::boss_layers(),
            CollisionEventsEnabled,
            DespawnOnExit(Screen::Gameplay),
        ));
    }
}

fn move_boss(
    time: Res<Time>,
    mut bosses: Query<(&mut BossMotion, &Transform, &mut LinearVelocity), With<Boss>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut motion, transform, mut velocity) in &mut bosses {
        let pos = transform.translation.xy();
        let target = match *motion {
            BossMotion::Entering => {
                let next = pos + Vec2::NEG_Y * BOSS_ENTRY_SPEED * dt;
                if next.y <= BOSS_HOLD_Y {
                    *motion = BossMotion::Swaying { time: 0.0 };
                    Vec2::new(next.x, BOSS_HOLD_Y)
                } else {
                    next
                }
            }
            BossMotion::Swaying { time } => {
                let t = time + dt;
                *motion = BossMotion::Swaying { time: t };
                let x = (t / BOSS_SWAY_PERIOD * TAU).sin() * BOSS_SWAY_AMPLITUDE;
                let bob = (t * 1.3).sin() * 20.0;
                Vec2::new(x, BOSS_HOLD_Y + bob)
            }
        };
        // Same trick as move_enemies: drive the kinematic body by velocity.
        velocity.0 = (target - pos) / dt;
    }
}

fn report_boss_death(
    dead: Query<(), (With<Boss>, Added<IsDead>)>,
    progress: Res<LevelProgress>,
    mut defeated: MessageWriter<BossDefeated>,
) {
    if !dead.is_empty() {
        defeated.write(BossDefeated { level: progress.level });
    }
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct BossAssets {
    mesh: Handle<Mesh>,
    material: Handle<ColorMaterial>,
}

impl FromWorld for BossAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Rectangle::new(BOSS_SIZE.x, BOSS_SIZE.y));
        let material = world
            .resource_mut::<Assets<ColorMaterial>>()
            .add(ColorMaterial::from_color(SADDLE_BROWN));
        Self { mesh, material }
    }
}