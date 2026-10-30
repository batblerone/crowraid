//! Game manager controls the game state, score tracking,
//! save states and overall progress of the game.

use bevy::prelude::*;

use crate::game::{player::*, progression::*, score::*};

pub const STARTING_LIVES: u32 = 3;
pub const MAX_LIVES: u32 = 5;
pub const RESPAWN_DELAY: f32 = 1.5;
pub const LEVEL_CLEAR_DELAY: f32 = 3.0;

pub(super) fn plugin(app: &mut App) {
    // All states live within this plugin, it manages game flow
    app.init_state::<Screen>();
    app.init_state::<Menu>();
    app.init_state::<Pause>();
    app.init_state::<GamePhase>();

    app.init_resource::<Lives>();
    app.init_resource::<RunStart>();
    app.init_resource::<RunSeed>();
    app.add_message::<PlayerDied>();
    app.add_message::<BossDefeated>();

    app.add_systems(OnEnter(Screen::Gameplay), start_run.in_set(RunSetup));
    app.add_systems(OnEnter(GamePhase::Respawning), |mut c: Commands| {
        c.insert_resource(PhaseTimer(Timer::from_seconds(
            RESPAWN_DELAY,
            TimerMode::Once,
        )));
    });
    app.add_systems(OnEnter(GamePhase::LevelClear), |mut c: Commands| {
        c.insert_resource(PhaseTimer(Timer::from_seconds(
            LEVEL_CLEAR_DELAY,
            TimerMode::Once,
        )));
    });

    app.add_systems(
        Update,
        (
            // Death first, boss second: if both happen in one frame
            // the level still clears
            (on_player_died, on_boss_defeated).chain(),
            finish_respawn.run_if(in_state(GamePhase::Respawning)),
            finish_level_clear.run_if(in_state(GamePhase::LevelClear)),
        )
            .run_if(in_state(Screen::Gameplay))
            .in_set(PausableSystems),
    );
}

// ------ State Data for managing game flow ------

/// What is happening inside of a run. Only exists during `Screen::Gameplay`.
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(Screen = Screen::Gameplay)]
pub enum GamePhase {
    #[default]
    Playing,
    /// The crow died, but has lives left
    Respawning,
    /// The boss fell, banner, checkpoint, then save and on to next level.
    LevelClear,
    /// No lives left, run over
    GameOver,
}

/// Systems that will set up a new run. Anything that spawns a level runs after it.
#[derive(SystemSet, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct RunSetup;

/// Menu sub states for spawning menu overlays.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Menu {
    #[default]
    None,
    Main,
    Credits,
    Settings,
    Pause,
}

/// The game's main screen states. Dictates what is played on screen for the player and controls game loop.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Screen {
    #[default]
    Splash,
    Title,
    Loading,
    Gameplay,
}

/// Whether or not the game is paused.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct Pause(pub bool);

/// A system set for systems that shouldn't run while the game is paused.
#[derive(SystemSet, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PausableSystems;

// ####### Run Data for managing entities within a run ######

/// The lives rmaining, counting the one in play.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Lives(pub u32);

impl Default for Lives {
    fn default() -> Self {
        Self(STARTING_LIVES)
    }
}

/// The seed for the runs level
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct RunSeed(pub u32);

/// Where a run picks up from. Saved after a level clear
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Checkpoint {
    pub level: u32,
    pub score: u32,
    pub lives: u32,
    pub seed: u32,
}

/// Set by main menu before a run enters `Screen::Gameplay`
#[derive(Resource, Debug, Clone, Default)]
pub enum RunStart {
    #[default]
    NewGame,
    Continue(Checkpoint),
}

#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerDied {
    #[allow(dead_code)] // for particle effects later
    pub position: Vec2,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct BossDefeated {
    pub level: u32,
}

#[derive(Resource)]
struct PhaseTimer(Timer);

// ------ The actual systems of the game manager ------

/// One place that resets everthing a run needs.
fn start_run(
    mut run_start: ResMut<RunStart>,
    mut lives: ResMut<Lives>,
    mut score: ResMut<Score>,
    mut progress: ResMut<LevelProgress>,
    mut seed: ResMut<RunSeed>,
) {
    // `take` resets `RunStart` to `NewGame`, so Retry after a Continue starts fresh.
    match std::mem::take(&mut *run_start) {
        RunStart::NewGame => {
            *lives = Lives::default();
            score.current = 0;
            *progress = LevelProgress::default();
            seed.0 = fastrand::u32(..);
        }
        RunStart::Continue(cp) => {
            lives.0 = cp.lives.max(1);
            score.current = cp.score;
            *progress = LevelProgress::starting_at(cp.level, cp.score);
            seed.0 = cp.seed;
        }
    }
}

fn on_player_died(
    mut died: MessageReader<PlayerDied>,
    mut lives: ResMut<Lives>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    for _ in died.read() {
        lives.0 = lives.0.saturating_sub(1);
        next.set(if lives.0 == 0 {
            GamePhase::GameOver
        } else {
            GamePhase::Respawning
        });
    }
}

fn on_boss_defeated(
    mut defeated: MessageReader<BossDefeated>,
    lives: Res<Lives>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    if defeated.read().last().is_some() && lives.0 > 0 {
        next.set(GamePhase::LevelClear);
    }
}

fn finish_respawn(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<PhaseTimer>,
    assets: Res<PlayerAssets>,
    selected: Res<SelectedPlayer>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        commands.spawn_scene(player(selected.0, &assets));
        next.set(GamePhase::Playing);
    }
}

fn finish_level_clear(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: ResMut<PhaseTimer>,
    score: Res<Score>,
    mut progress: ResMut<LevelProgress>,
    players: Query<(), With<Player>>,
    assets: Res<PlayerAssets>,
    selected: Res<SelectedPlayer>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    progress.advance(score.current);
    // The crow may have died in same frame as boss
    if players.is_empty() {
        commands.spawn_scene(player(selected.0, &assets));
    }
    next.set(GamePhase::Playing);
}
