//! A game: its state, the events that change it, and the effects each event has. The server applies
//! accepted events authoritatively; every client and the lab apply the same events to the same
//! state, so the state hash after each event must match everywhere.
//!
//! The sim never rolls dice. When the rules need something random (a weather forecast, new wind),
//! `pending_roll` says so, the server rolls it and applies it as an event, and until then players
//! can't act. Every log is therefore canonical: the same rolls at the same points.

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    ballistics::{self, Flight},
    base::{Res, Void},
    body::{HitFrame, Pose},
    damage::{self, Blast, Target},
    drops::Drop,
    items::{Item, Loadout, MAX_ITEMS},
    map::{MapAssets, MapRef},
    movement::{self, Footing, Landing, Step},
    shots,
    terrain::{DirtyRect, Terrain},
    tuning::{MAX_FLIGHT_SECONDS, MobileTuning, ShotTuning, TORNADO_HOLD_SECONDS, TURN_TIME_LIMIT, coilback, hookclaw, lanternwing, marblemole, sparkhorn},
    turn::{self, Clock, Ending},
    units::{Centis, Facing, Power, TICKS_PER_SECOND, Vec2, Wind},
    weather::{Column, Forecast, Weather, WeatherKind},
};

/// The most players a room seats: 4v4.
pub const MAX_SEATS: usize = 8;

/// The most times weather may redirect one shot (mirrors facing each other would otherwise
/// trade it back and forth until it falls).
const MAX_LEGS: usize = 16;

/// A team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Team {
    A,
    B,
}

/// The mobiles that exist so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MobileKind {
    Coilback,
    Lanternwing,
    Hookclaw,
    Sparkhorn,
    Marblemole,
}

impl MobileKind {
    /// Every mobile, in a fixed order.
    pub const ALL: [Self; 5] = [Self::Coilback, Self::Lanternwing, Self::Hookclaw, Self::Sparkhorn, Self::Marblemole];

    #[must_use]
    pub fn tuning(self) -> &'static MobileTuning {
        match self {
            Self::Coilback => &coilback::MOBILE,
            Self::Lanternwing => &lanternwing::MOBILE,
            Self::Hookclaw => &hookclaw::MOBILE,
            Self::Sparkhorn => &sparkhorn::MOBILE,
            Self::Marblemole => &marblemole::MOBILE,
        }
    }
}

/// Which of a mobile's shots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shot {
    S1,
    S2,
    Ss,
}

impl Shot {
    #[must_use]
    pub fn tuning(self, mobile: &MobileTuning) -> &ShotTuning {
        match self {
            Self::S1 => &mobile.s1,
            Self::S2 => &mobile.s2,
            Self::Ss => &mobile.ss,
        }
    }
}

/// One seat in the setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub team: Team,
    pub mobile: MobileKind,
    /// The column the mobile drops into at the start.
    pub spawn_x: i32,
    pub facing: Facing,
    pub items: Loadout,
}

/// What sudden death does once it starts (GunBound's room options: WP06, AK, GF-tiro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuddenDeathKind {
    /// Bigger craters, and no SS.
    BigBomb,
    /// Every S1 and S2 fires twice, and no SS.
    Double,
    /// SS every turn.
    SsDeath,
}

/// Sudden death: once `turn` turns are done, `kind` applies and items are off for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SuddenDeath {
    pub kind: SuddenDeathKind,
    pub turn: u32,
}

/// Everything a game starts from. The server rolls the random parts (`order`, `wind`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    pub map: MapRef,
    pub seats: Vec<Seat>,
    /// The starting turn order, as seat indices: a permutation the server rolls with teams
    /// alternating.
    pub order: Vec<u8>,
    pub wind: Wind,
    /// `None` plays without sudden death.
    pub sudden_death: Option<SuddenDeath>,
}

/// Something that happened, accepted by the server. A game is its setup plus these, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    /// The active player walks up to `steps` pixels (turning to `facing` first); the move budget
    /// and the terrain may stop it sooner.
    Walk { seat: u8, facing: Facing, steps: u16 },
    /// The active player fires, ending the turn. `time` is how long the turn took, as the server
    /// measured it.
    Shoot { seat: u8, shot: Shot, aim: i16, power: Power, time: Centis },
    /// The active player passes (or the server passes for them at the time limit).
    Pass { seat: u8, time: Centis },
    /// A player left or was kicked: their mobile dies where it stands.
    Forfeit { seat: u8 },
    /// The server rolled the wind it owed (`Roll::Wind`).
    Wind(Wind),
    /// The server rolled the forecast it owed (`Roll::Weather`).
    Weather(Forecast),
    /// The active player uses the item at `slot` in their loadout (one item a turn). Its effect
    /// is immediate (a heal, the wind) or rides on the turn's shot (Dual, Power Up).
    UseItem { seat: u8, slot: u8 },
    /// A dead player spins their slot machine; the server then owes `Roll::Drop`.
    Spin { seat: u8 },
    /// The server rolled what the slot machine dropped (`Roll::Drop`).
    Drop(Drop),
}

/// Something random the server owes the game before anyone may act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Roll {
    /// A new forecast for the back of the weather queue: any `WeatherKind`, and a column x in
    /// `0..width`.
    Weather { width: u32 },
    /// New wind (the Wind weather just came up): strength up to the room's max, any direction.
    Wind,
    /// A slot machine was spun: any `DropKind`, and a column x in `0..width`.
    Drop { width: u32 },
}

/// Why a mobile died.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Death {
    /// Its HP ran out.
    Blasted,
    /// It fell out of the map ("bunge").
    Bunged,
    /// Its player left or was kicked.
    Forfeited,
}

/// One stretch of a projectile's flight: weather may break a flight into several.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    pub flight: Flight,
    /// The leg's own ticks this stretch covers.
    pub from: u32,
    pub to: u32,
    /// The shot's elapsed ticks when the leg's own tick 0 happened (weather holds add to it).
    pub offset: u32,
}

/// What an event did, in order, for the client to show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Walked {
        seat: u8,
        steps: u16,
        to: Footing,
    },
    /// A projectile's whole path, leg by leg.
    Fired {
        seat: u8,
        legs: Vec<Leg>,
    },
    /// `tick` counts from the shot's launch.
    Exploded {
        at: Vec2,
        radius: f64,
        tick: u32,
        dirty: Option<DirtyRect>,
    },
    Damaged {
        seat: u8,
        amount: i32,
    },
    Fell {
        seat: u8,
        to: Footing,
    },
    Died {
        seat: u8,
        death: Death,
    },
    WindChanged(Wind),
    Forecast(Forecast),
    /// The front of the weather queue came up.
    WeatherChanged {
        columns: Vec<Column>,
        wind_owed: bool,
    },
    TurnStarted {
        seat: u8,
    },
    GameOver {
        winner: Option<Team>,
    },
}

/// What a projectile touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Contact {
    Terrain,
    Mobile(u8),
    /// It entered a weather column (`fly` reports it; `track` acts on it).
    Column(u8),
    /// It left the map's sides or bottom (or flew too long) without touching anything.
    Gone,
}

/// Where and when a projectile stopped.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    pub tick: u32,
    pub at: Vec2,
    pub contact: Contact,
}

/// A projectile followed through the weather to what it hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub legs: Vec<Leg>,
    /// `tick` counts from launch.
    pub impact: Impact,
    /// It passed through a Force column.
    pub forced: bool,
    /// The last leg and the leg's own tick at impact, to carry on from (Coilback's S2).
    pub last: Leg,
    pub last_tick: u32,
}

/// A seated player's state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub team: Team,
    pub mobile: MobileKind,
    pub footing: Footing,
    pub facing: Facing,
    pub tilt: f64,
    pub hp: i32,
    /// What's left of the shield (`MobileTuning::shield`), which soaks damage before HP.
    pub shield: i32,
    pub alive: bool,
    pub clock: Clock,
    pub moves_left: u16,
    /// The loadout's unused items, by position; `Event::UseItem` names a position.
    pub items: [Option<Item>; MAX_ITEMS],
    /// The item used this turn, if any: it rides on the turn's shot and adds to its delay.
    pub armed: Option<Item>,
    /// Own turns until SS may fire again; 0 when it may.
    pub ss_lock: u8,
}

impl Player {
    #[must_use]
    pub fn pose(&self) -> Pose {
        Pose {
            foot: self.footing.world(),
            facing: self.facing,
            tilt: self.tilt,
        }
    }

    #[must_use]
    pub fn tuning(&self) -> &'static MobileTuning {
        self.mobile.tuning()
    }
}

/// Whose turn it is, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Turn { seat: u8 },
    Over { winner: Option<Team> },
}

/// The whole game state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    terrain: Terrain,
    players: Vec<Player>,
    wind: Wind,
    weather: Weather,
    /// Turns per weather: one per seat (SW-weather).
    round: u32,
    /// Turns completed so far.
    turns: i64,
    phase: Phase,
}

impl Game {
    /// Starts a game, loading a drawn map's mask from `assets`.
    ///
    /// # Errors
    ///
    /// Fails on a bad setup: fewer than two teams, more than 8 seats, an order that is not a
    /// permutation of the seats, a map that can't be loaded, or a spawn with no ground under it.
    pub fn new(setup: &Setup, assets: &dyn MapAssets) -> Res<Self> {
        let seats = setup.seats.len();
        ensure!((2..=MAX_SEATS).contains(&seats), "a game needs 2 to {MAX_SEATS} seats, not {seats}");
        ensure!(setup.seats.iter().any(|s| s.team == Team::A) && setup.seats.iter().any(|s| s.team == Team::B), "both teams need a seat");
        let mut sorted = setup.order.clone();
        sorted.sort_unstable();
        ensure!(
            sorted.iter().copied().eq((0..seats).filter_map(|s| u8::try_from(s).ok())),
            "the order {:?} is not a permutation of the seats",
            setup.order
        );
        ensure!(setup.wind.direction < 360, "wind direction {} is not in 0..360", setup.wind.direction);

        let terrain = setup.map.terrain(assets)?;
        let mut players = Vec::with_capacity(seats);
        for (index, seat) in setup.seats.iter().enumerate() {
            let rank = setup.order.iter().position(|&s| usize::from(s) == index).unwrap_or(index);
            let Landing::On(footing) = movement::settle(&terrain, Footing { x: seat.spawn_x, y: 0 }) else {
                bail!("seat {index} spawns at x = {} with no ground under it", seat.spawn_x);
            };
            let tuning = seat.mobile.tuning();
            players.push(Player {
                team: seat.team,
                mobile: seat.mobile,
                footing,
                facing: seat.facing,
                tilt: movement::tilt(&terrain, footing, tuning.ground_probes.value),
                hp: tuning.hp.value,
                shield: tuning.shield.value,
                alive: true,
                // Before anyone acts, the first in the rolled order has waited longest.
                clock: Clock {
                    delay: 0,
                    last_turn: i64::try_from(rank).unwrap_or(0) - i64::try_from(seats).unwrap_or(0),
                },
                moves_left: 0,
                items: seat.items.positions(),
                armed: None,
                ss_lock: 0,
            });
        }

        let round = u32::try_from(seats).unwrap_or(1);
        let mut game = Self {
            terrain,
            players,
            wind: setup.wind,
            weather: Weather::new(round),
            round,
            turns: 0,
            phase: Phase::Over { winner: None },
        };
        game.start_next_turn(&mut Vec::new());
        Ok(game)
    }

    #[must_use]
    pub fn terrain(&self) -> &Terrain {
        &self.terrain
    }

    #[must_use]
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    #[must_use]
    pub fn wind(&self) -> Wind {
        self.wind
    }

    #[must_use]
    pub fn weather(&self) -> &Weather {
        &self.weather
    }

    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Turns completed so far.
    #[must_use]
    pub fn turns(&self) -> i64 {
        self.turns
    }

    /// What the server must roll before anyone may act, if anything. Wind first, then forecasts.
    #[must_use]
    pub fn pending_roll(&self) -> Option<Roll> {
        if matches!(self.phase, Phase::Over { .. }) {
            return None;
        }
        if self.weather.wind_owed {
            Some(Roll::Wind)
        } else if self.weather.short() {
            Some(Roll::Weather { width: self.terrain.width() })
        } else {
            None
        }
    }

    /// Whether `seat` may spin its slot machine now (`Event::Spin`), so the server knows when a
    /// dead bot can.
    #[must_use]
    pub fn can_spin(&self, seat: u8) -> bool {
        // The slot machine lands with the M3 sim work.
        let _ = seat;
        false
    }

    fn player(&self, seat: u8) -> Res<&Player> {
        self.players.get(usize::from(seat)).ok_or_else(|| anyhow::anyhow!("there is no seat {seat}"))
    }

    /// Applies one event and returns its effects.
    ///
    /// # Errors
    ///
    /// Rejects an event that the rules don't allow now: the wrong player, a finished game, a roll
    /// that isn't owed, a move while one is, or an aim outside the shot's range. A rejected event
    /// changes nothing.
    pub fn apply(&mut self, event: Event) -> Res<Vec<Effect>> {
        let mut effects = Vec::new();
        match event {
            Event::Wind(wind) => {
                ensure!(self.pending_roll() == Some(Roll::Wind), "no wind roll is owed");
                ensure!(wind.direction < 360, "wind direction {} is not in 0..360", wind.direction);
                self.wind = wind;
                self.weather.wind_owed = false;
                effects.push(Effect::WindChanged(wind));
            }
            Event::Weather(forecast) => {
                ensure!(matches!(self.pending_roll(), Some(Roll::Weather { .. })), "no forecast is owed");
                ensure!(u32::try_from(forecast.x).is_ok_and(|x| x < self.terrain.width()), "forecast x {} is off the map", forecast.x);
                self.weather.queue.push(forecast);
                effects.push(Effect::Forecast(forecast));
            }
            Event::Forfeit { seat } => {
                ensure!(!matches!(self.phase, Phase::Over { .. }), "the game is over");
                ensure!(self.player(seat)?.alive, "seat {seat} is already out");
                self.kill(seat, Death::Forfeited, &mut effects);
                if self.phase == (Phase::Turn { seat }) {
                    self.end_turn(seat, Ending::Pass, Centis(0), &mut effects);
                } else {
                    self.check_over(&mut effects);
                }
            }
            Event::Walk { seat, facing, steps } => {
                self.ensure_turn(seat)?;
                self.walk(seat, facing, steps, &mut effects);
            }
            Event::Shoot { seat, shot, aim, power, time } => {
                self.ensure_turn(seat)?;
                let player = self.player(seat)?;
                let range = shot.tuning(player.tuning()).aim.value;
                ensure!(range.contains(aim), "{} can't aim its {shot:?} at {aim} degrees ({}..={})", player.tuning().name, range.min, range.max);
                shots::fire(self, seat, shot, aim, power, &mut effects);
                self.settle_all(&mut effects);
                let delay = shot.tuning(self.player(seat)?.tuning()).delay.value;
                self.end_turn(seat, Ending::Shot(delay), time, &mut effects);
            }
            Event::Pass { seat, time } => {
                self.ensure_turn(seat)?;
                self.end_turn(seat, Ending::Pass, time, &mut effects);
            }
            Event::UseItem { .. } | Event::Spin { .. } | Event::Drop(_) => bail!("this build doesn't play items or the slot machine yet"),
        }
        Ok(effects)
    }

    fn ensure_turn(&self, seat: u8) -> Void {
        if let Some(roll) = self.pending_roll() {
            bail!("waiting for the server's {roll:?} roll");
        }
        match self.phase {
            Phase::Turn { seat: current } if current == seat => Ok(()),
            Phase::Turn { seat: current } => bail!("it is seat {current}'s turn, not seat {seat}'s"),
            Phase::Over { .. } => bail!("the game is over"),
        }
    }

    fn walk(&mut self, seat: u8, facing: Facing, steps: u16, effects: &mut Vec<Effect>) {
        let index = usize::from(seat);
        self.players[index].facing = facing;
        let mut taken = 0;
        let mut died = false;
        while taken < steps && self.players[index].moves_left > 0 {
            match movement::step(&self.terrain, self.players[index].footing, facing) {
                Step::Blocked => break,
                Step::Walked(footing) | Step::Fell(Landing::On(footing)) => self.players[index].footing = footing,
                Step::Fell(Landing::OffMap) => died = true,
            }
            self.players[index].moves_left -= 1;
            taken += 1;
            if died {
                break;
            }
        }
        let player = &mut self.players[index];
        player.tilt = movement::tilt(&self.terrain, player.footing, player.tuning().ground_probes.value);
        effects.push(Effect::Walked { seat, steps: taken, to: player.footing });
        if died {
            self.kill(seat, Death::Bunged, effects);
            // Walking off the map ends the turn like a pass.
            self.end_turn(seat, Ending::Pass, Centis(0), effects);
        }
    }

    /// The flight of a shot from `seat` at `aim` degrees and `bars` of power (continuous, so bots
    /// and tests can search between power steps).
    #[must_use]
    pub fn launch(&self, seat: u8, aim: i16, bars: f64) -> Option<Flight> {
        let player = self.players.get(usize::from(seat))?;
        let tuning = player.tuning();
        let pose = player.pose();
        Some(Flight {
            origin: pose.muzzle(tuning, aim),
            velocity: pose.aim_direction(aim) * ballistics::launch_speed(bars),
            accel: ballistics::acceleration(tuning.gravity.value, self.wind, tuning.wind_sensitivity.value),
        })
    }

    /// Flies `flight` from `from_tick` until it touches a living mobile or solid ground, enters a
    /// weather column, or leaves the map. Mobiles are checked before terrain, so a shot grazing a
    /// mobile's feet is direct.
    #[must_use]
    pub fn fly(&self, flight: &Flight, from_tick: u32) -> Impact {
        let width = f64::from(self.terrain.width());
        let height = f64::from(self.terrain.height());
        let last = MAX_FLIGHT_SECONDS.value * TICKS_PER_SECOND;
        let targets: Vec<(u8, HitFrame)> = self
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.alive)
            .filter_map(|(seat, p)| Some((u8::try_from(seat).ok()?, HitFrame::new(&p.pose(), p.tuning().hitbox.value))))
            .collect();
        let columns = &self.weather.columns;
        // Only entering a column counts, so a shot that starts inside one (a fresh leg) isn't
        // redirected again at once.
        let mut inside: Vec<bool> = columns.iter().map(|c| c.contains(flight.at(from_tick).x)).collect();

        for tick in from_tick..=last {
            let at = flight.at(tick);
            if at.x < 0.0 || at.x >= width || at.y >= height {
                return Impact { tick, at, contact: Contact::Gone };
            }
            if let Some(&(seat, _)) = targets.iter().find(|(_, frame)| frame.contains(at)) {
                return Impact { tick, at, contact: Contact::Mobile(seat) };
            }
            // In range: x is inside the map, and y is below it or above the bottom.
            #[allow(clippy::cast_possible_truncation)]
            if self.terrain.is_solid(at.x.floor() as i32, at.y.floor() as i32) {
                return Impact { tick, at, contact: Contact::Terrain };
            }
            for (index, column) in columns.iter().enumerate() {
                let now = column.contains(at.x);
                if now && !inside[index] {
                    let index = u8::try_from(index).unwrap_or(u8::MAX);
                    return Impact {
                        tick,
                        at,
                        contact: Contact::Column(index),
                    };
                }
                inside[index] = now;
            }
        }
        Impact {
            tick: last,
            at: flight.at(last),
            contact: Contact::Gone,
        }
    }

    /// Follows a projectile through the weather to what it hits. Force marks it; Mirror sends it
    /// back (its sideways speed and wind reversed); Tornado holds it, then flings it back up.
    /// `from_tick` is in `flight`'s own ticks, and `offset` is the shot's elapsed ticks at
    /// `flight`'s tick 0.
    #[must_use]
    pub fn track(&self, flight: Flight, from_tick: u32, offset: u32) -> Track {
        let mut legs = Vec::new();
        let mut leg = Leg {
            flight,
            from: from_tick,
            to: from_tick,
            offset,
        };
        // Where to fly on from; a Force beam moves it on without starting a new leg.
        let mut cursor = from_tick;
        let mut forced = false;
        loop {
            let impact = self.fly(&leg.flight, cursor);
            leg.to = impact.tick;
            let column = match impact.contact {
                Contact::Column(index) if legs.len() < MAX_LEGS => self.weather.columns.get(usize::from(index)).copied(),
                _ => None,
            };
            let Some(column) = column else {
                legs.push(leg);
                let global = Impact { tick: impact.tick + leg.offset, ..impact };
                let contact = if matches!(impact.contact, Contact::Column(_)) { Contact::Gone } else { impact.contact };
                return Track {
                    legs,
                    impact: Impact { contact, ..global },
                    forced,
                    last: leg,
                    last_tick: impact.tick,
                };
            };
            let velocity = leg.flight.velocity_at(impact.tick);
            let next = match column.kind {
                WeatherKind::Force => {
                    forced = true;
                    cursor = impact.tick + 1;
                    continue;
                }
                WeatherKind::Mirror => Some((
                    Flight {
                        origin: impact.at,
                        velocity: Vec2::new(-velocity.x, velocity.y),
                        accel: Vec2::new(-leg.flight.accel.x, leg.flight.accel.y),
                    },
                    0,
                )),
                WeatherKind::Tornado => {
                    // Seconds to ticks: a small whole-tick count, exact in f64.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let hold = (TORNADO_HOLD_SECONDS.value * f64::from(TICKS_PER_SECOND)) as u32;
                    Some((
                        Flight {
                            origin: impact.at,
                            velocity: Vec2::new(velocity.x, -velocity.y.abs()),
                            accel: leg.flight.accel,
                        },
                        hold,
                    ))
                }
                // Wind never stands as a column; were one there, it would do nothing.
                WeatherKind::Wind => {
                    cursor = impact.tick + 1;
                    continue;
                }
            };
            legs.push(leg);
            let Some((flight, hold)) = next else { continue };
            leg = Leg {
                flight,
                from: 0,
                to: 0,
                offset: leg.offset + impact.tick + hold,
            };
            cursor = 0;
        }
    }

    /// Detonates `blast` at `tick`: carves the terrain, then damages every living mobile in reach.
    pub(crate) fn explode(&mut self, blast: &Blast, direct: Option<u8>, tick: u32, effects: &mut Vec<Effect>) {
        let dirty = self.terrain.carve(blast.center, blast.radius);
        effects.push(Effect::Exploded {
            at: blast.center,
            radius: blast.radius,
            tick,
            dirty,
        });
        for seat in 0..self.players.len() {
            let player = self.players[seat];
            let Ok(seat) = u8::try_from(seat) else { continue };
            if !player.alive {
                continue;
            }
            let target = Target {
                pose: player.pose(),
                hitbox: player.tuning().hitbox.value,
                armor: player.tuning().armor.value,
            };
            let amount = damage::damage(blast, &target, direct == Some(seat));
            if amount <= 0 {
                continue;
            }
            let player = &mut self.players[usize::from(seat)];
            player.hp -= amount;
            effects.push(Effect::Damaged { seat, amount });
            if player.hp <= 0 {
                self.kill(seat, Death::Blasted, effects);
            }
        }
    }

    fn kill(&mut self, seat: u8, death: Death, effects: &mut Vec<Effect>) {
        let player = &mut self.players[usize::from(seat)];
        player.alive = false;
        player.hp = player.hp.max(0);
        effects.push(Effect::Died { seat, death });
    }

    /// Drops every living mobile whose ground is gone, and re-reads every tilt.
    fn settle_all(&mut self, effects: &mut Vec<Effect>) {
        for index in 0..self.players.len() {
            let Ok(seat) = u8::try_from(index) else { continue };
            let player = self.players[index];
            if !player.alive {
                continue;
            }
            match movement::settle(&self.terrain, player.footing) {
                Landing::On(footing) => {
                    if footing != player.footing {
                        effects.push(Effect::Fell { seat, to: footing });
                    }
                    let player = &mut self.players[index];
                    player.footing = footing;
                    player.tilt = movement::tilt(&self.terrain, footing, player.tuning().ground_probes.value);
                }
                Landing::OffMap => self.kill(seat, Death::Bunged, effects),
            }
        }
    }

    fn end_turn(&mut self, seat: u8, ending: Ending, time: Centis, effects: &mut Vec<Effect>) {
        let time = time.min(TURN_TIME_LIMIT.value);
        let player = &mut self.players[usize::from(seat)];
        let tuning = player.tuning();
        player.clock.delay += turn::turn_delay(tuning.base_delay.value, ending, 0, tuning.delay_per_second.value, time);
        player.clock.last_turn = self.turns;
        self.turns += 1;
        let before = (self.weather.columns.clone(), self.weather.wind_owed);
        self.weather.tick(self.round);
        if (self.weather.columns.clone(), self.weather.wind_owed) != before {
            effects.push(Effect::WeatherChanged {
                columns: self.weather.columns.clone(),
                wind_owed: self.weather.wind_owed,
            });
        }
        self.start_next_turn(effects);
    }

    /// Ends the game if fewer than two teams have anyone standing. Returns whether it did.
    fn check_over(&mut self, effects: &mut Vec<Effect>) -> bool {
        let alive: Vec<Team> = [Team::A, Team::B].into_iter().filter(|&team| self.players.iter().any(|p| p.alive && p.team == team)).collect();
        if alive.len() >= 2 {
            return false;
        }
        let winner = alive.first().copied();
        self.phase = Phase::Over { winner };
        effects.push(Effect::GameOver { winner });
        true
    }

    fn start_next_turn(&mut self, effects: &mut Vec<Effect>) {
        if self.check_over(effects) {
            return;
        }
        let next = turn::next(self.players.iter().enumerate().filter(|(_, p)| p.alive).map(|(seat, p)| (seat, &p.clock)));
        let Some(seat) = next.and_then(|seat| u8::try_from(seat).ok()) else {
            self.phase = Phase::Over { winner: None };
            effects.push(Effect::GameOver { winner: None });
            return;
        };
        let player = &mut self.players[usize::from(seat)];
        player.moves_left = player.tuning().move_budget.value;
        self.phase = Phase::Turn { seat };
        effects.push(Effect::TurnStarted { seat });
    }

    /// Stands `columns` on the map directly, for tests of weather on shots.
    #[cfg(test)]
    pub(crate) fn set_columns(&mut self, columns: Vec<Column>) {
        self.weather.columns = columns;
    }

    /// A hash of the whole state: equal on every target that applied the same events.
    ///
    /// # Errors
    ///
    /// Fails only if the state cannot be serialized.
    pub fn state_hash(&self) -> Res<u64> {
        let bytes = postcard::to_allocvec(self)?;
        Ok(crate::hash::fnv1a(&bytes))
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! Helpers for this crate's unit tests.

    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{drops::DropKind, map::NoAssets};

    pub fn duel_setup(distance: i32) -> Setup {
        Setup {
            map: MapRef::Flat { width: 2400, height: 600, ground: 500 },
            seats: vec![
                Seat {
                    team: Team::A,
                    mobile: MobileKind::Coilback,
                    spawn_x: 300,
                    facing: Facing::Right,
                    items: Loadout::default(),
                },
                Seat {
                    team: Team::B,
                    mobile: MobileKind::Coilback,
                    spawn_x: 300 + distance,
                    facing: Facing::Left,
                    items: Loadout::default(),
                },
            ],
            order: vec![0, 1],
            wind: Wind::default(),
            sudden_death: None,
        }
    }

    /// Answers every owed roll with calm, harmless weather: Force beams at the map's far-left edge
    /// (no test shot goes there), and no wind.
    pub fn answer_rolls(game: &mut Game) {
        while let Some(roll) = game.pending_roll() {
            let event = match roll {
                Roll::Weather { .. } => Event::Weather(Forecast { kind: WeatherKind::Force, x: 1 }),
                Roll::Wind => Event::Wind(Wind::default()),
                Roll::Drop { .. } => Event::Drop(Drop { kind: DropKind::Calm, x: 1 }),
            };
            game.apply(event).expect("an owed roll is accepted");
        }
    }

    /// A duel with the opening forecasts in, ready for seat 0 to act.
    pub fn duel(distance: i32) -> Game {
        let mut game = Game::new(&duel_setup(distance), &NoAssets).expect("a valid duel");
        answer_rolls(&mut game);
        game
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::{
        testing::{answer_rolls, duel, duel_setup},
        *,
    };
    use crate::map::NoAssets;

    #[test]
    fn a_new_game_seats_everyone_on_the_ground_and_starts_with_the_rolled_order() {
        let mut setup = duel_setup(800);
        setup.order = vec![1, 0];
        let game = Game::new(&setup, &NoAssets).unwrap();
        assert_eq!(game.phase(), Phase::Turn { seat: 1 });
        assert!(game.players().iter().all(|p| p.footing.y == 500 && p.alive && p.hp == 1000));
        assert_eq!(game.players()[1].moves_left, 90);
    }

    #[test]
    fn rejects_bad_setups() {
        let mut setup = duel_setup(800);
        setup.order = vec![0, 0];
        assert!(Game::new(&setup, &NoAssets).is_err());
        let mut setup = duel_setup(800);
        setup.seats[1].team = Team::A;
        assert!(Game::new(&setup, &NoAssets).is_err(), "one team");
        let mut setup = duel_setup(800);
        setup.seats[1].spawn_x = 5000;
        assert!(Game::new(&setup, &NoAssets).is_err(), "off the map");
    }

    #[test]
    fn the_server_owes_five_forecasts_before_anyone_moves() {
        let mut game = Game::new(&duel_setup(800), &NoAssets).unwrap();
        assert_eq!(game.pending_roll(), Some(Roll::Weather { width: 2400 }));
        assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_err(), "no moves while a roll is owed");
        assert!(game.apply(Event::Wind(Wind::default())).is_err(), "no wind is owed");
        for _ in 0..5 {
            game.apply(Event::Weather(Forecast { kind: WeatherKind::Mirror, x: 100 })).unwrap();
        }
        assert_eq!(game.pending_roll(), None);
        assert!(game.apply(Event::Weather(Forecast { kind: WeatherKind::Mirror, x: 100 })).is_err(), "no sixth");
        assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_ok());
    }

    #[test]
    fn weather_comes_up_every_round_and_the_wind_weather_owes_wind() {
        let mut game = Game::new(&duel_setup(800), &NoAssets).unwrap();
        for kind in [WeatherKind::Tornado, WeatherKind::Wind, WeatherKind::Force, WeatherKind::Force, WeatherKind::Force] {
            game.apply(Event::Weather(Forecast { kind, x: 900 })).unwrap();
        }
        // A round is two turns in a duel.
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        let effects = game.apply(Event::Pass { seat: 1, time: Centis(0) }).unwrap();
        assert!(
            effects.contains(&Effect::WeatherChanged {
                columns: vec![Column { kind: WeatherKind::Tornado, x: 900 }],
                wind_owed: false
            }),
            "{effects:?}"
        );
        assert_eq!(game.pending_roll(), Some(Roll::Weather { width: 2400 }), "the queue needs refilling");
        game.apply(Event::Weather(Forecast { kind: WeatherKind::Force, x: 5 })).unwrap();
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        game.apply(Event::Pass { seat: 1, time: Centis(0) }).unwrap();
        assert_eq!(game.pending_roll(), Some(Roll::Wind), "wind before the forecast");
        game.apply(Event::Wind(Wind::new(12, 45).unwrap())).unwrap();
        assert_eq!(game.wind(), Wind::new(12, 45).unwrap());
        assert!(game.weather().columns.is_empty(), "wind clears the columns");
    }

    #[test]
    fn only_the_active_player_may_act_and_only_in_range() {
        let mut game = duel(800);
        let before = game.clone();
        assert!(game.apply(Event::Pass { seat: 1, time: Centis(0) }).is_err());
        assert!(
            game.apply(Event::Shoot {
                seat: 0,
                shot: Shot::S1,
                aim: 80,
                power: Power::MAX,
                time: Centis(0)
            })
            .is_err()
        );
        assert_eq!(game, before, "a rejected event changes nothing");
    }

    #[test]
    fn passing_charges_delay_and_hands_over_the_turn() {
        let mut game = duel(800);
        let effects = game.apply(Event::Pass { seat: 0, time: Centis(500) }).unwrap();
        assert_eq!(effects, [Effect::TurnStarted { seat: 1 }]);
        assert_eq!(game.players()[0].clock.delay, 720 + 50);
    }

    #[test]
    fn forfeiting_kills_and_hands_over_or_ends_the_game() {
        let mut game = duel(800);
        let effects = game.apply(Event::Forfeit { seat: 1 }).unwrap();
        assert_eq!(effects, [Effect::Died { seat: 1, death: Death::Forfeited }, Effect::GameOver { winner: Some(Team::A) }]);

        let mut setup = duel_setup(800);
        setup.seats.push(Seat {
            team: Team::A,
            mobile: MobileKind::Coilback,
            spawn_x: 700,
            facing: Facing::Right,
            items: Loadout::default(),
        });
        setup.order = vec![0, 1, 2];
        let mut game = Game::new(&setup, &NoAssets).unwrap();
        answer_rolls(&mut game);
        let effects = game.apply(Event::Forfeit { seat: 0 }).unwrap();
        assert!(effects.contains(&Effect::TurnStarted { seat: 1 }), "the forfeiting player's turn passes: {effects:?}");
        assert!(game.apply(Event::Forfeit { seat: 0 }).is_err(), "already out");
    }

    #[test]
    fn walking_spends_the_budget_and_stops_at_it() {
        let mut game = duel(800);
        let effects = game
            .apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 500,
            })
            .unwrap();
        assert_eq!(
            effects,
            [Effect::Walked {
                seat: 0,
                steps: 90,
                to: Footing { x: 390, y: 500 }
            }]
        );
        assert_eq!(game.players()[0].moves_left, 0);
    }

    #[test]
    fn walking_off_the_map_is_a_bunge_and_ends_the_turn() {
        let mut game = duel(800);
        // Dig a one-pixel shaft right in front of seat 0, so its next step falls through.
        for y in 400..600 {
            game.terrain.carve(Vec2::new(301.5, f64::from(y) + 0.5), 0.6);
        }
        let effects = game.apply(Event::Walk { seat: 0, facing: Facing::Right, steps: 5 }).unwrap();
        assert!(effects.contains(&Effect::Died { seat: 0, death: Death::Bunged }), "{effects:?}");
        assert_eq!(game.phase(), Phase::Over { winner: Some(Team::B) });
    }

    #[test]
    fn the_state_hash_tracks_the_state() {
        let a = duel(800);
        let mut b = a.clone();
        assert_eq!(a.state_hash().unwrap(), b.state_hash().unwrap());
        b.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        assert_ne!(a.state_hash().unwrap(), b.state_hash().unwrap());
    }

    fn with_column(kind: WeatherKind, x: i32) -> Game {
        let mut game = duel(800);
        game.weather.columns = vec![Column { kind, x }];
        game
    }

    #[test]
    fn a_mirror_sends_the_shot_back() {
        let game = with_column(WeatherKind::Mirror, 600);
        let flight = game.launch(0, 45, 2.4).unwrap();
        let track = game.track(flight, 0, 0);
        assert_eq!(track.legs.len(), 2);
        assert!(track.legs[1].flight.velocity.x < 0.0 && track.impact.at.x < 600.0, "{track:?}");
    }

    #[test]
    fn a_tornado_holds_the_shot_and_flings_it_up() {
        let game = with_column(WeatherKind::Tornado, 600);
        let flight = game.launch(0, 45, 2.4).unwrap();
        let track = game.track(flight, 0, 0);
        assert_eq!(track.legs.len(), 2);
        let entry = track.legs[0].to;
        assert_eq!(track.legs[1].offset, entry + TICKS_PER_SECOND, "held one second");
        assert!(track.legs[1].flight.velocity.y < 0.0 && track.legs[1].flight.velocity.x > 0.0, "{track:?}");
    }

    #[test]
    fn force_marks_the_shot_without_turning_it() {
        let game = with_column(WeatherKind::Force, 600);
        let flight = game.launch(0, 45, 2.4).unwrap();
        let calm = duel(800).track(flight, 0, 0);
        let forced = game.track(flight, 0, 0);
        assert!(forced.forced && !calm.forced);
        assert_eq!(forced.impact, calm.impact, "the path is unchanged");
    }
}
