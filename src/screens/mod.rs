//! The game's main screen states and transitions between them.

pub mod gameplay;
mod loading;
mod splash;
mod title;

use crate::game_manager::*;
use bevy::prelude::*;

// Game window configuration constants
//pub const WINDOW_WIDTH: f32 = 800.0;
//pub const WINDOW_HEIGHT: f32 = 600.0;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Screen>();

    app.add_plugins((
        gameplay::plugin,
        loading::plugin,
        splash::plugin,
        title::plugin,
    ));
}
