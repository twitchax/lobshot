//! A game: its state, the events that change it, and the effects each event has. The server applies
//! accepted events authoritatively; every client and the lab apply the same events to the same
//! state, so the state hash after each event must match everywhere.
//!
//! The sim never rolls dice. When the rules need something random (a weather forecast, new wind,
//! a slot machine's drop), `pending_roll` says so, the server rolls it and applies it as an event,
//! and until then players can't act. Every log is therefore canonical: the same rolls at the same
//! points.
//!
//! What each rule does lives beside it: shots in `shots`, items in `items`, the slot machine in
//! `drops`. This module holds the state, decides which events are allowed, and runs the turn.

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    ballistics::{self, Flight, Spin},
    base::{Res, Void},
    body::{HitFrame, Pose},
    damage::{self, Blast, Target},
    drops::{self, Drop, Falling},
    items::{self, Item, Loadout, MAX_ITEMS},
    map::{MapAssets, MapRef},
    movement::{self, Footing, Landing, Step},
    shots::{self, Salvo},
    terrain::{DirtyRect, Terrain},
    tuning::{
        ArmorClass, BALM_BIONIC_FACTOR, BALM_HEAL_PERCENT, LOCK_SS_RECHARGE, MAX_FLIGHT_SECONDS, MobileTuning, SS_LOCK_TURNS, ShotTuning, TORNADO_HOLD_SECONDS, TURN_TIME_LIMIT, coilback,
        drops::FALL_TURNS, drops::RECHARGE_ROUNDS, frostfin, herdhound, hookclaw, kitewyrm, lanternwing, marblemole, sparkhorn, spoutpot, sudsotter, thistleback, tuskrack,
    },
    turn::{self, Clock, Ending},
    units::{Centis, Facing, Power, TICKS_PER_SECOND, Vec2, Wind, ticks},
    weather::{Column, Forecast, Marks, Odds, Skyeye, Weather, WeatherKind},
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
    Spoutpot,
    Tuskrack,
    Sudsotter,
    Frostfin,
    Herdhound,
    Thistleback,
    Kitewyrm,
}

impl MobileKind {
    /// Every mobile, in a fixed order.
    pub const ALL: [Self; 12] = [
        Self::Coilback,
        Self::Lanternwing,
        Self::Hookclaw,
        Self::Sparkhorn,
        Self::Marblemole,
        Self::Spoutpot,
        Self::Tuskrack,
        Self::Sudsotter,
        Self::Frostfin,
        Self::Herdhound,
        Self::Thistleback,
        Self::Kitewyrm,
    ];

    /// The room any mobile needs at a spawn column, in whole pixels: how far its hitbox reaches to
    /// either side of its feet, whichever way it faces, and how tall it stands. The widest and
    /// tallest of every mobile, so a map's spawns fit whatever each player picks.
    #[must_use]
    pub fn spawn_room() -> (i32, i32) {
        let most = |pick: fn(&crate::tuning::Rect) -> f64| Self::ALL.iter().map(|mobile| pick(&mobile.tuning().hitbox.value)).fold(0.0, f64::max);
        let wide = most(|hitbox| (-hitbox.x).max(hitbox.x + hitbox.w));
        let tall = most(|hitbox| -hitbox.y);
        // Hitboxes are tens of pixels, far inside i32.
        #[allow(clippy::cast_possible_truncation)]
        let room = (wide.ceil() as i32, tall.ceil() as i32);
        room
    }

    #[must_use]
    pub fn tuning(self) -> &'static MobileTuning {
        match self {
            Self::Coilback => &coilback::MOBILE,
            Self::Lanternwing => &lanternwing::MOBILE,
            Self::Hookclaw => &hookclaw::MOBILE,
            Self::Sparkhorn => &sparkhorn::MOBILE,
            Self::Marblemole => &marblemole::MOBILE,
            Self::Spoutpot => &spoutpot::MOBILE,
            Self::Tuskrack => &tuskrack::MOBILE,
            Self::Sudsotter => &sudsotter::MOBILE,
            Self::Frostfin => &frostfin::MOBILE,
            Self::Herdhound => &herdhound::MOBILE,
            Self::Thistleback => &thistleback::MOBILE,
            Self::Kitewyrm => &kitewyrm::MOBILE,
        }
    }

    /// Whether it floats over the ground (Kitewyrm's, `tuning::kitewyrm::FLOATS`): what walks,
    /// rolls or burrows along the ground passes under it and can't strike it, and a mine has nothing
    /// of it to go for. Shots that travel on the ground sense mobiles through `Game::grounded_frames`.
    #[must_use]
    pub fn floats(self) -> bool {
        self == Self::Kitewyrm && kitewyrm::FLOATS.value
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

    /// The other of S1 and S2 (Dual+'s second shot); SS has no other.
    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::S1 => Self::S2,
            Self::S2 => Self::S1,
            Self::Ss => Self::Ss,
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

impl SuddenDeathKind {
    /// Every kind, in the order a room's options list them.
    pub const ALL: [Self; 3] = [Self::BigBomb, Self::Double, Self::SsDeath];

    /// The player-facing name: GunBound's room options' own.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BigBomb => "Big Bomb",
            Self::Double => "Double",
            Self::SsDeath => "SS Death",
        }
    }
}

/// Whether a seat may fire SS, and if not, why (T-003; `Game::ss_state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SsState {
    Ready,
    /// Locked for this many more of the seat's own turns.
    Locked(u8),
    /// Off for the rest of the game: Big Bomb and Double sudden deaths take SS away.
    Off(SuddenDeathKind),
    /// Not with this item armed: Dual and Dual+ fire S1 or S2.
    Armed(Item),
}

/// Sudden death: once `turn` turns are done, `kind` applies and items are off for good.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SuddenDeath {
    pub kind: SuddenDeathKind,
    pub turn: u32,
}

/// Checks spawn columns sit far enough apart for two mobiles of any kind side by side, facing
/// either way (`MobileKind::spawn_room`).
///
/// # Errors
///
/// Names the first pair that is too close.
pub fn check_spawn_spacing(spawns: &[i32]) -> Void {
    let (wide, _) = MobileKind::spawn_room();
    let mut sorted = spawns.to_vec();
    sorted.sort_unstable();
    for pair in sorted.windows(2) {
        ensure!(pair[1] - pair[0] >= 2 * wide, "spawns {} and {} are closer than two mobiles", pair[0], pair[1]);
    }
    Ok(())
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
    /// is immediate (a heal, the wind) or rides on the turn's shot (Dual, Power Up). `time` is the
    /// turn time so far, as the server measured it: an item that ends the turn (Swap) is charged it.
    UseItem { seat: u8, slot: u8, time: Centis },
    /// A dead player spins their slot machine; the server then owes `Roll::Drop`.
    Spin { seat: u8 },
    /// The server rolled what the slot machine dropped (`Roll::Drop`).
    Drop(Drop),
}

/// Something random the server owes the game before anyone may act.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Roll {
    /// A new forecast for the back of the weather queue: a `WeatherKind` by the map's `odds`, and a
    /// column x in `0..width`.
    Weather { width: u32, odds: Odds },
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

/// One stretch of a projectile's flight: weather may break a flight into several, and a bouncing
/// ball starts a new one at each bounce.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Leg {
    pub flight: Flight,
    /// The leg's own ticks this stretch covers.
    pub from: u32,
    pub to: u32,
    /// The stage's elapsed ticks when the leg's own tick 0 happened (weather holds add to it).
    pub offset: u32,
}

impl Leg {
    /// Whether the wind turned the projectile around along this leg: its sideways motion at the
    /// end is the other way from the start (a boomer shot or a backshot; a mirror is a new leg).
    #[must_use]
    pub fn hooked(&self) -> bool {
        let (start, end) = (self.flight.velocity_at(self.from).x, self.flight.velocity_at(self.to).x);
        (start > 0.0 && end < 0.0) || (start < 0.0 && end > 0.0)
    }
}

/// One strand of a helix (`Effect::Helix`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Strand {
    /// Its phase about the path, radians (`ballistics::helix_point`).
    pub phase: f64,
    /// The stage tick it stopped at: where it struck, or left the map.
    pub to: u32,
}

/// One projectile of an orbit (`Effect::Orbit`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Orbiter {
    /// How far from the path it circles, px; 0 flies the path itself (`ballistics::orbit_point`).
    pub radius: f64,
    /// Where on its circle it starts, radians clockwise from `+x` on screen.
    pub phase: f64,
    /// The stage tick it stopped at: where it struck, or left the map.
    pub to: u32,
}

/// What an event did, in order, for the client to show.
///
/// **Stages.** An event's effects play as one or more stages. `Volley` and `Dropped` each start
/// one, once everything before it has played out (a Dual's second shot, a drop at a turn's end);
/// every `tick` in an effect counts from the start of its stage. `lobshot_protocol::pacing` turns
/// stages and ticks into seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Walked {
        seat: u8,
        steps: u16,
        to: Footing,
    },
    /// One projectile's whole path, leg by leg. A volley of several projectiles (Hookclaw's and
    /// Marblemole's S2) has one `Fired` each. A bouncing marble's legs run bounce to bounce, and a
    /// marble at rest has a leg with no velocity or acceleration. A projectile born in flight
    /// (Spoutpot's SS balls, after the blob's own `Fired`) or fired late (Tuskrack's rockets,
    /// rippling off the rack) starts at its first leg's offset. A walking blob (Sudsotter's) has a
    /// leg with no acceleration for each straight run of its walk, and a resting one at its end.
    Fired {
        seat: u8,
        legs: Vec<Leg>,
    },
    /// A blast at `tick`; `radius` is the crater's (Dig and Big Bomb widen it).
    Exploded {
        at: Vec2,
        radius: f64,
        tick: u32,
        dirty: Option<DirtyRect>,
    },
    /// `seat` lost `amount` HP, after its shield soaked what it could (`Shielded`, just before).
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
        /// The weather now (`Weather::now`).
        now: Option<WeatherKind>,
    },
    /// The turn passes to `seat`. Its shield regenerating (`ShieldRestored`) and its SS coming
    /// ready (`SsReady`) follow, as the turn starts.
    TurnStarted {
        seat: u8,
    },
    GameOver {
        winner: Option<Team>,
    },
    /// `seat` fires `shot`, starting a stage: the volley's projectiles (`Fired`, `Helix`, `Orbit`,
    /// `Beacon`) follow, then what they do in tick order. A Dual, Dual+ or Double turn has two
    /// volleys, the second from wherever the shooter stands once the first has settled.
    Volley {
        seat: u8,
        shot: Shot,
    },
    /// Lanternwing's and Spoutpot's S2: strands winding about one path, each a projectile of its
    /// own. `legs` is the path (the strands' center, which touches nothing itself); strand `i` is
    /// at `ballistics::helix_point(leg.flight, t, leg.offset, amplitude, hertz, strands[i].phase,
    /// merge)` for each leg tick `t` until its `to`.
    Helix {
        seat: u8,
        legs: Vec<Leg>,
        amplitude: f64,
        hertz: f64,
        strands: Vec<Strand>,
        /// The stage tick from which the strands run together on the path, if they ever do.
        merge: Option<u32>,
    },
    /// Thistleback's S2: projectiles circling one path in the screen's plane, each striking on its
    /// own. `legs` is the path, which touches nothing itself; orbiter `i` is at
    /// `ballistics::orbit_point(leg.flight, t, leg.offset, spin, orbiters[i].radius,
    /// orbiters[i].phase)` for each leg tick `t` until its `to`.
    Orbit {
        seat: u8,
        legs: Vec<Leg>,
        spin: Spin,
        orbiters: Vec<Orbiter>,
    },
    /// The Teleport item's beacon: one path like `Fired`, but it hurts and carves nothing. If it
    /// lands, `Teleported` follows.
    Beacon {
        seat: u8,
        legs: Vec<Leg>,
    },
    /// `seat` reappears at `to` at `tick`: where its beacon landed, settled onto the ground.
    Teleported {
        seat: u8,
        to: Footing,
        tick: u32,
    },
    /// A bolt of lightning from `from` (in the sky, or off a side of the map) to `to`, where it
    /// struck a mobile or the ground, at `tick`. Its crater (`Exploded`, if it struck the ground)
    /// and the damage it deals follow. Sparkhorn's shots, the Thunder item and the slot machine's
    /// bolt.
    Bolt {
        from: Vec2,
        to: Vec2,
        tick: u32,
    },
    /// A marble bounced at `at` (the ground, or a side of the map) at `tick`.
    Bounced {
        at: Vec2,
        tick: u32,
    },
    /// Marblemole's SS passed through `seat` from tick `from` to `to`, or Sudsotter's suds stayed
    /// within reach of it, hurting it all the while; its damage follows.
    Pierced {
        seat: u8,
        from: u32,
        to: u32,
    },
    /// `seat`'s shield soaked `amount` of a hit. Any damage past it follows as `Damaged`.
    Shielded {
        seat: u8,
        amount: i32,
    },
    /// Lanternwing's SS stripped `seat`'s shield of its last `amount` at `tick`, just before its
    /// blast.
    ShieldStripped {
        seat: u8,
        amount: i32,
        tick: u32,
    },
    /// `seat`'s shield regenerated `amount` as its turn started.
    ShieldRestored {
        seat: u8,
        amount: i32,
    },
    /// `seat` gained `amount` HP: a heal item, or the slot machine's chest landing on it.
    Healed {
        seat: u8,
        amount: i32,
    },
    /// `seat` used `item` from loadout position `slot`. Its immediate effects follow (`Healed`,
    /// `WindChanged`, `Damaged` for Sacrifice's cost, `Swapped`); the rest ride on the turn's shot.
    ItemUsed {
        seat: u8,
        slot: u8,
        item: Item,
    },
    /// Swap: `seat` and its teammate `with` traded places. The turn ends.
    Swapped {
        seat: u8,
        with: u8,
    },
    /// `seat` fired its SS: it is locked until the start of its `turns`-th turn from now.
    SsLocked {
        seat: u8,
        turns: u8,
    },
    /// `seat`'s SS came off its lock as its turn started.
    SsReady {
        seat: u8,
    },
    /// Dead `seat` spun its slot machine. Instant; the server's roll (`DropRolled`) comes next.
    Spun {
        seat: u8,
    },
    /// `seat`'s slot machine stopped on `drop`. It falls down column `drop.x` at the end of a later
    /// turn (`Game::drops` lists those still to come).
    DropRolled {
        seat: u8,
        drop: Drop,
    },
    /// `seat`'s drop falls, starting a stage: `legs` is its fall from the sky (empty for a bolt,
    /// which strikes at once, `Bolt` next). What it does where it lands follows: a blast, a heal,
    /// the wind.
    Dropped {
        seat: u8,
        drop: Drop,
        legs: Vec<Leg>,
    },
    /// Skyeye's beam ran from the satellite (`from`) to what it struck (`to`) at `tick`, at
    /// `level`; its blast's `Exploded` and damage follow.
    Beam {
        from: Vec2,
        to: Vec2,
        tick: u32,
        level: u8,
    },
    /// Skyeye reached `level`.
    SkyeyeLevel {
        level: u8,
    },
    /// A projectile passed through the Random column at `x`: it shows `kind`, what it hid, from now
    /// on.
    Revealed {
        kind: WeatherKind,
        x: i32,
    },
    /// Sudden death began: `kind` applies from the next turn on, and items are off for good.
    SuddenDeath {
        kind: SuddenDeathKind,
    },
    /// Frostfin's ice left `seat` brittle, just after hurting it: Physical damage against it is now
    /// `brittle` percent stronger, for the rest of the game (`Player::brittle`).
    Frosted {
        seat: u8,
        brittle: u8,
    },
    /// Herdhound's guide: a marker that flies on along the shot's path from where its warhead
    /// opened, hurting and carving nothing. `legs` start at that tick; the missiles (each a
    /// `Fired` from the same tick) home on it.
    Guide {
        seat: u8,
        legs: Vec<Leg>,
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
    /// What the columns it passed through did to it.
    pub marks: Marks,
    /// The last leg and the leg's own tick at impact, to carry on from (Coilback's S2).
    pub last: Leg,
    pub last_tick: u32,
}

/// A projectile followed up to a stage tick (`Game::track_until`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Course {
    pub track: Track,
    /// It reached the bound still flying: `track.impact` is where it was then (contact `Gone`),
    /// and it carries on from `track.last` at its own tick `track.last_tick`.
    pub flying: bool,
}

/// What a projectile stops at, beyond leaving the map. Weather columns always act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Senses {
    /// Living mobiles (Marblemole's SS and Sudsotter's sponges pass through them).
    pub mobiles: bool,
    /// Solid ground (a helix's center passes through it; only its strands collide).
    pub terrain: bool,
}

impl Senses {
    pub const ALL: Self = Self { mobiles: true, terrain: true };
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
    /// The game's turn count (`Game::turns`) when this seat last spun its slot machine.
    pub last_spin: Option<i64>,
    /// How much more Physical damage this mobile takes, in percent: Frostfin's S2 and SS leave it
    /// brittle, for the rest of the game.
    pub brittle: u8,
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

    /// The hitbox's center in the world.
    #[must_use]
    pub fn center(&self) -> Vec2 {
        self.pose().center(self.tuning().hitbox.value)
    }

    /// This mobile as the damage model sees it.
    #[must_use]
    pub fn target(&self) -> Target {
        Target {
            pose: self.pose(),
            hitbox: self.tuning().hitbox.value,
            armor: self.tuning().armor.value,
            brittle: self.brittle,
        }
    }
}

/// Whose turn it is, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Turn { seat: u8 },
    Over { winner: Option<Team> },
}

/// The whole game state. Fields the rule modules act on (`shots`, `items`, `drops`) are visible to
/// the crate; everything else reads it through the accessors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Game {
    pub(crate) terrain: Terrain,
    pub(crate) players: Vec<Player>,
    pub(crate) wind: Wind,
    weather: Weather,
    /// How often each weather comes up on this map.
    odds: Odds,
    /// Turns per weather: one per seat (SW-weather).
    round: u32,
    /// Turns completed so far.
    turns: i64,
    phase: Phase,
    /// As set up; it is on once `turns` reaches its turn.
    sudden_death: Option<SuddenDeath>,
    /// Drops on their way down, in the order they were rolled.
    pub(crate) drops: Vec<Falling>,
    /// The seat whose spin the server owes a `Roll::Drop` for.
    spin_owed: Option<u8>,
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
                last_spin: None,
                brittle: 0,
            });
        }

        let round = u32::try_from(seats).unwrap_or(1);
        let mut game = Self {
            terrain,
            players,
            wind: setup.wind,
            weather: Weather::new(round),
            odds: crate::tuning::weather::odds(&setup.map),
            round,
            turns: 0,
            phase: Phase::Over { winner: None },
            sudden_death: setup.sudden_death,
            drops: Vec::new(),
            spin_owed: None,
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

    /// Turns per round: one per seat. Weather comes up once a round, and a dead seat may spin once
    /// a round.
    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    /// The sudden death in force, once it has started.
    #[must_use]
    pub fn sudden_death(&self) -> Option<SuddenDeathKind> {
        self.sudden_death.filter(|sd| self.turns >= i64::from(sd.turn)).map(|sd| sd.kind)
    }

    /// Drops on their way down, in the order they were rolled, each with the turn it lands at.
    #[must_use]
    pub fn drops(&self) -> &[Falling] {
        &self.drops
    }

    /// What the server must roll before anyone may act, if anything: wind first, then forecasts,
    /// then a spun slot machine's drop.
    #[must_use]
    pub fn pending_roll(&self) -> Option<Roll> {
        if matches!(self.phase, Phase::Over { .. }) {
            return None;
        }
        if self.weather.wind_owed {
            Some(Roll::Wind)
        } else if self.weather.short() {
            Some(Roll::Weather {
                width: self.terrain.width(),
                odds: self.odds,
            })
        } else if self.spin_owed.is_some() {
            Some(Roll::Drop { width: self.terrain.width() })
        } else {
            None
        }
    }

    /// Whether `seat` may spin its slot machine now (`Event::Spin`), so the server knows when a
    /// dead bot can: it is dead, the game is on, no roll is owed, and a round has passed since its
    /// last spin.
    #[must_use]
    pub fn can_spin(&self, seat: u8) -> bool {
        self.check_spin(seat).is_ok()
    }

    /// Whether `seat` may fire `shot` now: it is its turn, and an SS is ready (`ss_state`). The aim
    /// is checked when it fires.
    #[must_use]
    pub fn can_shoot(&self, seat: u8, shot: Shot) -> bool {
        self.ensure_turn(seat).is_ok() && self.check_shot(seat, shot).is_ok()
    }

    /// Whether `seat` may fire SS, on its turn or not (`can_shoot` adds the turn): locked for
    /// `SS_LOCK_TURNS` own turns after each, off in Big Bomb and Double, free every turn in SS
    /// Death, and never with Dual or Dual+ armed.
    ///
    /// # Errors
    ///
    /// Fails when there is no such seat.
    pub fn ss_state(&self, seat: u8) -> Res<SsState> {
        let player = self.player(seat)?;
        Ok(match (self.sudden_death(), player.armed) {
            (Some(SuddenDeathKind::SsDeath), _) => SsState::Ready,
            (Some(kind), _) => SsState::Off(kind),
            (None, _) if player.ss_lock > 0 => SsState::Locked(player.ss_lock),
            (None, Some(item @ (Item::Dual | Item::DualPlus))) => SsState::Armed(item),
            (None, _) => SsState::Ready,
        })
    }

    /// Whether `seat` may use the item at `slot` now (`Event::UseItem`).
    #[must_use]
    pub fn can_use_item(&self, seat: u8, slot: u8) -> bool {
        self.check_item(seat, slot).is_ok()
    }

    pub(crate) fn player(&self, seat: u8) -> Res<&Player> {
        self.players.get(usize::from(seat)).ok_or_else(|| anyhow::anyhow!("there is no seat {seat}"))
    }

    /// Applies one event and returns its effects.
    ///
    /// # Errors
    ///
    /// Rejects an event that the rules don't allow now: the wrong player, a finished game, a roll
    /// that isn't owed, a move while one is, an aim outside the shot's range, a locked or disabled
    /// SS, an item that can't be used, or a spin from a living or recharging seat. A rejected
    /// event changes nothing.
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
                ensure!(self.on_map(forecast.x), "forecast x {} is off the map", forecast.x);
                ensure!(forecast.is_whole(), "a Random forecast, and only one, hides a column weather: {forecast:?}");
                self.weather.queue.push(forecast);
                effects.push(Effect::Forecast(forecast));
            }
            Event::Drop(drop) => {
                ensure!(matches!(self.pending_roll(), Some(Roll::Drop { .. })), "no drop is owed");
                ensure!(self.on_map(drop.x), "drop x {} is off the map", drop.x);
                let Some(seat) = self.spin_owed.take() else { bail!("no drop is owed") };
                self.drops.push(Falling {
                    seat,
                    drop,
                    lands: self.turns + FALL_TURNS.value,
                });
                effects.push(Effect::DropRolled { seat, drop });
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
                self.check_shot(seat, shot)?;
                let fired = effects.len();
                self.shoot(seat, shot, aim, power, &mut effects);
                self.reveal_passed(&mut effects, fired);
                let delay = shot.tuning(self.player(seat)?.tuning()).delay.value;
                self.end_turn(seat, Ending::Shot(delay), time, &mut effects);
            }
            Event::Pass { seat, time } => {
                self.ensure_turn(seat)?;
                self.end_turn(seat, Ending::Pass, time, &mut effects);
            }
            Event::UseItem { seat, slot, time } => {
                self.check_item(seat, slot)?;
                if items::use_item(self, seat, slot, &mut effects) {
                    self.end_turn(seat, Ending::Item, time, &mut effects);
                }
            }
            Event::Spin { seat } => {
                self.check_spin(seat)?;
                self.players[usize::from(seat)].last_spin = Some(self.turns);
                self.spin_owed = Some(seat);
                effects.push(Effect::Spun { seat });
            }
        }
        Ok(effects)
    }

    fn on_map(&self, x: i32) -> bool {
        u32::try_from(x).is_ok_and(|x| x < self.terrain.width())
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

    /// SS gating (T-003), by `ss_state`.
    fn check_shot(&self, seat: u8, shot: Shot) -> Void {
        if shot != Shot::Ss {
            return Ok(());
        }
        match self.ss_state(seat)? {
            SsState::Ready => Ok(()),
            SsState::Locked(turns) => bail!("seat {seat}'s SS is locked for {turns} more of its turns"),
            SsState::Off(kind) => bail!("SS is off in {kind:?} sudden death"),
            SsState::Armed(item) => bail!("{:?} fires S1 or S2, not SS", Some(item)),
        }
    }

    /// An item may be used on its owner's turn, before sudden death, one a turn; Swap needs a
    /// living teammate.
    fn check_item(&self, seat: u8, slot: u8) -> Void {
        self.ensure_turn(seat)?;
        ensure!(self.sudden_death().is_none(), "items are off in sudden death");
        ensure!(!self.weather.is(WeatherKind::Lock), "the Lock weather has locked every item");
        let player = self.player(seat)?;
        ensure!(player.armed.is_none(), "seat {seat} has used an item this turn");
        let Some(item) = player.items.get(usize::from(slot)).copied().flatten() else {
            bail!("seat {seat} has no item at slot {slot}");
        };
        ensure!(item != Item::Swap || self.swap_partner(seat).is_some(), "Swap needs a living teammate");
        Ok(())
    }

    fn check_spin(&self, seat: u8) -> Void {
        ensure!(!matches!(self.phase, Phase::Over { .. }), "the game is over");
        if let Some(roll) = self.pending_roll() {
            bail!("waiting for the server's {roll:?} roll");
        }
        let player = self.player(seat)?;
        ensure!(!player.alive, "seat {seat} is alive: only the dead spin");
        let recharge = i64::from(self.round) * RECHARGE_ROUNDS.value;
        if let Some(last) = player.last_spin {
            ensure!(self.turns - last >= recharge, "seat {seat}'s slot machine recharges for {} more turns", recharge - (self.turns - last));
        }
        Ok(())
    }

    /// The living teammate Swap trades places with: the lowest HP, then the lowest seat.
    pub(crate) fn swap_partner(&self, seat: u8) -> Option<u8> {
        let team = self.players.get(usize::from(seat))?.team;
        (0_u8..)
            .zip(&self.players)
            .filter(|&(other, p)| other != seat && p.alive && p.team == team)
            .min_by_key(|&(other, p)| (p.hp, other))
            .map(|(other, _)| other)
    }

    fn walk(&mut self, seat: u8, facing: Facing, steps: u16, effects: &mut Vec<Effect>) {
        let index = usize::from(seat);
        self.players[index].facing = facing;
        let mut taken = 0;
        let mut died = false;
        while taken < steps && self.players[index].moves_left > 0 {
            let climb = self.players[index].tuning().climb_px.value;
            match movement::step(&self.terrain, self.players[index].footing, facing, climb) {
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

    /// Fires the turn's volleys: the shot, then again for Dual or Double, or the other of S1 and
    /// S2 for Dual+. The second fires at the same aim (clamped to its own range) and power from
    /// wherever the shooter stands once the first has settled, if it still stands and so do both
    /// teams. An SS then locks.
    fn shoot(&mut self, seat: u8, shot: Shot, aim: i16, power: Power, effects: &mut Vec<Effect>) {
        let player = self.players[usize::from(seat)];
        let sudden_death = self.sudden_death();
        let second = match (player.armed, sudden_death) {
            _ if shot == Shot::Ss => None,
            (Some(Item::Dual), _) | (_, Some(SuddenDeathKind::Double)) => Some(shot),
            (Some(Item::DualPlus), _) => Some(shot.other()),
            _ => None,
        };
        let mut salvo = Salvo::new(seat, player.armed, sudden_death);
        shots::fire(self, &mut salvo, shot, aim, power, effects);
        self.settle_all(effects);
        if let Some(second) = second
            && self.players[usize::from(seat)].alive
            && !self.decided()
        {
            let range = second.tuning(player.tuning()).aim.value;
            shots::fire(self, &mut salvo, second, aim.clamp(range.min, range.max), power, effects);
            self.settle_all(effects);
        }
        if shot == Shot::Ss && sudden_death != Some(SuddenDeathKind::SsDeath) {
            // One more than the locked turns: the lock counts down at the start of each own turn and
            // lifts when it reaches 0.
            let turns = SS_LOCK_TURNS.value + 1;
            self.players[usize::from(seat)].ss_lock = turns;
            effects.push(Effect::SsLocked { seat, turns });
        }
    }

    /// The flight of `shot` from `seat` at `aim` degrees and `bars` of power (continuous, so bots
    /// and tests can search between power steps). It leaves the shot's own barrel
    /// (`MobileTuning::barrel`).
    #[must_use]
    pub fn launch(&self, seat: u8, shot: Shot, aim: i16, bars: f64) -> Option<Flight> {
        let player = self.players.get(usize::from(seat))?;
        let tuning = player.tuning();
        let pose = player.pose();
        Some(Flight {
            origin: pose.muzzle(tuning, shot, aim),
            velocity: pose.aim_direction(aim) * ballistics::launch_speed(bars),
            accel: ballistics::acceleration(tuning.gravity.value, self.wind, tuning.wind_sensitivity.value),
        })
    }

    /// Flies `flight` from `from_tick` until it touches a living mobile or solid ground, enters a
    /// weather column, or leaves the map. Mobiles are checked before terrain, so a shot grazing a
    /// mobile's feet is direct.
    #[must_use]
    pub fn fly(&self, flight: &Flight, from_tick: u32) -> Impact {
        self.fly_sensing(flight, from_tick, from_tick == 0, Senses::ALL)
    }

    /// `fly`, stopping only at what `senses` names. A `launched` flight is a projectile at its
    /// first tick, which no column has touched yet: one it starts inside catches it at once, as
    /// OB's weathers act on any projectile they haven't touched. Otherwise only entering a column
    /// counts, so a fresh leg out of one, or a shot flying on, isn't redirected again at once.
    pub(crate) fn fly_sensing(&self, flight: &Flight, from_tick: u32, launched: bool, senses: Senses) -> Impact {
        let last = MAX_FLIGHT_SECONDS.value * TICKS_PER_SECOND;
        self.fly_through(flight, from_tick, launched, senses, last).unwrap_or(Impact {
            tick: last,
            at: flight.at(last),
            contact: Contact::Gone,
        })
    }

    /// `fly_sensing` up to tick `last`: `None` if the flight touched nothing by then.
    fn fly_through(&self, flight: &Flight, from_tick: u32, launched: bool, senses: Senses, last: u32) -> Option<Impact> {
        let width = f64::from(self.terrain.width());
        let height = f64::from(self.terrain.height());
        let targets = if senses.mobiles { self.hit_frames() } else { Vec::new() };
        let columns = &self.weather.columns;
        let mut inside: Vec<bool> = columns.iter().map(|c| !launched && c.contains(flight.at(from_tick).x)).collect();

        for tick in from_tick..=last {
            let at = flight.at(tick);
            if at.x < 0.0 || at.x >= width || at.y >= height {
                return Some(Impact { tick, at, contact: Contact::Gone });
            }
            if let Some(&(seat, _)) = targets.iter().find(|(_, frame)| frame.contains(at)) {
                return Some(Impact { tick, at, contact: Contact::Mobile(seat) });
            }
            // In range: x is inside the map, and y is below it or above the bottom.
            #[allow(clippy::cast_possible_truncation)]
            if senses.terrain && self.terrain.is_solid(at.x.floor() as i32, at.y.floor() as i32) {
                return Some(Impact { tick, at, contact: Contact::Terrain });
            }
            for (index, column) in columns.iter().enumerate() {
                let now = column.contains(at.x);
                if now && !inside[index] {
                    let index = u8::try_from(index).unwrap_or(u8::MAX);
                    return Some(Impact {
                        tick,
                        at,
                        contact: Contact::Column(index),
                    });
                }
                inside[index] = now;
            }
        }
        None
    }

    /// Every living mobile's hitbox, by seat, for collision loops.
    pub(crate) fn hit_frames(&self) -> Vec<(u8, HitFrame)> {
        self.frames_where(|_| true)
    }

    /// `hit_frames` for what travels along the ground: every living mobile's hitbox but a floating
    /// one's (`MobileKind::floats`).
    pub(crate) fn grounded_frames(&self) -> Vec<(u8, HitFrame)> {
        self.frames_where(|p| !p.mobile.floats())
    }

    fn frames_where(&self, keep: impl Fn(&Player) -> bool) -> Vec<(u8, HitFrame)> {
        (0_u8..)
            .zip(&self.players)
            .filter(|(_, p)| p.alive && keep(p))
            .map(|(seat, p)| (seat, HitFrame::new(&p.pose(), p.tuning().hitbox.value)))
            .collect()
    }

    /// Follows a projectile through the weather to what it hits. Force, Drain and Electricity mark
    /// it; Mirror sends it back (its sideways speed and wind reversed); Tornado holds it, then
    /// flings it back up.
    /// `from_tick` is in `flight`'s own ticks, and `offset` is the shot's elapsed ticks at
    /// `flight`'s tick 0; both 0 is a launch (see `fly_sensing`).
    #[must_use]
    pub fn track(&self, flight: Flight, from_tick: u32, offset: u32) -> Track {
        self.track_sensing(flight, from_tick, offset, Senses::ALL)
    }

    /// `track`, stopping only at what `senses` names.
    pub(crate) fn track_sensing(&self, flight: Flight, from_tick: u32, offset: u32, senses: Senses) -> Track {
        self.follow(flight, from_tick, offset, from_tick == 0 && offset == 0, senses, None).0
    }

    /// `track` for a projectile launched `offset` ticks into its stage (one of a ripple off a rack):
    /// a column it starts inside catches it at once, as one does any shot it starts inside.
    pub(crate) fn track_launched(&self, flight: Flight, offset: u32) -> Track {
        self.follow(flight, 0, offset, true, Senses::ALL, None).0
    }

    /// `track_sensing`, only as far as stage tick `until`: whether the projectile was still flying
    /// then, and if so where (`Course`), so one that opens in flight (Spoutpot's SS) or steers
    /// (Herdhound's missiles) carries on from there. From tick 0 at offset 0 it is a launch.
    pub(crate) fn track_until(&self, flight: Flight, from_tick: u32, offset: u32, senses: Senses, until: u32) -> Course {
        let (track, flying) = self.follow(flight, from_tick, offset, from_tick == 0 && offset == 0, senses, Some(until));
        Course { track, flying }
    }

    /// Follows a projectile through the weather (`track`), stopping only at what `senses` names,
    /// and at stage tick `until` if there is one; also returns whether it was still flying then.
    /// `launched` says whether `flight` is a projectile leaving the barrel (`fly_sensing`). A
    /// deadline that falls while a tornado holds the projectile comes as the tornado lets go, its
    /// flung leg in the track (at its own tick 0) and `last`.
    fn follow(&self, flight: Flight, from_tick: u32, offset: u32, launched: bool, senses: Senses, until: Option<u32>) -> (Track, bool) {
        let guard = MAX_FLIGHT_SECONDS.value * TICKS_PER_SECOND;
        let mut legs = Vec::new();
        let mut leg = Leg {
            flight,
            from: from_tick,
            to: from_tick,
            offset,
        };
        // Where to fly on from; a column that only marks it moves it on without starting a new leg.
        let mut cursor = from_tick;
        let mut launched = launched;
        let mut marks = Marks::default();
        loop {
            let last = until.map_or(guard, |until| until.saturating_sub(leg.offset).min(guard));
            // Nothing touched by `last`: past the guard it's gone, short of it the deadline came.
            let (impact, flying) = self.fly_through(&leg.flight, cursor, launched, senses, last).map_or_else(
                || {
                    let gone = Impact {
                        tick: last,
                        at: leg.flight.at(last),
                        contact: Contact::Gone,
                    };
                    (gone, last < guard)
                },
                |impact| (impact, false),
            );
            launched = false;
            leg.to = impact.tick;
            let column = match impact.contact {
                Contact::Column(index) if legs.len() < MAX_LEGS => self.weather.columns.get(usize::from(index)).copied(),
                _ => None,
            };
            let Some(column) = column else {
                legs.push(leg);
                let global = Impact { tick: impact.tick + leg.offset, ..impact };
                let contact = if matches!(impact.contact, Contact::Column(_)) { Contact::Gone } else { impact.contact };
                let track = Track {
                    legs,
                    impact: Impact { contact, ..global },
                    marks,
                    last: leg,
                    last_tick: impact.tick,
                };
                return (track, flying);
            };
            // A Random column acts as what it hides.
            let kind = column.acts_as();
            if marks.mark(kind) {
                cursor = impact.tick + 1;
                continue;
            }
            let velocity = leg.flight.velocity_at(impact.tick);
            let next = match kind {
                WeatherKind::Mirror => Some((
                    Flight {
                        origin: impact.at,
                        velocity: Vec2::new(-velocity.x, velocity.y),
                        accel: Vec2::new(-leg.flight.accel.x, leg.flight.accel.y),
                    },
                    0,
                )),
                WeatherKind::Tornado => Some((
                    Flight {
                        origin: impact.at,
                        velocity: Vec2::new(velocity.x, -velocity.y.abs()),
                        accel: leg.flight.accel,
                    },
                    ticks(TORNADO_HOLD_SECONDS.value),
                )),
                // Marked above. Wind, Lock, Balm and Skyeye never stand as columns, and a Random
                // one always hides another kind; were one there, it would do nothing.
                WeatherKind::Force | WeatherKind::Electricity | WeatherKind::Drain | WeatherKind::Wind | WeatherKind::Lock | WeatherKind::Balm | WeatherKind::Random | WeatherKind::Skyeye => {
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

    /// Detonates `blast` at `tick`: carves a crater of radius `crater` (the blast's reach unless an
    /// item or sudden death widens it), then damages every living mobile in reach.
    pub(crate) fn explode(&mut self, blast: &Blast, crater: f64, direct: Option<u8>, tick: u32, effects: &mut Vec<Effect>) {
        self.explode_by(blast, crater, tick, effects, |seat, target| damage::damage(blast, target, direct == Some(seat)));
    }

    /// `explode`, with `harm` saying what the blast deals each living mobile (by seat), for a blast
    /// that hurts by a rule of its own (Thistleback's SS).
    pub(crate) fn explode_by(&mut self, blast: &Blast, crater: f64, tick: u32, effects: &mut Vec<Effect>, harm: impl Fn(u8, &Target) -> i32) {
        let dirty = self.terrain.carve(blast.center, crater);
        effects.push(Effect::Exploded {
            at: blast.center,
            radius: crater,
            tick,
            dirty,
        });
        for seat in 0..self.players.len() {
            let player = self.players[seat];
            let Ok(seat) = u8::try_from(seat) else { continue };
            if player.alive {
                self.hurt(seat, harm(seat, &player.target()), effects);
            }
        }
    }

    /// Deals `amount` to a living mobile: its shield soaks what it can, then its HP takes the rest.
    pub(crate) fn hurt(&mut self, seat: u8, amount: i32, effects: &mut Vec<Effect>) {
        let player = &mut self.players[usize::from(seat)];
        if amount <= 0 || !player.alive {
            return;
        }
        let soaked = amount.min(player.shield).max(0);
        if soaked > 0 {
            player.shield -= soaked;
            effects.push(Effect::Shielded { seat, amount: soaked });
        }
        let rest = amount - soaked;
        if rest > 0 {
            player.hp -= rest;
            effects.push(Effect::Damaged { seat, amount: rest });
            if player.hp <= 0 {
                self.kill(seat, Death::Blasted, effects);
            }
        }
    }

    /// Heals a living mobile by `percent` of its max HP (rounding down), up to the max.
    pub(crate) fn heal(&mut self, seat: u8, percent: u32, effects: &mut Vec<Effect>) {
        let player = &mut self.players[usize::from(seat)];
        let max = player.tuning().hp.value;
        let amount = crate::items::percent_of(max, percent).min(max - player.hp);
        if amount > 0 && player.alive {
            player.hp += amount;
            effects.push(Effect::Healed { seat, amount });
        }
    }

    fn kill(&mut self, seat: u8, death: Death, effects: &mut Vec<Effect>) {
        let player = &mut self.players[usize::from(seat)];
        player.alive = false;
        player.hp = player.hp.max(0);
        effects.push(Effect::Died { seat, death });
    }

    /// Drops every living mobile whose ground is gone, and re-reads every tilt.
    pub(crate) fn settle_all(&mut self, effects: &mut Vec<Effect>) {
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
                    self.place(seat, footing);
                }
                Landing::OffMap => self.kill(seat, Death::Bunged, effects),
            }
        }
    }

    /// Stands `seat` at `footing`, tilted to the ground there.
    pub(crate) fn place(&mut self, seat: u8, footing: Footing) {
        let player = &mut self.players[usize::from(seat)];
        player.footing = footing;
        player.tilt = movement::tilt(&self.terrain, footing, player.tuning().ground_probes.value);
    }

    fn end_turn(&mut self, seat: u8, ending: Ending, time: Centis, effects: &mut Vec<Effect>) {
        let time = time.min(TURN_TIME_LIMIT.value);
        let player = &mut self.players[usize::from(seat)];
        let tuning = player.tuning();
        let items = player.armed.take().map_or(0, |item| item.tuning().delay.value + tuning.item_surcharge.value);
        player.clock.delay += turn::turn_delay(tuning.base_delay.value, ending, items, tuning.delay_per_second.value, time);
        player.clock.last_turn = self.turns;
        self.turns += 1;
        let before = (self.weather.columns.clone(), self.weather.wind_owed, self.weather.now);
        self.weather.tick(self.round);
        if (self.weather.columns.clone(), self.weather.wind_owed, self.weather.now) != before {
            effects.push(Effect::WeatherChanged {
                columns: self.weather.columns.clone(),
                wind_owed: self.weather.wind_owed,
                now: self.weather.now,
            });
        }
        let dropped = effects.len();
        drops::land_due(self, effects);
        self.reveal_passed(effects, dropped);
        // A game that just ended doesn't announce sudden death on its way out.
        if let Some(sudden_death) = self.sudden_death
            && self.turns == i64::from(sudden_death.turn)
            && !self.decided()
        {
            effects.push(Effect::SuddenDeath { kind: sudden_death.kind });
        }
        self.start_next_turn(effects);
    }

    /// Shows what each Random column hides once a projectile's path among `effects[from..]` has
    /// passed through it. Only paths drawn while the column stood count, so `from` is where they
    /// start.
    fn reveal_passed(&mut self, effects: &mut Vec<Effect>, from: usize) {
        if !self.weather.columns.iter().any(Column::is_hidden) {
            return;
        }
        let paths: Vec<&Leg> = effects[from..]
            .iter()
            .flat_map(|effect| match effect {
                Effect::Fired { legs, .. } | Effect::Helix { legs, .. } | Effect::Orbit { legs, .. } | Effect::Beacon { legs, .. } | Effect::Dropped { legs, .. } | Effect::Guide { legs, .. } => {
                    legs.as_slice()
                }
                _ => &[],
            })
            .collect();
        let mut revealed = Vec::new();
        for column in self.weather.columns.iter_mut().filter(|c| c.is_hidden()) {
            if paths.iter().any(|leg| (leg.from..=leg.to).any(|tick| column.contains(leg.flight.at(tick).x))) {
                *column = column.revealed();
                revealed.push(Effect::Revealed { kind: column.kind, x: column.x });
            }
        }
        effects.extend(revealed);
    }

    /// Whether fewer than two teams have anyone standing.
    pub(crate) fn decided(&self) -> bool {
        [Team::A, Team::B].into_iter().filter(|&team| self.players.iter().any(|p| p.alive && p.team == team)).count() < 2
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

    /// Hands the turn to whoever has the least delay. As it starts, their shield regenerates
    /// (not under Lock), their SS lock counts down (twice under Lock), and Balm heals them.
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
        self.phase = Phase::Turn { seat };
        effects.push(Effect::TurnStarted { seat });
        let player = &mut self.players[usize::from(seat)];
        let tuning = player.tuning();
        player.moves_left = tuning.move_budget.value;
        let locked = self.weather.is(WeatherKind::Lock);
        let regen = if locked { 0 } else { tuning.shield_regen.value.min(tuning.shield.value - player.shield) };
        if regen > 0 {
            player.shield += regen;
            effects.push(Effect::ShieldRestored { seat, amount: regen });
        }
        if player.ss_lock > 0 {
            player.ss_lock = player.ss_lock.saturating_sub(if locked { LOCK_SS_RECHARGE.value } else { 1 });
            if player.ss_lock == 0 {
                effects.push(Effect::SsReady { seat });
            }
        }
        if self.weather.is(WeatherKind::Balm) {
            let bionic = tuning.armor.value == ArmorClass::Bionic;
            self.heal(seat, BALM_HEAL_PERCENT.value * if bionic { BALM_BIONIC_FACTOR.value } else { 1 }, effects);
        }
    }

    pub(crate) fn skyeye_mut(&mut self) -> &mut Skyeye {
        &mut self.weather.skyeye
    }

    /// The game as a player sees it, for searching shots ahead (SearchBot, the lab's trace): a
    /// Random column no shot has passed through is left out, since no one knows what it hides.
    #[must_use]
    pub fn as_seen(&self) -> Self {
        let mut seen = self.clone();
        seen.weather.columns.retain(|column| !column.is_hidden());
        seen
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

    pub fn seat(team: Team, mobile: MobileKind, spawn_x: i32, facing: Facing) -> Seat {
        Seat {
            team,
            mobile,
            spawn_x,
            facing,
            items: Loadout::default(),
        }
    }

    pub fn duel_setup(distance: i32) -> Setup {
        Setup {
            map: MapRef::Flat { width: 2400, height: 600, ground: 500 },
            seats: vec![
                seat(Team::A, MobileKind::Coilback, 300, Facing::Right),
                seat(Team::B, MobileKind::Coilback, 300 + distance, Facing::Left),
            ],
            order: vec![0, 1],
            wind: Wind::default(),
            sudden_death: None,
        }
    }

    /// Answers every owed roll with calm, harmless weather: Force beams at the map's far-left edge
    /// (no test shot goes there), no wind, and a Calm drop.
    pub fn answer_rolls(game: &mut Game) {
        while let Some(roll) = game.pending_roll() {
            let event = match roll {
                Roll::Weather { .. } => Event::Weather(Forecast::new(WeatherKind::Force, 1)),
                Roll::Wind => Event::Wind(Wind::default()),
                Roll::Drop { .. } => Event::Drop(Drop { kind: DropKind::Calm, x: 1 }),
            };
            game.apply(event).expect("an owed roll is accepted");
        }
    }

    /// A game from `setup` with the opening forecasts in, ready for its first seat to act.
    pub fn start(setup: &Setup) -> Game {
        let mut game = Game::new(setup, &NoAssets).expect("a valid setup");
        answer_rolls(&mut game);
        game
    }

    /// A duel with the opening forecasts in, ready for seat 0 to act.
    pub fn duel(distance: i32) -> Game {
        start(&duel_setup(distance))
    }

    /// A duel between `a` (seat 0, x = 300, facing right) and `b`, `distance` px away.
    pub fn duel_of(a: MobileKind, b: MobileKind, distance: i32) -> Game {
        let mut setup = duel_setup(distance);
        setup.seats[0].mobile = a;
        setup.seats[1].mobile = b;
        start(&setup)
    }

    /// A game on the duel's flat map with these seats, acting in seat order.
    pub fn game_of(seats: Vec<Seat>) -> Game {
        let order = (0..seats.len()).map(|s| u8::try_from(s).unwrap()).collect();
        start(&Setup { seats, order, ..duel_setup(800) })
    }

    pub fn shoot(game: &mut Game, seat: u8, shot: Shot, aim: i16, power: Power) -> Vec<Effect> {
        let effects = game.apply(Event::Shoot { seat, shot, aim, power, time: Centis(0) }).unwrap();
        answer_rolls(game);
        effects
    }

    pub fn pass(game: &mut Game, seat: u8) -> Vec<Effect> {
        let effects = game.apply(Event::Pass { seat, time: Centis(0) }).unwrap();
        answer_rolls(game);
        effects
    }

    /// Everything `seat` took: shield and HP.
    pub fn damage_to(effects: &[Effect], seat: u8) -> i32 {
        effects
            .iter()
            .map(|effect| match *effect {
                Effect::Damaged { seat: s, amount } | Effect::Shielded { seat: s, amount } if s == seat => amount,
                _ => 0,
            })
            .sum()
    }

    pub fn explosions(effects: &[Effect]) -> usize {
        effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count()
    }

    /// The power step whose plain S1 from seat 0 at `aim` strikes seat 1's hitbox closest to its
    /// center.
    pub fn direct_hit_power(game: &Game, aim: i16) -> Power {
        direct_hit_power_of(game, Shot::S1, aim)
    }

    /// `direct_hit_power` for a plain projectile leaving `shot`'s barrel.
    pub fn direct_hit_power_of(game: &Game, shot: Shot, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let impact = game.track(game.launch(0, shot, aim, power.bars())?, 0, 0).impact;
                (impact.contact == Contact::Mobile(1)).then(|| (impact.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .expect("some power hits")
    }

    /// Serves one drawn mask, so tests can play on terrain the synthetic maps can't make.
    pub struct Drawn(pub Terrain);

    impl MapAssets for Drawn {
        fn mask(&self, _: &str, _: &str) -> Res<Terrain> {
            Ok(self.0.clone())
        }
    }

    /// A game on `terrain` with these seats, acting in seat order.
    pub fn game_on(terrain: Terrain, seats: Vec<Seat>) -> Game {
        let order = (0..seats.len()).map(|s| u8::try_from(s).unwrap()).collect();
        let map = MapRef::Asset {
            name: "drawn".to_owned(),
            variant: "A".to_owned(),
            mask_hash: terrain.mask_hash(),
        };
        let mut game = Game::new(&Setup { map, seats, order, ..duel_setup(800) }, &Drawn(terrain)).expect("a valid setup");
        answer_rolls(&mut game);
        game
    }

    /// Plays turns (passing) until it is `seat`'s.
    pub fn pass_until(game: &mut Game, seat: u8) {
        for _ in 0..64 {
            match game.phase() {
                Phase::Turn { seat: current } if current == seat => return,
                Phase::Turn { seat: current } => {
                    pass(game, current);
                }
                Phase::Over { .. } => panic!("the game ended before seat {seat}'s turn"),
            }
        }
        panic!("seat {seat}'s turn never came");
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::{
        testing::{answer_rolls, damage_to, direct_hit_power, duel, duel_of, duel_setup, explosions, pass, pass_until, shoot, start},
        *,
    };
    use crate::{drops::DropKind, map::NoAssets};

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
    fn spawns_leave_room_for_the_widest_mobile_facing_either_way() {
        let (wide, tall) = MobileKind::spawn_room();
        // Lanternwing's 66 px body is the widest, reaching 33 px from its feet either way.
        assert!(wide >= 33 && tall >= 58, "{wide} x {tall}");
        assert!(check_spawn_spacing(&[100, 100 + 2 * wide, 900]).is_ok());
        assert!(check_spawn_spacing(&[900, 100, 99 + 2 * wide]).is_err(), "order doesn't matter");
    }

    #[test]
    fn only_a_floating_mobile_is_left_out_of_what_the_ground_can_touch() {
        assert_eq!(MobileKind::ALL.iter().filter(|m| m.floats()).collect::<Vec<_>>(), [&MobileKind::Kitewyrm]);
        let game = duel_of(MobileKind::Kitewyrm, MobileKind::Coilback, 800);
        let seats = |frames: Vec<(u8, HitFrame)>| frames.into_iter().map(|(seat, _)| seat).collect::<Vec<u8>>();
        assert_eq!(seats(game.hit_frames()), [0, 1]);
        assert_eq!(seats(game.grounded_frames()), [1]);
        // Its body floats over its feet: nothing just above the ground under it is inside it.
        let kitewyrm = game.players()[0];
        let hover = crate::tuning::kitewyrm::HOVER.value;
        assert!(!kitewyrm.pose().hit(kitewyrm.tuning().hitbox.value, kitewyrm.footing.world() - Vec2::new(0.0, hover - 1.0)));
        assert!(kitewyrm.pose().hit(kitewyrm.tuning().hitbox.value, kitewyrm.footing.world() - Vec2::new(0.0, hover + 1.0)));
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
        assert!(matches!(game.pending_roll(), Some(Roll::Weather { width: 2400, .. })));
        assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_err(), "no moves while a roll is owed");
        assert!(game.apply(Event::Wind(Wind::default())).is_err(), "no wind is owed");
        for _ in 0..5 {
            game.apply(Event::Weather(Forecast::new(WeatherKind::Mirror, 100))).unwrap();
        }
        assert_eq!(game.pending_roll(), None);
        assert!(game.apply(Event::Weather(Forecast::new(WeatherKind::Mirror, 100))).is_err(), "no sixth");
        assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_ok());
    }

    #[test]
    fn weather_comes_up_every_round_and_the_wind_weather_owes_wind() {
        let mut game = Game::new(&duel_setup(800), &NoAssets).unwrap();
        for kind in [WeatherKind::Tornado, WeatherKind::Wind, WeatherKind::Force, WeatherKind::Force, WeatherKind::Force] {
            game.apply(Event::Weather(Forecast::new(kind, 900))).unwrap();
        }
        // A round is two turns in a duel.
        game.apply(Event::Pass { seat: 0, time: Centis(0) }).unwrap();
        let effects = game.apply(Event::Pass { seat: 1, time: Centis(0) }).unwrap();
        assert!(
            effects.contains(&Effect::WeatherChanged {
                columns: vec![Column::new(WeatherKind::Tornado, 900)],
                wind_owed: false,
                now: Some(WeatherKind::Tornado),
            }),
            "{effects:?}"
        );
        assert!(matches!(game.pending_roll(), Some(Roll::Weather { width: 2400, .. })), "the queue needs refilling");
        game.apply(Event::Weather(Forecast::new(WeatherKind::Force, 5))).unwrap();
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
    fn each_mobile_walks_its_own_budget_and_climbs_its_own_step() {
        for mobile in MobileKind::ALL {
            let tuning = mobile.tuning();
            let mut flat = duel_of(mobile, MobileKind::Coilback, 1600);
            let effects = flat
                .apply(Event::Walk {
                    seat: 0,
                    facing: Facing::Right,
                    steps: 500,
                })
                .unwrap();
            assert!(matches!(effects[..], [Effect::Walked { steps, .. }] if steps == tuning.move_budget.value), "{mobile:?}: {effects:?}");

            // A step up as tall as its climb, then one a pixel taller.
            let climb = u32::try_from(tuning.climb_px.value).unwrap();
            let steps = Terrain::from_fn(2400, 600, |x, y| {
                y >= if x < 310 {
                    500
                } else if x < 320 {
                    500 - climb
                } else {
                    499 - 2 * climb
                }
            })
            .unwrap();
            let seats = vec![testing::seat(Team::A, mobile, 300, Facing::Right), testing::seat(Team::B, MobileKind::Coilback, 1900, Facing::Left)];
            let mut game = testing::game_on(steps, seats);
            game.apply(Event::Walk {
                seat: 0,
                facing: Facing::Right,
                steps: 50,
            })
            .unwrap();
            let expected = Footing { x: 319, y: 500 - tuning.climb_px.value };
            assert_eq!(game.players()[0].footing, expected, "{mobile:?} climbs {climb} px, and no more");
        }
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
        game.weather.columns = vec![Column::new(kind, x)];
        game
    }

    /// `game` with `kind` the weather now, and the next one far off.
    fn under(mut game: Game, kind: WeatherKind) -> Game {
        game.weather.now = Some(kind);
        game.weather.turns_left = 100;
        game
    }

    #[test]
    fn lock_bars_items_stops_shields_and_doubles_the_ss_countdown() {
        let mut game = under(duel_of(MobileKind::Coilback, MobileKind::Lanternwing, 800), WeatherKind::Lock);
        game.players[0].items = Loadout::new(&[Item::Heal]).unwrap().positions();
        assert!(!game.can_use_item(0, 0));
        assert!(game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).is_err());
        game.players[1].shield -= 100;
        game.players[1].ss_lock = 3;
        let effects = pass(&mut game, 0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::ShieldRestored { .. })), "{effects:?}");
        assert_eq!(game.players[1].ss_lock, 1, "three own turns of lock, less two");
    }

    #[test]
    fn balm_heals_each_player_as_their_turn_starts_and_bionic_double() {
        let mut game = under(duel_of(MobileKind::Coilback, MobileKind::Hookclaw, 800), WeatherKind::Balm);
        game.players[0].hp = 500;
        game.players[1].hp = 500;
        let hookclaw = game.players[1].tuning().hp.value;
        assert!(pass(&mut game, 0).contains(&Effect::Healed { seat: 1, amount: hookclaw * 6 / 100 }), "Hookclaw is Bionic");
        let coilback = game.players[0].tuning().hp.value;
        pass_until(&mut game, 0);
        assert_eq!(game.players[0].hp, 500 + coilback * 3 / 100, "healed once, as its turn started");
    }

    fn beams(effects: &[Effect]) -> Vec<(Vec2, Vec2, u8)> {
        effects
            .iter()
            .filter_map(|e| if let Effect::Beam { from, to, level, .. } = *e { Some((from, to, level)) } else { None })
            .collect()
    }

    #[test]
    fn under_skyeye_a_beam_follows_each_landing_from_the_satellite() {
        let calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let plain = damage_to(&shoot(&mut calm.clone(), 0, Shot::S1, 35, power), 1);
        let mut game = under(calm.clone(), WeatherKind::Skyeye);
        let effects = shoot(&mut game, 0, Shot::S1, 35, power);
        let fired = beams(&effects);
        assert_eq!(fired.len(), 1, "{effects:?}");
        let (from, to, level) = fired[0];
        assert_eq!((from, level), (Skyeye::at(2400), 1));
        assert!(to.distance(game.players()[1].center()) < 40.0, "it struck the target: {to:?}");
        let dealt = damage_to(&effects, 1);
        assert!((plain + 50..=plain + 100).contains(&dealt), "a level 1 beam on top of {plain}: {dealt}");
        assert_eq!(game.weather.skyeye.xp, u32::try_from(dealt - plain).unwrap(), "only the beam's damage counts");
        assert!(beams(&shoot(&mut calm.clone(), 0, Shot::S1, 35, power)).is_empty(), "no beam when Skyeye isn't up");
    }

    #[test]
    fn skyeye_leaves_a_weatherproof_mobiles_shots_alone() {
        let mut game = under(duel_of(MobileKind::Sudsotter, MobileKind::Coilback, 800), WeatherKind::Skyeye);
        let effects = shoot(&mut game, 0, Shot::S1, 45, Power::nearest(2.0));
        assert!(effects.iter().any(|e| matches!(e, Effect::Exploded { .. })), "the sponge went off: {effects:?}");
        assert!(beams(&effects).is_empty(), "{effects:?}");
    }

    #[test]
    fn skyeye_beams_once_for_a_projectile_that_bursts_many_times() {
        let calm = duel_of(MobileKind::Thistleback, MobileKind::Coilback, 800);
        let power = direct_hit_power(&calm, 30);
        let mut game = under(calm, WeatherKind::Skyeye);
        let effects = shoot(&mut game, 0, Shot::Ss, 30, power);
        let blasts = effects.iter().filter(|e| matches!(e, Effect::Exploded { .. })).count();
        assert!(blasts >= 9, "the pumpkin's ring went off: {effects:?}");
        assert_eq!(beams(&effects).len(), 1, "one pumpkin, one beam");
    }

    #[test]
    fn skyeye_levels_up_on_the_damage_its_beams_deal() {
        let calm = duel(800);
        let power = direct_hit_power(&calm, 35);
        let mut game = under(calm, WeatherKind::Skyeye);
        game.weather.skyeye.xp = 195;
        let effects = shoot(&mut game, 0, Shot::S1, 35, power);
        assert!(effects.contains(&Effect::SkyeyeLevel { level: 2 }), "{effects:?}");
        assert_eq!(game.weather.skyeye.level(), 2);
    }

    fn random_hiding(hides: WeatherKind, x: i32) -> Column {
        Column {
            kind: WeatherKind::Random,
            x,
            hides: Some(hides),
        }
    }

    #[test]
    fn a_forecast_says_what_it_hides_only_when_random() {
        let mut game = Game::new(&duel_setup(800), &NoAssets).unwrap();
        let bare = Forecast::new(WeatherKind::Random, 900);
        assert!(game.apply(Event::Weather(bare)).is_err());
        let odd = Forecast { hides: Some(WeatherKind::Lock), ..bare };
        assert!(game.apply(Event::Weather(odd)).is_err(), "Lock is no column to hide");
        let extra = Forecast {
            hides: Some(WeatherKind::Mirror),
            ..Forecast::new(WeatherKind::Force, 900)
        };
        assert!(game.apply(Event::Weather(extra)).is_err());
        assert!(game.apply(Event::Weather(Forecast { hides: Some(WeatherKind::Mirror), ..bare })).is_ok());
    }

    #[test]
    fn a_random_column_acts_as_what_it_hides_and_shows_it_once_passed() {
        let mut game = duel(800);
        game.weather.columns = vec![random_hiding(WeatherKind::Mirror, 600), random_hiding(WeatherKind::Force, 2000)];
        let effects = shoot(&mut game, 0, Shot::S1, 45, Power::nearest(2.4));
        assert!(effects.contains(&Effect::Revealed { kind: WeatherKind::Mirror, x: 600 }), "{effects:?}");
        let fired = effects.iter().find_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.len()) } else { None });
        assert_eq!(fired, Some(2), "sent back, as by a Mirror");
        assert_eq!(game.weather.columns[0], Column::new(WeatherKind::Mirror, 600));
        assert!(game.weather.columns[1].is_hidden(), "nothing passed the far one");
    }

    #[test]
    fn as_a_player_sees_it_a_hidden_column_is_not_there() {
        let mut game = duel(800);
        game.weather.columns = vec![random_hiding(WeatherKind::Mirror, 600), Column::new(WeatherKind::Force, 900)];
        assert_eq!(game.as_seen().weather().columns, [Column::new(WeatherKind::Force, 900)]);
    }

    #[test]
    fn a_mirror_sends_the_shot_back() {
        let game = with_column(WeatherKind::Mirror, 600);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let track = game.track(flight, 0, 0);
        assert_eq!(track.legs.len(), 2);
        assert!(track.legs[1].flight.velocity.x < 0.0 && track.impact.at.x < 600.0, "{track:?}");
    }

    #[test]
    fn a_mirror_sends_back_a_shot_fired_from_inside_its_beam() {
        // The beam spans 320 to 360, and the muzzle stands at about 334.
        let game = with_column(WeatherKind::Mirror, 340);
        let flight = game.launch(0, Shot::S1, 25, 2.0).unwrap();
        assert!(game.weather.columns[0].contains(flight.origin.x), "{flight:?}");
        let track = game.track(flight, 0, 0);
        assert_eq!(track.legs.len(), 2, "{track:?}");
        assert!(track.legs[1].flight.velocity.x < 0.0, "{track:?}");
    }

    #[test]
    fn force_marks_a_shot_fired_from_inside_its_beam() {
        let game = with_column(WeatherKind::Force, 340);
        let flight = game.launch(0, Shot::S1, 25, 2.0).unwrap();
        assert!(game.track(flight, 0, 0).marks.forced);
    }

    #[test]
    fn a_tornado_holds_the_shot_and_flings_it_up() {
        let game = with_column(WeatherKind::Tornado, 600);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let track = game.track(flight, 0, 0);
        assert_eq!(track.legs.len(), 2);
        let entry = track.legs[0].to;
        assert_eq!(track.legs[1].offset, entry + TICKS_PER_SECOND, "held one second");
        assert!(track.legs[1].flight.velocity.y < 0.0 && track.legs[1].flight.velocity.x > 0.0, "{track:?}");
    }

    #[test]
    fn a_bounded_track_stops_on_the_whole_path_still_flying() {
        let game = duel(800);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let whole = game.track(flight, 0, 0);
        let course = game.track_until(flight, 0, 0, Senses::ALL, 1000);
        assert!(course.flying);
        assert_eq!((course.track.last_tick, course.track.impact.tick), (1000, 1000));
        assert_eq!(course.track.impact.at, whole.legs[0].flight.at(1000));
        // Past where it lands, it lands as the whole track does.
        let landed = game.track_until(flight, 0, 0, Senses::ALL, whole.impact.tick + 10);
        assert!(!landed.flying);
        assert_eq!(landed.track, whole);
    }

    #[test]
    fn force_marks_the_shot_without_turning_it() {
        let game = with_column(WeatherKind::Force, 600);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let calm = duel(800).track(flight, 0, 0);
        let forced = game.track(flight, 0, 0);
        assert!(forced.marks.forced && !calm.marks.forced);
        assert_eq!(forced.impact, calm.impact, "the path is unchanged");
    }

    #[test]
    fn a_track_with_a_deadline_stops_a_shot_still_flying_then() {
        let game = duel(800);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let whole = game.track(flight, 0, 0);
        let after = game.track_until(flight, 0, 0, Senses::ALL, whole.impact.tick + 10);
        assert_eq!((after.track, after.flying), (whole, false), "a deadline after it lands changes nothing");
        let Course { track: cut, flying } = game.track_until(flight, 0, 0, Senses::ALL, 1000);
        assert!(flying);
        assert_eq!((cut.legs.len(), cut.last.to, cut.last_tick, cut.impact.tick), (1, 1000, 1000, 1000));
        assert_eq!(cut.impact.at, flight.at(1000));
    }

    #[test]
    fn a_deadline_counts_only_the_columns_passed_before_it() {
        let game = with_column(WeatherKind::Force, 600);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        // The beam spans 580 to 620.
        let reaches = (0..TICKS_PER_SECOND).find(|&tick| flight.at(tick).x >= 560.0).unwrap();
        assert!(!game.track_until(flight, 0, 0, Senses::ALL, reaches).track.marks.forced);
        assert!(game.track_until(flight, 0, 0, Senses::ALL, reaches + 200).track.marks.forced);
    }

    #[test]
    fn a_deadline_inside_a_tornado_comes_as_it_lets_go() {
        let game = with_column(WeatherKind::Tornado, 600);
        let flight = game.launch(0, Shot::S1, 45, 2.4).unwrap();
        let entry = game.track(flight, 0, 0).legs[0].to;
        let Course { track: held, flying } = game.track_until(flight, 0, 0, Senses::ALL, entry + TICKS_PER_SECOND / 2);
        assert!(flying);
        assert_eq!(held.legs.len(), 2);
        assert_eq!((held.last.offset, held.last_tick), (entry + TICKS_PER_SECOND, 0), "{held:?}");
    }

    // ── SS gating ──

    #[test]
    fn ss_locks_for_three_own_turns_and_comes_back_on_the_fourth() {
        let mut game = duel(800);
        let effects = shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(2.0));
        assert!(effects.contains(&Effect::SsLocked { seat: 0, turns: 4 }), "{effects:?}");
        for own_turn in 1..=3 {
            pass_until(&mut game, 0);
            assert!(!game.can_shoot(0, Shot::Ss), "own turn {own_turn}");
            assert!(
                game.apply(Event::Shoot {
                    seat: 0,
                    shot: Shot::Ss,
                    aim: 30,
                    power: Power::nearest(2.0),
                    time: Centis(0)
                })
                .is_err()
            );
            assert!(game.can_shoot(0, Shot::S1));
            pass(&mut game, 0);
        }
        pass_until(&mut game, 1);
        let effects = pass(&mut game, 1);
        assert!(effects.contains(&Effect::SsReady { seat: 0 }), "{effects:?}");
        assert!(game.can_shoot(0, Shot::Ss));
    }

    #[test]
    fn dual_and_dual_plus_refuse_ss() {
        for item in [Item::Dual, Item::DualPlus] {
            let mut game = duel(800);
            game.players[0].items = Loadout::new(&[item]).unwrap().positions();
            game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).unwrap();
            assert!(!game.can_shoot(0, Shot::Ss), "{item:?}");
            assert_eq!(game.ss_state(0).unwrap(), SsState::Armed(item));
            assert!(game.can_shoot(0, Shot::S2));
        }
    }

    #[test]
    fn ss_state_says_why_on_any_turn() {
        let mut game = duel(800);
        assert_eq!(game.ss_state(1).unwrap(), SsState::Ready, "not its turn, but ready");
        shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(2.0));
        assert_eq!(game.ss_state(0).unwrap(), SsState::Locked(4), "locked as it fired");
        assert!(game.ss_state(9).is_err());
        for kind in [SuddenDeathKind::BigBomb, SuddenDeathKind::Double] {
            assert_eq!(sudden(kind, 0).ss_state(0).unwrap(), SsState::Off(kind));
        }
        let mut game = sudden(SuddenDeathKind::SsDeath, 0);
        game.players[0].ss_lock = 3;
        assert_eq!(game.ss_state(0).unwrap(), SsState::Ready, "SS Death lifts the lock");
    }

    #[test]
    fn sudden_deaths_have_their_own_names() {
        let names = SuddenDeathKind::ALL.map(SuddenDeathKind::name);
        assert_eq!(names, ["Big Bomb", "Double", "SS Death"]);
    }

    // ── Sudden death ──

    fn sudden(kind: SuddenDeathKind, turn: u32) -> Game {
        start(&Setup {
            sudden_death: Some(SuddenDeath { kind, turn }),
            ..duel_setup(1200)
        })
    }

    #[test]
    fn sudden_death_starts_once_its_turns_are_done_and_turns_items_off() {
        let mut game = sudden(SuddenDeathKind::BigBomb, 2);
        game.players[0].items = Loadout::new(&[Item::Heal]).unwrap().positions();
        assert_eq!(game.sudden_death(), None);
        assert!(pass(&mut game, 0).iter().all(|e| !matches!(e, Effect::SuddenDeath { .. })));
        let effects = pass(&mut game, 1);
        assert!(effects.contains(&Effect::SuddenDeath { kind: SuddenDeathKind::BigBomb }), "{effects:?}");
        let started = effects.iter().position(|e| matches!(e, Effect::SuddenDeath { .. })).unwrap();
        let turn = effects.iter().position(|e| matches!(e, Effect::TurnStarted { .. })).unwrap();
        assert!(started < turn, "it begins before the next turn does");
        assert_eq!(game.sudden_death(), Some(SuddenDeathKind::BigBomb));
        assert!(!game.can_use_item(0, 0), "items are off");
        assert!(game.apply(Event::UseItem { seat: 0, slot: 0, time: Centis(0) }).is_err());
    }

    #[test]
    fn big_bomb_and_double_turn_ss_off_and_ss_death_frees_it() {
        for kind in [SuddenDeathKind::BigBomb, SuddenDeathKind::Double] {
            let game = sudden(kind, 0);
            assert!(!game.can_shoot(0, Shot::Ss), "{kind:?}");
        }
        let mut game = sudden(SuddenDeathKind::SsDeath, 0);
        game.players[0].ss_lock = 3;
        assert!(game.can_shoot(0, Shot::Ss), "a lock means nothing in SS Death");
        let effects = shoot(&mut game, 0, Shot::Ss, 30, Power::nearest(2.0));
        assert!(!effects.iter().any(|e| matches!(e, Effect::SsLocked { .. })));
    }

    #[test]
    fn big_bomb_widens_every_crater() {
        let short = Power::nearest(1.5);
        let crater = |effects: &[Effect]| effects.iter().find_map(|e| if let Effect::Exploded { radius, .. } = e { Some(*radius) } else { None }).unwrap();
        let mut calm = duel(1200);
        let mut bomb = sudden(SuddenDeathKind::BigBomb, 0);
        let plain = crater(&shoot(&mut calm, 0, Shot::S1, 45, short));
        assert!((crater(&shoot(&mut bomb, 0, Shot::S1, 45, short)) - plain * 1.5).abs() < 1e-9);
    }

    #[test]
    fn double_fires_every_shot_twice_without_item_delay() {
        let mut game = sudden(SuddenDeathKind::Double, 0);
        let effects = shoot(&mut game, 0, Shot::S1, 45, Power::nearest(1.5));
        assert_eq!(effects.iter().filter(|e| matches!(e, Effect::Volley { .. })).count(), 2, "{effects:?}");
        assert_eq!(explosions(&effects), 2);
        assert_eq!(game.players()[0].clock.delay, 770, "base and S1 only");
    }

    // ── Shields ──

    #[test]
    fn a_shield_soaks_damage_before_hp_and_regenerates_on_its_turn() {
        let mut game = duel_of(MobileKind::Coilback, MobileKind::Sparkhorn, 800);
        let power = direct_hit_power(&game, 35);
        let effects = shoot(&mut game, 0, Shot::S1, 35, power);
        let soaked: i32 = effects.iter().map(|e| if let Effect::Shielded { seat: 1, amount } = e { *amount } else { 0 }).sum();
        assert!(
            soaked > 0 && !effects.iter().any(|e| matches!(e, Effect::Damaged { seat: 1, .. })),
            "300 shield soaks one S1: {effects:?}"
        );
        let sparkhorn = game.players()[1];
        assert_eq!(sparkhorn.hp, 750);
        // Its turn just started, so it has already regenerated: the hit, less 20.
        assert_eq!(sparkhorn.shield, 300 - soaked + 20);
        assert!(effects.contains(&Effect::ShieldRestored { seat: 1, amount: 20 }), "{effects:?}");
    }

    #[test]
    fn a_full_shield_does_not_regenerate_past_its_max() {
        let mut game = duel_of(MobileKind::Coilback, MobileKind::Lanternwing, 800);
        let effects = pass(&mut game, 0);
        assert!(!effects.iter().any(|e| matches!(e, Effect::ShieldRestored { .. })), "{effects:?}");
        assert_eq!(game.players()[1].shield, 220);
    }

    #[test]
    fn damage_past_the_shield_reaches_hp() {
        let mut game = duel_of(MobileKind::Coilback, MobileKind::Lanternwing, 800);
        game.players[1].shield = 10;
        let mut effects = Vec::new();
        game.hurt(1, 100, &mut effects);
        assert_eq!(effects, [Effect::Shielded { seat: 1, amount: 10 }, Effect::Damaged { seat: 1, amount: 90 }]);
        assert_eq!((game.players()[1].shield, game.players()[1].hp), (0, 640));
        assert_eq!(damage_to(&effects, 1), 100);
    }

    // ── The slot machine ──

    /// A 2v1 where seat 2 (team A) is dead and seat 0 is to act.
    fn with_a_dead_seat() -> Game {
        let mut setup = duel_setup(800);
        setup.seats.push(testing::seat(Team::A, MobileKind::Coilback, 1800, Facing::Left));
        setup.order = vec![0, 1, 2];
        let mut game = start(&setup);
        game.apply(Event::Forfeit { seat: 2 }).unwrap();
        game
    }

    #[test]
    fn only_the_dead_spin_and_then_the_server_owes_a_drop() {
        let mut game = with_a_dead_seat();
        assert!(!game.can_spin(0), "seat 0 is alive");
        assert!(game.can_spin(2));
        let effects = game.apply(Event::Spin { seat: 2 }).unwrap();
        assert_eq!(effects, [Effect::Spun { seat: 2 }]);
        assert_eq!(game.pending_roll(), Some(Roll::Drop { width: 2400 }));
        assert!(game.apply(Event::Pass { seat: 0, time: Centis(0) }).is_err(), "no moves while it is owed");
        assert!(!game.can_spin(2));
        let drop = Drop { kind: DropKind::Hammer, x: 1100 };
        assert!(game.apply(Event::Drop(Drop { x: 9999, ..drop })).is_err(), "off the map");
        let effects = game.apply(Event::Drop(drop)).unwrap();
        assert_eq!(effects, [Effect::DropRolled { seat: 2, drop }]);
        assert_eq!(game.drops(), [Falling { seat: 2, drop, lands: 4 }]);
        assert!(game.apply(Event::Drop(drop)).is_err(), "only one was owed");
    }

    #[test]
    fn a_slot_machine_recharges_once_a_round() {
        let mut game = with_a_dead_seat();
        game.apply(Event::Spin { seat: 2 }).unwrap();
        answer_rolls(&mut game);
        // Three seats: a round is three turns.
        for _ in 0..2 {
            let Phase::Turn { seat } = game.phase() else { panic!() };
            pass(&mut game, seat);
            assert!(!game.can_spin(2));
        }
        let Phase::Turn { seat } = game.phase() else { panic!() };
        pass(&mut game, seat);
        assert!(game.can_spin(2));
    }

    #[test]
    fn a_drop_lands_at_the_end_of_the_fourth_turn_after_its_spin() {
        let mut game = with_a_dead_seat();
        game.apply(Event::Spin { seat: 2 }).unwrap();
        let target = game.players()[1].footing.x;
        game.apply(Event::Drop(Drop { kind: DropKind::Hammer, x: target })).unwrap();
        for turn in 1..=4 {
            let Phase::Turn { seat } = game.phase() else { panic!() };
            let effects = pass(&mut game, seat);
            let landed = effects.iter().position(|e| matches!(e, Effect::Dropped { .. }));
            assert_eq!(landed.is_some(), turn == 4, "turn {turn}: {effects:?}");
            if let Some(landed) = landed {
                let next = effects.iter().position(|e| matches!(e, Effect::TurnStarted { .. })).unwrap();
                assert!(landed < next, "before the next turn starts");
                assert!(damage_to(&effects, 1) > 0, "the hammer lands on seat 1: {effects:?}");
            }
        }
        assert_eq!(game.drops(), &[] as &[Falling], "all landed");
    }
}
