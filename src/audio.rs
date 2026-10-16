use bevy::{audio::Volume, prelude::*};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AudioSettings>();
    app.add_systems(
        Update,
        apply_volume_settings
            .run_if(resource_changed::<GlobalVolume>.or_eager(resource_changed::<AudioSettings>)),
    );
}

/// An organizational marker component that should be added to a spawned [`AudioPlayer`] if it's in the
/// general "music" category (e.g. global background music, soundtrack).
///
/// This can then be used to query for and operate on sounds in that category.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Music;

/// A resource for managing specific audio settings.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            music_volume: 1.0,
            sfx_volume: 1.0,
        }
    }
}

/// A music audio instance.
pub fn music(handle: Handle<AudioSource>) -> impl Bundle {
    (AudioPlayer(handle), PlaybackSettings::LOOP, Music)
}

/// An organizational marker component that should be added to a spawned [`AudioPlayer`] if it's in the
/// general "sound effect" category (e.g. footsteps, the sound of a magic spell, a door opening).
///
/// This can then be used to query for and operate on sounds in that category.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct SoundEffect;

/// A sound effect audio instance.
pub fn sound_effect(handle: Handle<AudioSource>) -> impl Bundle {
    (AudioPlayer(handle), PlaybackSettings::DESPAWN, SoundEffect)
}

/// [`GlobalVolume`] doesn't apply to already-running audio entities, so this system will update them.
fn apply_volume_settings(
    global_volume: Res<GlobalVolume>,
    audio_settings: Res<AudioSettings>,
    // This will query all entities that are audio sinks, including their category and playback settings.
    mut audio_query: Query<
        (
            &PlaybackSettings,
            &mut AudioSink,
            Option<&Music>,
            Option<&SoundEffect>,
        ),
        Or<(Changed<AudioSink>, With<Music>, With<SoundEffect>)>,
    >,
) {
    if !audio_settings.is_changed() && !global_volume.is_changed() {
        return;
    }

    for (playback, mut sink, music_marker, sfx_marker) in &mut audio_query {
        let mut final_mult = playback.volume;

        if music_marker.is_some() {
            final_mult *= Volume::Linear(audio_settings.music_volume);
        } else if sfx_marker.is_some() {
            final_mult *= Volume::Linear(audio_settings.sfx_volume);
        }

        sink.set_volume(global_volume.volume * final_mult);
    }
}

// fn apply_audio_settings(
//     audio_settings: Res<AudioSettings>,
//     mut music_sinks: Query<&mut AudioSink, (With<Music>, Changed<AudioSink>)>,
//     mut sfx_sinks: Query<&mut AudioSink, (With<SoundEffect>, Changed<AudioSink>)>,
// ) {
//     // Apply changes when new audio spawns or are modified
//     if audio_settings.is_changed() {
//         for mut sink in &mut music_sinks {
//             sink.set_volume(Volume::Linear(audio_settings.music_volume));
//         }
//         for mut sink in &mut sfx_sinks {
//             sink.set_volume(Volume::Linear(audio_settings.sfx_volume));
//         }
//     }
// }
