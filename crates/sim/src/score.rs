//! The bonus tally (T-008): each seat's damage to enemies, its kills, and GunBound's bonuses
//! (mechanics.md section 4) under lobshot's own labels, without their gold. Cosmetic: nothing in
//! the game reads it, and clients show it when the game ends.
//!
//! A tally is a pure function of the log. `Scoreboard::record` scores one event from the state just
//! before it and the effects it had: a shot's damage and kills go to its shooter, a drop's to the
//! seat that spun it, and nothing else (a walk off the map, Sacrifice's cost) goes to anyone. The
//! trick shots are judged on the shot's first projectile, flown again from the state before it.

use serde::{Deserialize, Serialize};

use crate::{
    base::Res,
    game::{Death, Effect, Event, Game, Leg},
    log::Log,
    map::MapAssets,
    tuning::score::{HIGH_ANGLE_AIM, HIGH_ANGLE_SECONDS, HIT_TIERS, MULTI_KILL, TRICK_DAMAGE, ULTRA_HIGH_ANGLE_SECONDS},
    units::seconds,
    weather::WeatherKind,
};

/// A bonus, under lobshot's name (GunBound's in each doc comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Bonus {
    /// Two or more enemies killed by one shot (Shot of God).
    MultiKill,
    /// An enemy knocked out of the map (Bunge).
    Overboard,
    /// An enemy killed (Ending).
    Finisher,
    /// 150 to 249 damage to enemies in one shot (Shot).
    NiceHit,
    /// 250 to 499 (Good).
    GreatHit,
    /// 500 or more (Excellent).
    MassiveHit,
    /// A hit through a tornado (Hurricane).
    Twister,
    /// A hit off a mirror.
    BankShot,
    /// A hit aimed 70 degrees or higher that flew at least 2.5 s (High Angle).
    SkyLob,
    /// The same, flying at least 4 s (Ultra High Angle).
    Moonshot,
    /// A hit the wind turned around and brought back (Boomer Shot).
    Boomerang,
    /// A hit fired away from its target that the wind brought back behind its shooter (Back Shot).
    BehindTheBack,
    /// A teammate killed (Team kill): a penalty.
    FriendlyFire,
}

impl Bonus {
    /// Every bonus, in the order a tally lists them.
    pub const ALL: [Self; 13] = [
        Self::MultiKill,
        Self::Overboard,
        Self::Finisher,
        Self::NiceHit,
        Self::GreatHit,
        Self::MassiveHit,
        Self::Twister,
        Self::BankShot,
        Self::SkyLob,
        Self::Moonshot,
        Self::Boomerang,
        Self::BehindTheBack,
        Self::FriendlyFire,
    ];

    /// The player-facing label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MultiKill => "Multi-kill",
            Self::Overboard => "Overboard",
            Self::Finisher => "Finisher",
            Self::NiceHit => "Nice hit",
            Self::GreatHit => "Great hit",
            Self::MassiveHit => "Massive hit",
            Self::Twister => "Twister",
            Self::BankShot => "Bank shot",
            Self::SkyLob => "Sky lob",
            Self::Moonshot => "Moonshot",
            Self::Boomerang => "Boomerang",
            Self::BehindTheBack => "Behind the back",
            Self::FriendlyFire => "Friendly fire",
        }
    }

    /// Whether it counts against the seat that earned it.
    #[must_use]
    pub const fn is_penalty(self) -> bool {
        matches!(self, Self::FriendlyFire)
    }
}

/// One seat's tally.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// Damage dealt to enemies, shields included.
    pub damage: i32,
    /// Enemies killed.
    pub kills: u32,
    /// Bonuses, in the order they were earned.
    pub bonuses: Vec<Bonus>,
}

impl Tally {
    /// How many times it earned `bonus`.
    #[must_use]
    pub fn count(&self, bonus: Bonus) -> usize {
        self.bonuses.iter().filter(|&&earned| earned == bonus).count()
    }
}

/// Every seat's tally, built event by event.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scoreboard {
    tallies: Vec<Tally>,
}

/// One attack within an event: a shot, or a drop landing, and what it did to whom.
#[derive(Debug, Clone, Copy, Default)]
struct Attack {
    by: Option<u8>,
    damage: i32,
    kills: u32,
    overboard: u32,
    team_kills: u32,
    /// It is the event's shot, so the trick-shot bonuses apply.
    shot: bool,
}

impl Scoreboard {
    /// A scoreboard for `seats` seats, every tally empty.
    #[must_use]
    pub fn new(seats: usize) -> Self {
        Self { tallies: vec![Tally::default(); seats] }
    }

    /// Each seat's tally, by seat.
    #[must_use]
    pub fn tallies(&self) -> &[Tally] {
        &self.tallies
    }

    /// Scores one applied event: `before` is the game as it stood just before `event`, and
    /// `effects` is what applying it returned.
    pub fn record(&mut self, before: &Game, event: &Event, effects: &[Effect]) {
        let shooter = if let Event::Shoot { seat, .. } = *event { Some(seat) } else { None };
        let mut attack = Attack {
            by: shooter,
            shot: true,
            ..Attack::default()
        };
        let team = |seat: u8| before.players().get(usize::from(seat)).map(|p| p.team);
        for effect in effects {
            match *effect {
                Effect::Dropped { seat, .. } => {
                    self.close(before, event, &attack);
                    attack = Attack { by: Some(seat), ..Attack::default() };
                }
                Effect::Damaged { seat, amount } | Effect::Shielded { seat, amount } => {
                    if let Some(by) = attack.by
                        && team(seat) != team(by)
                    {
                        attack.damage += amount;
                    }
                }
                Effect::Died {
                    seat,
                    death: death @ (Death::Blasted | Death::Bunged),
                } => {
                    let Some(by) = attack.by else { continue };
                    if team(seat) != team(by) {
                        attack.kills += 1;
                        attack.overboard += u32::from(death == Death::Bunged);
                    } else if seat != by {
                        attack.team_kills += 1;
                    }
                }
                _ => {}
            }
        }
        self.close(before, event, &attack);
    }

    /// Adds a finished attack to its attacker's tally.
    fn close(&mut self, before: &Game, event: &Event, attack: &Attack) {
        let Some(tally) = attack.by.and_then(|by| self.tallies.get_mut(usize::from(by))) else { return };
        tally.damage += attack.damage;
        tally.kills += attack.kills;
        let repeat = |bonus: Bonus, times: u32| std::iter::repeat_n(bonus, usize::try_from(times).unwrap_or(0));
        tally.bonuses.extend(repeat(Bonus::Finisher, attack.kills));
        tally.bonuses.extend(repeat(Bonus::Overboard, attack.overboard));
        if attack.kills >= MULTI_KILL.value {
            tally.bonuses.push(Bonus::MultiKill);
        }
        let [nice, great, massive] = HIT_TIERS.value;
        let tier = match attack.damage {
            damage if damage >= massive => Some(Bonus::MassiveHit),
            damage if damage >= great => Some(Bonus::GreatHit),
            damage if damage >= nice => Some(Bonus::NiceHit),
            _ => None,
        };
        tally.bonuses.extend(tier);
        if attack.shot && attack.damage >= TRICK_DAMAGE.value {
            tally.bonuses.extend(tricks(before, event));
        }
        tally.bonuses.extend(repeat(Bonus::FriendlyFire, attack.team_kills));
    }
}

/// The trick shots `event` (a shot) was, judged on its first projectile flown from `before`.
fn tricks(before: &Game, event: &Event) -> Vec<Bonus> {
    let Event::Shoot { seat, shot, aim, power, .. } = *event else { return Vec::new() };
    let Some(flight) = before.launch(seat, shot, aim, power.bars()) else { return Vec::new() };
    let track = before.track(flight, 0, 0);
    let columns = &before.weather().columns;
    // Weather starts a new leg where it redirects the shot; its kind is the column the leg starts in.
    let through = |kind: WeatherKind| track.legs.iter().skip(1).any(|leg| columns.iter().any(|c| c.kind == kind && c.contains(leg.flight.origin.x)));
    let mut bonuses = Vec::new();
    if through(WeatherKind::Tornado) {
        bonuses.push(Bonus::Twister);
    }
    if through(WeatherKind::Mirror) {
        bonuses.push(Bonus::BankShot);
    }
    let flown = seconds(track.impact.tick);
    if aim >= HIGH_ANGLE_AIM.value && flown >= ULTRA_HIGH_ANGLE_SECONDS.value {
        bonuses.push(Bonus::Moonshot);
    } else if aim >= HIGH_ANGLE_AIM.value && flown >= HIGH_ANGLE_SECONDS.value {
        bonuses.push(Bonus::SkyLob);
    }
    if track.legs.iter().any(Leg::hooked) {
        let shooter = before.players()[usize::from(seat)].center().x;
        // It landed on the far side of its shooter from where it set off: brought back behind.
        let behind = (track.impact.at.x - shooter) * flight.velocity.x < 0.0;
        bonuses.push(if behind { Bonus::BehindTheBack } else { Bonus::Boomerang });
    }
    bonuses
}

/// Every seat's tally for a whole game.
///
/// # Errors
///
/// Fails if the log doesn't replay (`Log::replay`).
pub fn tally(log: &Log, assets: &dyn MapAssets) -> Res<Vec<Tally>> {
    let mut game = Game::new(&log.setup, assets)?;
    let mut board = Scoreboard::new(log.setup.seats.len());
    for event in &log.events {
        let before = game.clone();
        let effects = game.apply(*event)?;
        board.record(&before, event, &effects);
    }
    Ok(board.tallies)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        drops::{Drop, DropKind},
        game::{
            Contact, MobileKind, Phase, Shot, Team,
            testing::{direct_hit_power, duel, duel_of, duel_setup, game_of, seat},
        },
        map::NoAssets,
        units::{Centis, Facing, Power, Wind},
        weather::Column,
    };

    fn shot(seat: u8, aim: i16, power: Power) -> Event {
        Event::Shoot {
            seat,
            shot: Shot::S1,
            aim,
            power,
            time: Centis(0),
        }
    }

    /// A 2v2 on the duel map: seats 0 and 2 on team A, 1 and 3 on team B.
    fn two_on_two() -> Game {
        game_of(vec![
            seat(Team::A, MobileKind::Coilback, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1100, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 700, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1500, Facing::Left),
        ])
    }

    fn scored(game: &Game, event: Event, effects: &[Effect]) -> Scoreboard {
        let mut board = Scoreboard::new(game.players().len());
        board.record(game, &event, effects);
        board
    }

    #[test]
    fn a_shot_credits_its_shooter_with_damage_to_enemies_and_its_tier() {
        let game = two_on_two();
        for (damage, tier) in [(149, None), (150, Some(Bonus::NiceHit)), (250, Some(Bonus::GreatHit)), (500, Some(Bonus::MassiveHit))] {
            // Two enemies' damage adds up, shields included.
            let effects = [Effect::Shielded { seat: 1, amount: damage / 2 }, Effect::Damaged { seat: 3, amount: damage - damage / 2 }];
            let board = scored(&game, shot(0, 35, Power::nearest(0.5)), &effects);
            let tally = &board.tallies()[0];
            assert_eq!(tally.damage, damage);
            assert_eq!(tally.bonuses, tier.into_iter().collect::<Vec<_>>(), "{damage}");
        }
    }

    #[test]
    fn kills_overboards_and_a_multi_kill() {
        let game = two_on_two();
        let effects = [Effect::Died { seat: 1, death: Death::Blasted }, Effect::Died { seat: 3, death: Death::Bunged }];
        let board = scored(&game, shot(0, 35, Power::nearest(0.5)), &effects);
        let tally = &board.tallies()[0];
        assert_eq!(tally.kills, 2);
        assert_eq!(tally.bonuses, [Bonus::Finisher, Bonus::Finisher, Bonus::Overboard, Bonus::MultiKill]);
    }

    #[test]
    fn hurting_a_teammate_scores_nothing_and_killing_one_is_a_penalty() {
        let game = two_on_two();
        let effects = [
            Effect::Damaged { seat: 2, amount: 300 },
            Effect::Died { seat: 2, death: Death::Blasted },
            Effect::Died { seat: 0, death: Death::Bunged },
        ];
        let board = scored(&game, shot(0, 35, Power::nearest(0.5)), &effects);
        let tally = &board.tallies()[0];
        assert_eq!((tally.damage, tally.kills), (0, 0));
        assert_eq!(tally.bonuses, [Bonus::FriendlyFire], "its own fall is no one's bonus");
        assert!(Bonus::FriendlyFire.is_penalty() && !Bonus::Finisher.is_penalty());
    }

    #[test]
    fn a_drop_scores_for_the_seat_that_spun_it() {
        let game = two_on_two();
        let drop = Drop { kind: DropKind::Dynamite, x: 1100 };
        let effects = [
            Effect::Damaged { seat: 1, amount: 40 },
            Effect::Dropped { seat: 2, drop, legs: Vec::new() },
            Effect::Damaged { seat: 1, amount: 55 },
            Effect::Died { seat: 1, death: Death::Blasted },
        ];
        let board = scored(&game, shot(0, 35, Power::nearest(0.5)), &effects);
        assert_eq!(board.tallies()[0].damage, 40);
        assert_eq!((board.tallies()[2].damage, board.tallies()[2].kills), (55, 1));
    }

    #[test]
    fn what_no_one_fired_scores_for_no_one() {
        let game = two_on_two();
        let sacrifice = scored(&game, Event::UseItem { seat: 0, slot: 0, time: Centis(0) }, &[Effect::Damaged { seat: 0, amount: 80 }]);
        let walk = scored(&game, Event::Walk { seat: 0, facing: Facing::Left, steps: 5 }, &[Effect::Died { seat: 0, death: Death::Bunged }]);
        assert!(sacrifice.tallies().iter().chain(walk.tallies()).all(|tally| *tally == Tally::default()));
    }

    /// The trick bonuses a shot from seat 0 of `game` at `aim` and `power` earns, if it hits.
    fn tricks_of(game: &Game, aim: i16, power: Power) -> Vec<Bonus> {
        let board = scored(game, shot(0, aim, power), &[Effect::Damaged { seat: 1, amount: 100 }]);
        board.tallies()[0].bonuses.clone()
    }

    #[test]
    fn a_hit_through_a_tornado_or_off_a_mirror() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 35);
        assert_eq!(tricks_of(&game, 35, power), [], "a plain hit");
        for (kind, bonus) in [(WeatherKind::Tornado, Bonus::Twister), (WeatherKind::Mirror, Bonus::BankShot)] {
            game.set_columns(vec![Column::new(kind, 700)]);
            assert_eq!(tricks_of(&game, 35, power), [bonus], "{kind:?}");
        }
    }

    #[test]
    fn a_high_lob_is_a_sky_lob_and_a_longer_one_a_moonshot() {
        let game = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 150);
        let mut found = Vec::new();
        for aim in 70..=89 {
            for twentieths in 0..=Power::MAX.twentieths() {
                let power = Power::from_twentieths(twentieths).unwrap();
                let track = game.track(game.launch(0, Shot::S1, aim, power.bars()).unwrap(), 0, 0);
                if track.impact.contact == Contact::Mobile(1) {
                    let flown = seconds(track.impact.tick);
                    let expected = if flown >= 4.0 {
                        vec![Bonus::Moonshot]
                    } else if flown >= 2.5 {
                        vec![Bonus::SkyLob]
                    } else {
                        Vec::new()
                    };
                    assert_eq!(tricks_of(&game, aim, power), expected, "{aim} degrees, {flown:.2} s");
                    found.extend(expected);
                }
            }
        }
        assert!(found.contains(&Bonus::SkyLob) && found.contains(&Bonus::Moonshot), "{found:?}");
        let low = direct_hit_power(&game, 60);
        assert_eq!(tricks_of(&game, 60, low), [], "below 70 degrees");
    }

    #[test]
    fn a_hook_is_a_boomerang_out_front_and_behind_the_back_behind() {
        // Facing the target into a gale, high: it sails out, turns, and comes down in front.
        let mut game = duel_of(MobileKind::Hookclaw, MobileKind::Coilback, 300);
        game.wind = Wind::new(26, 180).unwrap();
        assert_eq!(tricks_of(&game, 60, Power::nearest(2.0)), [Bonus::Boomerang]);
        // Facing away with the gale behind the target: it comes back over its shooter's head.
        game.wind = Wind::new(26, 0).unwrap();
        game.players[0].facing = Facing::Left;
        assert_eq!(tricks_of(&game, 60, Power::nearest(2.0)), [Bonus::Boomerang], "turned, but not back past its shooter");
        assert_eq!(tricks_of(&game, 75, Power::nearest(2.5)), [Bonus::SkyLob, Bonus::BehindTheBack], "tricks stack");
    }

    #[test]
    fn a_whole_log_tallies_as_its_events_do() {
        // A duel to the end: each side lobs its best S1 at the other, rolls answered as tests do.
        let mut game = Game::new(&duel_setup(800), &NoAssets).unwrap();
        let mut events = Vec::new();
        let mut board = Scoreboard::new(2);
        while events.len() < 200 {
            let event = match (game.pending_roll(), game.phase()) {
                (Some(_), _) => roll_for(&game),
                (None, Phase::Turn { seat }) => shot(seat, 35, best_power(&game, seat, 1 - seat)),
                (None, Phase::Over { .. }) => break,
            };
            let before = game.clone();
            let effects = game.apply(event).unwrap();
            board.record(&before, &event, &effects);
            events.push(event);
        }
        assert!(matches!(game.phase(), Phase::Over { .. }), "the duel plays out");
        let log = Log { setup: duel_setup(800), events };
        assert_eq!(tally(&log, &NoAssets).unwrap(), board.tallies());
        let winner = board.tallies().iter().find(|tally| tally.kills == 1).expect("someone won");
        assert!(winner.damage >= 1000 && winner.count(Bonus::Finisher) == 1, "{:?}", board.tallies());
    }

    /// The S1 power from `seat` at 35 degrees that strikes `target` nearest its center.
    fn best_power(game: &Game, seat: u8, target: u8) -> Power {
        let center = game.players()[usize::from(target)].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(seat, Shot::S1, 35, power.bars())?, 0, 0).impact;
                (impact.contact == Contact::Mobile(target)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(Power::nearest(2.4), |(_, power)| power)
    }

    /// What `answer_rolls` would answer the owed roll with.
    fn roll_for(game: &Game) -> Event {
        match game.pending_roll() {
            Some(crate::game::Roll::Wind) => Event::Wind(Wind::default()),
            Some(crate::game::Roll::Drop { .. }) => Event::Drop(Drop { kind: DropKind::Calm, x: 1 }),
            _ => Event::Weather(crate::weather::Forecast::new(WeatherKind::Force, 1)),
        }
    }
}
