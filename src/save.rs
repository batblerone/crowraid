//! Save data: settings, best score and the last checkpoint, stored as one
//! versioned JSON document with a backup copy.

use bevy::{audio::Volume, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{
    audio::AudioSettings,
    game::{progression::LevelProgress, score::Score},
    game_manager::{Checkpoint, GamePhase, Lives, Menu, RunSeed},
};

const SAVE_VERSION: u32 = 1;
const MAIN: &str = "save";
const BACKUP: &str = "save_backup";

pub(super) fn plugin(app: &mut App) {
    app.add_message::<SaveRequest>();
    app.add_systems(Startup, load_save);
    app.add_systems(OnExit(Menu::Settings), request_save);
    // OnExit, not OnEnter: advance() has run, so this is the level you resume at.
    app.add_systems(OnExit(GamePhase::LevelClear), record_checkpoint);
    app.add_systems(OnEnter(GamePhase::GameOver), record_game_over);
    app.add_systems(Last, write_save);
}

/// Send this to save at the end of the frame. Several in one frame = one write.
#[derive(Message, Debug, Clone, Copy)]
pub struct SaveRequest;

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct SaveData {
    pub version: u32,
    pub settings: SettingsSave,
    pub best_score: u32,
    pub highest_level: u32,
    pub checkpoint: Option<Checkpoint>,
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            settings: SettingsSave::default(),
            best_score: 0,
            highest_level: 1,
            checkpoint: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
#[serde(default)]
pub struct SettingsSave {
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl Default for SettingsSave {
    fn default() -> Self {
        Self { master_volume: 1.0, music_volume: 1.0, sfx_volume: 1.0 }
    }
}

/// Bring older saves up to date. Add a branch each time SAVE_VERSION goes up.
fn migrate(mut data: SaveData) -> SaveData {
    // if data.version < 2 { ...convert old fields... }
    data.version = SAVE_VERSION;
    data
}

fn load_from_disk() -> SaveData {
    for key in [MAIN, BACKUP] {
        let Some(text) = backend::read(key) else { continue };
        match serde_json::from_str::<SaveData>(&text) {
            Ok(data) => return migrate(data),
            Err(err) => warn!("Save '{key}' is unreadable, trying the next one: {err}"),
        }
    }
    SaveData::default()
}

fn store(data: &SaveData) -> Result<(), String> {
    let text = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    // Keep the last good save as a backup before replacing it.
    if let Some(previous) = backend::read(MAIN) {
        backend::write(BACKUP, &previous).map_err(|e| e.to_string())?;
    }
    backend::write(MAIN, &text).map_err(|e| e.to_string())
}

// ---- Systems ----

fn load_save(
    mut commands: Commands,
    mut audio: ResMut<AudioSettings>,
    mut global: ResMut<GlobalVolume>,
    mut score: ResMut<Score>,
) {
    let data = load_from_disk();
    global.volume = Volume::Linear(data.settings.master_volume);
    audio.music_volume = data.settings.music_volume;
    audio.sfx_volume = data.settings.sfx_volume;
    score.best_score = data.best_score;
    commands.insert_resource(data);
}

fn request_save(mut save: MessageWriter<SaveRequest>) {
    save.write(SaveRequest);
}

fn record_checkpoint(
    mut data: ResMut<SaveData>,
    progress: Res<LevelProgress>,
    score: Res<Score>,
    lives: Res<Lives>,
    seed: Res<RunSeed>,
    mut save: MessageWriter<SaveRequest>,
) {
    // OnExit(LevelClear) also runs when quitting to the title mid-banner, before
    // `advance()`. The boss flag is only cleared by `advance()`, so skip that case.
    if progress.boss_active {
        return;
    }
    data.checkpoint = Some(Checkpoint {
        level: progress.level,
        score: score.current,
        lives: lives.0,
        seed: seed.0,
    });
    data.highest_level = data.highest_level.max(progress.level);
    save.write(SaveRequest);
}

fn record_game_over(mut data: ResMut<SaveData>, mut save: MessageWriter<SaveRequest>) {
    // Arcade rules: a finished run can't be continued.
    data.checkpoint = None;
    save.write(SaveRequest);
}

/// Copies live settings and the best score in, then writes once.
fn write_save(
    mut requests: MessageReader<SaveRequest>,
    data: Option<ResMut<SaveData>>,
    audio: Res<AudioSettings>,
    global: Res<GlobalVolume>,
    score: Res<Score>,
) {
    if requests.read().count() == 0 {
        return;
    }
    let Some(mut data) = data else { return };
    data.settings = SettingsSave {
        master_volume: global.volume.to_linear(),
        music_volume: audio.music_volume,
        sfx_volume: audio.sfx_volume,
    };
    data.best_score = data.best_score.max(score.best_score);
    if let Err(err) = store(&data) {
        error!("Saving failed: {err}");
    }
}

// ---- Storage backends ----

#[cfg(not(target_arch = "wasm32"))]
mod backend {
    use std::{fs, io, path::PathBuf};

    fn path(key: &str) -> Option<PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "batblerone", "Crowraid")?;
        Some(dirs.data_dir().join(format!("{key}.json")))
    }

    pub fn read(key: &str) -> Option<String> {
        fs::read_to_string(path(key)?).ok()
    }

    /// Write a temp file, then rename it into place: a crash mid-write
    /// leaves the old file untouched.
    pub fn write(key: &str, contents: &str) -> io::Result<()> {
        let path = path(key).ok_or_else(|| io::Error::other("no data folder"))?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, contents)?;
        fs::rename(&tmp, &path)
    }
}

#[cfg(target_arch = "wasm32")]
mod backend {
    use std::io;

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn read(key: &str) -> Option<String> {
        storage()?.get_item(&format!("crowraid_{key}")).ok()?
    }

    pub fn write(key: &str, contents: &str) -> io::Result<()> {
        storage()
            .ok_or_else(|| io::Error::other("localStorage unavailable"))?
            .set_item(&format!("crowraid_{key}"), contents)
            .map_err(|_| io::Error::other("localStorage write failed"))
    }
}