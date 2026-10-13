//! Player sprite animation.
//! This is based on multiple examples and may be very different for your game.
//! - [Sprite flipping](https://github.com/bevyengine/bevy/blob/latest/examples/2d/sprite_flipping.rs)
//! - [Sprite animation](https://github.com/bevyengine/bevy/blob/latest/examples/2d/sprite_animation.rs)
//! - [Timers](https://github.com/bevyengine/bevy/blob/latest/examples/time/timers.rs)

use bevy::prelude::*;
use rand::prelude::*;
use std::time::Duration;

use crate::{
    AppSystems, PausableSystems,
    audio::sound_effect,
    game::{movement::MovementController, player::PlayerAssets},
};

pub(super) fn plugin(app: &mut App) {
    // Animate and play sound effects based on controls.
    app.add_systems(
        Update,
        (
            update_animation_timer.in_set(AppSystems::TickTimers),
            (
                (update_animation_movement, update_animation_atlas).chain(),
                trigger_step_sound_effect,
            )
                .chain()
                .in_set(AppSystems::Update),
        )
            .in_set(PausableSystems),
    );
}

/// Update the animation timer.
fn update_animation_timer(time: Res<Time>, mut query: Query<&mut PlayerAnimation>) {
    for mut animation in &mut query {
        animation.update_timer(time.delta());
    }
}

/// Update the sprite direction and animation state (idling/walking).
fn update_animation_movement(
    mut player_query: Query<(&MovementController, &mut PlayerAnimation)>,
) {
    for (controller, mut animation) in &mut player_query {
        let intent = controller.intent;

        // We now map movement to a direciton, instead of a animation set
        let direction = if intent == Vec2::ZERO {
            PlayerDirection::Neutral
        } else if intent.y > intent.x.abs() {
            PlayerDirection::Up
        } else if intent.y < -intent.x.abs() {
            PlayerDirection::Down
        } else if intent.x < 0.0 {
            PlayerDirection::Left
        } else {
            PlayerDirection::Right
        };

        animation.update_direction(direction);
    }
    // Legacy animation controler for a sprite sheet with multiple animation sets (Idel, walking, running etc.)
    // for (controller, mut sprite, mut animation) in &mut player_query {
    //     let dx = controller.intent.x;
    //     if dx != 0.0 {
    //         sprite.flip_x = dx < 0.0;
    //     }

    //     let animation_state = if controller.intent == Vec2::ZERO {
    //         PlayerAnimationState::Idling
    //     } else {
    //         PlayerAnimationState::Walking
    //     };
    //     animation.update_state(animation_state);
    // }
}

/// Update the texture atlas to reflect changes in the animation.
fn update_animation_atlas(mut query: Query<(&PlayerAnimation, &mut Sprite)>) {
    for (animation, mut sprite) in &mut query {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };

        let target_index = animation.get_atlas_index();
        if atlas.index != target_index {
            atlas.index = target_index;
        }
    }
}

/// If the player is moving, play a step sound effect synchronized with the
/// animation.
fn trigger_step_sound_effect(
    mut commands: Commands,
    player_assets: If<Res<PlayerAssets>>,
    mut step_query: Query<&PlayerAnimation>,
) {
    for animation in &mut step_query {
        if animation.state != PlayerDirection::Neutral
            && animation.changed()
            && (animation.frame == 2 || animation.frame == 5)
        {
            let rng = &mut rand::rng();
            let random_step = player_assets.steps.choose(rng).unwrap().clone();
            commands.spawn(sound_effect(random_step));
        }
    }
}

/// Component that tracks player's animation state.
/// It is tightly bound to the texture atlas we use.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct PlayerAnimation {
    timer: Timer,
    frame: usize,
    state: PlayerDirection, // Change to PlayerAnimationState for alternate spritesheets.
}

#[derive(Reflect, PartialEq)]
pub enum PlayerAnimationState {
    Idling,
    Walking,
}

#[derive(Reflect, PartialEq, Eq, Clone, Copy)]
pub enum PlayerDirection {
    Neutral,
    Up,
    Down,
    Left,
    Right,
}

impl PlayerAnimation {
    /// The number of idle frames.
    const _IDLE_FRAMES: usize = 2;
    /// The duration of each idle frame. Also used for directional animation.
    const IDLE_INTERVAL: Duration = Duration::from_millis(500);
    /// The number of walking frames.
    const _WALKING_FRAMES: usize = 6;
    /// The duration of each walking frame.
    const _WALKING_INTERVAL: Duration = Duration::from_millis(50);
    /// The number of frames per directional animation loop
    const FRAMES_PER_DIRECTION: usize = 1; // One frame each direction

    fn idling() -> Self {
        Self {
            timer: Timer::new(Self::IDLE_INTERVAL, TimerMode::Repeating),
            frame: 0,
            state: PlayerDirection::Neutral,
        }
    }

    // Walking animations are not needed in Directional animation. Other functions for running or attacking
    // are added as needed for more complex spritesheets.
    // fn walking() -> Self {
    //     Self {
    //         timer: Timer::new(Self::WALKING_INTERVAL, TimerMode::Repeating),
    //         frame: 0,
    //         state: PlayerAnimationState::Walking,
    //     }
    // }

    pub fn new() -> Self {
        Self::idling()
    }

    /// Update animation timers.
    pub fn update_timer(&mut self, delta: Duration) {
        self.timer.tick(delta);
        if self.timer.is_finished() {
            self.frame = (self.frame + 1) % Self::FRAMES_PER_DIRECTION;
        }
        // if !self.timer.is_finished() {
        //     return;
        // }
        // self.frame = (self.frame + 1)
        //     % match self.state {
        //         PlayerAnimationState::Idling => Self::IDLE_FRAMES,
        //         PlayerAnimationState::Walking => Self::WALKING_FRAMES,
        //     };
    }

    /// Update direction for animation state
    pub fn update_direction(&mut self, direction: PlayerDirection) {
        if self.state != direction {
            self.state = direction;
            self.frame = 0;
            self.timer.reset();
        }
    }

    // /// Update animation state if it changes.
    // pub fn update_state(&mut self, state: PlayerAnimationState) {
    //     if self.state != state {
    //         match state {
    //             PlayerAnimationState::Idling => *self = Self::idling(),
    //             PlayerAnimationState::Walking => *self = Self::walking(),
    //         }
    //     }
    // }

    /// Whether animation changed this tick.
    pub fn changed(&self) -> bool {
        self.timer.is_finished()
    }

    /// Returns a sprite index in the atlas based on direction.
    /// We can specify that each direction occupies a n to n + x number
    /// of indicies. For now our n is 1 and each animation is only
    /// 1 frame, so 1 + 0.
    pub fn get_atlas_index(&self) -> usize {
        let base_index = match self.state {
            PlayerDirection::Neutral => 0,
            PlayerDirection::Down => 1,
            PlayerDirection::Up => 2,
            PlayerDirection::Right => 3,
            PlayerDirection::Left => 4,
        };

        base_index + self.frame
    }

    // /// Return sprite index in the atlas.
    // pub fn get_atlas_index(&self) -> usize {
    //     match self.state {
    //         PlayerAnimationState::Idling => self.frame,
    //         PlayerAnimationState::Walking => 6 + self.frame,
    //     }
    // }
}
