//! The game's menus and transitions between them.

mod credits;
mod main;
mod pause;
mod settings;

use crate::game_manager::*;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Menu>();

    app.add_plugins((
        credits::plugin,
        main::plugin,
        settings::plugin,
        pause::plugin,
    ));
}
