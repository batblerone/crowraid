//! The main menu (seen on the title screen).

use bevy::prelude::*;

use crate::{
    asset_tracking::ResourceHandles,
    game_manager::{RunStart, Screen},
    menus::Menu,
    save::SaveData,
    theme::widget,
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
}

fn spawn_main_menu(mut commands: Commands, save: Res<SaveData>) {
    let has_checkpoint = save.checkpoint.is_some();
    commands
        .spawn((
            widget::ui_root("Main Menu"),
            GlobalZIndex(2),
            DespawnOnExit(Menu::Main),
        ))
        .with_children(|menu| {
            if has_checkpoint {
                menu.spawn(widget::button("Continue", continue_run));
            }
            menu.spawn(widget::button("New game", enter_loading_or_gameplay_screen));
            menu.spawn(widget::button("Settings", open_settings_menu));
            menu.spawn(widget::button("Credits", open_credits_menu));
            #[cfg(not(target_family = "wasm"))]
            menu.spawn(widget::button("Exit", exit_app));
        });
}

fn continue_run(
    _: On<PointerClick>,
    save: Res<SaveData>,
    mut run_start: ResMut<RunStart>,
    resource_handles: Res<ResourceHandles>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if let Some(checkpoint) = save.checkpoint {
        *run_start = RunStart::Continue(checkpoint);
    }
    next_screen.set(if resource_handles.is_all_done() {
        Screen::Gameplay
    } else {
        Screen::Loading
    });
}

fn enter_loading_or_gameplay_screen(
    _: On<PointerClick>,
    resource_handles: Res<ResourceHandles>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if resource_handles.is_all_done() {
        next_screen.set(Screen::Gameplay);
    } else {
        next_screen.set(Screen::Loading);
    }
}

fn open_settings_menu(_: On<PointerClick>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Settings);
}

fn open_credits_menu(_: On<PointerClick>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Credits);
}

#[cfg(not(target_family = "wasm"))]
fn exit_app(_: On<PointerClick>, mut app_exit: MessageWriter<AppExit>) {
    app_exit.write(AppExit::Success);
}
