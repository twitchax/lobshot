//! Sound effects: every `lobshot_assets::Sound` loads at startup, and a `Play` message plays one.
//! The game plays its own (`game::playback` on the scene's schedule, `game::sounds` for the turn
//! and the result); button presses and arriving chat lines sound from here.
//!
//! Browsers keep a page's audio suspended until its first click or key press; `index.html` resumes
//! it then, so anything played before that is silent.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use lobshot_assets::{Sound, sound_path};

use crate::{model::Session, ui::Clicked};

/// Plays a sound once.
#[derive(Message, Debug, Clone, Copy)]
pub struct Play(pub Sound);

#[derive(Resource)]
struct Sounds(HashMap<Sound, Handle<AudioSource>>);

/// What has loaded and played, for the browser smoke test (`window.lobshot.sounds`).
#[derive(Resource, Default)]
pub struct SoundLog {
    pub loaded: u32,
    pub played: BTreeMap<&'static str, u32>,
}

fn load(server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(Sounds(Sound::ALL.into_iter().map(|sound| (sound, server.load(sound_path(sound)))).collect()));
}

fn play(mut requests: MessageReader<Play>, sounds: Res<Sounds>, sources: Res<Assets<AudioSource>>, mut log: ResMut<SoundLog>, mut commands: Commands) {
    for &Play(sound) in requests.read() {
        if let Some(handle) = sounds.0.get(&sound) {
            commands.spawn((AudioPlayer(handle.clone()), PlaybackSettings::DESPAWN));
            *log.played.entry(sound.name()).or_default() += 1;
        }
    }
    let loaded = sounds.0.values().filter(|handle| sources.contains(*handle)).count();
    let loaded = u32::try_from(loaded).unwrap_or(u32::MAX);
    if log.loaded != loaded {
        log.loaded = loaded;
    }
}

fn clicks(mut clicked: MessageReader<Clicked>, mut play: MessageWriter<Play>) {
    if clicked.read().count() > 0 {
        play.write(Play(Sound::Click));
    }
}

/// One chime however many lines arrive at once.
fn chat(session: Res<Session>, mut heard: Local<usize>, mut play: MessageWriter<Play>) {
    let lines = session.chat.len();
    if lines > *heard {
        play.write(Play(Sound::Chat));
    }
    *heard = lines;
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Play>()
            .init_resource::<SoundLog>()
            .add_systems(Startup, load)
            .add_systems(Update, ((clicks, chat), play).chain());
    }
}
