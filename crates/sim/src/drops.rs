//! The dead players' slot machine (WP06): in Solo, a dead player spins it (`Event::Spin`), the
//! server rolls what it lands on (`Roll::Drop`), and that falls down its column at the end of a
//! later turn (`tuning::drops::FALL_TURNS`). `Game` decides when a spin is allowed; this module
//! says what each drop does when it lands.

use serde::{Deserialize, Serialize};

use crate::{
    ballistics::{self, Flight},
    damage::Blast,
    game::{Contact, Effect, Game},
    shots::bolt,
    tuning::{
        BOLT_CRATER, BOLT_REACH, SKY,
        drops::{BOLT_DAMAGE, BOLT_WEAPON, CHEST_HEAL_PERCENT, DYNAMITE_BLAST, DYNAMITE_DAMAGE, DYNAMITE_WEAPON, DYNAMITE_WIND, GRAVITY, HAMMER_BLAST, HAMMER_DAMAGE, HAMMER_WEAPON},
    },
    units::Vec2,
};

/// What the slot machine can drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DropKind {
    /// Blows up where it lands; the wind carries it as it falls.
    Dynamite,
    /// Lands hard on whatever is under it.
    Hammer,
    /// A bolt straight down, which the wind can't move.
    Bolt,
    /// A treasure chest: it heals whoever it lands on.
    Chest,
    /// The wind dies.
    Calm,
    /// The wind turns around.
    Reverse,
}

impl DropKind {
    /// Every drop; the server rolls one uniformly.
    pub const ALL: [Self; 6] = [Self::Dynamite, Self::Hammer, Self::Bolt, Self::Chest, Self::Calm, Self::Reverse];

    /// The player-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dynamite => "Dynamite",
            Self::Hammer => "Hammer",
            Self::Bolt => "Bolt",
            Self::Chest => "Chest",
            Self::Calm => "Calm",
            Self::Reverse => "Reverse",
        }
    }
}

/// A rolled drop: what, and the column it falls down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drop {
    pub kind: DropKind,
    pub x: i32,
}

/// A drop on its way down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Falling {
    /// Who spun it.
    pub seat: u8,
    pub drop: Drop,
    /// It lands at the end of the turn that brings the game's turn count (`Game::turns`) to this.
    pub lands: i64,
}

/// Lands every drop whose fall is done, in the order they were rolled, at the end of a turn before
/// the next starts. Once a team is gone, the rest are moot.
pub(crate) fn land_due(game: &mut Game, effects: &mut Vec<Effect>) {
    let turns = game.turns();
    if !game.drops.iter().any(|falling| falling.lands <= turns) {
        return;
    }
    let (due, later): (Vec<Falling>, Vec<Falling>) = game.drops.iter().partition(|falling| falling.lands <= turns);
    game.drops = later;
    for falling in due {
        if game.decided() {
            break;
        }
        land(game, falling, effects);
        game.settle_all(effects);
    }
}

/// One drop falls from the sky down its column and does what it does where it lands.
fn land(game: &mut Game, Falling { seat, drop, .. }: Falling, effects: &mut Vec<Effect>) {
    let x = f64::from(drop.x) + 0.5;
    if drop.kind == DropKind::Bolt {
        effects.push(Effect::Dropped { seat, drop, legs: Vec::new() });
        let strike = bolt::from_sky(game, x);
        bolt::flash(game, &strike, BOLT_CRATER.value, 0, effects);
        let blast = Blast {
            center: strike.at,
            radius: BOLT_REACH.value,
            damage: BOLT_DAMAGE.value,
            weapon: BOLT_WEAPON.value,
            false_angle: false,
            boost: 1.0,
        };
        bolt::zap(game, &strike, &blast, effects);
        return;
    }
    let push = if drop.kind == DropKind::Dynamite { DYNAMITE_WIND.value } else { 0.0 };
    let flight = Flight {
        origin: Vec2::new(x, -SKY.value),
        velocity: Vec2::ZERO,
        accel: ballistics::acceleration(GRAVITY.value, game.wind(), push),
    };
    let track = game.track(flight, 0, 0);
    effects.push(Effect::Dropped { seat, drop, legs: track.legs.clone() });
    let impact = track.impact;
    let direct = if let Contact::Mobile(hit) = impact.contact { Some(hit) } else { None };
    let landed = matches!(impact.contact, Contact::Terrain | Contact::Mobile(_));
    let blast = |damage, radius, weapon| Blast {
        center: impact.at,
        radius,
        damage,
        weapon,
        false_angle: false,
        boost: track.marks.boost(),
    };
    match drop.kind {
        DropKind::Dynamite if landed => {
            game.explode(
                &blast(DYNAMITE_DAMAGE.value, DYNAMITE_BLAST.value, DYNAMITE_WEAPON.value),
                DYNAMITE_BLAST.value,
                direct,
                impact.tick,
                effects,
            );
        }
        DropKind::Hammer if landed => {
            game.explode(&blast(HAMMER_DAMAGE.value, HAMMER_BLAST.value, HAMMER_WEAPON.value), HAMMER_BLAST.value, direct, impact.tick, effects);
        }
        DropKind::Chest => {
            if let Some(hit) = direct {
                game.heal(hit, CHEST_HEAL_PERCENT.value, effects);
            }
        }
        DropKind::Calm => {
            game.wind.strength = 0;
            effects.push(Effect::WindChanged(game.wind));
        }
        DropKind::Reverse => {
            game.wind.direction = (game.wind.direction + 180) % 360;
            effects.push(Effect::WindChanged(game.wind));
        }
        DropKind::Dynamite | DropKind::Hammer | DropKind::Bolt => {}
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{MobileKind, testing::duel_of},
        units::Wind,
    };

    #[test]
    fn every_drop_has_its_own_name() {
        let mut names: Vec<&str> = DropKind::ALL.iter().map(|kind| kind.name()).collect();
        assert!(names.iter().all(|name| !name.is_empty()));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), DropKind::ALL.len());
    }

    /// Lands `kind` on seat 1's column of a duel at once, and returns what it did.
    fn land_on_seat_1(kind: DropKind, wind: Wind) -> (Game, Vec<Effect>) {
        let mut game = duel_of(MobileKind::Coilback, MobileKind::Coilback, 800);
        game.wind = wind;
        let x = game.players()[1].footing.x;
        let mut effects = Vec::new();
        land(
            &mut game,
            Falling {
                seat: 0,
                drop: Drop { kind, x },
                lands: 0,
            },
            &mut effects,
        );
        (game, effects)
    }

    fn damage(effects: &[Effect]) -> i32 {
        effects.iter().map(|e| if let Effect::Damaged { seat: 1, amount } = e { *amount } else { 0 }).sum()
    }

    #[test]
    fn dynamite_and_the_hammer_blow_up_on_what_they_land_on() {
        let (_, effects) = land_on_seat_1(DropKind::Dynamite, Wind::default());
        assert!(matches!(&effects[0], Effect::Dropped { legs, .. } if !legs.is_empty()), "{effects:?}");
        assert!((50..=60).contains(&damage(&effects)), "{effects:?}");
        let (_, effects) = land_on_seat_1(DropKind::Hammer, Wind::default());
        assert_eq!(damage(&effects), 48, "50, Physical against Mechanical: {effects:?}");
    }

    #[test]
    fn the_wind_carries_dynamite_but_not_the_hammer() {
        let strong = Wind::new(26, 0).unwrap();
        let landing = |effects: &[Effect]| {
            effects
                .iter()
                .find_map(|e| {
                    if let Effect::Dropped { legs, .. } = e {
                        legs.last().map(|leg| leg.flight.at(leg.to).x)
                    } else {
                        None
                    }
                })
                .unwrap()
        };
        let (game, dynamite) = land_on_seat_1(DropKind::Dynamite, strong);
        let x = f64::from(game.players()[1].footing.x);
        assert!(landing(&dynamite) > x + 40.0, "blown downwind: {dynamite:?}");
        let (_, hammer) = land_on_seat_1(DropKind::Hammer, strong);
        assert!((landing(&hammer) - x).abs() < 1.0, "straight down: {hammer:?}");
    }

    #[test]
    fn the_bolt_strikes_at_once_down_its_column() {
        let (_, effects) = land_on_seat_1(DropKind::Bolt, Wind::new(26, 0).unwrap());
        assert!(matches!(&effects[0], Effect::Dropped { legs, .. } if legs.is_empty()));
        assert!(matches!(effects[1], Effect::Bolt { tick: 0, .. }), "{effects:?}");
        // Electric against Mechanical: 76-80 x 1.04.
        assert!((79..=83).contains(&damage(&effects)), "{effects:?}");
    }

    #[test]
    fn the_chest_heals_whoever_it_lands_on() {
        let mut game = duel_of(MobileKind::Coilback, MobileKind::Coilback, 800);
        game.players[1].hp = 500;
        let x = game.players()[1].footing.x;
        let mut effects = Vec::new();
        land(
            &mut game,
            Falling {
                seat: 0,
                drop: Drop { kind: DropKind::Chest, x },
                lands: 0,
            },
            &mut effects,
        );
        assert!(effects.contains(&Effect::Healed { seat: 1, amount: 120 }), "{effects:?}");
    }

    #[test]
    fn calm_and_reverse_change_the_wind() {
        let wind = Wind::new(12, 45).unwrap();
        let (game, effects) = land_on_seat_1(DropKind::Calm, wind);
        assert_eq!(game.wind(), Wind::new(0, 45).unwrap());
        assert!(effects.contains(&Effect::WindChanged(game.wind())));
        let (game, _) = land_on_seat_1(DropKind::Reverse, wind);
        assert_eq!(game.wind(), Wind::new(12, 225).unwrap());
    }
}
