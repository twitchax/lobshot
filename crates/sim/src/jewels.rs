//! Jewels (Jewel mode, PRD-0007 T-003): things that fall onto the map and are worth points to the
//! team that deals one enough damage in a turn. Like a mine (`sim::mines`), a jewel is part of the
//! game state, so the state hash covers it, a replayed log rebuilds it, and a client that joins
//! late sees it.
//!
//! The server rolls each one (`Roll::Jewel`, what it ranges over in `sim::referee`): what it is,
//! and the column it falls down from the sky. It sits on the ground where it lands; one that misses
//! the ground is lost. Every blast, beam and bolt in reach deals it damage as it would a mobile with
//! no armor (`blast`, `zap`), and the shots that meet one on their way say so (`crossed`): its
//! damage this turn adds up, and once it reaches what the jewel needs, the turn's team takes it and
//! its points (`hit`). Its count starts again each turn. A jewel whose ground is blown away falls,
//! as a mobile does, and one that leaves the map is gone. J.D.'s pull drags them (`drag`).
//!
//! Everything a jewel does reaches the client as effects naming it by `JewelId`: dropped, hit,
//! fallen, taken, gone and turned. The rules' numbers are in `tuning::jewels`, and their sources in
//! `docs/research/modes-jewel.md`.

use serde::{Deserialize, Serialize};

use crate::{
    body::Pose,
    damage::{self, Blast},
    game::{Effect, Game, Leg, Team},
    mode,
    movement::{self, Footing, Landing},
    shots::bolt::{self, Strike},
    tuning::{
        Rect,
        jewels::{MOST, NEEDS, ONYX_ROUNDS, OPENING, POINTS, SIZE},
    },
    units::{Facing, Vec2},
};

/// What a jewel is: what it is worth, and what it takes. Named for GunBound's colors: blue, yellow,
/// red and black (NAMU, GF-cyber).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JewelKind {
    /// Blue: 5 points, for 100 damage in a turn.
    Sapphire,
    /// Yellow: 10 points, for 200.
    Topaz,
    /// Red: 25 points, for 300.
    Ruby,
    /// Black: it costs the team that takes it 5 points, for 100, until it turns blue
    /// (`tuning::jewels::ONYX_ROUNDS`).
    Onyx,
}

impl JewelKind {
    /// Every kind, in `tuning::jewels`'s order.
    pub const ALL: [Self; 4] = [Self::Sapphire, Self::Topaz, Self::Ruby, Self::Onyx];

    /// The player-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sapphire => "Sapphire",
            Self::Topaz => "Topaz",
            Self::Ruby => "Ruby",
            Self::Onyx => "Onyx",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Sapphire => 0,
            Self::Topaz => 1,
            Self::Ruby => 2,
            Self::Onyx => 3,
        }
    }

    /// What it is worth to the team that takes it.
    #[must_use]
    pub const fn points(self) -> i32 {
        POINTS.value[self.index()]
    }

    /// The damage it must take in one turn to be taken.
    #[must_use]
    pub const fn needs(self) -> i32 {
        NEEDS.value[self.index()]
    }
}

/// A rolled jewel: what, and the column it falls down (`Event::Jewel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct JewelDrop {
    pub kind: JewelKind,
    pub x: i32,
}

/// A jewel's name for the game's whole length: the order it fell in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JewelId(pub u32);

/// A jewel on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jewel {
    pub id: JewelId,
    pub kind: JewelKind,
    /// The ground it sits on, as a mobile stands.
    pub at: Footing,
    /// The damage it has taken this turn.
    pub damage: i32,
    /// The game's turn count (`Game::turns`) when it landed: an Onyx turns blue some turns later.
    pub landed: i64,
}

impl Jewel {
    /// Its box in its own frame: `SIZE` square, sitting on the ground.
    #[must_use]
    pub fn hitbox() -> Rect {
        let size = SIZE.value;
        Rect {
            x: -size / 2.0,
            y: -size,
            w: size,
            h: size,
        }
    }

    /// Where it sits, as the damage model sees a body: upright on the ground.
    #[must_use]
    pub fn pose(&self) -> Pose {
        Self::pose_at(self.at)
    }

    fn pose_at(at: Footing) -> Pose {
        Pose {
            foot: at.world(),
            facing: Facing::Right,
            tilt: 0.0,
        }
    }

    /// Its middle: what a shot aims for.
    #[must_use]
    pub fn middle(&self) -> Vec2 {
        Self::middle_at(self.at)
    }

    /// The middle of a jewel sitting at `at`.
    #[must_use]
    pub fn middle_at(at: Footing) -> Vec2 {
        Self::pose_at(at).center(Self::hitbox())
    }

    /// Whether `point` is inside it.
    #[must_use]
    pub fn contains(&self, point: Vec2) -> bool {
        self.pose().hit(Self::hitbox(), point)
    }
}

/// The jewels on the map, in the order they fell, and what is owed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jewelry {
    jewels: Vec<Jewel>,
    /// How many have fallen.
    dropped: u32,
    /// How many the server still owes a roll for (`Roll::Jewel`).
    owed: u32,
    /// The seat whose turn the damage counts toward: the turn that is on, or starting.
    scorer: Option<u8>,
}

impl Jewelry {
    /// A Jewel game's as it starts: the opening jewels owed.
    pub(crate) fn opening() -> Self {
        Self { owed: OPENING.value, ..Self::default() }
    }

    #[must_use]
    pub fn jewels(&self) -> &[Jewel] {
        &self.jewels
    }

    /// Whether the server owes a roll for a jewel.
    #[must_use]
    pub fn owed(&self) -> bool {
        self.owed > 0
    }

    fn take(&mut self, id: JewelId) -> Option<Jewel> {
        let index = self.jewels.iter().position(|jewel| jewel.id == id)?;
        Some(self.jewels.remove(index))
    }

    fn put(&mut self, jewel: Jewel) {
        if let Some(slot) = self.jewels.iter_mut().find(|other| other.id == jewel.id) {
            *slot = jewel;
        }
    }
}

/// Where a jewel dropped down column `x` lands: it falls from the sky to the first ground it
/// touches, or out of the map. Mobiles and weather columns don't stop it, nor does the wind move it.
#[must_use]
pub fn fall(game: &Game, x: i32) -> (Leg, Option<Footing>) {
    mode::fall_in_wind(game, x, 0.0)
}

/// The rolled jewel falls: down its column to the ground, where it sits, or out of the map.
pub(crate) fn drop(game: &mut Game, drop: JewelDrop, effects: &mut Vec<Effect>) {
    let (leg, to) = fall(game, drop.x);
    let landed = game.turns();
    let jewelry = &mut game.mode_state.jewelry;
    jewelry.owed = jewelry.owed.saturating_sub(1);
    let id = JewelId(jewelry.dropped);
    jewelry.dropped += 1;
    let jewel = to.map(|at| Jewel {
        id,
        kind: drop.kind,
        at,
        damage: 0,
        landed,
    });
    jewelry.jewels.extend(jewel);
    effects.push(Effect::JewelDropped { id, drop, legs: vec![leg], to });
}

/// A turn ended: each Onyx that has sat `ONYX_ROUNDS` rounds turns blue ("Black Jewels turn into
/// Blue Jewels after a set amount of time", GF-cyber, SW-modes), and one more jewel is owed while
/// the map has fewer than `MOST`.
pub(crate) fn turn_ended(game: &mut Game, effects: &mut Vec<Effect>) {
    let (on, turns) = (game.mode_state.mode().jewels(), game.turns());
    let ripe = i64::from(game.round()) * ONYX_ROUNDS.value;
    let jewelry = &mut game.mode_state.jewelry;
    for jewel in jewelry.jewels.iter_mut().filter(|jewel| jewel.kind == JewelKind::Onyx && turns - jewel.landed >= ripe) {
        jewel.kind = JewelKind::Sapphire;
        effects.push(Effect::JewelTurned { id: jewel.id, kind: jewel.kind });
    }
    if on && jewelry.jewels.len() + usize::try_from(jewelry.owed).unwrap_or(usize::MAX) < MOST.value {
        jewelry.owed += 1;
    }
}

/// `seat`'s turn is starting: the damage the jewels took counts from nothing again, toward it.
pub(crate) fn turn_starts(game: &mut Game, seat: u8) {
    let jewelry = &mut game.mode_state.jewelry;
    jewelry.scorer = Some(seat);
    for jewel in &mut jewelry.jewels {
        jewel.damage = 0;
    }
}

/// Deals `amount` to jewel `id` at stage tick `tick`. Once its damage this turn reaches what it
/// needs, the turn's team takes it: it is off the map, and the team has its points.
pub(crate) fn hit(game: &mut Game, id: JewelId, amount: i32, tick: u32, effects: &mut Vec<Effect>) {
    let Some(team) = scoring(game) else { return };
    let jewelry = &mut game.mode_state.jewelry;
    let Some(jewel) = jewelry.jewels.iter_mut().find(|jewel| jewel.id == id) else { return };
    if amount <= 0 {
        return;
    }
    jewel.damage += amount;
    effects.push(Effect::JewelHit { id, amount, tick });
    if jewel.damage < jewel.kind.needs() {
        return;
    }
    let kind = jewel.kind;
    jewelry.take(id);
    effects.push(Effect::JewelTaken { id, team, tick });
    mode::score(game, team, kind.points(), effects);
}

/// `blast` goes off at stage tick `tick`: each jewel in reach takes what the blast would deal a
/// mobile with no armor there (`damage::unarmored`), direct when it went off inside the jewel.
/// Every blast calls it (`Game::explode_by`).
pub(crate) fn blast(game: &mut Game, blast: &Blast, tick: u32, effects: &mut Vec<Effect>) {
    let dealt: Vec<(JewelId, i32)> = game
        .mode_state
        .jewelry
        .jewels
        .iter()
        .map(|jewel| (jewel.id, damage::unarmored(blast, &jewel.pose(), Jewel::hitbox(), jewel.contains(blast.center))))
        .collect();
    for (id, amount) in dealt {
        hit(game, id, amount, tick, effects);
    }
}

/// A bolt's damage (`blast`'s model) to each jewel it struck or passed near, as `bolt::zap` deals
/// mobiles: the point of its path nearest each jewel stands in for the blast's center, and one it
/// ran through takes it directly. Every bolt calls it (`bolt::zap`).
pub(crate) fn zap(game: &mut Game, strike: &Strike, blast: &Blast, tick: u32, effects: &mut Vec<Effect>) {
    let dealt: Vec<(JewelId, i32)> = game
        .mode_state
        .jewelry
        .jewels
        .iter()
        .map(|jewel| {
            let center = bolt::nearest(strike, jewel.middle());
            let near = Blast { center, ..*blast };
            (jewel.id, damage::unarmored(&near, &jewel.pose(), Jewel::hitbox(), jewel.contains(center)))
        })
        .collect();
    for (id, amount) in dealt {
        hit(game, id, amount, tick, effects);
    }
}

/// The jewels a path passes through, each with the stage tick it first enters one, in that order:
/// for a shot that meets jewels on its way (Augerdillo's SS goes off on the first, Flarecrow's
/// calls a beam onto each, as each does for a mine: `mines::crossed`).
pub(crate) fn crossed(game: &Game, legs: &[Leg]) -> Vec<(Jewel, u32)> {
    let mut met: Vec<(Jewel, u32)> = Vec::new();
    for leg in legs {
        for tick in leg.from..=leg.to {
            let at = leg.flight.at(tick);
            for jewel in &game.mode_state.jewelry.jewels {
                if jewel.contains(at) && !met.iter().any(|(seen, _)| seen.id == jewel.id) {
                    met.push((*jewel, leg.offset + tick));
                }
            }
        }
    }
    met.sort_by_key(|&(jewel, tick)| (tick, jewel.id));
    met
}

/// Whether `point` is inside any jewel: for what walks into one (Tinbadger's big beetle).
#[must_use]
pub fn at(game: &Game, point: Vec2) -> bool {
    game.mode_state.jewelry.jewels.iter().any(|jewel| jewel.contains(point))
}

/// Drags every jewel within `reach` of `at` to the ground under `at`, as its mobiles fall: J.D.'s
/// S2 "sucks in surrounding mobiles, jewels and also mini-Raons" (WP06). One with no ground under
/// `at` is gone.
#[cfg_attr(not(test), expect(dead_code, reason = "Lodestoad's S2 pull calls it beside mines::drag once it merges"))]
pub(crate) fn drag(game: &mut Game, at: Vec2, reach: f64, effects: &mut Vec<Effect>) {
    // The pull's center is on the map, in i32 range.
    #[allow(clippy::cast_possible_truncation)]
    let under = Footing {
        x: at.x.floor() as i32,
        y: at.y.floor() as i32,
    };
    let landing = movement::settle(game.terrain(), under);
    let pulled: Vec<Jewel> = game.mode_state.jewelry.jewels.iter().filter(|jewel| jewel.middle().distance(at) <= reach).copied().collect();
    for jewel in pulled {
        match landing {
            Landing::On(to) => {
                game.mode_state.jewelry.put(Jewel { at: to, ..jewel });
                effects.push(Effect::JewelFell { id: jewel.id, to });
            }
            Landing::OffMap => {
                game.mode_state.jewelry.take(jewel.id);
                effects.push(Effect::JewelGone { id: jewel.id });
            }
        }
    }
}

/// Drops every jewel whose ground is gone: onto the ground below, or off the map.
pub(crate) fn settle(game: &mut Game, effects: &mut Vec<Effect>) {
    for jewel in game.mode_state.jewelry.jewels.clone() {
        match movement::settle(game.terrain(), jewel.at) {
            Landing::On(to) if to == jewel.at => {}
            Landing::On(to) => {
                game.mode_state.jewelry.put(Jewel { at: to, ..jewel });
                effects.push(Effect::JewelFell { id: jewel.id, to });
            }
            Landing::OffMap => {
                game.mode_state.jewelry.take(jewel.id);
                effects.push(Effect::JewelGone { id: jewel.id });
            }
        }
    }
}

/// The team the turn's damage counts toward, if a turn is on.
fn scoring(game: &Game) -> Option<Team> {
    game.mode_state.jewelry.scorer.and_then(|seat| game.players().get(usize::from(seat))).map(|player| player.team)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Contact, Death, Event, MobileKind, Phase, Setup, Shot,
            testing::{answer_rolls, damage_to, duel_setup, game_in, pass, seat, shoot, start},
        },
        items::{Item, Loadout},
        log::Log,
        map::NoAssets,
        mines,
        mode::Mode,
        shots::skyeye,
        terrain::Terrain,
        tuning::{
            BOLT_REACH, WeaponType,
            jewels::{RESPAWN_TURNS, TARGET_POINTS},
        },
        units::{Centis, Power},
    };

    /// A Jewel duel on the flat map: `a` at x = 300 facing right, `b` at 1100, on the ground at 500.
    fn duel_of(a: MobileKind, b: MobileKind) -> Game {
        let mut setup = Setup { mode: Mode::Jewel, ..duel_setup(800) };
        setup.seats[0].mobile = a;
        setup.seats[1].mobile = b;
        start(&setup)
    }

    fn duel() -> Game {
        duel_of(MobileKind::Coilback, MobileKind::Coilback)
    }

    /// Drops a `kind` jewel down column `x`, as the server's roll would.
    fn drop_at(game: &mut Game, kind: JewelKind, x: i32) -> Vec<Effect> {
        let mut effects = Vec::new();
        drop(game, JewelDrop { kind, x }, &mut effects);
        effects
    }

    /// The jewel dropped last.
    fn last(game: &Game) -> Jewel {
        *game.jewels().last().unwrap()
    }

    /// A blast of `damage` (min and max alike) and reach 30 centered at `at`.
    fn blast_at(at: Vec2, damage: i32) -> Blast {
        Blast {
            center: at,
            radius: 30.0,
            damage: crate::tuning::DamageRange { min: damage, max: damage },
            weapon: WeaponType::Explosion,
            false_angle: false,
            boost: 1.0,
        }
    }

    fn whole(game: &Game) -> bool {
        game.players()
            .iter()
            .all(|player| player.alive && player.hp == player.tuning().hp.value && player.shield == player.tuning().shield.value)
    }

    /// The power step whose `shot` from seat 0 at `aim` comes down nearest seat 1's center.
    fn power_at_seat_1(game: &Game, shot: Shot, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, shot, aim, power.bars())?, 0, 0).impact;
                (impact.contact != Contact::Gone).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .unwrap()
    }

    #[test]
    fn a_jewel_game_owes_the_opening_jewels_and_one_more_a_turn_up_to_the_most() {
        let mut game = crate::game::Game::new(&Setup { mode: Mode::Jewel, ..duel_setup(800) }, &NoAssets).unwrap();
        // The opening forecasts come first, then the jewels.
        let mut rolls = Vec::new();
        while let Some(roll) = game.pending_roll() {
            rolls.push(roll);
            let event = crate::referee::answer(roll, 0, &mut crate::referee::XorShift::new(5));
            game.apply(event).unwrap();
        }
        let jewels = rolls.iter().filter(|roll| matches!(roll, crate::game::Roll::Jewel { width: 2400 })).count();
        assert_eq!(rolls.len() - jewels, 5, "the forecasts first: {rolls:?}");
        assert!(rolls[5..].iter().all(|roll| matches!(roll, crate::game::Roll::Jewel { .. })), "{rolls:?}");
        assert_eq!(jewels, usize::try_from(OPENING.value).unwrap());
        let mut turns = 0;
        while game.jewels().len() < MOST.value {
            let Phase::Turn { seat } = game.phase() else { panic!() };
            pass(&mut game, seat);
            turns += 1;
            assert!(turns <= MOST.value, "one a turn");
        }
        assert_eq!(turns, MOST.value - usize::try_from(OPENING.value).unwrap());
        let Phase::Turn { seat } = game.phase() else { panic!() };
        pass(&mut game, seat);
        assert_eq!(game.jewels().len(), MOST.value, "no more while the map is full");
        // Solo owes none.
        let solo = start(&duel_setup(800));
        assert_eq!(solo.jewels(), []);
    }

    #[test]
    fn a_jewel_falls_from_the_sky_onto_the_ground_and_one_down_a_pit_is_lost() {
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 && !(700..720).contains(&x)).unwrap();
        let mut game = game_in(
            Mode::Jewel,
            terrain,
            vec![seat(Team::A, MobileKind::Coilback, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        );
        let before = game.jewels().len();
        let effects = drop_at(&mut game, JewelKind::Topaz, 900);
        let Effect::JewelDropped { id, legs, to: Some(to), .. } = &effects[0] else {
            panic!("{effects:?}")
        };
        assert!(legs[0].flight.origin.y < 0.0, "from the sky");
        assert_eq!(*to, Footing { x: 900, y: 500 });
        assert_eq!(
            last(&game),
            Jewel {
                id: *id,
                kind: JewelKind::Topaz,
                at: *to,
                damage: 0,
                landed: game.turns()
            }
        );
        let effects = drop_at(&mut game, JewelKind::Ruby, 710);
        assert!(matches!(effects[..], [Effect::JewelDropped { to: None, .. }]), "{effects:?}");
        assert_eq!(game.jewels().len(), before + 1, "lost down the pit");
    }

    #[test]
    fn bullets_dont_hurt_players_whatever_fires_them() {
        // Every mobile's every shot, at its target, hurts no one; the same shot in Solo does.
        for mobile in MobileKind::ALL {
            for shot in [Shot::S1, Shot::S2, Shot::Ss] {
                let mut game = duel_of(mobile, MobileKind::Coilback);
                let aim = shot.tuning(mobile.tuning()).aim.value;
                let aim = i16::midpoint(aim.true_min, aim.true_max);
                if mobile.tuning().s1.aim.value.backward() {
                    game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
                }
                let power = power_at_seat_1(&game, shot, aim);
                let effects = shoot(&mut game, 0, shot, aim, power);
                assert_eq!(damage_to(&effects, 0) + damage_to(&effects, 1), 0, "{mobile:?} {shot:?}: {effects:?}");
                assert!(whole(&game), "{mobile:?} {shot:?}: {:?}", game.players());
                assert!(
                    !effects.iter().any(|e| matches!(e, Effect::ShieldStripped { .. } | Effect::Frosted { .. })),
                    "{mobile:?} {shot:?}: {effects:?}"
                );
            }
        }
        let mut solo = crate::game::testing::duel(800);
        let power = power_at_seat_1(&solo, Shot::S1, 45);
        assert!(damage_to(&shoot(&mut solo, 0, Shot::S1, 45, power), 1) > 0, "the same shot hurts in Solo");
    }

    #[test]
    fn mines_lasers_beams_and_bolts_hurt_no_one_either_but_still_dig() {
        let mut game = duel_of(MobileKind::Tinbadger, MobileKind::Coilback);
        let target = game.players()[1];
        // A mine at the Coilback's middle goes off at once.
        let mut effects = Vec::new();
        mines::lay(&mut game, 0, target.center() + Vec2::new(0.0, 25.0), Facing::Right, 0, &mut effects);
        assert!(effects.iter().any(|e| matches!(e, Effect::MineGone { tick: Some(0), .. })), "{effects:?}");
        // Skyeye's beam, and a bolt from the sky, onto it.
        skyeye::beam(&mut game, target.center(), 10, &mut effects);
        let strike = bolt::from_sky(&game, target.center().x);
        bolt::flash(&mut game, &strike, 12.0, 20, &mut effects);
        bolt::zap(&mut game, &strike, &blast_at(strike.at, 300), 20, &mut effects);
        // An ordinary blast, direct.
        game.explode(&blast_at(target.center(), 300), 30.0, Some(1), 30, &mut effects);
        assert_eq!(damage_to(&effects, 1), 0, "{effects:?}");
        assert!(whole(&game));
        assert!(effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count() >= 3, "they dig: {effects:?}");
        // A satellite's laser (Lensowl's S1), and Lanternwing's shield-stripping SS.
        for mobile in [MobileKind::Lensowl, MobileKind::Lanternwing] {
            let mut game = duel_of(mobile, MobileKind::Coilback);
            let shot = if mobile == MobileKind::Lensowl { Shot::S1 } else { Shot::Ss };
            let aim = shot.tuning(mobile.tuning()).aim.value;
            let aim = i16::midpoint(aim.true_min, aim.true_max);
            let power = power_at_seat_1(&game, shot, aim);
            let effects = shoot(&mut game, 0, shot, aim, power);
            assert!(effects.iter().any(|e| matches!(e, Effect::Laser { .. } | Effect::Exploded { .. })), "{mobile:?}: {effects:?}");
            assert_eq!(damage_to(&effects, 1), 0, "{mobile:?}: {effects:?}");
            assert!(whole(&game), "{mobile:?}");
        }
    }

    #[test]
    fn a_blast_deals_a_jewel_damage_as_it_would_a_bare_mobile_and_enough_takes_it() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 700);
        let jewel = last(&game);
        // Next to it, short of what it needs.
        let mut effects = Vec::new();
        game.explode(&blast_at(jewel.middle() + Vec2::new(30.0, 0.0), 120), 10.0, None, 7, &mut effects);
        let Some(&Effect::JewelHit { id, amount, tick: 7 }) = effects.iter().find(|e| matches!(e, Effect::JewelHit { .. })) else {
            panic!("{effects:?}")
        };
        assert_eq!(id, jewel.id);
        assert!((1..100).contains(&amount), "a near miss: {amount}");
        assert_eq!(game.jewels().iter().find(|j| j.id == jewel.id).unwrap().damage, amount);
        // On it: direct, and taken by the turn's team (seat 0's, A).
        let mut effects = Vec::new();
        game.explode(&blast_at(jewel.middle(), 120), 10.0, None, 9, &mut effects);
        let taken = effects.iter().position(|e| *e == Effect::JewelTaken { id: jewel.id, team: Team::A, tick: 9 }).unwrap();
        assert_eq!(effects[taken + 1], Effect::Points { team: Team::A, points: 5 });
        assert_eq!(game.points(Team::A), Some(5));
        assert_eq!(game.points(Team::B), Some(0));
        assert!(!game.jewels().iter().any(|j| j.id == jewel.id), "off the map");
    }

    #[test]
    fn a_jewels_damage_adds_up_over_a_turn_and_starts_again_the_next() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Topaz, 700);
        let id = last(&game).id;
        let mut effects = Vec::new();
        hit(&mut game, id, 150, 0, &mut effects);
        pass(&mut game, 0);
        hit(&mut game, id, 150, 0, &mut effects);
        assert!(game.jewels().iter().any(|j| j.id == id), "300 over two turns isn't 200 in one");
        hit(&mut game, id, 50, 0, &mut effects);
        assert!(!game.jewels().iter().any(|j| j.id == id), "200 in one turn is");
        assert_eq!(game.points(Team::B), Some(10), "seat 1's turn, team B's points");
    }

    #[test]
    fn a_duals_two_shots_add_up_on_one_jewel() {
        let mut setup = Setup { mode: Mode::Jewel, ..duel_setup(800) };
        setup.seats[0].items = Loadout::new(&[Item::Dual]).unwrap();
        let mut game = start(&setup);
        drop_at(&mut game, JewelKind::Ruby, 800);
        let jewel = last(&game);
        // The S1 that comes down nearest it.
        let (aim, power) = (20..=70)
            .flat_map(|aim| (0..=Power::MAX.twentieths()).map(move |t| (aim, Power::from_twentieths(t).unwrap())))
            .min_by(|a, b| {
                let miss = |&(aim, power): &(i16, Power)| game.track(game.launch(0, Shot::S1, aim, power.bars()).unwrap(), 0, 0).impact.at.distance(jewel.middle());
                miss(a).total_cmp(&miss(b))
            })
            .unwrap();
        let alone = {
            let mut once = game.clone();
            shoot(&mut once, 0, Shot::S1, aim, power)
        };
        let dealt = |effects: &[Effect]| effects.iter().map(|e| if let Effect::JewelHit { amount, .. } = e { *amount } else { 0 }).sum::<i32>();
        assert!((1..300).contains(&dealt(&alone)), "one S1 isn't enough: {alone:?}");
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, aim, power);
        assert!(dealt(&effects) > dealt(&alone), "{effects:?}");
        assert_eq!(
            effects.iter().any(|e| matches!(e, Effect::JewelTaken { .. })),
            dealt(&effects) >= JewelKind::Ruby.needs(),
            "{effects:?}"
        );
    }

    #[test]
    fn a_bolt_hits_a_jewel_it_runs_through_or_by() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 700);
        let jewel = last(&game);
        let strike = bolt::from_sky(&game, jewel.middle().x);
        assert_eq!(strike.contact, Contact::Terrain, "a jewel doesn't stop a bolt");
        let mut effects = Vec::new();
        bolt::zap(&mut game, &strike, &blast_at(strike.at, 150), 12, &mut effects);
        assert!(effects.contains(&Effect::JewelTaken { id: jewel.id, team: Team::A, tick: 12 }), "{effects:?}");
        // A bolt out of reach does nothing.
        drop_at(&mut game, JewelKind::Sapphire, 1500);
        let far = last(&game);
        let strike = bolt::from_sky(&game, far.middle().x + BOLT_REACH.value + 40.0);
        let mut effects = Vec::new();
        bolt::zap(&mut game, &strike, &blast_at(strike.at, 150), 12, &mut effects);
        assert!(!effects.iter().any(|e| matches!(e, Effect::JewelHit { .. })), "{effects:?}");
    }

    #[test]
    fn a_fake_costs_points_but_never_below_none() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Onyx, 700);
        let id = last(&game).id;
        let mut effects = Vec::new();
        hit(&mut game, id, 100, 0, &mut effects);
        assert!(effects.contains(&Effect::Points { team: Team::A, points: 0 }), "{effects:?}");
        mode::score(&mut game, Team::A, 20, &mut effects);
        drop_at(&mut game, JewelKind::Onyx, 700);
        let id = last(&game).id;
        hit(&mut game, id, 100, 0, &mut effects);
        assert_eq!(game.points(Team::A), Some(15));
    }

    #[test]
    fn the_first_team_to_its_points_wins() {
        let mut game = duel();
        mode::score(&mut game, Team::A, TARGET_POINTS.value - 5, &mut Vec::new());
        drop_at(&mut game, JewelKind::Sapphire, 700);
        let jewel = last(&game);
        let mut effects = Vec::new();
        game.explode(&blast_at(jewel.middle(), 150), 10.0, None, 0, &mut effects);
        assert_eq!(game.points(Team::A), Some(TARGET_POINTS.value));
        let effects = pass(&mut game, 0);
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::A) });
        assert!(effects.contains(&Effect::GameOver { winner: Some(Team::A) }), "{effects:?}");
    }

    /// Flat ground at y = 500 with a shaft through it just right of x = 300.
    fn pitted() -> Terrain {
        Terrain::from_fn(2400, 600, |x, y| y >= 500 && !(330..350).contains(&x)).unwrap()
    }

    /// Seat 0 walks into the pit on its turn.
    fn bunge_seat_0(game: &mut Game) -> Vec<Effect> {
        let effects = game
            .apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 60,
            })
            .unwrap();
        answer_rolls(game);
        effects
    }

    #[test]
    fn a_bunged_mobile_drops_back_in_two_turns_after_its_pick_while_its_team_stands() {
        let seats = vec![
            seat(Team::A, MobileKind::Coilback, 300, Facing::Right),
            seat(Team::B, MobileKind::Coilback, 1100, Facing::Left),
            seat(Team::A, MobileKind::Coilback, 1700, Facing::Left),
        ];
        let mut game = game_in(Mode::Jewel, pitted(), seats);
        let effects = bunge_seat_0(&mut game);
        assert!(effects.contains(&Effect::Died { seat: 0, death: Death::Bunged }), "{effects:?}");
        assert!(!effects.iter().any(|e| matches!(e, Effect::Points { .. })), "a bunge is free: {effects:?}");
        assert!(matches!(game.phase(), Phase::Turn { .. }), "seat 2 still stands");
        assert!(!game.can_spin(0), "no slot machine in Jewel");
        assert!(game.can_respawn(0));
        game.apply(Event::Respawn { seat: 0, x: 1000 }).unwrap();
        let turns = game.turns();
        assert_eq!(game.respawns()[0].pick.unwrap().lands, turns + RESPAWN_TURNS.value + 1);
        let mut landed = Vec::new();
        while !game.players()[0].alive {
            let Phase::Turn { seat } = game.phase() else { panic!() };
            landed = pass(&mut game, seat);
        }
        assert_eq!(game.turns(), turns + RESPAWN_TURNS.value + 1, "the second turn after the pick's");
        assert!(landed.iter().any(|e| matches!(e, Effect::Respawned { seat: 0, to: Some(_), .. })), "{landed:?}");
    }

    #[test]
    fn a_team_with_everyone_bunged_at_once_loses() {
        let mut game = game_in(
            Mode::Jewel,
            pitted(),
            vec![seat(Team::A, MobileKind::Coilback, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        );
        mode::score(&mut game, Team::A, 90, &mut Vec::new());
        let effects = bunge_seat_0(&mut game);
        assert!(effects.contains(&Effect::GameOver { winner: Some(Team::B) }), "points or not: {effects:?}");
    }

    #[test]
    fn a_player_who_leaves_a_duel_loses_it() {
        let mut game = duel();
        let effects = game.apply(Event::Forfeit { seat: 1 }).unwrap();
        assert!(effects.contains(&Effect::GameOver { winner: Some(Team::A) }), "{effects:?}");
    }

    #[test]
    fn a_jewel_whose_ground_is_blown_away_falls_and_one_with_none_left_is_gone() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 700);
        drop_at(&mut game, JewelKind::Sapphire, 750);
        let (first, second) = (game.jewels()[game.jewels().len() - 2], last(&game));
        for y in 500..530 {
            game.terrain.carve(Vec2::new(700.5, f64::from(y) + 0.5), 3.0);
        }
        for y in 500..600 {
            game.terrain.carve(Vec2::new(750.5, f64::from(y) + 0.5), 0.6);
        }
        let mut effects = Vec::new();
        game.settle_all(&mut effects);
        assert!(effects.iter().any(|e| matches!(e, Effect::JewelFell { id, to } if *id == first.id && to.y > 525)), "{effects:?}");
        assert!(effects.contains(&Effect::JewelGone { id: second.id }), "{effects:?}");
    }

    #[test]
    fn a_pull_drags_jewels_in_reach_to_its_center() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Sapphire, 600);
        drop_at(&mut game, JewelKind::Topaz, 900);
        let (near, far) = (game.jewels()[game.jewels().len() - 2], last(&game));
        let mut effects = Vec::new();
        drag(&mut game, Vec2::new(620.5, 480.0), 60.0, &mut effects);
        assert_eq!(
            effects,
            [Effect::JewelFell {
                id: near.id,
                to: Footing { x: 620, y: 500 }
            }]
        );
        assert_eq!(game.jewels().iter().find(|j| j.id == far.id).unwrap().at.x, 900, "out of reach");
    }

    #[test]
    fn augerdillos_ss_goes_off_on_a_jewel_it_meets() {
        // The SS's path toward seat 1 over flat ground, rolling through rock.
        let flat = duel_of(MobileKind::Augerdillo, MobileKind::Coilback);
        let range = Shot::Ss.tuning(MobileKind::Augerdillo.tuning()).aim.value;
        let aim = i16::midpoint(range.true_min, range.true_max);
        let fire = |game: &mut Game| {
            game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        };
        let mut aimed = flat.clone();
        fire(&mut aimed);
        let power = power_at_seat_1(&aimed, Shot::Ss, aim);
        let track = aimed.track_sensing(aimed.launch(0, Shot::Ss, aim, power.bars()).unwrap(), 0, 0, crate::game::Senses { mobiles: true, terrain: false });
        // Where it passes x = 900, a pillar holds a Ruby up in its way.
        let over = track.legs.iter().flat_map(|leg| (leg.from..=leg.to).map(|t| leg.flight.at(t))).find(|at| at.x >= 900.0).unwrap();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let top = over.y.floor() as u32 + 8;
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((895..=905).contains(&x) && y >= top)).unwrap();
        let mut game = game_in(
            Mode::Jewel,
            terrain,
            vec![seat(Team::A, MobileKind::Augerdillo, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)],
        );
        fire(&mut game);
        drop_at(&mut game, JewelKind::Ruby, 900);
        let jewel = last(&game);
        assert!(jewel.contains(over) || jewel.middle().distance(over) < 12.0, "{jewel:?} {over:?}");
        let effects = shoot(&mut game, 0, Shot::Ss, aim, power);
        let Some(&Effect::Exploded { at, tick, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!("{effects:?}")
        };
        assert!(at.distance(jewel.middle()) < 1.0, "on the jewel, short of seat 1: {effects:?}");
        assert!(effects.contains(&Effect::JewelTaken { id: jewel.id, team: Team::A, tick }), "its 350-500 takes a Ruby: {effects:?}");
    }

    #[test]
    fn flarecrows_ss_calls_a_beam_onto_a_jewel_it_passes_through() {
        // Its SS flies through everything; where its path goes into the ground, a Topaz.
        let mut game = duel_of(MobileKind::Flarecrow, MobileKind::Coilback);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let power = Power::nearest(1.2);
        let flight = game.launch(0, Shot::Ss, 150, power.bars()).unwrap();
        let track = game.track_sensing(flight, 0, 0, crate::game::Senses { mobiles: false, terrain: false });
        let into = track.legs.iter().flat_map(|leg| (leg.from..=leg.to).map(|t| leg.flight.at(t))).find(|at| at.y >= 495.0).unwrap();
        #[allow(clippy::cast_possible_truncation)]
        drop_at(&mut game, JewelKind::Topaz, into.x.floor() as i32);
        let jewel = last(&game);
        assert!(jewel.contains(into), "{into:?} {jewel:?}");
        let effects = shoot(&mut game, 0, Shot::Ss, 150, power);
        assert!(effects.iter().any(|e| matches!(e, Effect::Beam { to, .. } if to.distance(jewel.middle()) < SIZE.value)), "{effects:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::JewelHit { id, .. } if *id == jewel.id)), "{effects:?}");
    }

    #[test]
    fn tinbadgers_big_beetle_goes_off_on_a_jewel_in_its_way() {
        let mut setup = Setup { mode: Mode::Jewel, ..duel_setup(1600) };
        setup.seats[0].mobile = MobileKind::Tinbadger;
        let mut game = start(&setup);
        let power = Power::nearest(1.6);
        let landing = game.track(game.launch(0, Shot::Ss, 45, power.bars()).unwrap(), 0, 0).impact.at;
        #[allow(clippy::cast_possible_truncation)]
        drop_at(&mut game, JewelKind::Sapphire, landing.x.floor() as i32 + 60);
        let jewel = last(&game);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let Some(&Effect::Exploded { at, .. }) = effects.iter().find(|e| matches!(e, Effect::Exploded { .. })) else {
            panic!("{effects:?}")
        };
        assert!((at.x - jewel.middle().x).abs() <= SIZE.value, "where it met the jewel: {at:?} {jewel:?}");
        assert!(effects.iter().any(|e| matches!(e, Effect::JewelTaken { id, .. } if *id == jewel.id)), "{effects:?}");
    }

    #[test]
    fn an_onyx_that_sits_long_enough_turns_blue() {
        let mut game = duel();
        drop_at(&mut game, JewelKind::Onyx, 700);
        let id = last(&game).id;
        let ripe = i64::from(game.round()) * ONYX_ROUNDS.value;
        let mut turned = Vec::new();
        for _ in 0..ripe {
            assert_eq!(game.jewels().iter().find(|j| j.id == id).unwrap().kind, JewelKind::Onyx);
            let Phase::Turn { seat } = game.phase() else { panic!() };
            turned = pass(&mut game, seat);
        }
        assert!(turned.contains(&Effect::JewelTurned { id, kind: JewelKind::Sapphire }), "{turned:?}");
        let mut effects = Vec::new();
        hit(&mut game, id, 100, 0, &mut effects);
        assert!(effects.iter().any(|e| matches!(e, Effect::Points { points: 5, .. })), "worth 5 now: {effects:?}");
    }

    #[test]
    fn sacrifice_costs_nothing_where_there_is_no_health_to_lose() {
        let mut setup = Setup { mode: Mode::Jewel, ..duel_setup(800) };
        setup.seats[0].items = Loadout::new(&[Item::Sacrifice]).unwrap();
        let mut game = start(&setup);
        let effects = game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        assert!(effects.iter().all(|e| !matches!(e, Effect::Damaged { .. })), "{effects:?}");
        assert!(whole(&game));
        assert_eq!(game.players()[0].armed, Some(Item::Sacrifice), "its boost still rides on the shot");
    }

    #[test]
    fn the_state_hash_covers_the_jewels_and_a_log_replays_them() {
        let setup = Setup { mode: Mode::Jewel, ..duel_setup(800) };
        let mut game = crate::game::Game::new(&setup, &NoAssets).unwrap();
        let mut events = Vec::new();
        while let Some(roll) = game.pending_roll() {
            let event = crate::referee::answer(roll, 0, &mut crate::referee::XorShift::new(3));
            game.apply(event).unwrap();
            events.push(event);
        }
        assert_eq!(game.jewels().len(), usize::try_from(OPENING.value).unwrap());
        let replayed = Log { setup, events }.replay(&NoAssets).unwrap();
        assert_eq!(replayed.jewels(), game.jewels());
        assert_eq!(replayed.state_hash().unwrap(), game.state_hash().unwrap());
        let before = game.state_hash().unwrap();
        let id = game.jewels()[0].id;
        hit(&mut game, id, 1, 0, &mut Vec::new());
        assert_ne!(game.state_hash().unwrap(), before, "the hash covers a jewel's damage");
    }
}
