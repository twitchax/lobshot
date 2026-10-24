//! Lensowl (A.Sate): every shot is a tracer, and once it lands the satellite fires lasers at it
//! (`satellite`). S1 is one laser from low over Lensowl, aimed high when it fires leftward; S2
//! three from three times as high, raining down left to right onto the tracer; the SS glides the
//! satellite over the landing and sweeps five lasers straight down, left to right.

use super::{
    Gun, beam,
    satellite::{self, Hit},
};
use crate::{
    ballistics::Flight,
    game::{Contact, Effect, Game, Impact, Shot},
    tuning::lensowl::{
        LASER_DELAY_SECONDS, LASER_GAP_SECONDS, RISE_SECONDS, S1_BLAST, S1_DAMAGE, S1_HIGH, S2_BLAST, S2_DAMAGE, S2_LASERS, S2_LEAN_DEGREES, SS_BLAST, SS_DAMAGE, SS_GLIDE_SECONDS, SS_LASERS, SS_STEP,
    },
    units::{Vec2, ticks, to_radians},
};

/// Straight down, for the SS's lasers: a point one pixel below where one fires from.
const DOWN: Vec2 = Vec2::new(0.0, 1.0);

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(home) = satellite::home(game, gun.seat, shot) else { return };
    let track = satellite::tracer(game, gun, effects);
    satellite::glide(gun, home, 0, ticks(RISE_SECONDS.value), effects);
    let Some(track) = track else { return };
    let guide = track.impact.at;
    let gap = ticks(LASER_GAP_SECONDS.value);
    match shot {
        Shot::S1 => {
            let (from, through) = lead(home, shot, guide);
            let tick = track.impact.tick + ticks(LASER_DELAY_SECONDS.value);
            let hit = Hit {
                radius: S1_BLAST.value,
                damage: S1_DAMAGE.value,
            };
            satellite::laser(game, gun, from, through, tick, hit, effects);
        }
        Shot::S2 => {
            let first = track.impact.tick + ticks(LASER_DELAY_SECONDS.value);
            let hit = Hit {
                radius: S2_BLAST.value,
                damage: S2_DAMAGE.value,
            };
            for (order, leans) in (0..S2_LASERS.value).rev().enumerate() {
                let through = leaned(home, guide, to_radians(S2_LEAN_DEGREES.value) * f64::from(leans));
                let tick = first + gap * u32::try_from(order).unwrap_or(0);
                satellite::laser(game, gun, home, through, tick, hit, effects);
            }
        }
        Shot::Ss => {
            // It keeps its height, gliding sideways to the first of its steps over the landing.
            let span = SS_STEP.value * f64::from(SS_LASERS.value - 1);
            let start = Vec2::new(guide.x - span / 2.0, home.y);
            let arrived = track.impact.tick + ticks(SS_GLIDE_SECONDS.value);
            satellite::glide(gun, start, track.impact.tick, arrived, effects);
            let hit = Hit {
                radius: SS_BLAST.value,
                damage: SS_DAMAGE.value,
            };
            let mut tick = arrived + ticks(LASER_DELAY_SECONDS.value);
            for step in 0..SS_LASERS.value {
                let at = start + Vec2::new(SS_STEP.value * f64::from(step), 0.0);
                if step > 0 {
                    satellite::glide(gun, at, tick, tick + gap, effects);
                    tick += gap;
                }
                satellite::laser(game, gun, at, at + DOWN, tick, hit, effects);
            }
        }
    }
}

/// The lead laser's line, from where it fires through where it aims, for a tracer that landed at
/// `guide` with the satellite at `home`: S1's one, aimed high when it fires leftward (`S1_HIGH`);
/// S2's last, straight at the tracer; the SS's middle one, straight down over it.
fn lead(home: Vec2, shot: Shot, guide: Vec2) -> (Vec2, Vec2) {
    match shot {
        Shot::S1 => (home, satellite::sighted(home, guide, S1_HIGH.value)),
        Shot::S2 => (home, guide),
        Shot::Ss => {
            let over = Vec2::new(guide.x, home.y);
            (over, over + DOWN)
        }
    }
}

/// A point on the line from `from` through `through`, turned `lean` radians about `from` toward
/// the left of the screen.
fn leaned(from: Vec2, through: Vec2, lean: f64) -> Vec2 {
    let path = through - from;
    // `rotated` turns counterclockwise on screen: right for a beam heading down, left for one
    // heading up.
    let toward_left = if path.y >= 0.0 { -lean } else { lean };
    from + path.rotated(toward_left)
}

/// Where the lead laser strikes if `seat` fires `shot`'s tracer along `flight`, for SearchBot's aim
/// pass: that is where the shot does its work. Where the tracer went if it left the map.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight) -> Impact {
    let impact = game.track(flight, 0, 0).impact;
    if !matches!(impact.contact, Contact::Terrain | Contact::Mobile(_)) {
        return impact;
    }
    let Some(home) = satellite::home(game, seat, shot) else { return impact };
    let (from, through) = lead(home, shot, impact.at);
    beam::strike(game, from, through).map_or(impact, |strike| Impact {
        tick: impact.tick,
        at: strike.at,
        contact: strike.contact,
    })
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Event, MobileKind, Team,
            testing::{damage_to, direct_hit_power, duel_of, explosions, game_of, game_on, seat, shoot},
        },
        items::{Item, Loadout},
        movement::Footing,
        terrain::Terrain,
        tuning::lensowl::{S2_RISE, SATELLITE_HEIGHT, SS_RISE},
        units::{Centis, Facing, Power},
        weather::{Column, WeatherKind},
    };

    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Lensowl, MobileKind::Coilback, distance)
    }

    /// Each laser, as (from, to, tick).
    fn lasers(effects: &[Effect]) -> Vec<(Vec2, Vec2, u32)> {
        effects
            .iter()
            .filter_map(|e| if let Effect::Laser { from, to, tick, .. } = *e { Some((from, to, tick)) } else { None })
            .collect()
    }

    /// Where the tracer landed.
    fn guide(effects: &[Effect]) -> Vec2 {
        effects
            .iter()
            .find_map(|e| if let Effect::Fired { legs, .. } = e { legs.last().map(|leg| leg.flight.at(leg.to)) } else { None })
            .unwrap()
    }

    /// Where `seat`'s feet are, in the world.
    fn feet(game: &Game, seat: usize) -> Vec2 {
        game.players()[seat].footing.world()
    }

    /// The line through `from` and `to` at world x `x`.
    fn height_at(from: Vec2, to: Vec2, x: f64) -> f64 {
        from.y + (to.y - from.y) * (x - from.x) / (to.x - from.x)
    }

    #[test]
    fn the_satellite_hovers_at_each_shots_height() {
        let foot = Vec2::new(500.0, 600.0);
        let over = |height: f64| Some(Vec2::new(500.0, 600.0 - height));
        // Evenly spaced over it: S1's, the SS's, S2's (GF-asate2).
        assert_eq!(MobileKind::Lensowl.satellite(foot, Shot::S1), over(130.0));
        assert_eq!(MobileKind::Lensowl.satellite(foot, Shot::Ss), over(260.0));
        assert_eq!(MobileKind::Lensowl.satellite(foot, Shot::S2), over(390.0));
        assert_eq!(MobileKind::Lensowl.satellite(foot, Shot::S2), over(SATELLITE_HEIGHT.value * S2_RISE.value));
        assert_eq!(MobileKind::Lensowl.satellite(foot, Shot::Ss), over(SATELLITE_HEIGHT.value * SS_RISE.value));
        assert_eq!(MobileKind::Coilback.satellite(foot, Shot::S1), None);
    }

    #[test]
    fn s1s_tracer_hurts_nothing_and_the_satellite_lasers_where_it_lands() {
        let mut game = duel(800);
        let home = feet(&game, 0) - Vec2::new(0.0, 130.0);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S1, 30, power);
        assert!(matches!(effects[1], Effect::Fired { seat: 0, .. }), "the tracer first: {effects:?}");
        assert!(
            effects.contains(&Effect::Satellite {
                seat: 0,
                at: home,
                from: 0,
                to: ticks(0.25)
            }),
            "{effects:?}"
        );
        let lasers = lasers(&effects);
        assert_eq!(lasers.len(), 1, "{effects:?}");
        assert_eq!(lasers[0].0, home, "from the satellite over Lensowl");
        assert_eq!(explosions(&effects), 1, "only the laser bursts: {effects:?}");
        // Laser is neutral against Mechanical.
        let dealt = damage_to(&effects, 1);
        assert!((100..=160).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn s1_aims_half_a_centimeter_high_only_when_it_fires_leftward() {
        for (facing, high) in [(Facing::Right, 0.0), (Facing::Left, 13.0)] {
            // Mid-map, a long way from the target, so the tracer lands on open ground.
            let mut game = game_of(vec![seat(Team::A, MobileKind::Lensowl, 1200, facing), seat(Team::B, MobileKind::Coilback, 2300, Facing::Left)]);
            let effects = shoot(&mut game, 0, Shot::S1, 40, Power::nearest(1.6));
            let guide = guide(&effects);
            let (from, to, _) = lasers(&effects)[0];
            assert!((height_at(from, to, guide.x) - (guide.y - high)).abs() < 1e-6, "{facing:?}: {from:?} to {to:?}, guide {guide:?}");
            assert!((to.x - guide.x).abs() < 1.0 || high > 0.0, "{facing:?}: on target from the left");
            if high > 0.0 {
                assert!(to.x < guide.x - 20.0, "from the right it overshoots, past the guide: {to:?} against {guide:?}");
            }
        }
    }

    /// Lensowl at 300 and a Coilback at 1000 on flat ground at 500, a pillar between them up to
    /// row 400 at x 600 to 640.
    fn behind_a_pillar() -> Game {
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((600..640).contains(&x) && y >= 400)).unwrap();
        game_on(
            terrain,
            vec![seat(Team::A, MobileKind::Lensowl, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1000, Facing::Left)],
        )
    }

    /// The power step whose tracer from seat 0 at `aim` strikes seat 1, if any.
    fn tracer_on_target(game: &Game, shot: Shot, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, shot, aim, power.bars())?, 0, 0).impact;
                (impact.contact == crate::game::Contact::Mobile(1)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .unwrap()
    }

    #[test]
    fn ground_between_the_satellite_and_the_tracer_takes_the_laser() {
        let mut game = behind_a_pillar();
        let power = tracer_on_target(&game, Shot::S1, 55);
        let effects = shoot(&mut game, 0, Shot::S1, 55, power);
        let (_, to, _) = lasers(&effects)[0];
        assert!((600.0..=640.0).contains(&to.x), "the pillar took it: {to:?}");
        assert_eq!(damage_to(&effects, 1), 0, "{effects:?}");
        // S2's satellite, three times as high, clears it.
        let mut game = behind_a_pillar();
        let power = tracer_on_target(&game, Shot::S2, 55);
        let effects = shoot(&mut game, 0, Shot::S2, 55, power);
        assert!(damage_to(&effects, 1) > 0, "{effects:?}");
    }

    #[test]
    fn s2_rains_three_lasers_from_left_to_right_onto_the_tracer_whichever_way_it_faces() {
        for facing in [Facing::Right, Facing::Left] {
            let mut game = game_of(vec![seat(Team::A, MobileKind::Lensowl, 1200, facing), seat(Team::B, MobileKind::Coilback, 2300, Facing::Left)]);
            let home = feet(&game, 0) - Vec2::new(0.0, 390.0);
            let effects = shoot(&mut game, 0, Shot::S2, 40, Power::nearest(1.6));
            let guide = guide(&effects);
            let lasers = lasers(&effects);
            assert_eq!(lasers.len(), 3, "{effects:?}");
            assert!(lasers.iter().all(|&(from, ..)| from == home), "three times as high: {lasers:?}");
            let xs: Vec<f64> = lasers.iter().map(|&(_, to, _)| to.x).collect();
            assert!(xs[0] < xs[1] && xs[1] < xs[2], "{facing:?}: left to right: {xs:?}");
            assert!((xs[2] - guide.x).abs() < 1.0, "{facing:?}: the last at the tracer: {xs:?} against {guide:?}");
            let ticks: Vec<u32> = lasers.iter().map(|&(.., tick)| tick).collect();
            assert!(ticks[0] < ticks[1] && ticks[1] < ticks[2], "one after another: {ticks:?}");
        }
    }

    #[test]
    fn s2s_lasers_bunch_close_in_and_spread_far_off() {
        let spread = |power: f64| {
            let mut game = game_of(vec![seat(Team::A, MobileKind::Lensowl, 600, Facing::Right), seat(Team::B, MobileKind::Coilback, 2300, Facing::Left)]);
            let effects = shoot(&mut game, 0, Shot::S2, 45, Power::nearest(power));
            let lasers = lasers(&effects);
            (lasers[2].1.x - lasers[0].1.x, guide(&effects).x - 600.0)
        };
        let (near, near_distance) = spread(1.0);
        let (far, far_distance) = spread(2.4);
        assert!(near_distance < 300.0 && far_distance > 700.0, "{near_distance} and {far_distance}");
        assert!(near < 40.0, "bunched: {near}");
        assert!(far > 2.5 * near, "spread: {far} against {near}");
    }

    #[test]
    fn ss_glides_over_the_landing_and_sweeps_five_lasers_straight_down() {
        let mut game = duel(1200);
        let level = feet(&game, 0).y - 260.0;
        let effects = shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(1.6));
        let guide = guide(&effects);
        let lasers = lasers(&effects);
        assert_eq!(lasers.len(), 5, "{effects:?}");
        for (step, &(from, to, _)) in (0_u32..).zip(&lasers) {
            assert_eq!(from, Vec2::new(guide.x - 26.0 + 13.0 * f64::from(step), level), "{step}: from the left, a step apart");
            assert!((to.x - from.x).abs() < 1e-9 && to.y > from.y, "{step}: straight down");
        }
        let glides: Vec<Vec2> = effects.iter().filter_map(|e| if let Effect::Satellite { at, .. } = *e { Some(at) } else { None }).collect();
        assert_eq!(glides.len(), 6, "up to its height, over the landing, then a step at a time: {glides:?}");
        assert_eq!(glides[1], lasers[0].0);
    }

    #[test]
    fn ss_on_target_lands_its_whole_sweep() {
        let mut game = duel(800);
        let power = tracer_on_target(&game, Shot::Ss, 40);
        let effects = shoot(&mut game, 0, Shot::Ss, 40, power);
        let dealt = damage_to(&effects, 1);
        assert!((400..=550).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn every_laser_that_lands_calls_thunder() {
        let mut game = duel(800);
        game.players[0].items = Loadout::new(&[Item::Thunder]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S2, 40, Power::nearest(2.0));
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count(), 3, "{effects:?}");
    }

    #[test]
    fn a_laser_that_crosses_force_hits_harder() {
        let mut calm = duel(800);
        let power = direct_hit_power(&calm, 30);
        let mut forced = calm.clone();
        // Behind Lensowl: the tracer never passes it, but its laser, from over Lensowl, does.
        let x = forced.players()[0].footing.x + 10;
        forced.set_columns(vec![Column::new(WeatherKind::Force, x)]);
        let plain = damage_to(&shoot(&mut calm, 0, Shot::S1, 30, power), 1);
        let boosted = damage_to(&shoot(&mut forced, 0, Shot::S1, 30, power), 1);
        assert!((f64::from(boosted) / f64::from(plain) - 1.33).abs() < 0.02, "{boosted} against {plain}");
    }

    #[test]
    fn a_tracer_off_the_map_calls_no_laser() {
        let mut game = duel(800);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 30, Power::MAX);
        assert!(lasers(&effects).is_empty(), "{effects:?}");
        assert_eq!(explosions(&effects), 0);
    }

    #[test]
    fn searchbot_aims_by_where_the_lead_laser_strikes() {
        let game = behind_a_pillar();
        let power = tracer_on_target(&game, Shot::S1, 55);
        let flight = game.launch(0, Shot::S1, 55, power.bars()).unwrap();
        let impact = aim_point(&game, 0, Shot::S1, flight);
        assert_eq!(impact.contact, crate::game::Contact::Terrain, "the pillar: {impact:?}");
        assert!((600.0..=640.0).contains(&impact.at.x), "{impact:?}");
        let flight = game.launch(0, Shot::S2, 55, tracer_on_target(&game, Shot::S2, 55).bars()).unwrap();
        assert_eq!(aim_point(&game, 0, Shot::S2, flight).contact, crate::game::Contact::Mobile(1));
    }

    #[test]
    fn a_satellite_over_its_feet_moves_with_it() {
        let mut game = duel(800);
        game.place(0, Footing { x: 420, y: 500 });
        let effects = shoot(&mut game, 0, Shot::S1, 30, Power::nearest(2.0));
        assert_eq!(lasers(&effects)[0].0, Vec2::new(420.5, 370.0));
    }
}
