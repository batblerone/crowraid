//! Game manager controls the game state, score tracking,
//! save states and overall progress of the game.

use bevy::prelude::*;

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

/// The game manager struct, tracking score and other game globals
pub struct GameManager {
    pub screen: Screen,
    pub score_increase_rate: f32,
    pub current_score: f32,
    pub high_score: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub master_volume: f32,
    pub fullscreen: f32,
}

