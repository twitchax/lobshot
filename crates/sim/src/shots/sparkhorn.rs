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
            gun.land(game, &gun.blast_on(&track, SS_BALL_BLAST.value, NO_DAMAGE), SS_BALL_BLAST.value, &track.impact, track.marks, effects);
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
        let blast = gun.blast(strike.at, BOLT_REACH.value, SS_BOLT_DAMAGE.value, track.marks.boost());
        bolt::zap(game, &strike, &blast, effects);
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{
            Contact, Effect, Event, Game, MobileKind, Shot, Team,
            testing::{damage_to, direct_hit_power, duel_of, game_of, seat, shoot},
        },
        items::{Item, Loadout},
        units::{Centis, Facing, Power},
    };

    fn bolts(effects: &[Effect]) -> Vec<&Effect> {
        effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).collect()
    }

    #[test]
    fn s1_calls_a_bolt_down_onto_its_target_for_the_splash() {
        let mut game = duel_of(MobileKind::Sparkhorn, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        assert_eq!(bolts(&effects).len(), 1, "{effects:?}");
        // The ball's 74-174 x 1.04 (Electric against Mechanical), and the flat 76 splash.
        let dealt = damage_to(&effects, 1);
        assert!((152..=256).contains(&dealt), "{dealt} {effects:?}");
    }

    #[test]
    fn s2_brings_two_bolts_down_in_a_v() {
        let mut game = duel_of(MobileKind::Sparkhorn, MobileKind::Coilback, 1200);
        let effects = shoot(&mut game, 0, Shot::S2, 30, Power::nearest(1.8));
        let froms: Vec<f64> = effects.iter().filter_map(|e| if let Effect::Bolt { from, to, .. } = e { Some(from.x - to.x) } else { None }).collect();
        assert_eq!(froms.len(), 2, "{effects:?}");
        assert!(froms[0] * froms[1] < 0.0, "one from each side: {froms:?}");
    }

    #[test]
    fn splash_lands_once_a_turn_even_with_dual() {
        let mut game = duel_of(MobileKind::Sparkhorn, MobileKind::Coilback, 800);
        let power = direct_hit_power(&game, 30);
        game.players[0].items = Loadout::new(&[Item::Dual]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        assert_eq!(bolts(&effects).len(), 2, "two balls, two bolts: {effects:?}");
        let splashes = effects.iter().filter(|e| matches!(e, Effect::Damaged { seat: 1, amount: 76 })).count();
        assert!(splashes <= 1, "{effects:?}");
    }

    /// Sparkhorn at 300; two enemies close together at 1000 and 1090; a teammate far off at 2000.
    fn crowd() -> Game {
        game_of(vec![
            seat(Team::A, MobileKind::Sparkhorn, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1000, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1090, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 2000, Facing::Left),
        ])
    }

    #[test]
    fn ss_strikes_everyone_within_reach_with_a_bolt() {
        let mut game = crowd();
        let between = f64::midpoint(game.players()[1].center().x, game.players()[2].center().x);
        let power = (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, Shot::Ss, 30, power.bars())?, 0, 0).impact;
                (impact.contact != Contact::Gone).then(|| ((impact.at.x - between).abs(), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .unwrap();
        let effects = shoot(&mut game, 0, Shot::Ss, 30, power);
        assert_eq!(bolts(&effects).len(), 2, "the two enemies, not the far teammate or the shooter: {effects:?}");
        for seat in [1, 2] {
            // 150-200 x 1.04, plus whatever the neighbor's bolt adds.
            assert!(damage_to(&effects, seat) >= 156, "seat {seat}: {effects:?}");
        }
        assert_eq!(damage_to(&effects, 3), 0);
    }
}
