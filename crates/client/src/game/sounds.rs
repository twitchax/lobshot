//! The game's sounds that follow its state rather than a scene's schedule: a chime when your turn
//! begins, a tick for each of its last seconds, and the result. Shots, blasts and falls sound from
//! `playback`, on the schedule their pictures keep.

use bevy::prelude::*;
use lobshot_assets::Sound;

use super::{
    Timeline,
    playback::{Ending, Outcome, TurnClock},
};
use crate::{
    model::{Match, Screen, Session},
    sound::Play,
};

/// Your turn's last seconds tick.
const TICKING: u32 = 5;

/// What `turn` has sounded already: the turn (by the clock's count) and its last tick.
#[derive(Default)]
struct Heard {
    starts: u32,
    ticked: Option<u32>,
}

fn turn(clock: Res<TurnClock>, session: Res<Session>, timeline: Res<Timeline>, time: Res<Time>, mut heard: Local<Heard>, mut play: MessageWriter<Play>) {
    let mine = clock.seat.is_some() && clock.seat == session.my_seat();
    if clock.starts != heard.starts {
        heard.starts = clock.starts;
        if mine {
            play.write(Play(Sound::Turn));
        }
    }
    // Not while your shot is still flying: its turn is spent.
    if !mine || timeline.busy() {
        heard.ticked = None;
        return;
    }
    let left = clock.seconds_left(&session, time.elapsed_secs_f64());
    if (1..=TICKING).contains(&left) && heard.ticked != Some(left) {
        play.write(Play(Sound::Tick));
        heard.ticked = Some(left);
    }
}

fn result(outcome: Res<Outcome>, game: Option<Res<Match>>, session: Res<Session>, mut heard: Local<Option<Ending>>, mut play: MessageWriter<Play>) {
    if outcome.0 == *heard {
        return;
    }
    *heard = outcome.0;
    let (Some(Ending::Winner(winner)), Some(game)) = (outcome.0, game) else { return };
    // Spectators hear no verdict, and neither does a draw.
    match session.my_team(&game.predicted) {
        Some(mine) if mine == winner => play.write(Play(Sound::Win)),
        Some(_) => play.write(Play(Sound::Lose)),
        None => return,
    };
}

pub struct SoundsPlugin;

impl Plugin for SoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (turn, result).run_if(in_state(Screen::Game)));
    }
}
