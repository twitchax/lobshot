//! Frostfin (Ice): an icicle from the cannon that digs a wide hole; a ball of ice from the tusk,
//! a lower muzzle that can jam in a wall the cannon clears; and a snowflake from the tusk that
//! bursts wide. Every mobile the ball or the snowflake hurts turns brittle: Physical damage against
//! it is stronger for the rest of the game (`damage::Target::against`).

use super::Gun;
use crate::{
    ballistics::Flight,
    game::{Effect, Game, Shot},
    tuning::{
        DamageRange,
        frostfin::{BRITTLE_MAX, BRITTLE_PER_HIT, S1_BLAST, S1_CRATER, S1_DAMAGE, S2_BLAST, S2_DAMAGE, SS_BLAST, SS_CRATER, SS_DAMAGE},
    },
};

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    // `gun` launches from the shot's barrel: S2 and SS leave the tusk.
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 => {
            ice(game, gun, flight, (S1_BLAST.value, S1_CRATER.value, S1_DAMAGE.value), effects);
        }
        Shot::S2 => {
            let start = effects.len();
            ice(game, gun, flight, (S2_BLAST.value, S2_BLAST.value, S2_DAMAGE.value), effects);
            frost(game, start, effects);
        }
        Shot::Ss => {
            let start = effects.len();
            ice(game, gun, flight, (SS_BLAST.value, SS_CRATER.value, SS_DAMAGE.value), effects);
            frost(game, start, effects);
        }
    }
}

/// One projectile of ice: it flies, and a blast of `(radius, crater, damage)` goes off where it
/// lands.
fn ice(game: &mut Game, gun: &Gun, flight: Flight, (radius, crater, damage): (f64, f64, DamageRange), effects: &mut Vec<Effect>) {
    let track = game.track(flight, 0, 0);
    effects.push(Effect::Fired { seat: gun.seat, legs: track.legs.clone() });
    gun.land(game, &gun.blast_on(&track, radius, damage), crater, &track.impact, track.marks, effects);
}

/// Leaves every living mobile the projectile's blast hurt (`effects[start..]`, up to any bolt it
/// called down: `Gun::land` reports the blast's damage first) `BRITTLE_PER_HIT` more brittle, up to
/// `BRITTLE_MAX`, each `Frosted` just after the blast's damage. A shield soaking the hit still
/// counts: any damage frosts (WP06).
fn frost(game: &mut Game, start: usize, effects: &mut Vec<Effect>) {
    let blast = &effects[start..];
    let end = start + blast.iter().position(|e| matches!(e, Effect::Bolt { .. })).unwrap_or(blast.len());
    let mut hurt: Vec<u8> = Vec::new();
    for effect in &effects[start..end] {
        if let Effect::Damaged { seat, .. } | Effect::Shielded { seat, .. } = *effect
            && !hurt.contains(&seat)
        {
            hurt.push(seat);
        }
    }
    let mut frosted = Vec::new();
    for seat in hurt {
        let player = &mut game.players[usize::from(seat)];
        let brittle = player.brittle.saturating_add(BRITTLE_PER_HIT.value).min(BRITTLE_MAX.value);
        if player.alive && brittle > player.brittle {
            player.brittle = brittle;
            frosted.push(Effect::Frosted { seat, brittle });
        }
    }
    effects.splice(end..end, frosted);
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{
            Contact, Effect, Event, Game, MobileKind, Shot, Team,
            testing::{damage_to, direct_hit_power, direct_hit_power_of, duel_of, game_of, game_on, seat, shoot},
        },
        items::{Item, Loadout},
        terrain::Terrain,
        units::{Centis, Facing, Power},
    };

    fn duel() -> Game {
        duel_of(MobileKind::Frostfin, MobileKind::Coilback, 800)
    }

    fn frosted(effects: &[Effect]) -> Vec<(u8, u8)> {
        effects
            .iter()
            .filter_map(|e| if let Effect::Frosted { seat, brittle } = *e { Some((seat, brittle)) } else { None })
            .collect()
    }

    fn crater(effects: &[Effect]) -> f64 {
        effects.iter().find_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None }).unwrap()
    }

    #[test]
    fn s1_is_an_icicle_from_the_cannon_that_digs_a_wide_hole() {
        let mut game = duel();
        let muzzle = game.players()[0].pose().muzzle(MobileKind::Frostfin.tuning(), Shot::S1, 45);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::S1, 45, power);
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("{effects:?}")
        };
        assert!(legs[0].flight.origin.distance(muzzle) < 1e-9, "it leaves the cannon");
        // 120-160 x 0.96 (Physical against Mechanical).
        let dealt = damage_to(&effects, 1);
        assert!((115..=153).contains(&dealt), "{dealt}");
        assert!((crater(&effects) - 42.0).abs() < 1e-9, "wider than its 30 px reach");
        assert_eq!(frosted(&effects), [], "only S2 and SS frost: {effects:?}");
        assert_eq!(game.players()[1].brittle, 0);
    }

    #[test]
    fn s2_is_a_ball_of_ice_from_the_tusk_that_leaves_its_target_brittle() {
        let mut game = duel();
        let pose = game.players()[0].pose();
        let tusk = pose.muzzle(MobileKind::Frostfin.tuning(), Shot::S2, 45);
        assert!(tusk.y > pose.muzzle(MobileKind::Frostfin.tuning(), Shot::S1, 45).y, "the lower muzzle");
        let power = direct_hit_power_of(&game, Shot::S2, 45);
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        let Some(Effect::Fired { legs, .. }) = effects.iter().find(|e| matches!(e, Effect::Fired { .. })) else {
            panic!("{effects:?}")
        };
        assert!(legs[0].flight.origin.distance(tusk) < 1e-9, "it leaves the tusk");
        // 150-170 x 0.96.
        let dealt = damage_to(&effects, 1);
        assert!((144..=163).contains(&dealt), "{dealt}");
        assert_eq!(frosted(&effects), [(1, 5)]);
        let damaged = effects.iter().position(|e| matches!(e, Effect::Damaged { seat: 1, .. })).unwrap();
        let frost = effects.iter().position(|e| matches!(e, Effect::Frosted { .. })).unwrap();
        assert!(damaged < frost, "hurt, then frosted: {effects:?}");
        assert_eq!(game.players()[1].brittle, 5);
    }

    #[test]
    fn brittleness_stacks_to_its_cap_and_lasts() {
        let mut game = duel();
        let power = direct_hit_power_of(&game, Shot::S2, 45);
        game.players[1].brittle = 45;
        let mut next = game.clone();
        assert_eq!(frosted(&shoot(&mut next, 0, Shot::S2, 45, power)), [(1, 50)]);
        game.players[1].brittle = 50;
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        assert!(damage_to(&effects, 1) > 0);
        assert_eq!(frosted(&effects), [], "already as brittle as it gets: {effects:?}");
        assert_eq!(game.players()[1].brittle, 50);
        // The target's own turn changes nothing.
        crate::game::testing::pass(&mut game, 1);
        assert_eq!(game.players()[1].brittle, 50);
    }

    #[test]
    fn a_brittle_mobile_takes_more_from_every_physical_shot_and_no_more_from_the_rest() {
        let hit = |shooter: MobileKind, brittle: u8| {
            let mut game = duel_of(shooter, MobileKind::Coilback, 800);
            game.players[1].brittle = brittle;
            let power = direct_hit_power(&game, 40);
            damage_to(&shoot(&mut game, 0, Shot::S1, 40, power), 1)
        };
        // Frostfin's own icicle and Hookclaw's boomerang are Physical; Coilback's shell is not.
        for shooter in [MobileKind::Frostfin, MobileKind::Hookclaw] {
            let (sound, brittle) = (hit(shooter, 0), hit(shooter, 50));
            assert!((f64::from(brittle) / f64::from(sound) - 1.5).abs() < 0.02, "{shooter:?}: {brittle} against {sound}");
        }
        assert_eq!(hit(MobileKind::Coilback, 50), hit(MobileKind::Coilback, 0));
    }

    #[test]
    fn ss_is_a_snowflake_from_the_tusk_that_frosts_everyone_its_wide_blast_hurts() {
        // Two Coilbacks 76 px apart, hitbox to hitbox, and the snowflake on the ground between them.
        let seats = vec![
            seat(Team::A, MobileKind::Frostfin, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1000, Facing::Left),
            seat(Team::B, MobileKind::Coilback, 1140, Facing::Left),
        ];
        let mut game = game_of(seats);
        let between = 1068.0;
        let (aim, power) = (50..=60)
            .flat_map(|aim| (0..=Power::MAX.twentieths()).filter_map(move |twentieths| Some((aim, Power::from_twentieths(twentieths).ok()?))))
            .min_by(|a, b| {
                let off = |&(aim, power): &(i16, Power)| {
                    let impact = game.track(game.launch(0, Shot::Ss, aim, power.bars()).unwrap(), 0, 0).impact;
                    if impact.contact == Contact::Terrain { (impact.at.x - between).abs() } else { f64::MAX }
                };
                off(a).total_cmp(&off(b))
            })
            .unwrap();
        let mut narrow = game.clone();
        let effects = shoot(&mut game, 0, Shot::Ss, aim, power);
        assert!(damage_to(&effects, 1) > 0 && damage_to(&effects, 2) > 0, "{effects:?}");
        assert_eq!(frosted(&effects), [(1, 5), (2, 5)]);
        // An S2 landing there reaches neither.
        let effects = shoot(&mut narrow, 0, Shot::S2, aim, power);
        assert_eq!((damage_to(&effects, 1), damage_to(&effects, 2)), (0, 0), "{effects:?}");
        assert_eq!(frosted(&effects), []);
    }

    #[test]
    fn s2_jams_in_a_wall_that_s1_clears() {
        // Frostfin at x = 300 on ground at y = 600, a 55 px wall just past its nose, and the target
        // beyond it.
        let terrain = Terrain::from_fn(1600, 800, |x, y| y >= 600 || ((335..380).contains(&x) && y >= 545)).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Frostfin, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)];
        let game = game_on(terrain, seats);
        let s2 = game.track(game.launch(0, Shot::S2, 30, 2.4).unwrap(), 0, 0).impact;
        assert_eq!((s2.contact, s2.tick), (Contact::Terrain, 0), "the tusk's tip is in the wall: {s2:?}");
        let s1 = game.track(game.launch(0, Shot::S1, 30, 2.4).unwrap(), 0, 0).impact;
        assert!(s1.at.x > 600.0, "the cannon clears it: {s1:?}");
    }

    #[test]
    fn a_dual_s2_frosts_twice() {
        let mut game = duel();
        let power = direct_hit_power_of(&game, Shot::S2, 45);
        game.players[0].items = Loadout::new(&[Item::Dual]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        assert_eq!(frosted(&effects), [(1, 5), (1, 10)], "{effects:?}");
    }
}
