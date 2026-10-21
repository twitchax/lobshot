//! Sparkhorn (Lightning): a ball that calls bolts down onto where it lands. S1's bolt strikes
//! straight down the impact's column, S2's two come in a V from either side, and the SS sends one
//! down onto every mobile within reach. Land above a target blocks its bolt (DB-mob).

use super::{Gun, bolt};
use crate::{
    game::{Contact, Effect, Game, Shot, Track},
    tuning::{
        BOLT_CRATER, BOLT_DELAY_SECONDS, BOLT_REACH, DamageRange,
        sparkhorn::{S1_BALL_BLAST, S1_BALL_DAMAGE, S2_BALL_BLAST, S2_BALL_DAMAGE, S2_BOLT_DEGREES, SPLASH, SS_BALL_BLAST, SS_BOLT_DAMAGE, SS_REACH},
    },
    units::{Vec2, ticks, to_radians},
    weather::Weather,
};

/// The SS ball only carves; its bolts do the damage (DB-mob).
const NO_DAMAGE: DamageRange = DamageRange { min: 0, max: 0 };

pub(super) fn fire(game: &mut Game, gun: &mut Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            let track = gun.plain(game, flight, S1_BALL_BLAST.value, S1_BALL_DAMAGE.value, effects);
            if landed(&track) {
                let strike = bolt::from_sky(game, track.impact.at.x);
                splash(game, gun, &strike, &track, effects);
            }
        }
        Shot::S2 => {
            let track = gun.plain(game, flight, S2_BALL_BLAST.value, S2_BALL_DAMAGE.value, effects);
            if landed(&track) {
                let angle = to_radians(S2_BOLT_DEGREES.value);
                for side in [-1.0, 1.0] {
                    let direction = Vec2::new(side * libm::sin(angle), libm::cos(angle));
                    let strike = bolt::toward(game, track.impact.at, direction);
                    splash(game, gun, &strike, &track, effects);
                }
            }
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
            gun.land(game, &gun.blast_on(&track, SS_BALL_BLAST.value, NO_DAMAGE), SS_BALL_BLAST.value, &track.impact, effects);
            if landed(&track) {
                storm(game, gun, &track, effects);
            }
        }
    }
}

fn landed(track: &Track) -> bool {
    matches!(track.impact.contact, Contact::Terrain | Contact::Mobile(_))
}

/// A bolt the ball called strikes a beat after it landed, scorches the ground, and splashes
/// whoever is near its path.
fn splash(game: &mut Game, gun: &mut Gun, strike: &bolt::Strike, track: &Track, effects: &mut Vec<Effect>) {
    let tick = track.impact.tick + ticks(BOLT_DELAY_SECONDS.value);
    bolt::flash(game, strike, BOLT_CRATER.value * gun.salvo.crater, tick, effects);
    bolt::splash(game, &mut gun.salvo.splashed, strike, SPLASH.value, BOLT_REACH.value, effects);
}

/// The SS: every living mobile within reach of the landing takes a bolt straight down its own
/// column.
fn storm(game: &mut Game, gun: &Gun, track: &Track, effects: &mut Vec<Effect>) {
    let at = track.impact.at;
    let columns: Vec<f64> = game
        .players()
        .iter()
        .filter(|p| p.alive && p.pose().gap(p.tuning().hitbox.value, at) <= SS_REACH.value)
        .map(|p| p.center().x)
        .collect();
    let tick = track.impact.tick + ticks(BOLT_DELAY_SECONDS.value);
    for x in columns {
        let strike = bolt::from_sky(game, x);
        bolt::flash(game, &strike, BOLT_CRATER.value * gun.salvo.crater, tick, effects);
        let blast = gun.blast(strike.at, BOLT_REACH.value, SS_BOLT_DAMAGE.value, Weather::force_multiplier(track.forced));
        bolt::zap(game, &strike, &blast, effects);
    }
}
