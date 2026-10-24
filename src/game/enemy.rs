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

use avian2d::collision::collider::Collider;
use bevy::window::PrimaryWindow;
use bevy::{color::palettes::css::*, prelude::*};
use avian2d::prelude::*;

use crate::AppSystems;
use crate::asset_tracking::LoadResource;
use crate::game::player::Player;
use crate::game_manager::{PausableSystems, Screen};
use crate::screens::gameplay::{Health, IsDead};
use crate::game::physics::*;

/// New enemies will appear this far above the screen.
const SPAWN_Y_MARGIN: f32 = 64.0;
/// The longest gap between an enemy spawn, the real gap is random
const MAX_ENEMY_GAP: f32 = 10.0; // Seconds
/// We want to keep spawns within the wall boundries (Half channel width: 400, +/- 150)
const SPAWN_X_RANGE: f32 = 200.0;
/// Margin for despawning enemies
const OFFSCREEN_MARGIN: f32 = 128.0;

/// Constants for the squarees
const SQUARE_SPEED: f32 = 100.0;
const SQUARE_HEALTH: f32 = 10.0;
const SQUARE_SCALE: f32 = 1.0;

/// Constants for the triangles
const TRI_SPEED: f32 = 300.0;
const TRI_HEALTH: f32 = 50.0;
// How fast a triangle can turn it's direciton, in radians/second
const TRI_TURN_RATE: f32 = 2.0;
const TRI_SCALE: f32 = 1.0;

/// Constants for the donuts
const DONUT_SPEED: f32 = 500.0;
const DONUT_HEALTH: f32 = 80.0;
// How fast a donuts loop center sinks down the screen
const DONUT_DECENT: f32 = 120.0;
// Each donut will get a random lop size within this range
const DONUT_MIN_RADIUS: f32 = 60.0;
const DONUT_MAX_RADIUS: f32 = 120.0;
const DONUT_SCALE: f32 = 1.0;

pub fn plugin(app: &mut App) {
    app.load_resource::<EnemyAssets>();
    app.add_systems(OnEnter(Screen::Gameplay), reset_enemy_timer);
    app.add_systems(
        Update,
        (spawn_enemies, move_enemies, despawn_offscreen_enemies)
            .chain()
            .run_if(in_state(Screen::Gameplay))
            .in_set(AppSystems::Update)
            .in_set(PausableSystems),
    );
}

#[derive(Resource)]
pub struct EnemyTimer(Timer);

fn random_enemy_gap() -> Timer {
    Timer::from_seconds(
        0.4 + fastrand::f32() * (MAX_ENEMY_GAP - 0.5),
        TimerMode::Once,
    )
}

fn reset_enemy_timer(mut commands: Commands) {
    commands.insert_resource(EnemyTimer(random_enemy_gap()));
}

/// A component to define how / where enemies are to move
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub enum EnemyMotion {
    /// Moves Straight at constant speed
    Straight { speed: f32 },
    /// Steers towards player, turing at a 'turn_rate' of radians per second.
    Homing {
        velocity: Vec2,
        turn_rate: f32,
        committed: bool, /* Has it locked on */
    },
    /// Circles an anchor point, where the anchor sinks down the screen
    Looping {
        anchor: Vec2,
        phase: f32,
        radius: f32,
        angular_speed: f32,
        descent: f32,
    },
}

impl Default for EnemyMotion {
    fn default() -> Self {
        Self::Straight { speed: 0.0 }
    }
}

/// A constructor is used to define each enemy kind motion starting state.
impl EnemyMotion {
    pub fn for_kind(kind: EnemyKind, spawn: Vec2) -> Self {
        let speed = kind.enemy_stats().speed;
        match kind {
            EnemyKind::Square => Self::Straight { speed },
            EnemyKind::Triangle => Self::Homing {
                velocity: Vec2::NEG_Y * speed,
                turn_rate: TRI_TURN_RATE,
                committed: false,
            },
            EnemyKind::Donut => {
                let radius =
                    DONUT_MIN_RADIUS + fastrand::f32() * (DONUT_MAX_RADIUS - DONUT_MIN_RADIUS);
                let phase = fastrand::f32() * TAU;
                // Choose either clockwise or counter randomly
                let direction = if fastrand::bool() { 1.0 } else { -1.0 };
                Self::Looping {
                    // Put the anchor where the donut is already on it's circle,
                    // so there is no jumping in first frame.
                    anchor: spawn - Vec2::from_angle(phase) * radius,
                    phase,
                    radius,
                    angular_speed: direction * speed / radius,
                    descent: DONUT_DECENT,
                }
            }
        }
    }
}

// The marker for enemies
#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct Enemy;

// A match struct for enemy stats to build within scenes
pub struct EnemyStats {
    pub health: f32,
    pub speed: f32,
    #[allow(dead_code)] // This will come into play once projectiles are set up.
    pub score: u32,
    pub scale: f32,
}

/// From here on we are defining and implementing the assets for the enemies
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
                health: SQUARE_HEALTH,
                speed: SQUARE_SPEED,
                score: 0,
                scale: SQUARE_SCALE,
            },
            EnemyKind::Triangle => EnemyStats {
                health: TRI_HEALTH,
                speed: TRI_SPEED,
                score: 0,
                scale: TRI_SCALE,
            },
            EnemyKind::Donut => EnemyStats {
                health: DONUT_HEALTH,
                speed: DONUT_SPEED,
                score: 0,
                scale: DONUT_SCALE,
            },
        }
    }
    
    pub fn enemy_collider(self) -> Collider {
        match self {
            EnemyKind::Square => Collider::rectangle(32.0, 32.0),
            EnemyKind::Triangle => Collider::triangle(
                Vec2::new(0.0, 20.0),
                Vec2::new(-18.0, -14.0),
                Vec2::new(18.0, -14.0),
            ),
            EnemyKind::Donut => Collider::circle(30.0),
        }
    }
}

fn pick_enemy_kind() -> EnemyKind {
    match fastrand::u32(0..100) {
        0..40 => EnemyKind::Square,
        40..75 => EnemyKind::Triangle,
        _ => EnemyKind::Donut,
    }
}

/// The scene definition of an enemy for our game, shared between all enemies within the game
fn enemy_base(kind: EnemyKind) -> impl Scene {
    let health = kind.enemy_stats().health;
    bsn! {
        Enemy
        kind
        Health { current: health, max: health }
    }
}

/// One of any given kind of enemies
pub fn enemy(kind: EnemyKind, position: Vec2, assets: &EnemyAssets) -> impl Scene {
    let EnemyVisual { mesh, material } = assets.enemy_visuals(kind).clone();
    let transform = Transform::from_translation(position.extend(1.0))
        .with_scale(Vec3::splat(kind.enemy_stats().scale));
    let motion = EnemyMotion::for_kind(kind, position);
    let collider = kind.enemy_collider();
    let layers = GameLayer::enemy_layers();
    let despawn = DespawnOnExit(Screen::Gameplay);
    bsn! {
        @enemy_base(kind)
        Mesh2d(mesh)
        MeshMaterial2d::<ColorMaterial>(material)
        transform
        motion
        RigidBody::Kinematic
        collider
        layers
        CollisionEventsEnabled
        despawn
    }
}

pub fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    window: Single<&Window, With<PrimaryWindow>>,
    timer: Option<ResMut<EnemyTimer>>,
    player_query: Query<(), (With<Player>, Without<IsDead>)>,
) {
    let Some(mut timer) = timer else { return };
    if !timer.0.tick(time.delta()).is_finished() {
        return;
    }
    timer.0 = random_enemy_gap();
    
    if player_query.single().is_err() { return; };

    let kind = pick_enemy_kind();
    let x = (fastrand::f32() * 2.0 - 1.0) * SPAWN_X_RANGE;
    let y = window.height() / 2.0 + SPAWN_Y_MARGIN;
    commands.spawn_scene(enemy(kind, Vec2::new(x, y), &assets));
}

/// Move enemeies according to game scope
pub fn move_enemies(
    player: Query<&Transform, (With<Player>, Without<Enemy>, Without<IsDead>)>,
    time: Res<Time>,
    mut enemies: Query<(&mut EnemyMotion, &mut Transform, &mut LinearVelocity), With<Enemy>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 { return; }; // Don't divide by zero in first frame
    let player_pos = player.single().ok().map(|t| t.translation.xy());

    for (mut motion, mut transform, mut velocity) in &mut enemies {
        let pos = transform.translation.xy();

        let new_pos = match &mut *motion {
            EnemyMotion::Straight { speed } => pos + Vec2::NEG_Y * *speed * dt,
            EnemyMotion::Homing {
                velocity,
                turn_rate,
                committed,
            } => {
                if let Some(target) = player_pos.filter(|_| !*committed) {
                    if pos.y < target.y {
                        // Once the entity is beyond player, ignore it and fly down
                        *committed = true;
                    } else {
                        let max_turn = *turn_rate * dt;
                        let turn = velocity.angle_to(target - pos).clamp(-max_turn, max_turn);
                        *velocity = Vec2::from_angle(turn).rotate(*velocity);
                    }
                }
                // The mesh nose points up (+Y), so - 90 degrees to face along velocity
                transform.rotation = Quat::from_rotation_z(velocity.to_angle() - FRAC_PI_2);
                pos + *velocity * dt
            }

            EnemyMotion::Looping {
                anchor,
                phase,
                radius,
                angular_speed,
                descent,
            } => {
                anchor.y -= *descent * dt;
                *phase = (*phase + *angular_speed * dt).rem_euclid(TAU);
                *anchor + Vec2::from_angle(*phase) * *radius
            }
        };
        
        velocity.0 = (new_pos - pos) / dt;
    }
}

/// Removes enemy entities out of visible window
pub fn despawn_offscreen_enemies(
    mut commands: Commands,
    window: Single<&Window, With<PrimaryWindow>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    let half = window.size() / 2.0 + OFFSCREEN_MARGIN;
    for (entity, transform) in &enemies {
        let pos = transform.translation.xy();
        // only the bottom and sides are checked, enemys spawn top and move down
        if pos.y < -half.y || pos.x.abs() > half.x {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Clone, Reflect)]
pub struct EnemyVisual {
    pub mesh: Handle<Mesh>,
    pub material: Handle<ColorMaterial>,
}

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
                meshes.add(Annulus::new(10.0, 30.0)),
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
