//! Tag (PRD-0007 T-002): each player brings two mobiles and plays one at a time, switching in the
//! one sitting out once a turn. The player is out when the mobile in play falls, "regardless of the
//! other mobile" (SW-modes): GunBound Classic's rule, not DragonBound's switch to the backup at half
//! HP. The mobile sitting out is off the map: nothing touches it, and nothing on it changes until
//! it switches back in. Sources and every pick are in `docs/research/modes-tag.md`.
//!
//! `ModeState` keeps each seat's bench; `Game` asks here whether a switch is allowed and makes it.

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    base::Void,
    game::{Effect, Game, MobileKind, Seat},
    items::percent_of,
    mode::Mode,
    tuning::modes::TAG_SECOND_HP_PERCENT,
};

/// A Tag player's mobile sitting out, and what it keeps there until it switches back in: its HP,
/// shield, SS lock and brittleness, none of which change while it is off the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bench {
    pub mobile: MobileKind,
    pub hp: i32,
    pub shield: i32,
    pub ss_lock: u8,
    pub brittle: u8,
    /// The turn count (`Game::turns`) during the seat's last switch: it switches once a turn.
    pub switched: Option<i64>,
}

impl Bench {
    /// A player's second `mobile` as a game starts: on half HP (`TAG_SECOND_HP_PERCENT`), its shield
    /// full and its SS ready, and no switch made yet.
    pub(crate) fn fresh(mobile: MobileKind) -> Self {
        let tuning = mobile.tuning();
        Self {
            mobile,
            hp: percent_of(tuning.hp.value, TAG_SECOND_HP_PERCENT.value).max(1),
            shield: tuning.shield.value,
            ss_lock: 0,
            brittle: 0,
            switched: None,
        }
    }
}

/// Checks each seat brings the mobiles `mode` plays with: two in Tag, one in every other mode.
pub(crate) fn check_seats(mode: Mode, seats: &[Seat]) -> Void {
    let tag = mode == Mode::Tag;
    for (index, seat) in seats.iter().enumerate() {
        match (tag, seat.second) {
            (true, None) => bail!("seat {index} needs a second mobile in Tag"),
            (false, Some(_)) => bail!("seat {index} brings a second mobile, but only Tag plays two"),
            _ => {}
        }
    }
    Ok(())
}

/// Whether `seat` may switch in its mobile sitting out (`Event::Switch`), on its turn (the caller
/// checks that): in Tag, and once a turn (SW-modes).
pub(crate) fn check_switch(game: &Game, seat: u8) -> Void {
    ensure!(game.mode() == Mode::Tag, "there's no switching in {}: one mobile each", game.mode().name());
    let Some(bench) = game.bench(seat) else { bail!("seat {seat} has no second mobile") };
    ensure!(bench.switched != Some(game.turns()), "seat {seat} has switched this turn: once a turn");
    Ok(())
}

/// `seat` switches: the mobile sitting out stands where the other stood, facing its way, and takes
/// up what it kept on the bench (HP, shield, SS lock, brittleness); the other sits out with its
/// own. The player's loadout, any item armed this turn and its turn clock stay with the player, and
/// the steps walked this turn count against the new mobile's budget. Its mines stay on the map,
/// crawling on its turns as before: they go only when its player is out (`mines::orphan`).
pub(crate) fn switch(game: &mut Game, seat: u8, effects: &mut Vec<Effect>) {
    let turns = game.turns();
    let index = usize::from(seat);
    let (Some(bench), Some(player)) = (game.mode_state.bench_mut(seat), game.players.get_mut(index)) else {
        return;
    };
    let walked = player.tuning().move_budget.value.saturating_sub(player.moves_left);
    let out = Bench {
        mobile: player.mobile,
        hp: player.hp,
        shield: player.shield,
        ss_lock: player.ss_lock,
        brittle: player.brittle,
        switched: Some(turns),
    };
    player.mobile = bench.mobile;
    player.hp = bench.hp;
    player.shield = bench.shield;
    player.ss_lock = bench.ss_lock;
    player.brittle = bench.brittle;
    player.moves_left = player.tuning().move_budget.value.saturating_sub(walked);
    let switched = Effect::Switched {
        seat,
        mobile: player.mobile,
        hp: player.hp,
        shield: player.shield,
        brittle: player.brittle,
    };
    let footing = player.footing;
    *bench = out;
    game.place(seat, footing);
    effects.push(switched);
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        base::Res,
        game::{
            Death, Event, Phase, Setup, Shot, Team,
            testing::{answer_rolls, duel_setup, game_in, pass, pass_until, seat},
        },
        map::NoAssets,
        mines,
        terrain::Terrain,
        tuning::frostfin,
        units::{Centis, Facing, Vec2},
    };

    /// Where the pit is: a shaft off the map just right of seat 0, at x = 300.
    const PIT: std::ops::Range<i32> = 330..350;

    /// Flat ground at y = 500 with a shaft through it at `PIT`.
    fn pitted() -> Terrain {
        Terrain::from_fn(2400, 600, |x, y| y >= 500 && !PIT.contains(&i32::try_from(x).unwrap())).unwrap()
    }

    /// A Tag game on `pitted` ground, a pair of mobiles a seat at `spawns`, alternating teams from
    /// A, acting in seat order.
    fn tag(pairs: &[(MobileKind, MobileKind)], spawns: &[i32]) -> Game {
        let seats = pairs
            .iter()
            .zip(spawns)
            .enumerate()
            .map(|(i, (&(first, second), &x))| {
                let (team, facing) = if i % 2 == 0 { (Team::A, Facing::Right) } else { (Team::B, Facing::Left) };
                Seat {
                    second: Some(second),
                    ..seat(team, first, x, facing)
                }
            })
            .collect();
        game_in(Mode::Tag, pitted(), seats)
    }

    /// A Tag duel: seat 0 plays `first` with `second` sitting out, at x = 300; seat 1 a Coilback with
    /// a Lanternwing, at x = 1500.
    fn duel(first: MobileKind, second: MobileKind) -> Game {
        tag(&[(first, second), (MobileKind::Coilback, MobileKind::Lanternwing)], &[300, 1500])
    }

    fn switch(game: &mut Game, seat: u8) -> Res<Vec<Effect>> {
        game.apply(Event::Switch { seat })
    }

    #[test]
    fn in_tag_every_seat_brings_two_mobiles_and_in_no_other_mode_does() {
        let game = duel(MobileKind::Coilback, MobileKind::Hookclaw);
        assert_eq!(game.mode(), Mode::Tag);
        assert_eq!(game.bench(0).map(|bench| bench.mobile), Some(MobileKind::Hookclaw));
        assert_eq!(game.bench(1).map(|bench| bench.mobile), Some(MobileKind::Lanternwing));
        let one = Setup { mode: Mode::Tag, ..duel_setup(800) };
        assert_eq!(Game::new(&one, &NoAssets).unwrap_err().to_string(), "seat 0 needs a second mobile in Tag");
        let mut two = duel_setup(800);
        two.seats[1].second = Some(MobileKind::Hookclaw);
        assert_eq!(Game::new(&two, &NoAssets).unwrap_err().to_string(), "seat 1 brings a second mobile, but only Tag plays two");
        let solo = Game::new(&duel_setup(800), &NoAssets).unwrap();
        assert_eq!(solo.bench(0), None);
    }

    #[test]
    fn the_second_mobile_starts_on_half_its_hp_and_its_shield_full() {
        let game = duel(MobileKind::Coilback, MobileKind::Sparkhorn);
        let (first, second) = (MobileKind::Coilback.tuning(), MobileKind::Sparkhorn.tuning());
        assert_eq!(game.players()[0].hp, first.hp.value, "the first is whole (SW-modes)");
        let bench = game.bench(0).unwrap();
        assert_eq!((bench.hp, bench.shield), (second.hp.value / 2, second.shield.value));
        assert_eq!((bench.ss_lock, bench.brittle, bench.switched), (0, 0, None));
    }

    #[test]
    fn a_switch_stands_the_other_mobile_where_the_first_stood_and_benches_the_first() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Lanternwing);
        let before = game.players()[0];
        let effects = switch(&mut game, 0).unwrap();
        let lanternwing = MobileKind::Lanternwing.tuning();
        let half = lanternwing.hp.value / 2;
        assert_eq!(
            effects,
            [Effect::Switched {
                seat: 0,
                mobile: MobileKind::Lanternwing,
                hp: half,
                shield: lanternwing.shield.value,
                brittle: 0,
            }]
        );
        let now = game.players()[0];
        assert_eq!(now.mobile, MobileKind::Lanternwing);
        assert_eq!((now.footing, now.facing, now.alive), (before.footing, before.facing, true), "in the first one's place");
        assert_eq!((now.hp, now.shield), (half, lanternwing.shield.value));
        assert_eq!(now.clock, before.clock, "the clock is the player's");
        assert_eq!(now.items, before.items, "and so is the loadout");
        let bench = *game.bench(0).unwrap();
        assert_eq!((bench.mobile, bench.hp, bench.shield), (MobileKind::Coilback, before.hp, before.shield));
        assert_eq!(game.phase(), Phase::Turn { seat: 0 }, "the turn goes on");
    }

    #[test]
    fn a_player_switches_once_a_turn_and_only_on_its_own() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Hookclaw);
        assert!(game.can_switch(0) && !game.can_switch(1));
        assert_eq!(switch(&mut game, 1).unwrap_err().to_string(), "it is seat 0's turn, not seat 1's");
        switch(&mut game, 0).unwrap();
        assert!(!game.can_switch(0));
        let before = game.clone();
        assert_eq!(switch(&mut game, 0).unwrap_err().to_string(), "seat 0 has switched this turn: once a turn");
        assert_eq!(game, before, "a refused switch changes nothing");
        pass(&mut game, 0);
        assert!(game.can_switch(1), "the other player switches on its own turn");
        pass_until(&mut game, 0);
        assert!(game.can_switch(0), "and so does this one, again, next turn");
        switch(&mut game, 0).unwrap();
        assert_eq!(game.players()[0].mobile, MobileKind::Coilback, "back to the first");
    }

    #[test]
    fn only_tag_switches() {
        let mut solo = crate::game::testing::duel(800);
        assert!(!solo.can_switch(0));
        assert_eq!(switch(&mut solo, 0).unwrap_err().to_string(), "there's no switching in Solo: one mobile each");
    }

    #[test]
    fn each_mobile_keeps_its_own_hp_shield_ss_lock_and_brittleness_while_it_sits_out() {
        let mut game = duel(MobileKind::Lanternwing, MobileKind::Coilback);
        let me = &mut game.players[0];
        (me.hp, me.shield, me.ss_lock, me.brittle) = (400, 30, 2, frostfin::BRITTLE_PER_HIT.value);
        switch(&mut game, 0).unwrap();
        let fresh = MobileKind::Coilback.tuning();
        let now = game.players()[0];
        assert_eq!((now.hp, now.shield, now.ss_lock, now.brittle), (fresh.hp.value / 2, 0, 0, 0), "the second is its own mobile");
        let bench = *game.bench(0).unwrap();
        assert_eq!((bench.hp, bench.shield, bench.ss_lock, bench.brittle), (400, 30, 2, frostfin::BRITTLE_PER_HIT.value));
        // Turns go by, the benched mobile's shield doesn't regenerate, nor its SS lock count down.
        for _ in 0..3 {
            pass(&mut game, 0);
            pass_until(&mut game, 0);
        }
        assert_eq!(*game.bench(0).unwrap(), bench, "nothing changes on the bench");
        switch(&mut game, 0).unwrap();
        let back = game.players()[0];
        assert_eq!(
            (back.mobile, back.hp, back.shield, back.ss_lock, back.brittle),
            (MobileKind::Lanternwing, 400, 30, 2, frostfin::BRITTLE_PER_HIT.value)
        );
    }

    #[test]
    fn the_steps_walked_this_turn_count_against_the_new_mobiles_budget() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Needlemouse);
        let walk = Event::Walk { seat: 0, facing: Facing::Left, steps: 30 };
        game.apply(walk).unwrap();
        switch(&mut game, 0).unwrap();
        assert_eq!(game.players()[0].moves_left, MobileKind::Needlemouse.tuning().move_budget.value - 30);
        pass(&mut game, 0);
        pass_until(&mut game, 0);
        assert_eq!(game.players()[0].moves_left, MobileKind::Needlemouse.tuning().move_budget.value, "a new turn, a whole budget");
    }

    #[test]
    fn the_mobile_that_ends_the_turn_pays_its_delay() {
        // Lanternwing's base delay is 500 against Coilback's 520: a pass after switching costs its.
        let mut game = duel(MobileKind::Coilback, MobileKind::Lanternwing);
        let before = game.players()[0].clock.delay;
        switch(&mut game, 0).unwrap();
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        let tuning = MobileKind::Lanternwing.tuning();
        assert_eq!(game.players()[0].clock.delay - before, tuning.base_delay.value + crate::tuning::PASS_DELAY.value);
    }

    #[test]
    fn the_player_is_out_when_the_mobile_in_play_falls_whatever_the_other_has_left() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Hookclaw);
        let effects = game
            .apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 60,
            })
            .unwrap();
        answer_rolls(&mut game);
        assert!(effects.contains(&Effect::Died { seat: 0, death: Death::Bunged }), "{effects:?}");
        assert_eq!(game.bench(0).unwrap().hp, MobileKind::Hookclaw.tuning().hp.value / 2, "the other has all it started with");
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::B) }, "but its player is out (SW-modes)");
    }

    #[test]
    fn the_dead_spin_the_slot_machine_in_tag_and_dont_switch() {
        let mut game = tag(
            &[
                (MobileKind::Coilback, MobileKind::Hookclaw),
                (MobileKind::Coilback, MobileKind::Hookclaw),
                (MobileKind::Coilback, MobileKind::Hookclaw),
                (MobileKind::Coilback, MobileKind::Hookclaw),
            ],
            &[300, 900, 1500, 2100],
        );
        game.apply(Event::Walk {
            seat: 0,
            facing: Facing::Right,
            steps: 60,
        })
        .unwrap();
        answer_rolls(&mut game);
        assert!(!game.players()[0].alive);
        assert!(matches!(game.phase(), Phase::Turn { .. }), "its teammate still stands");
        assert!(game.can_spin(0) && !game.can_respawn(0));
        pass_until(&mut game, 2);
        pass_until(&mut game, 1);
        assert!(!game.can_switch(0), "out is out");
    }

    #[test]
    fn a_tinbadgers_mines_stay_and_crawl_on_its_turns_while_it_sits_out() {
        let mut game = duel(MobileKind::Tinbadger, MobileKind::Coilback);
        // A mine in sense of seat 1, short of its hitbox.
        let target = game.players()[1];
        let x = (0..1500)
            .rev()
            .find(|&x| target.pose().gap(target.tuning().hitbox.value, mines::Mine::middle_at(crate::movement::Footing { x, y: 500 })) > 40.0)
            .unwrap();
        let mut laid = Vec::new();
        mines::lay(&mut game, 0, Vec2::new(f64::from(x) + 0.5, 499.5), Facing::Right, 0, &mut laid);
        switch(&mut game, 0).unwrap();
        assert_eq!(game.mines().len(), 1, "switching out isn't dying: the beetles stay");
        pass(&mut game, 0);
        let effects = pass(&mut game, 1);
        assert!(
            effects.iter().any(|e| matches!(e, Effect::MineCrawled { seat: 0, .. })),
            "and crawl as its seat's turn comes up: {effects:?}"
        );
    }

    #[test]
    fn a_grounded_mobile_switched_in_over_a_mine_sets_it_off_and_a_kitewyrm_floats_over_it() {
        let mut game = duel(MobileKind::Kitewyrm, MobileKind::Coilback);
        let me = game.players()[0];
        #[allow(clippy::cast_possible_truncation)]
        let middle = me.center().x.floor() as i32;
        let mut laid = Vec::new();
        mines::lay(&mut game, 1, Vec2::new(f64::from(middle) + 0.5, 499.5), Facing::Left, 0, &mut laid);
        assert_eq!(game.mines().len(), 1, "it floats over the mine");
        let effects = switch(&mut game, 0).unwrap();
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { tick: Some(_), .. })), "the Coilback sets it off: {effects:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::Damaged { seat: 0, .. })), "{effects:?}");
        assert_eq!(game.phase(), Phase::Turn { seat: 0 }, "a mobile that lives plays on");
    }

    #[test]
    fn a_satellite_hovers_over_the_mobile_in_play() {
        let mut game = duel(MobileKind::Coilback, MobileKind::Lensowl);
        let foot = game.players()[0].footing.world();
        let satellite = |game: &Game| game.players()[0].mobile.satellite(foot, Shot::S1);
        assert_eq!(satellite(&game), None);
        switch(&mut game, 0).unwrap();
        assert!(satellite(&game).is_some_and(|at| at.y < foot.y), "the owl's moon comes in with it");
        let effects = game
            .apply(Event::Shoot {
                seat: 0,
                shot: Shot::S1,
                aim: 45,
                power: crate::units::Power::from_twentieths(40).unwrap(),
                time: Centis(0),
            })
            .unwrap();
        assert!(effects.iter().any(|e| matches!(e, Effect::Laser { seat: 0, .. })), "and fires: {effects:?}");
    }
}
