//! The screen state for the main gameplay.

use avian2d::prelude::*;
use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::{
    game::{
        boss::Boss,
        level::*,
        physics::*,
        pickup::Pickup,
        player::*,
        score::{EnemyDefeated, PointValue},
    },
    game_manager::{GamePhase, Menu, PausableSystems, Pause, PlayerDied, RunSetup},
    screens::Screen,
};

/// How fast (px/s) things are to get shoved appart
const BOUNCE_SPEED: f32 = 600.0;
/// Seconds of invulnerability to contact damage after a hit
const HURT_COOLDOWN: f32 = 0.5;

const HEALTHBAR_WIDTH: f32 = 50.0;
const HEALTHBAR_HEIGHT: f32 = 6.0;
const HEALTHBAR_Y_OFFSET: f32 = 43.0;
const HEALTHBAR_Z_OFFSET: f32 = 1.0;
/// Small z bump so the foreground always renders on top of the background.
const HEALTHBAR_FG_Z_BUMP: f32 = 0.01;
/// Marker: this entity is the colored fill of a healthbar.
#[derive(Component)]
pub struct HealthBarForeground;
/// Links a healthbar entity back to its owner character.
#[derive(Component)]
pub struct HealthBarOwner(pub Entity);

/// A health component for damageable entities
#[derive(Component, Reflect, Clone)]
#[reflect(Component)]
#[require(Knockback, HurtCooldown)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

/// Just a marker for when players die
#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct IsDead;

impl Default for Health {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    // Returns health as a ratio [0, 1]
    pub fn h_ratio(&self) -> f32 {
        self.current / self.max
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }
}

pub fn mark_dead(
    mut commands: Commands,
    query: Query<(Entity, &Health), (Changed<Health>, Without<IsDead>)>,
) {
    for (entity, health) in &query {
        if !health.is_alive() {
            commands.entity(entity).insert(IsDead);
        }
    }
}

/// Decides what happens when something dies.
///
/// This is the one place that confirms kills. Also means where kills are reported to score.
/// Enemies that leave the screen are despawned by `despawn_offscreen_enemies` instead and
/// never get here.
pub fn handle_deaths(
    mut commands: Commands,
    dead: Query<(Entity, Has<Player>, Option<&PointValue>, &Transform), Added<IsDead>>,
    mut defeated: MessageWriter<EnemyDefeated>,
    mut player_died: MessageWriter<PlayerDied>,
) {
    for (entity, is_player, points, transform) in &dead {
        let position = transform.translation.xy();
        if is_player {
            player_died.write(PlayerDied { position });
        } else if let Some(points) = points {
            defeated.write(EnemyDefeated {
                points: points.0,
                position,
            });
        }

        commands.entity(entity).despawn();
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Gameplay), spawn_level.after(RunSetup));

    // Toggle pause on key press.
    app.add_systems(
        Update,
        (
            (pause, spawn_pause_overlay, open_pause_menu).run_if(
                in_state(Screen::Gameplay)
                    .and_then(in_state(Menu::None))
                    .and_then(
                        input_just_pressed(KeyCode::KeyP)
                            .or_else(input_just_pressed(KeyCode::Escape)),
                    )
                    .and_then(not(in_state(GamePhase::GameOver))),
            ),
            close_menu.run_if(
                in_state(Screen::Gameplay)
                    .and_then(not(in_state(Menu::None)))
                    .and_then(input_just_pressed(KeyCode::KeyP)),
            ),
            collision_damage_and_bounce.in_set(PausableSystems),
            spawn_healthbars,
            update_healthbars,
        ),
    );
    app.add_systems(OnExit(Screen::Gameplay), (close_menu, unpause));
    app.add_systems(
        OnEnter(Menu::None),
        unpause.run_if(in_state(Screen::Gameplay)),
    );
    app.add_systems(
        PostUpdate,
        (mark_dead, handle_deaths)
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );
}

fn unpause(mut next_pause: ResMut<NextState<Pause>>) {
    next_pause.set(Pause(false));
}

fn pause(mut next_pause: ResMut<NextState<Pause>>) {
    next_pause.set(Pause(true));
}

fn spawn_pause_overlay(mut commands: Commands) {
    commands.spawn((
        Name::new("Pause Overlay"),
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        GlobalZIndex(1),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
        DespawnOnExit(Pause(true)),
    ));
}

fn open_pause_menu(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Pause);
}

fn close_menu(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::None);
}

/// Any collision damages, bounces, and any side that contains `Health`
///
/// player vs wall: Player hurt and bounces off wall
/// Player vs Enemy: Player and Enemy hurt, both bounce appart
/// Which pairs that collide is still determined by `GameLayer`
/// and defined collision masks
/// Bullets are sensors, handled by `projectile_hits` instead.
pub fn collision_damage_and_bounce(
    mut collision_reader: MessageReader<CollisionStart>,
    collisions: Collisions,
    mut bodies: Query<
        (
            &mut Health,
            &mut Knockback,
            &mut HurtCooldown,
            Has<Invincible>,
        ),
        Without<IsDead>,
    >,
    contact_damage: Query<&ContactDamage>,
    transforms: Query<&GlobalTransform>,
    sensors: Query<(), With<Sensor>>,
) {
    for event in collision_reader.read() {
        let (a, b) = (event.collider1, event.collider2);

        // Sensors (the pewpew) only report overlaps, don't actually bump things
        if sensors.contains(a) || sensors.contains(b) {
            continue;
        }

        // We run logic from each side's point of view, so if both
        // have `Health` both get hurt and bounce
        for (me, other) in [(a, b), (b, a)] {
            let Ok((mut health, mut knockback, mut cooldown, invincible)) = bodies.get_mut(me)
            else {
                continue; // For when `me` has no `health` (is wall, is dead)
            };

            // Bounces every time, even while invulnerable so no stick to walls
            let away = push_direction(&collisions, &transforms, me, other);
            knockback.0 = away * BOUNCE_SPEED;

            if cooldown.0 > 0.0 || invincible {
                continue;
            };

            let damage = contact_damage
                .get(other)
                .map_or(DEFAULT_CONTACT_DAMAGE, |d| d.0);
            health.take_damage(damage);
            cooldown.0 = HURT_COOLDOWN;
        }
    }
}

/// Finds the unit direction that pushes `me` away from `other`
///
/// Prefers the real contact normal form Avian (correct for long walls and odd shapes)
/// falls back to "Away from other center"
fn push_direction(
    collisions: &Collisions,
    transforms: &Query<&GlobalTransform>,
    me: Entity,
    other: Entity,
) -> Vec2 {
    if let Some(pair) = collisions.get(me, other)
        && let Some(manifold) = pair.manifolds.first()
    {
        // Avians normal points form Collider1 to Collider2.
        // Lets get it pointing *towards* `me`, away from `other`
        return if pair.collider1 == me {
            -manifold.normal
        } else {
            manifold.normal
        };
    }

    let (Ok(mine), Ok(theirs)) = (transforms.get(me), transforms.get(other)) else {
        return Vec2::ZERO;
    };

    (mine.translation().xy() - theirs.translation().xy()).normalize_or_zero()
}

/// Spawns a background + foreground healthbar pair for each entity that gains Health.
pub fn spawn_healthbars(
    mut commands: Commands,
    new_health: Query<
        (Entity, &GlobalTransform, &Health),
        (Without<Pickup>, Without<Boss>, Added<Health>),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (owner, transform, health) in &new_health {
        let pos = transform.translation();
        let bg_pos = Vec3::new(
            pos.x,
            pos.y + HEALTHBAR_Y_OFFSET,
            pos.z + HEALTHBAR_Z_OFFSET,
        );
        let fg_pos = Vec3::new(
            pos.x,
            pos.y + HEALTHBAR_Y_OFFSET,
            pos.z + HEALTHBAR_Z_OFFSET + HEALTHBAR_FG_Z_BUMP,
        );
        // Background: dark gray
        let bg_mesh = meshes.add(Rectangle::new(HEALTHBAR_WIDTH, HEALTHBAR_HEIGHT));
        let bg_mat = materials.add(ColorMaterial::from(Color::srgb(0.2, 0.2, 0.2)));
        commands.spawn((
            Mesh2d(bg_mesh),
            MeshMaterial2d(bg_mat),
            Transform::from_translation(bg_pos),
            HealthBarOwner(owner),
        ));
        // Foreground: color derived from actual health ratio
        let ratio = health.h_ratio();
        let fg_mesh = meshes.add(Rectangle::new(HEALTHBAR_WIDTH, HEALTHBAR_HEIGHT));
        let fg_mat = materials.add(ColorMaterial::from(health_color(ratio)));
        commands.spawn((
            Mesh2d(fg_mesh),
            MeshMaterial2d(fg_mat),
            Transform::from_translation(fg_pos).with_scale(Vec3::new(ratio.max(0.001), 1.0, 1.0)),
            HealthBarOwner(owner),
            HealthBarForeground,
        ));
    }
}

/// Each frame: repositions bars to follow owner, updates foreground width and color.
/// Despawns bars whose owner no longer exists.
pub fn update_healthbars(
    mut commands: Commands,
    mut bars: Query<(
        Entity,
        &HealthBarOwner,
        &mut Transform,
        Has<HealthBarForeground>,
        &MeshMaterial2d<ColorMaterial>,
    )>,
    owners: Query<(&GlobalTransform, &Health)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (bar_entity, owner_ref, mut transform, is_foreground, mat_handle) in bars.iter_mut() {
        let Ok((owner_transform, health)) = owners.get(owner_ref.0) else {
            // If the owner is gone (despawned), clean up the health bar
            commands.entity(bar_entity).despawn();
            continue;
        };
        let owner_pos = owner_transform.translation();
        let ratio = health.h_ratio();
        if is_foreground {
            // Scale foreground width to match health ratio
            transform.scale.x = ratio.max(0.001);
            // Reposition foreground to stay left-aligned as it shrinks
            // (Scaling happens from center, so we need to offset position)
            transform.translation = Vec3::new(
                owner_pos.x - (HEALTHBAR_WIDTH * (1.0 - ratio) / 2.0),
                owner_pos.y + HEALTHBAR_Y_OFFSET,
                owner_pos.z + HEALTHBAR_Z_OFFSET + HEALTHBAR_FG_Z_BUMP,
            );
            // Update color (Green -> Yellow -> Red)
            if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
                mat.color = health_color(ratio);
            }
        } else {
            // Background simply stays centered above the owner
            transform.translation = Vec3::new(
                owner_pos.x,
                owner_pos.y + HEALTHBAR_Y_OFFSET,
                owner_pos.z + HEALTHBAR_Z_OFFSET,
            );
        }
    }
}
/// Green → Yellow → Red, continuous at ratio = 0.5.
fn health_color(ratio: f32) -> Color {
    if ratio >= 0.5 {
        let t = (ratio - 0.5) * 2.0; // 1.0 at full, 0.0 at half
        Color::srgb(1.0 - t * 0.8, 0.8, 0.2)
    } else {
        let t = ratio * 2.0; // 1.0 at half, 0.0 at empty
        Color::srgb(1.0, t * 0.8, 0.2)
    }
}
