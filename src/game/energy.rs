//! A crow needs energy to fly. They must pick up seeds to refuel as they fly.
//!
//! Energy drains throughout flight, faster when crows fly further up and climbs.
//! If a crow has no energy to fly, they lose health slowly.

use bevy::prelude::*;

use crate::{
    AppSystems, PausableSystems,
    game::{movement::MovementController, player::Player},
    game_manager::GamePhase,
    screens::gameplay::{Health, IsDead},
};

/// Flying up the screen (flapps hard) multiplies the drain by this amount
const CLIMB_DRAIN_MULTIPLIER: f32 = 2.0;
/// Health lost per second while energy is empty
const STARVING_DAMAGE_PER_SEC: f32 = 15.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (drain_energy, starve)
            .chain()
            // Not during the level-clear banner or respawn invincibility
            .run_if(in_state(GamePhase::Playing))
            .in_set(AppSystems::Update)
            .in_set(PausableSystems),
    );
}

#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct Energy {
    pub current: f32,
    pub max: f32,
    // energy lost while cruising normally
    pub drain_per_sec: f32,
}

impl Energy {
    pub fn new(max: f32, drain_per_sec: f32) -> Self {
        Self {
            current: max,
            max,
            drain_per_sec,
        }
    }
    
    pub fn restore(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }
    
    pub fn ratio(&self) -> f32 {
        if self.max > 0.0 { self.current / self.max } else { 0.0 }
    }
    
    pub fn is_empty(&self) -> bool {
        self.current <= 0.0
    }
}

fn drain_energy(
    time: Res<Time>,
    mut crows: Query<(&mut Energy, &MovementController), With<Player>>,
) {
    let dt = time.delta_secs();
    for (mut energy, controller) in &mut crows {
        let effort = if controller.intent.y > 0.0 { CLIMB_DRAIN_MULTIPLIER } else { 1.0 };
        energy.current = (energy.current - energy.drain_per_sec * effort * dt).max(0.0);
    }
}

fn starve(
    time: Res<Time>,
    mut crows: Query<(&Energy, &mut Health), (With<Player>, Without<IsDead>)>,
) {
    for (energy, mut health) in &mut crows {
        if energy.is_empty() {
            health.take_damage(STARVING_DAMAGE_PER_SEC * time.delta_secs());
        }
    }
}