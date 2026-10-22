//! Each mobile's shots, one module per mobile, over the machinery they share: what rides on a
//! turn's shots (`Salvo`: items, sudden death, splash already dealt), one volley's blasts and
//! landings (`Gun`), projectiles that land in turn (`barrage`), bolts (`bolt`), helix strands
//! (`helix`) and bouncing balls (`bounce`).

pub(crate) mod bolt;
mod bounce;
mod coilback;
mod helix;
mod hookclaw;
mod lanternwing;
mod marblemole;
mod sparkhorn;
mod spoutpot;

use crate::{
    ballistics::Flight,
    damage::Blast,
    game::{Contact, Effect, Game, Impact, Leg, MAX_SEATS, MobileKind, Shot, SuddenDeathKind, Track},
    items::Item,
    movement::{self, Footing, Landing},
    tuning::{
        BIG_BOMB_CRATER, BOLT_CRATER, BOLT_DELAY_SECONDS, BOLT_REACH, DamageRange, MobileTuning,
        items::{DAMAGE_BOOST, DIG_CRATER, THUNDER_DAMAGE, THUNDER_WEAPON},
    },
    units::{Power, Vec2, ticks},
    weather::Marks,
};

/// What rides on one turn's shots: the armed item, sudden death, and who splash has already hit.
#[derive(Debug, Clone)]
pub(crate) struct Salvo {
    pub seat: u8,
    /// The damage multiplier from the turn's item (Power Up, Sacrifice).
    pub boost: f64,
    /// The crater multiplier (Dig, Big Bomb); damage reach is unchanged.
    pub crater: f64,
    /// A bolt strikes wherever each projectile lands (Thunder).
    pub thunder: bool,
    /// The shot is a beacon that carries its shooter (Teleport).
    pub teleport: bool,
    /// Who splash has hit this turn: it lands once a turn on each mobile, Dual or not (GF-max,
    /// WP06).
    pub splashed: [bool; MAX_SEATS],
}

impl Salvo {
    pub fn new(seat: u8, armed: Option<Item>, sudden_death: Option<SuddenDeathKind>) -> Self {
        let dig = if armed == Some(Item::Dig) { DIG_CRATER.value } else { 1.0 };
        let big_bomb = if sudden_death == Some(SuddenDeathKind::BigBomb) { BIG_BOMB_CRATER.value } else { 1.0 };
        Self {
            seat,
            boost: if matches!(armed, Some(Item::PowerUp | Item::Sacrifice)) { DAMAGE_BOOST.value } else { 1.0 },
            crater: dig * big_bomb,
            thunder: armed == Some(Item::Thunder),
            teleport: armed == Some(Item::Teleport),
            splashed: [false; MAX_SEATS],
        }
    }
}

/// One volley being fired: who, how it leaves the barrel, and the turn's salvo.
pub(crate) struct Gun<'s> {
    pub salvo: &'s mut Salvo,
    pub seat: u8,
    pub aim: i16,
    pub bars: f64,
    pub tuning: &'static MobileTuning,
    pub false_angle: bool,
    /// The damage multiplier on a projectile the wind turned around (Hookclaw's); 1 for the rest.
    pub hook: f64,
}

impl Gun<'_> {
    /// The flight of a projectile `aim` degrees and `bars` off the volley's.
    pub fn launch(&self, game: &Game, aim: i16, bars: f64) -> Option<Flight> {
        game.launch(self.seat, self.aim + aim, self.bars + bars)
    }

    /// A blast at `center` from this volley's weapon, scaled by `boost` (weather) and the turn's
    /// item.
    pub fn blast(&self, center: Vec2, radius: f64, damage: DamageRange, boost: f64) -> Blast {
        Blast {
            center,
            radius,
            damage,
            weapon: self.tuning.weapon.value,
            false_angle: self.false_angle,
            boost: boost * self.salvo.boost,
        }
    }

    /// The blast where `track` landed: what the columns it passed through did to it, and the hook
    /// if the wind turned it around.
    pub fn blast_on(&self, track: &Track, radius: f64, damage: DamageRange) -> Blast {
        let hook = if track.legs.iter().any(Leg::hooked) { self.hook } else { 1.0 };
        self.blast(track.impact.at, radius, damage, track.marks.boost() * hook)
    }

    /// A projectile lands: `blast` goes off, direct on a mobile it struck, carving a crater of
    /// `crater` (widened by the salvo), and a bolt follows if Thunder is armed or the projectile
    /// passed through Electricity (one bolt either way: the item is the weather, GF-tiro). A
    /// projectile that left the map does nothing.
    pub fn land(&self, game: &mut Game, blast: &Blast, crater: f64, impact: &Impact, marks: Marks, effects: &mut Vec<Effect>) {
        let direct = match impact.contact {
            Contact::Gone | Contact::Column(_) => return,
            Contact::Terrain => None,
            Contact::Mobile(seat) => Some(seat),
        };
        game.explode(blast, crater * self.salvo.crater, direct, impact.tick, effects);
        if self.salvo.thunder || marks.charged {
            let strike = bolt::from_sky(game, blast.center.x);
            bolt::flash(game, &strike, BOLT_CRATER.value * self.salvo.crater, impact.tick + ticks(BOLT_DELAY_SECONDS.value), effects);
            let thunder = Blast {
                center: strike.at,
                radius: BOLT_REACH.value,
                damage: THUNDER_DAMAGE.value,
                weapon: THUNDER_WEAPON.value,
                false_angle: false,
                boost: 1.0,
            };
            bolt::zap(game, &strike, &thunder, effects);
        }
    }

    /// One plain projectile: it flies, and its blast goes off where it lands. Returns its track.
    pub fn plain(&self, game: &mut Game, flight: Flight, radius: f64, damage: DamageRange, effects: &mut Vec<Effect>) -> Track {
        let track = game.track(flight, 0, 0);
        effects.push(Effect::Fired {
            seat: self.seat,
            legs: track.legs.clone(),
        });
        self.land(game, &self.blast_on(&track, radius, damage), radius, &track.impact, track.marks, effects);
        track
    }
}

/// Fires one volley of `shot` from the salvo's seat: `Volley`, then its projectiles and what they
/// do. The caller has checked it is the seat's turn, the aim is in range and the shot is allowed.
pub(crate) fn fire(game: &mut Game, salvo: &mut Salvo, shot: Shot, aim: i16, power: Power, effects: &mut Vec<Effect>) {
    let seat = salvo.seat;
    effects.push(Effect::Volley { seat, shot });
    let Some(player) = game.players().get(usize::from(seat)).copied() else { return };
    let tuning = player.tuning();
    let mut gun = Gun {
        salvo,
        seat,
        aim,
        bars: power.bars(),
        tuning,
        false_angle: !shot.tuning(tuning).aim.value.is_true(aim),
        hook: 1.0,
    };
    if gun.salvo.teleport {
        teleport(game, &gun, effects);
        return;
    }
    match player.mobile {
        MobileKind::Coilback => coilback::fire(game, &gun, shot, effects),
        MobileKind::Lanternwing => lanternwing::fire(game, &gun, shot, effects),
        MobileKind::Hookclaw => hookclaw::fire(game, &mut gun, shot, effects),
        MobileKind::Sparkhorn => sparkhorn::fire(game, &mut gun, shot, effects),
        MobileKind::Marblemole => marblemole::fire(game, &gun, shot, effects),
        MobileKind::Spoutpot => spoutpot::fire(game, &gun, shot, effects),
    }
}

/// The Teleport item: the shot is a beacon that hurts and carves nothing, and carries its shooter
/// to where it lands, settled onto the ground there. If it leaves the map, nothing happens.
fn teleport(game: &mut Game, gun: &Gun, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    let track = game.track(flight, 0, 0);
    effects.push(Effect::Beacon { seat: gun.seat, legs: track.legs });
    if matches!(track.impact.contact, Contact::Gone | Contact::Column(_)) {
        return;
    }
    // A contact is inside the map, so its pixel is a valid column and row.
    #[allow(clippy::cast_possible_truncation)]
    let (x, y) = (track.impact.at.x.floor() as i32, track.impact.at.y.floor() as i32);
    if let Landing::On(to) = movement::settle(game.terrain(), Footing { x, y }) {
        game.place(gun.seat, to);
        effects.push(Effect::Teleported {
            seat: gun.seat,
            to,
            tick: track.impact.tick,
        });
    }
}

/// Flies several projectiles at once. Each lands (`land`, with its index) in the order it hits,
/// so the later ones meet the craters, and the deaths, of the earlier ones. Returns each one's
/// path, by index, and the landings' effects in order, so the caller can show the paths first.
pub(crate) fn barrage(game: &mut Game, flights: &[Flight], land: impl FnMut(&mut Game, usize, &Track, &mut Vec<Effect>)) -> (Vec<Vec<Leg>>, Vec<Effect>) {
    barrage_from(game, flights, 0, land)
}

/// `barrage` for projectiles that leave together at stage tick `offset` (Spoutpot's SS balls,
/// born in flight). Past a launch, no column catches them as they leave (`Game::track`).
pub(crate) fn barrage_from(game: &mut Game, flights: &[Flight], offset: u32, mut land: impl FnMut(&mut Game, usize, &Track, &mut Vec<Effect>)) -> (Vec<Vec<Leg>>, Vec<Effect>) {
    let mut paths = vec![Vec::new(); flights.len()];
    let mut effects = Vec::new();
    let mut waiting: Vec<usize> = (0..flights.len()).collect();
    loop {
        // Re-fly the rest after each landing: lost ground and mobiles can make a projectile land
        // later, never sooner, so the order holds.
        let next = waiting
            .iter()
            .enumerate()
            .map(|(place, &index)| (place, game.track(flights[index], 0, offset)))
            .min_by_key(|(place, track)| (track.impact.tick, *place));
        let Some((place, track)) = next else { break };
        let index = waiting.remove(place);
        land(game, index, &track, &mut effects);
        paths[index] = track.legs;
    }
    (paths, effects)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use crate::{
        game::{
            Effect, Event, MobileKind, Phase, Shot, Team,
            testing::{damage_to, direct_hit_power, duel, duel_of, explosions, game_of, seat, shoot},
        },
        items::{Item, Loadout},
        units::{Centis, Facing, Power},
        weather::{Column, WeatherKind},
    };

    /// A duel where seat 0 carries `item` and has just used it.
    fn armed(item: Item, distance: i32) -> crate::game::Game {
        let mut game = duel(distance);
        game.players[0].items = Loadout::new(&[item]).unwrap().positions();
        game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
        game
    }

    fn crater(effects: &[Effect]) -> f64 {
        effects.iter().find_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None }).unwrap()
    }

    #[test]
    fn every_volley_starts_with_its_shot_then_its_paths() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::S1, 35, Power::nearest(2.4));
        assert_eq!(effects[0], Effect::Volley { seat: 0, shot: Shot::S1 });
        assert!(matches!(effects[1], Effect::Fired { seat: 0, .. }), "{effects:?}");
    }

    #[test]
    fn force_boosts_a_shot_that_passes_through_it() {
        let mut calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let mut forced = calm.clone();
        let midway = forced.players()[0].footing.x + 400;
        forced.set_columns(vec![Column::new(WeatherKind::Force, midway)]);
        let calm_damage = damage_to(&shoot(&mut calm, 0, Shot::S1, 35, power), 1);
        let forced_effects = shoot(&mut forced, 0, Shot::S1, 35, power);
        assert!(damage_to(&forced_effects, 1) > calm_damage, "{forced_effects:?}");
    }

    /// `game` with a column of `kind` halfway between the duelists.
    fn midway(game: &crate::game::Game, kind: WeatherKind) -> crate::game::Game {
        let mut game = game.clone();
        let x = game.players()[0].footing.x + 400;
        game.set_columns(vec![Column::new(kind, x)]);
        game
    }

    fn bolts(effects: &[Effect]) -> usize {
        effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count()
    }

    #[test]
    fn drain_halves_a_shot_that_passes_through_it() {
        let mut calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let mut drained = midway(&calm, WeatherKind::Drain);
        let calm_damage = damage_to(&shoot(&mut calm, 0, Shot::S1, 35, power), 1);
        let drained_damage = damage_to(&shoot(&mut drained, 0, Shot::S1, 35, power), 1);
        assert!((f64::from(drained_damage) / f64::from(calm_damage) - 0.5).abs() < 0.02, "{drained_damage} against {calm_damage}");
    }

    #[test]
    fn electricity_strikes_where_a_shot_through_it_lands() {
        let calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let mut charged = midway(&calm, WeatherKind::Electricity);
        let effects = shoot(&mut charged, 0, Shot::S1, 35, power);
        assert_eq!(bolts(&effects), 1, "{effects:?}");
        assert_eq!(bolts(&shoot(&mut calm.clone(), 0, Shot::S1, 35, power)), 0);
    }

    #[test]
    fn thunder_through_electricity_still_strikes_once() {
        let game = armed(Item::Thunder, 800);
        let power = direct_hit_power(&game, 35);
        let mut charged = midway(&game, WeatherKind::Electricity);
        assert_eq!(bolts(&shoot(&mut charged, 0, Shot::S1, 35, power)), 1, "the item is the weather (GF-tiro)");
    }

    #[test]
    fn a_shot_off_the_side_of_the_map_does_nothing() {
        let mut game = duel(800);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let effects = shoot(&mut game, 0, Shot::S1, 30, Power::MAX);
        assert_eq!(explosions(&effects), 0, "{effects:?}");
    }

    #[test]
    fn power_up_and_sacrifice_hit_a_third_harder() {
        let mut plain = duel(800);
        let power = direct_hit_power(&plain, 35);
        let base = damage_to(&shoot(&mut plain, 0, Shot::S1, 35, power), 1);
        for item in [Item::PowerUp, Item::Sacrifice] {
            let mut game = armed(item, 800);
            let boosted = damage_to(&shoot(&mut game, 0, Shot::S1, 35, power), 1);
            assert!((f64::from(boosted) / f64::from(base) - 1.33).abs() < 0.02, "{item:?}: {boosted} against {base}");
        }
    }

    #[test]
    fn dig_widens_the_crater_but_not_the_damage() {
        let mut plain = duel(1200);
        let short = Power::nearest(1.5);
        let plain_effects = shoot(&mut plain, 0, Shot::S1, 45, short);
        let mut game = armed(Item::Dig, 1200);
        let dug = shoot(&mut game, 0, Shot::S1, 45, short);
        assert!((crater(&dug) - crater(&plain_effects) * 1.25).abs() < 1e-9);
        let mut plain = duel(800);
        let power = direct_hit_power(&plain, 35);
        let mut game = armed(Item::Dig, 800);
        assert_eq!(damage_to(&shoot(&mut game, 0, Shot::S1, 35, power), 1), damage_to(&shoot(&mut plain, 0, Shot::S1, 35, power), 1));
    }

    #[test]
    fn thunder_strikes_where_each_projectile_lands() {
        let mut game = armed(Item::Thunder, 800);
        let power = direct_hit_power(&game, 30);
        let effects = shoot(&mut game, 0, Shot::S2, 30, power);
        let bolts = effects.iter().filter(|e| matches!(e, Effect::Bolt { .. })).count();
        assert_eq!(bolts, 2, "Coilback's S2 lands two warheads: {effects:?}");
    }

    #[test]
    fn dual_fires_the_shot_twice_and_pays_its_delay_once() {
        let mut plain = duel(800);
        let power = direct_hit_power(&plain, 35);
        let once = damage_to(&shoot(&mut plain, 0, Shot::S1, 35, power), 1);
        let mut game = armed(Item::Dual, 800);
        let effects = shoot(&mut game, 0, Shot::S1, 35, power);
        let volleys: Vec<&Effect> = effects.iter().filter(|e| matches!(e, Effect::Volley { .. })).collect();
        assert_eq!(volleys, [&Effect::Volley { seat: 0, shot: Shot::S1 }, &Effect::Volley { seat: 0, shot: Shot::S1 }]);
        assert!(damage_to(&effects, 1) > once, "{effects:?}");
        // Coilback: base 520 + S1 250 + Dual 600 + Armor's item surcharge 40.
        assert_eq!(game.players()[0].clock.delay, 1410);
    }

    #[test]
    fn dual_plus_fires_the_other_shot_second() {
        let mut game = armed(Item::DualPlus, 800);
        let effects = shoot(&mut game, 0, Shot::S2, 35, Power::nearest(2.2));
        let volleys: Vec<Shot> = effects.iter().filter_map(|e| if let Effect::Volley { shot, .. } = e { Some(*shot) } else { None }).collect();
        assert_eq!(volleys, [Shot::S2, Shot::S1]);
        assert_eq!(game.players()[0].clock.delay, 520 + 440 + 250 + 40);
    }

    #[test]
    fn dual_waits_for_its_shooter_to_land_and_stops_if_it_died() {
        // Seat 0 shoots itself at point blank: straight down is out of range, so stand on a
        // pillar one shot can cut out from under it.
        let mut game = armed(Item::Dual, 800);
        for y in 500..600 {
            for x in 260..340 {
                if !(295..306).contains(&x) {
                    game.terrain.carve(crate::units::Vec2::new(f64::from(x) + 0.5, f64::from(y) + 0.5), 0.6);
                }
            }
        }
        // A weak lob lands beside the pillar's foot.
        let effects = shoot(&mut game, 0, Shot::S1, 55, Power::nearest(0.6));
        let volleys = effects.iter().filter(|e| matches!(e, Effect::Volley { .. })).count();
        if effects.iter().any(|e| matches!(e, Effect::Died { seat: 0, .. })) {
            assert_eq!(volleys, 1, "a dead shooter fires no second shot: {effects:?}");
        } else {
            assert_eq!(volleys, 2, "{effects:?}");
        }
    }

    #[test]
    fn teleport_carries_the_shooter_to_where_its_beacon_lands() {
        let mut game = armed(Item::Teleport, 1200);
        let effects = shoot(&mut game, 0, Shot::S1, 45, Power::nearest(1.8));
        assert!(matches!(effects[1], Effect::Beacon { seat: 0, .. }), "{effects:?}");
        let Some(&Effect::Teleported { seat: 0, to, .. }) = effects.iter().find(|e| matches!(e, Effect::Teleported { .. })) else {
            panic!("no teleport: {effects:?}")
        };
        assert_eq!(game.players()[0].footing, to);
        assert!(to.x > 400 && to.y == 500, "{to:?}");
        assert_eq!(explosions(&effects), 0, "a beacon carves nothing");
        assert_eq!(damage_to(&effects, 1), 0);
        assert_eq!(game.players()[0].clock.delay, 520 + 250 + 150 + 40, "the shot's delay and the item's");
    }

    #[test]
    fn a_beacon_off_the_map_teleports_no_one() {
        let mut game = armed(Item::Teleport, 800);
        game.apply(Event::Walk { seat: 0, facing: Facing::Left, steps: 0 }).unwrap();
        let before = game.players()[0].footing;
        let effects = shoot(&mut game, 0, Shot::S1, 30, Power::MAX);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Teleported { .. })));
        assert_eq!(game.players()[0].footing, before);
    }

    #[test]
    fn every_mobile_fires_every_shot_and_ends_its_turn() {
        for mobile in MobileKind::ALL {
            for shot in [Shot::S1, Shot::S2, Shot::Ss] {
                let mut game = duel_of(mobile, MobileKind::Coilback, 700);
                let range = shot.tuning(mobile.tuning()).aim.value;
                let effects = shoot(&mut game, 0, shot, range.true_min, Power::nearest(2.4));
                assert!(effects.iter().any(|e| matches!(e, Effect::TurnStarted { seat: 1 })), "{mobile:?} {shot:?}: {effects:?}");
                assert_eq!(game.phase(), Phase::Turn { seat: 1 });
            }
        }
    }

    #[test]
    fn several_volleys_land_in_one_game_without_desync() {
        let seats = vec![
            seat(Team::A, MobileKind::Hookclaw, 300, Facing::Right),
            seat(Team::B, MobileKind::Marblemole, 900, Facing::Left),
            seat(Team::A, MobileKind::Sparkhorn, 1500, Facing::Right),
            seat(Team::B, MobileKind::Lanternwing, 2100, Facing::Left),
        ];
        let mut game = game_of(seats);
        let replay = game.clone();
        let mut events = Vec::new();
        for (seat, shot) in [(0, Shot::S2), (1, Shot::S2), (2, Shot::S2), (3, Shot::S2)] {
            if game.phase() != (Phase::Turn { seat }) {
                break;
            }
            let range = shot.tuning(game.players()[usize::from(seat)].tuning()).aim.value;
            let event = Event::Shoot {
                seat,
                shot,
                aim: range.true_min,
                power: Power::nearest(2.2),
                time: Centis(0),
            };
            game.apply(event).unwrap();
            events.push(event);
        }
        let mut again = replay;
        for event in events {
            again.apply(event).unwrap();
        }
        assert_eq!(again.state_hash().unwrap(), game.state_hash().unwrap());
    }
}
