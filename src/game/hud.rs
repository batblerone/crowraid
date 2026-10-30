//! The in game HUD showing: Score, Level, Lives, Health and Energy
//!
//! Will also hold the banners that show on in game events. Handling GameOver
//! LevelClear and phase transition banners

use bevy::prelude::*;

use crate::{
    game::{
        energy::Energy,
        player::{Player, PlayerAssets, SelectedPlayer},
        progression::LevelProgress,
        score::{Score, score_label},
        boss::Boss,
    },
    game_manager::{GamePhase, Lives, MAX_LIVES, RunSetup, Screen},
    screens::gameplay::Health,
    theme::{palette::LABEL_TEXT, widget},
};

const BAR_WIDTH: f32 = 220.0;
const BAR_HEIGHT: f32 = 14.0;
const LIFE_ICON_SIZE: f32 = 32.0;
const LOW_ENERGY: f32 = 0.25;

const BAR_BACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.5);
const HEALTH_COLOR: Color = Color::srgb(0.85, 0.25, 0.25);
const ENERGY_COLOR: Color = Color::srgb(0.95, 0.8, 0.3);
const ENERGY_WARN_COLOR: Color = Color::srgb(1.0, 0.4, 0.1);

const BOSS_COLOR: Color = Color::srgb(0.6, 0.2, 0.7);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Gameplay), spawn_hud.after(RunSetup));
    app.add_systems(
        Update,
        (
            update_score_text.run_if(resource_changed::<Score>),
            update_level_text.run_if(resource_changed::<LevelProgress>),
            update_life_icons.run_if(resource_changed::<Lives>),
            update_player_bars,
        )
            .run_if(in_state(Screen::Gameplay)),
    );
    app.add_systems(OnEnter(GamePhase::Respawning), spawn_respawn_banner);
    app.add_systems(OnEnter(GamePhase::LevelClear), spawn_level_clear_banner);
    app.add_systems(OnEnter(GamePhase::GameOver), spawn_game_over_panel);
}

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct LevelText;

/// The nth crow icon in the lives row.
#[derive(Component)]
struct LifeIcon(u32);

#[derive(Component)]
struct HealthFill;

#[derive(Component)]
struct EnergyFill;

#[derive(Component)]
struct BossBar;
#[derive(Component)]
struct BossFill;

fn spawn_hud(
    mut commands: Commands,
    score: Res<Score>,
    progress: Res<LevelProgress>,
    lives: Res<Lives>,
    player_assets: Res<PlayerAssets>,
    selected: Res<SelectedPlayer>,
) {
    let hud = commands
        .spawn((
            Name::new("HUD"),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(px(16)),
                ..default()
            },
            Pickable::IGNORE,
            DespawnOnExit(Screen::Gameplay),
        ))
        .id();

    // Left column: score, lives, bars.
    let left = commands
        .spawn((
            Name::new("HUD Left"),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(hud),
        ))
        .id();
    commands.spawn((
        widget::label(score_label(score.current)),
        ScoreText,
        ChildOf(left),
    ));

    let lives_row = commands
        .spawn((
            Name::new("Lives"),
            Node {
                column_gap: px(4),
                ..default()
            },
            ChildOf(left),
        ))
        .id();
    // One icon per possible life; update_life_icons hides the spare ones.
    let sheet = player_assets.sheet(selected.0);
    let layout = player_assets.layout(selected.0);
    for i in 0..MAX_LIVES {
        commands.spawn((
            Name::new("Life Icon"),
            LifeIcon(i),
            ImageNode::from_atlas_image(
                sheet.clone(),
                TextureAtlas {
                    layout: layout.clone(),
                    index: 0,
                },
            ),
            Node {
                width: px(LIFE_ICON_SIZE),
                height: px(LIFE_ICON_SIZE),
                display: if i < lives.0 {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            ChildOf(lives_row),
        ));
    }

    spawn_stat_bar(&mut commands, left, "Health", HEALTH_COLOR, HealthFill);
    spawn_stat_bar(&mut commands, left, "Energy", ENERGY_COLOR, EnergyFill);

    // Right side: level number.
    commands.spawn((
        widget::label(level_label(progress.level)),
        LevelText,
        ChildOf(hud),
    ));

    // Big ass boss health bar
    let boss_bar = commands
        .spawn((
            Name::new("Boss Bar"),
            BossBar,
            Node {
                position_type: PositionType::Absolute,
                top: px(56),
                left: percent(25),
                width: percent(50),
                height: px(18),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BAR_BACK),
            ChildOf(hud),
        ))
        .id();
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(BOSS_COLOR),
        BossFill,
        ChildOf(boss_bar),
    ));

    // Add to the HUD Update tuple:
    fn update_boss_bar(
        boss: Query<&Health, With<Boss>>,
        mut bar: Single<&mut Node, (With<BossBar>, Without<BossFill>)>,
        mut fill: Single<&mut Node, (With<BossFill>, Without<BossBar>)>,
    ) {
        match boss.single() {
            Ok(health) => {
                bar.display = Display::Flex;
                fill.width = Val::Percent(health.h_ratio() * 100.0);
            }
            Err(_) => bar.display = Display::None,
        }
    }
}

/// "Energy [=========     ]": a label, a dark track and a coloured fill.
fn spawn_stat_bar(
    commands: &mut Commands,
    parent: Entity,
    name: &'static str,
    color: Color,
    marker: impl Component,
) {
    let row = commands
        .spawn((
            Name::new(format!("{name} Bar")),
            Node {
                column_gap: px(8),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    commands.spawn((
        Text::new(name),
        TextFont::from_font_size(18.0),
        TextColor(LABEL_TEXT),
        Node {
            width: px(70),
            ..default()
        },
        ChildOf(row),
    ));
    let track = commands
        .spawn((
            Node {
                width: px(BAR_WIDTH),
                height: px(BAR_HEIGHT),
                ..default()
            },
            BackgroundColor(BAR_BACK),
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(color),
        marker,
        ChildOf(track),
    ));
}

fn level_label(level: u32) -> String {
    format!("Level {level}")
}

fn update_score_text(score: Res<Score>, mut text: Single<&mut Text, With<ScoreText>>) {
    text.0 = score_label(score.current);
}

fn update_level_text(progress: Res<LevelProgress>, mut text: Single<&mut Text, With<LevelText>>) {
    text.0 = level_label(progress.level);
}

fn update_life_icons(lives: Res<Lives>, mut icons: Query<(&LifeIcon, &mut Node)>) {
    for (icon, mut node) in &mut icons {
        node.display = if icon.0 < lives.0 {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn update_player_bars(
    time: Res<Time>,
    crow: Query<(&Health, &Energy), With<Player>>,
    mut health_fill: Single<&mut Node, (With<HealthFill>, Without<EnergyFill>)>,
    mut energy_fill: Single<
        (&mut Node, &mut BackgroundColor),
        (With<EnergyFill>, Without<HealthFill>),
    >,
) {
    // Between lives there is no crow; show full bars for the one that's coming.
    let (health, energy) = crow
        .single()
        .map(|(h, e)| (h.h_ratio(), e.ratio()))
        .unwrap_or((1.0, 1.0));

    health_fill.width = Val::Percent(health * 100.0);

    let (node, color) = &mut *energy_fill;
    node.width = Val::Percent(energy * 100.0);
    // Blink when low, four times a second.
    let blink = energy < LOW_ENERGY && (time.elapsed_secs() * 4.0) as u32 % 2 == 0;
    color.0 = if blink {
        ENERGY_WARN_COLOR
    } else {
        ENERGY_COLOR
    };
}

// ---- Phase overlays ----

fn banner(text: String, phase: GamePhase) -> impl Bundle {
    (
        widget::ui_root("Banner"),
        GlobalZIndex(1),
        DespawnOnExit(phase),
        children![widget::header(text)],
    )
}

fn spawn_respawn_banner(mut commands: Commands, lives: Res<Lives>) {
    let crows = if lives.0 == 1 { "crow" } else { "crows" };
    commands.spawn(banner(
        format!("{} {crows} left", lives.0),
        GamePhase::Respawning,
    ));
}

fn spawn_level_clear_banner(mut commands: Commands, progress: Res<LevelProgress>) {
    // `advance` runs at the end of LevelClear, so this is the level just beaten.
    commands.spawn(banner(
        format!("Level {} cleared!", progress.level),
        GamePhase::LevelClear,
    ));
}

fn spawn_game_over_panel(mut commands: Commands, score: Res<Score>) {
    commands.spawn((
        widget::ui_root("Game Over"),
        GlobalZIndex(2),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        DespawnOnExit(GamePhase::GameOver),
        children![
            widget::header("Game Over"),
            widget::label(format!("Score: {}", score.current)),
            widget::label(format!("Best: {}", score.best_score)),
            widget::button("Retry", retry),
            widget::button("Quit to title", quit_to_title),
        ],
    ));
}

fn retry(_: On<PointerClick>, mut next_screen: ResMut<NextState<Screen>>) {
    // Loading flips straight back to Gameplay, re-running every OnEnter system.
    next_screen.set(Screen::Loading);
}

fn quit_to_title(_: On<PointerClick>, mut next_screen: ResMut<NextState<Screen>>) {
    next_screen.set(Screen::Title);
}
