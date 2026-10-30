//! Pickups are interactable items that crows can gather on their flight
//!
//! Bird seed (energy)  as well as berries (adds health), and a rare
//! golden feather (extra life).

use std::collections::HashSet;

use avian2d::prelude::*;
use bevy::{color::palettes::css::*, prelude::*};

use crate::{
    AppSystems, PausableSystems,
    asset_tracking::LoadResource,
    game::{energy::Energy, physics::GameLayer, player::Player, score::PointValue},
    game_manager::{Lives, MAX_LIVES, Screen},
    screens::gameplay::Health,
};

const SEED_ENERGY: f32 = 35.0;
const BERRY_HEALTH: f32 = 30.0;
const PICKUP_RADIUS: f32 = 14.0;
const PICKUP_Z: f32 = 1.5;
/// Rows of floor between pickups (a row is `SEGMENT_HEIGHT pixels tall).
const MIN_ROWS_BETWEEN: u32 = 120;
const MAX_ROWS_BETWEEN: u32 = 260;
/// Keep pickups this far from sides
const BANK_MARGIN: f32 = 40.0;

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<PickupAssets>();
    app.add_message::<PickupCollected>();
    app.add_systems(
        Update,
        collect_pickups
            .run_if(in_state(Screen::Gameplay))
            .in_set(AppSystems::Update)
            .in_set(PausableSystems),
    );
}

#[derive(Component, Reflect, Copy, Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Component)]
pub enum PickupKind {
    #[default]
    BirdSeed,
    Berry,
    GoldenFeather,
}

impl PickupKind {
    pub fn random() -> Self {
        match fastrand::u32(0..100) {
            0..85 => Self::BirdSeed,
            85..97 => Self::Berry,
            _ => Self::GoldenFeather,
        }
    }
}

#[derive(Component, Reflect, Copy, Clone, Debug, Default)]
#[reflect(Component)]
pub struct Pickup(pub PickupKind);

/// For sound effects, Hud flashes, and "+##" pop-ups
#[derive(Message, Debug, Clone, Copy)]
pub struct PickupCollected {
    #[allow(dead_code)]
    pub kind: PickupKind,
}

/// Counts down to the next pickup. Lives on generator config.
#[derive(Debug, Clone, Copy)]
pub struct PickupSpacing {
    rows_left: u32,
}

impl Default for PickupSpacing {
    fn default() -> Self {
        Self {
            rows_left: fastrand::u32(MIN_ROWS_BETWEEN..MAX_ROWS_BETWEEN),
        }
    }
}

impl PickupSpacing {
    /// Call once per generated row with that rows's open floor spans
    /// (left, right) in world x. Returns a local x to place a pickup at.
    pub fn next_row(&mut self, floor: &[(f32, f32)]) -> Option<f32> {
        self.rows_left = self.rows_left.saturating_sub(1);
        if self.rows_left > 0 {
            return None;
        }
        // Use widest seciton of channel, so seed is reachable and visible
        let (left, right) = floor
            .iter()
            .copied()
            .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))?;
        if right - left < BANK_MARGIN * 2.0 + PICKUP_RADIUS * 2.0 {
            return None; // it is too narrow, try again elsewhere
        }
        *self = Self::default();
        let lo = left + BANK_MARGIN;
        let hi = right - BANK_MARGIN;
        Some(lo + fastrand::f32() * (hi - lo))
    }
}

/// A pickup, positioned relative to the channel segments it is spawned under
pub fn pickup(kind: PickupKind, x: f32, assets: &PickupAssets) -> impl Bundle {
    (
        Name::new("Pickup"),
        Pickup(kind),
        Health::new(1.0),
        PointValue(15),
        Mesh2d(assets.mesh.clone()),
        MeshMaterial2d(assets.material(kind)),
        Transform::from_xyz(x, 0.0, PICKUP_Z),
        RigidBody::Static,
        Collider::circle(PICKUP_RADIUS),
        Sensor,
        GameLayer::pickup_layers(),
        CollisionEventsEnabled,
        DespawnOnExit(Screen::Gameplay),
    )
}

fn collect_pickups(
    mut commands: Commands,
    mut starts: MessageReader<CollisionStart>,
    pickups: Query<&Pickup>,
    mut crows: Query<(&mut Energy, &mut Health), With<Player>>,
    mut lives: ResMut<Lives>,
    mut collected: MessageWriter<PickupCollected>,
) {
    let mut used = HashSet::new();
    for event in starts.read() {
        let (item, other) = if pickups.contains(event.collider1) {
            (event.collider1, event.collider2)
        } else if pickups.contains(event.collider2) {
            (event.collider2, event.collider1)
        } else {
            continue;
        };
        
        let Ok((mut energy, mut health)) = crows.get_mut(other) else {
            continue;
        };
        
        if !used.insert(item) {
            continue; // Crow already ate this frame
        }
        
        let Ok(&Pickup(kind)) = pickups.get(item) else {
            continue;
        };
        
        match kind {
            PickupKind::BirdSeed => energy.restore(SEED_ENERGY),
            PickupKind::Berry => health.heal(BERRY_HEALTH),
            PickupKind::GoldenFeather => lives.0 = (lives.0 + 1).min(MAX_LIVES),
        }
        
        collected.write(PickupCollected {
            kind
        });
        
        commands.entity(item).despawn();
    }
}

/// Placeholders for item visual for now, like `ProjectileAssets`
/// Swap out for sprite sheet later (and mark the handle #[dependency]).
#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
pub struct PickupAssets {
    mesh: Handle<Mesh>,
    seed: Handle<ColorMaterial>,
    berry: Handle<ColorMaterial>,
    feather: Handle<ColorMaterial>,
}

impl PickupAssets {
    fn material(&self, kind: PickupKind) -> Handle<ColorMaterial> {
        match kind {
            PickupKind::BirdSeed => self.seed.clone(),
            PickupKind::Berry => self.berry.clone(),
            PickupKind::GoldenFeather => self.feather.clone(),
        }
    }
}

impl FromWorld for PickupAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = world.resource_mut::<Assets<Mesh>>().add(Circle::new(PICKUP_RADIUS));
        let mut materials = world.resource_mut::<Assets<ColorMaterial>>();
        Self {
            mesh,
            seed: materials.add(ColorMaterial::from_color(WHEAT)),
            berry: materials.add(ColorMaterial::from_color(CRIMSON)),
            feather: materials.add(ColorMaterial::from_color(GOLD)),
        }
    }
}