//! Demo gameplay. All of these modules are only intended for demonstration
//! purposes and should be replaced with your own game logic.
//! Feel free to change the logic found here if you feel like tinkering around
//! to get a feeling for the template.

use bevy::prelude::*;

mod animation;
pub mod boss;
pub mod enemy;
pub mod energy;
pub mod hud;
pub mod level;
mod movement;
pub mod physics;
pub mod pickup;
pub mod player;
pub mod progression;
pub mod projectile;
pub mod river;
pub mod score;
pub mod tiles;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        animation::plugin,
        boss::plugin,
        enemy::plugin,
        energy::plugin,
        hud::plugin,
        level::plugin,
        movement::plugin,
        physics::plugin,
        pickup::plugin,
        player::plugin,
        progression::plugin,
        projectile::plugin,
        score::plugin,
        progression::plugin,
        tiles::plugin,
    ));
}
