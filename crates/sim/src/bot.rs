//! SearchBot v0: picks a shot by searching aim and power in the sim, the way a player who has
//! done their homework would. It sees only what a player sees (the game state, including wind and
//! the weather on the map) and nothing the server hasn't rolled yet.
//!
//! Search: S1 over the shot's whole aim range, coarse (every 3 degrees, every 2 power steps), then
//! fine around the best candidate. The whole range, not just the true angles: from a slope or
//! behind a hill only a false angle may clear the ground, and 70% damage beats none. Score:
//! distance from the impact to the nearest enemy's center, with a direct hit on any enemy best (a
//! false-angle hit a little less so) and anything that would hit a teammate worst. Difficulty is
//! aim noise added afterward, from a seed the server rolls, so a bot's turn is still a pure
//! function of its inputs.

use serde::{Deserialize, Serialize};

use crate::{
    game::{Contact, Event, Game, Phase, Shot},
    units::{Centis, Facing, Power},
};

/// How sloppy the bot's hands are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Level {
    /// Up to 3 power steps and 2 degrees off.
    Easy,
    /// Up to 1 power step off.
    Normal,
    /// Exactly what the search found.
    Hard,
}

impl Level {
    fn noise(self) -> (i32, i32) {
        match self {
            Self::Easy => (3, 2),
            Self::Normal => (1, 0),
            Self::Hard => (0, 0),
        }
    }
}

/// What the bot does with `seat`'s turn, as events to apply in order (the server stamps the
/// shot's time). `None` when it isn't `seat`'s turn or a roll is owed.
#[must_use]
pub fn plan(game: &Game, seat: u8, level: Level, seed: u64) -> Option<Vec<Event>> {
    if game.phase() != (Phase::Turn { seat }) || game.pending_roll().is_some() {
        return None;
    }
    let me = *game.players().get(usize::from(seat))?;
    let target = game.players().iter().filter(|p| p.alive && p.team != me.team).min_by_key(|p| (p.footing.x - me.footing.x).abs())?;
    let facing = if target.footing.x >= me.footing.x { Facing::Right } else { Facing::Left };

    // Search as the bot will stand: turned toward the target.
    let mut events = Vec::new();
    let mut scratch = game.clone();
    if me.facing != facing {
        let turn = Event::Walk { seat, facing, steps: 0 };
        scratch.apply(turn).ok()?;
        events.push(turn);
    }

    let shot = Shot::S1;
    let range = shot.tuning(me.tuning()).aim.value;
    let score = |aim: i16, twentieths: u8| -> f64 {
        let Some(flight) = scratch.launch(seat, aim, f64::from(twentieths) / 20.0) else { return f64::MAX };
        let impact = scratch.track(flight, 0, 0).impact;
        match impact.contact {
            Contact::Gone | Contact::Column(_) => f64::MAX / 2.0,
            Contact::Mobile(hit) if scratch.players()[usize::from(hit)].team == me.team => f64::MAX / 4.0,
            Contact::Mobile(_) if range.is_true(aim) => -1000.0 + impact.at.distance(center_of(target)),
            // A false-angle hit deals 70%: still a hit, but a true-angle one is worth finding.
            Contact::Mobile(_) => -700.0 + impact.at.distance(center_of(target)),
            Contact::Terrain => impact.at.distance(center_of(target)),
        }
    };

    let max = Power::MAX.twentieths();
    let coarse = (range.min..=range.max).step_by(3).flat_map(|aim| (0..=max).step_by(2).map(move |power| (aim, power)));
    let best = |candidates: &mut dyn Iterator<Item = (i16, u8)>| candidates.map(|(aim, power)| (score(aim, power), aim, power)).min_by(|a, b| a.0.total_cmp(&b.0));
    let (_, aim, power) = best(&mut coarse.into_iter())?;
    let fine = ((aim - 2).max(range.min)..=(aim + 2).min(range.max)).flat_map(|aim| (power.saturating_sub(3)..=(power + 3).min(max)).map(move |power| (aim, power)));
    let (_, aim, power) = best(&mut fine.into_iter())?;

    let (power_noise, aim_noise) = level.noise();
    let mut rng = XorShift(seed | 1);
    let power = i32::from(power) + rng.within(power_noise);
    let aim = (aim + i16::try_from(rng.within(aim_noise)).unwrap_or(0)).clamp(range.min, range.max);
    let power = Power::from_twentieths(u8::try_from(power.clamp(0, i32::from(max))).ok()?).ok()?;

    events.push(Event::Shoot { seat, shot, aim, power, time: Centis(0) });
    Some(events)
}

fn center_of(player: &crate::game::Player) -> crate::units::Vec2 {
    player.pose().center(player.tuning().hitbox.value)
}

/// xorshift64: a tiny, fully specified generator, so a seed means the same noise on every target.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in `-spread..=spread`.
    fn within(&mut self, spread: i32) -> i32 {
        if spread == 0 {
            return 0;
        }
        let span = u64::try_from(2 * spread + 1).unwrap_or(1);
        i32::try_from(self.next() % span).unwrap_or(0) - spread
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::game::{
        Effect,
        testing::{answer_rolls, duel},
    };

    #[test]
    fn a_hard_bot_hits_its_target() {
        let mut game = duel(700);
        let events = plan(&game, 0, Level::Hard, 7).unwrap();
        let mut damage = 0;
        for event in events {
            for effect in game.apply(event).unwrap() {
                if let Effect::Damaged { seat: 1, amount } = effect {
                    damage += amount;
                }
            }
        }
        assert!(damage >= 120, "a direct hit: {damage}");
    }

    #[test]
    fn it_turns_to_face_its_target_first() {
        let mut game = duel(700);
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        answer_rolls(&mut game);
        // Seat 1 starts facing left, toward seat 0: no turn needed. Face it away and ask again.
        game.apply(Event::Walk { seat: 1, facing: Facing::Right, steps: 0 }).unwrap();
        let events = plan(&game, 1, Level::Hard, 7).unwrap();
        assert_eq!(events[0], Event::Walk { seat: 1, facing: Facing::Left, steps: 0 });
        assert!(matches!(events[1], Event::Shoot { seat: 1, .. }));
    }

    #[test]
    fn it_only_plans_its_own_turn_and_is_a_pure_function_of_its_seed() {
        let game = duel(700);
        assert_eq!(plan(&game, 1, Level::Hard, 7), None);
        assert_eq!(plan(&game, 0, Level::Easy, 99), plan(&game, 0, Level::Easy, 99));
    }

    #[test]
    fn noise_stays_within_its_spread() {
        let mut rng = XorShift(12345);
        let draws: Vec<i32> = (0..1000).map(|_| rng.within(3)).collect();
        assert!(draws.iter().all(|d| (-3..=3).contains(d)));
        assert!((-3..=3).all(|v| draws.contains(&v)), "every value comes up");
        assert_eq!(XorShift(5).within(0), 0);
    }
}

#[cfg(test)]
mod over_a_hill {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::{Effect, MobileKind, Seat, Setup, Team, testing::answer_rolls},
        items::Loadout,
        map::{MapRef, NoAssets},
        units::Wind,
    };

    /// The practice layout: rolling hills (slopes up to about 23 degrees) with a crest between the
    /// two, in strong wind from every side. The true angles alone can't always reach.
    #[test]
    fn a_hard_bot_reaches_its_target_over_a_hill_in_any_wind() {
        for direction in [0_u16, 90, 160, 270] {
            let setup = Setup {
                map: MapRef::Hills {
                    width: 1800,
                    height: 1000,
                    ground: 840,
                    amplitude: 120,
                    period: 900,
                },
                seats: vec![
                    Seat {
                        team: Team::A,
                        mobile: MobileKind::Coilback,
                        spawn_x: 360,
                        facing: Facing::Right,
                        items: Loadout::default(),
                    },
                    Seat {
                        team: Team::B,
                        mobile: MobileKind::Coilback,
                        spawn_x: 1440,
                        facing: Facing::Left,
                        items: Loadout::default(),
                    },
                ],
                order: vec![1, 0],
                wind: Wind::new(24, direction).unwrap(),
                sudden_death: None,
            };
            let mut game = Game::new(&setup, &NoAssets).unwrap();
            answer_rolls(&mut game);
            let damage: i32 = plan(&game, 1, Level::Hard, 7)
                .unwrap()
                .into_iter()
                .flat_map(|event| game.apply(event).unwrap())
                .map(|effect| if let Effect::Damaged { seat: 0, amount } = effect { amount } else { 0 })
                .sum();
            assert!(damage > 0, "wind toward {direction}: the bot should reach its target");
        }
    }
}
