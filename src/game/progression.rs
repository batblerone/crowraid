//! Level progression: watches the score and decides when a boss should appear.
//!
//! The boss doesn't exist yet, for now this module only keeps the accounting and
//! sends the appropriate [`BossThresholdReached`] message once per level. A boss
//! spawner can be dropped in later without touching any of the score code.

use bevy::prelude::*;

use crate::{
    game::score::{Score, add_points_for_kills},
    game_manager::{Screen, GamePhase},
};

/// Points needed before the first boss shows up
const FIRST_BOSS_AT: u32 = 500;
/// After a boss is beaten, the next one needs this many more points
/// TODO: add more progression later
const POINTS_BETWEEN_BOSSES: u32 = 750;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LevelProgress>();
    app.add_message::<BossThresholdReached>();

    // Reset by `start_run` in game_manager.rs, which knows about Continue.
    app.add_systems(
        Update,
        check_boss_threshold
            .run_if(resource_changed::<Score>)
            .run_if(in_state(GamePhase::Playing))
            // Look at score after this frames kills have been processed
            .after(add_points_for_kills)
            .run_if(in_state(Screen::Gameplay))
    );
}

/// Where the player is in the level progression
#[derive(Resource, Debug)]
pub struct LevelProgress {
    /// Starts at 1, goes up each time a boss is beaten
    pub level: u32,
    /// jthe score that summons this level's boss
    pub boss_threshold: u32,
    /// True from the moment the threshold is hit until the boss is beaten.
    /// Stops the same boss being summoned again while points keep coming in.
    pub boss_active: bool,
}

impl Default for LevelProgress {
    fn default() -> Self {
        Self {
            level: 1,
            boss_threshold: FIRST_BOSS_AT,
            boss_active: false,
        }
    }
}

impl LevelProgress {
    /// Call this when the boss dies. The next threshold is counted from the
    /// score at the moment it is called. A big boss reward can't instantly
    /// summon the next boss.
    pub fn advance(&mut self, current_score: u32) {
        self.level += 1;
        self.boss_threshold = current_score.saturating_add(POINTS_BETWEEN_BOSSES);
        self.boss_active = false;
    }

    /// Constructor for continued runs
    pub fn starting_at(level: u32, score: u32) -> Self {
        Self {
            level,
            boss_threshold: if level <= 1 {
                FIRST_BOSS_AT
            } else {
                score.saturating_add(POINTS_BETWEEN_BOSSES)
            },
            boss_active: false,
        }
    }
    
    /// 0.0 on level 1, rising 0.15 per level, capped at 1.0 (level 7+).
    pub fn difficulty(&self) -> f32 {
            (self.level.saturating_sub(1) as f32 * 0.15).min(1.0)
    }
}

/// jsent once when the score reaches the current level's boss threshold.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossThresholdReached {
    pub level: u32,
}

pub fn check_boss_threshold(
    score: Res<Score>,
    mut progress: ResMut<LevelProgress>,
    mut reached: MessageWriter<BossThresholdReached>,
) {
    // `>=`, not `==`: kills are worth 10, 25, or 50, so the score usually
    // jump over the exact threshold rather than lands
    if progress.boss_active || score.current < progress.boss_threshold {
        return;
    }

    progress.boss_active = true;
    reached.write(BossThresholdReached {
        level: progress.level,
    });
}
