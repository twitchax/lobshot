//! Needlemouse (Knight): Lensowl's satellite with pins for lasers (`satellite`). Every shot is a
//! tracer; once it lands, the pincushion fires pins at it, each a projectile flying straight at
//! it. S1 is three pins from low over Needlemouse, aimed high when they fly leftward; S2 five from
//! twice as high; the SS glides the cushion over the landing and sweeps seven pins down onto it.

use super::{
    Gun,
    satellite::{self, Hit, Pins},
};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Shot},
    tuning::needlemouse::{
        PIN_BLAST, PIN_CRATER, PIN_DELAY_SECONDS, PIN_LIFE_SECONDS, PIN_SPACING, PIN_SPEED, RISE_SECONDS, S1_DAMAGE, S1_HIGH, S1_PINS, S2_DAMAGE, S2_PINS, SS_DAMAGE, SS_FIRST_SECONDS, SS_GAP_SECONDS,
        SS_HEIGHT, SS_PINS, SS_SWEEP,
    },
    units::{Vec2, ticks, to_radians},
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(home) = satellite::home(game, gun.seat, shot) else { return };
    let track = satellite::tracer(game, gun, effects);
    satellite::glide(gun, home, 0, ticks(RISE_SECONDS.value), effects);
    let Some(track) = track else { return };
    let (guide, landed) = (track.impact.at, track.impact.tick);
    let leave = landed + ticks(PIN_DELAY_SECONDS.value);
    let (launches, target, damage) = match shot {
        Shot::S1 => {
            let target = satellite::sighted(home, guide, S1_HIGH.value);
            (fan(home, target, S1_PINS.value, leave), target, S1_DAMAGE.value)
        }
        Shot::S2 => (fan(home, guide, S2_PINS.value, leave), guide, S2_DAMAGE.value),
        Shot::Ss => {
            let launches = sweep(guide, landed);
            // The cushion glides over the tracer, then on to each pin's point as it leaves.
            let mut from = landed;
            for &(at, tick) in &launches {
                satellite::glide(gun, at, from, tick, effects);
                from = tick;
            }
            (launches, guide, SS_DAMAGE.value)
        }
    };
    let pins = Pins {
        speed: PIN_SPEED.value,
        life: ticks(PIN_LIFE_SECONDS.value),
        hit: Hit { radius: PIN_BLAST.value, damage },
        crater: PIN_CRATER.value,
    };
    satellite::pins(game, gun, &launches, target, &pins, effects);
}

/// `count` pins leaving the cushion at `from` together at stage tick `tick`, side by side across
/// the line to `target`, `PIN_SPACING` apart.
fn fan(from: Vec2, target: Vec2, count: u32, tick: u32) -> Vec<(Vec2, u32)> {
    let path = target - from;
    let length = path.length();
    if length <= 0.0 {
        return vec![(from, tick); usize::try_from(count).unwrap_or(0)];
    }
    let across = Vec2::new(-path.y / length, path.x / length);
    let middle = f64::from(count.saturating_sub(1)) / 2.0;
    (0..count).map(|pin| (from + across * ((f64::from(pin) - middle) * PIN_SPACING.value), tick)).collect()
}

/// The SS's pins, each where it leaves and when: `SS_HEIGHT` over the tracer that landed at `guide`
/// at stage tick `landed`, swept back and forth across it.
fn sweep(guide: Vec2, landed: u32) -> Vec<(Vec2, u32)> {
    let over = guide - Vec2::new(0.0, SS_HEIGHT.value);
    (0..SS_PINS.value)
        .map(|pin| {
            let swing = SS_SWEEP.value * libm::cos(to_radians(60.0 * f64::from(pin)));
            let at = over - Vec2::new(swing, 0.0);
            (at, landed + ticks(SS_FIRST_SECONDS.value + SS_GAP_SECONDS.value * f64::from(pin)))
        })
        .collect()
}

/// Where the middle pin strikes if `seat` fires `shot`'s tracer along `flight`, for SearchBot's aim
/// pass: that is where the shot does its work. Where the tracer went if it left the map.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Impact {
    let impact = game.track(flight, 0, 0).impact;
    if !matches!(impact.contact, Contact::Terrain | Contact::Mobile(_)) {
        return impact;
    }
    let Some(home) = satellite::home(game, seat, shot) else { return impact };
    let (from, target) = match shot {
        Shot::S1 => (home, satellite::sighted(home, impact.at, S1_HIGH.value)),
        Shot::S2 => (home, impact.at),
        Shot::Ss => (impact.at - Vec2::new(0.0, SS_HEIGHT.value), impact.at),
    };
    satellite::pin_strike(game, from, target, PIN_SPEED.value).map_or(impact, |strike| Impact { tick: impact.tick, ..strike })
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, Leg, MobileKind, Team,
            testing::{damage_to, duel_of, game_of, game_on, seat, shoot},
        },
        items::{Item, Loadout},
        terrain::Terrain,
        units::{Centis, Facing, Power, TICKS_PER_SECOND},
        weather::{Column, WeatherKind},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Needlemouse, MobileKind::Coilback, distance)
    }

    /// Each projectile's path, the tracer's first.
    fn paths(effects: &[Effect]) -> Vec<Vec<Leg>> {
        effects.iter().filter_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.clone()) } else { None }).collect()
    }

    /// The pins: every path after the tracer's, as (where it left, when, which way).
    fn pins(effects: &[Effect]) -> Vec<(Vec2, u32, Vec2)> {
        paths(effects)
            .into_iter()
            .skip(1)
            .map(|legs| (legs[0].flight.origin, legs[0].offset, legs[0].flight.velocity))
            .collect()
    }

    /// Where the tracer landed.
    fn guide(effects: &[Effect]) -> Vec2 {
        let legs = &paths(effects)[0];
        let last = legs.last().unwrap();
        last.flight.at(last.to)
    }

    /// The power step whose tracer from seat 0 at `aim` strikes seat 1 closest to its center.
    fn tracer_on_target(game: &Game, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, Shot::S1, aim, power.bars())?, 0, 0).impact;
                (impact.contact == Contact::Mobile(1)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .unwrap()
    }

    /// The point where the line from `from` along `velocity` crosses world x `x`.
    fn at_x(from: Vec2, velocity: Vec2, x: f64) -> Vec2 {
        from + velocity * ((x - from.x) / velocity.x)
    }

    #[test]
    fn the_pincushion_hovers_low_for_s1_and_twice_as_high_for_s2_and_the_ss() {
        let foot = Vec2::new(500.0, 600.0);
        let over = |height: f64| Some(Vec2::new(500.0, 600.0 - height));
        assert_eq!(MobileKind::Needlemouse.satellite(foot, Shot::S1), over(150.0));
        assert_eq!(MobileKind::Needlemouse.satellite(foot, Shot::S2), over(300.0));
        assert_eq!(MobileKind::Needlemouse.satellite(foot, Shot::Ss), over(300.0));
    }

    #[test]
    fn s1s_tracer_calls_three_pins_side_by_side_that_converge_on_it() {
        let mut game = duel(800);
        let home = game.players()[0].footing.world() - Vec2::new(0.0, 150.0);
        let power = tracer_on_target(&game, 40);
        let effects = shoot(&mut game, 0, Shot::S1, 40, power);
        let pins = pins(&effects);
        assert_eq!(pins.len(), 3, "{effects:?}");
        assert_eq!(pins[1].0, home, "the middle one from the cushion over Needlemouse");
        assert!(
            (pins[0].0.distance(home) - 15.0).abs() < 1e-9 && (pins[2].0.distance(home) - 15.0).abs() < 1e-9,
            "15 px either side: {pins:?}"
        );
        let guide = guide(&effects);
        for &(from, tick, velocity) in &pins {
            assert!((velocity.length() - 800.0).abs() < 1e-6, "{velocity:?}");
            assert!(at_x(from, velocity, guide.x).distance(guide) < 1e-6, "every pin flies at the tracer: {from:?}");
            assert!(tick > 0, "they leave once it has landed");
        }
        // Laser is neutral against Mechanical: three pins of 67-83.
        let dealt = damage_to(&effects, 1);
        assert!((200..=250).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn s1s_pins_aim_half_a_centimeter_high_when_they_fly_leftward() {
        let mut game = game_of(vec![
            seat(Team::A, MobileKind::Needlemouse, 1200, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 2300, Facing::Left),
        ]);
        let effects = shoot(&mut game, 0, Shot::S1, 40, Power::nearest(1.6));
        let guide = guide(&effects);
        let (from, _, velocity) = pins(&effects)[1];
        assert!((at_x(from, velocity, guide.x).y - (guide.y - 13.0)).abs() < 1e-6, "{from:?} along {velocity:?}, guide {guide:?}");
    }

    #[test]
    fn s2_fires_five_pins_from_twice_as_high() {
        let mut game = duel(800);
        let home = game.players()[0].footing.world() - Vec2::new(0.0, 300.0);
        let power = tracer_on_target(&game, 40);
        let effects = shoot(&mut game, 0, Shot::S2, 40, power);
        let pins = pins(&effects);
        assert_eq!(pins.len(), 5, "{effects:?}");
        assert_eq!(pins[2].0, home);
        assert!((pins[0].0.distance(home) - 30.0).abs() < 1e-9);
        assert!(damage_to(&effects, 1) >= 230, "{effects:?}");
    }

    #[test]
    fn ss_glides_the_cushion_over_the_tracer_and_sweeps_seven_pins_down_onto_it() {
        let mut game = duel(1200);
        let effects = shoot(&mut game, 0, Shot::Ss, 40, Power::nearest(1.8));
        let guide = guide(&effects);
        let over = guide - Vec2::new(0.0, 300.0);
        let pins = pins(&effects);
        assert_eq!(pins.len(), 7, "{effects:?}");
        let landed = paths(&effects)[0].last().map(|leg| leg.offset + leg.to).unwrap();
        let swing = [45.0, 22.5, -22.5, -45.0, -22.5, 22.5, 45.0];
        let mut glides = effects.iter().filter_map(|e| if let Effect::Satellite { at, to, .. } = *e { Some((at, to)) } else { None }).skip(1);
        for (i, &(from, tick, velocity)) in pins.iter().enumerate() {
            assert!(from.distance(over - Vec2::new(swing[i], 0.0)) < 1e-9, "{i}: {from:?}");
            let expected = landed + ticks(0.4 + f64::from(u32::try_from(i).unwrap()) / 7.5);
            assert_eq!(tick, expected, "{i}");
            assert!(at_x(from, velocity, guide.x).distance(guide) < 1e-6, "{i}: at the tracer");
            assert_eq!(glides.next(), Some((from, tick)), "{i}: the cushion is there as it leaves");
        }
    }

    #[test]
    fn ss_on_target_lands_its_range() {
        let mut game = duel(800);
        let power = tracer_on_target(&game, 40);
        let effects = shoot(&mut game, 0, Shot::Ss, 40, power);
        let dealt = damage_to(&effects, 1);
        assert!((400..=550).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn ground_in_a_pins_way_takes_it() {
        // Needlemouse at 300 and the target at 1000 on flat ground at 500, a pillar between them up
        // to row 370 at x 600 to 640: the tracer lobs over it, and S1's low pins meet it.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((600..640).contains(&x) && y >= 370)).unwrap();
        let mut game = game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Needlemouse, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1000, Facing::Left)],
        );
        let power = tracer_on_target(&game, 55);
        let effects = shoot(&mut game, 0, Shot::S1, 55, power);
        assert_eq!(damage_to(&effects, 1), 0, "{effects:?}");
        for legs in paths(&effects).iter().skip(1) {
            let last = legs.last().unwrap();
            assert!((600.0..=640.0).contains(&last.flight.at(last.to).x), "the pillar took it: {legs:?}");
        }
    }

    #[test]
    fn a_pin_a_tornado_flings_off_is_gone_within_its_life() {
        let mut game = duel(800);
        let x = game.players()[0].footing.x + 400;
        game.set_columns(vec![Column::new(WeatherKind::Tornado, x)]);
        let effects = shoot(&mut game, 0, Shot::S1, 40, Power::nearest(2.4));
        for legs in paths(&effects).iter().skip(1) {
            let (first, last) = (legs[0], legs.last().unwrap());
            assert!(last.offset + last.to <= first.offset + 3 * TICKS_PER_SECOND, "{legs:?}");
        }
    }

    #[test]
    fn every_pin_that_lands_calls_thunder() {
        let mut game = duel(800);
        game.players[0].items = Loadout::new(&[Item::Thunder]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 40, Power::nearest(2.0));
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count(), 3, "{effects:?}");
    }

    #[test]
    fn searchbot_aims_by_where_the_middle_pin_strikes() {
        let game = duel(800);
        let flight = game.launch(0, Shot::S2, 40, tracer_on_target(&game, 40).bars()).unwrap();
        assert_eq!(aim_point(&game, 0, Shot::S2, flight).contact, Contact::Mobile(1));
        let wide = game.launch(0, Shot::S1, 40, Power::nearest(1.2).bars()).unwrap();
        assert_eq!(aim_point(&game, 0, Shot::S1, wide).contact, Contact::Terrain);
    }
}
