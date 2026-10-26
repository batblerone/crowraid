//! Score: the points the player earns by defeating enemies, and the HUD
//! readout in the top-left corner that shows them.
//!
//! Points only ever enter the score through [`EnemyDefeated`] messages, and
//! the only thing that sends one is `handle_deaths` (in `screens/gameplay.rs`)
//! when something carrying a [`PointValue`] is killed. Enemies that fly off
//! the bottom of the screen are despawned without ever being marked `IsDead`,
//! so they never award anything.

use bevy::prelude::*;

use crate::{game_manager::Screen, theme::widget};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Score>();
    app.add_message::<EnemyDefeated>();

    app.add_systems(OnEnter(Screen::Gameplay), (reset_score, spawn_score_hud));
    app.add_systems(
        Update,
        (
            add_points_for_kills,
            update_score_hud.run_if(resource_changed::<Score>),
        )
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );
}

/// How many points the player earns for defeating an entity.
///
/// It lives within the entity (instead of being looked up from `EnemyKind`)
/// so that anything can be worth points: Such as tougher variants, or bosses,
/// without score code knowing about them.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct PointValue(pub u32);

/// Sent once for every enemy the player defeats.
#[derive(Message, Debug, Clone, Copy)]
pub struct EnemyDefeated {
    /// Points that each kill is worth
    pub points: u32,
    /// Where the enemy died. For use later for pop-ups, or effects (+25 pts).
    #[allow(dead_code)] // Not in use yet
    pub position: Vec2,
}

/// The players actual score.
#[derive(Resource, Debug, Default)]
pub struct Score {
    /// Points earned this run
    pub current: u32,
    /// Highest score reached since game start
    pub best_score: u32,
}

impl Score {
    pub fn add(&mut self, points: u32) {
        // saturating_add can'd overflow and panic, however logn a run lasts.
        self.current = self.current.saturating_add(points);
        self.best_score = self.best_score.max(self.current);
    }
}

/// Every run starts from zero. `best` is kept.
pub fn reset_score(mut score: ResMut<Score>) {
    score.current = 0;
}

/// Turns kill messages into points
pub fn add_points_for_kills(mut defeated: MessageReader<EnemyDefeated>, mut score: ResMut<Score>) {
    for kill in defeated.read() {
        score.add(kill.points);
    }
}

/// Marks the text node that shows the score
#[derive(Component)]
struct ScoreText;

pub fn spawn_score_hud(mut commands: Commands) {
    commands.spawn((
        Name::new("Score HUD"),
        Node {
            position_type: PositionType::Absolute,
            left: px(16),
            top: px(16),
            ..default()
        },
        // Clicks are to go thorugh the hud
        Pickable::IGNORE,
        DespawnOnExit(Screen::Gameplay),
        children![(widget::label(score_label(0)), ScoreText)],
    ));
}

/// Only runs on frames where `Score` actually changed
pub fn update_score_hud(score: Res<Score>, mut texts: Query<&mut Text, With<ScoreText>>) {
    for mut text in &mut texts {
        text.0 = score_label(score.current);
    }
}

pub fn score_label(points: u32) -> String {
    format!("Score: {points}")
}
