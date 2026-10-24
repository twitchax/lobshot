//! The interface LLM agents play through (`DESIGN.md`, "Bots and the lab"): one seat in a hosted
//! game, beside and against SearchBot. An agent sees what a player sees (`View`), may ask where a
//! shot would land under the wind the HUD shows (`Session::trace`: what SearchBot's search knows,
//! nothing more), and acts with the wire's own `Action`. Its turns are charged the bot think time,
//! so a slow model plays on the same clock as a quick one. The CLI (`lobshot-lab play`) and the
//! MCP server (`lobshot-lab mcp`) are thin shells over a `Session`.

use std::fmt::Write as _;

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

use lobshot_assets::{maps::Maps, mobile_slug};
use lobshot_protocol::{Action, RoomOptions};
use lobshot_sim::{
    base::Res,
    bot::{self, Level},
    game::{Death, Effect, Event, Game, MobileKind, Phase, Player, Shot, SsState, Team},
    items::Item,
    map::MapRef,
    referee::{self, Entrant, XorShift},
    tuning,
    units::{Facing, Power},
    weather::{Skyeye, WeatherKind},
};

use crate::{
    balance::{LevelName, level_name, mobile},
    host::{Happened, Host, Saved},
};

/// An agent's game that hasn't ended by this many turns stops there.
pub const TURN_CAP: i64 = 300;

/// The agent's terrain picture: this many columns across, whatever the map's width.
const PICTURE_COLUMNS: u32 = 100;

/// How to set up an agent's game. Everything has a default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NewGame {
    /// Your mobile, by name; random when left out.
    pub mobile: Option<String>,
    /// The other team's mobiles, by name, one a seat; random when left out.
    pub against: Vec<String>,
    /// A lobby map, or `flat`; the server's default when left out.
    pub map: Option<String>,
    /// Players a team, 1 to 4 (1 when left out). Your teammates are bots too.
    pub team_size: Option<u8>,
    /// How well the bots play: easy, normal or hard (normal when left out).
    pub level: Option<String>,
    /// Where the game's rolls start; a fresh one when left out (every view reports it).
    pub seed: Option<u64>,
}

/// An agent's game, saved: the host's state, and which seat is the agent's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSession {
    pub host: Saved,
    pub you: u8,
    pub level: Level,
    pub seed: u64,
    /// How much of the log the agent has been told about.
    pub seen: usize,
}

/// One agent's game.
#[derive(Debug, Clone)]
pub struct Session {
    host: Host,
    you: u8,
    level: Level,
    seed: u64,
    seen: usize,
}

impl Session {
    /// Sets up a game as `new` asks and plays the bots' turns up to the agent's first.
    ///
    /// # Errors
    ///
    /// Fails on a name that isn't a mobile, map or level, a team size outside 1 to 4, or a map
    /// that can't seat everyone.
    pub fn new(new: &NewGame, maps: &Maps) -> Res<Self> {
        let size = new.team_size.unwrap_or(1);
        ensure!((1..=4).contains(&size), "teams are 1 to 4 players, not {size}");
        ensure!(new.against.len() <= usize::from(size), "{} opponents for a team of {size}", new.against.len());
        let level = new.level.as_deref().map(str::parse::<LevelName>).transpose()?.map_or(Level::Normal, |l| l.0);
        let seed = new.seed.unwrap_or_else(fresh_seed);
        let options = RoomOptions {
            team_size: size,
            ..RoomOptions::default()
        };
        let map = new.map.clone().unwrap_or_else(|| options.map.clone());
        let mut dice = XorShift::new(seed);
        let arena = referee::arena(maps.arenas(&map)?, &mut dice)?;

        let you = new.mobile.as_deref().map(mobile).transpose()?;
        let against: Vec<MobileKind> = new.against.iter().map(|name| mobile(name)).collect::<Res<_>>()?;
        let mut entrants = vec![Entrant {
            team: Team::A,
            mobile: you,
            items: bot::LOADOUT,
        }];
        entrants.extend((1..size).map(|_| Entrant {
            team: Team::A,
            mobile: None,
            items: bot::LOADOUT,
        }));
        entrants.extend((0..usize::from(size)).map(|i| Entrant {
            team: Team::B,
            mobile: against.get(i).copied(),
            items: bot::LOADOUT,
        }));
        let mut bots = vec![Some(level); entrants.len()];
        bots[0] = None;
        let host = Host::seat(&arena, &entrants, &options, bots, dice, maps)?;
        let mut session = Self { host, you: 0, level, seed, seen: 0 };
        session.play_bots()?;
        Ok(session)
    }

    /// Picks a saved game back up.
    ///
    /// # Errors
    ///
    /// Fails when its log doesn't replay.
    pub fn load(saved: SavedSession, maps: &Maps) -> Res<Self> {
        Ok(Self {
            host: Host::load(saved.host, maps)?,
            you: saved.you,
            level: saved.level,
            seed: saved.seed,
            seen: saved.seen,
        })
    }

    #[must_use]
    pub fn save(&self) -> SavedSession {
        SavedSession {
            host: self.host.saved().clone(),
            you: self.you,
            level: self.level,
            seed: self.seed,
            seen: self.seen,
        }
    }

    #[must_use]
    pub fn game(&self) -> &Game {
        self.host.game()
    }

    /// What the agent sees now, with what happened since it last looked.
    ///
    /// # Errors
    ///
    /// Fails only if the log no longer replays: a bug.
    pub fn view(&mut self, maps: &Maps) -> Res<View> {
        let happened = self.since_seen(maps)?;
        self.seen = self.host.log().events.len();
        Ok(view(self, &happened))
    }

    /// The agent's move. Then the bots play until it's the agent's turn again or the game ends.
    ///
    /// # Errors
    ///
    /// Fails when the game refuses the action (not your turn, an aim out of range, an item you
    /// can't use); nothing changes then.
    pub fn act(&mut self, action: Action) -> Res<()> {
        if let Phase::Over { .. } = self.game().phase() {
            anyhow::bail!("the game is over");
        }
        // Every agent turn takes the bot's think time (the module's note on clocks).
        let event = action.event(self.you, tuning::bot::THINK.value);
        self.host.commit(event).context("the game refused that")?;
        self.play_bots()
    }

    /// Where a shot would land if fired now, and whom it would hurt: the shot fired in a copy of
    /// the game, in the wind as it is, with the item at `item` armed. A Random column that still
    /// hides its kind isn't in the copy: the trace knows only what the agent can see. It fires the way the agent
    /// faces, as `Shoot` will; an agent turns first with a `Walk` of 0 steps, which is free.
    ///
    /// # Errors
    ///
    /// Fails when the shot couldn't be fired now: not your turn, an aim out of range, an SS that
    /// is locked, an item you can't use.
    pub fn trace(&self, request: &TraceRequest) -> Res<Trace> {
        let mut scratch = self.game().as_seen();
        let you = self.you;
        if let Some(slot) = request.item {
            scratch.apply(Event::UseItem {
                seat: you,
                slot,
                time: lobshot_sim::units::Centis(0),
            })?;
        }
        let power = Power::from_twentieths(request.power)?;
        let effects = scratch.apply(Event::Shoot {
            seat: you,
            shot: request.shot,
            aim: request.aim,
            power,
            time: lobshot_sim::units::Centis(0),
        })?;
        Ok(trace(self.game(), you, &effects))
    }

    /// Plays every bot turn up to the agent's next one. A dead agent's slot machine is SearchBot's
    /// to spin from then on.
    fn play_bots(&mut self) -> Res<()> {
        if !self.game().players()[usize::from(self.you)].alive {
            self.host.hand_to_bot(self.you, self.level);
        }
        self.host.run(TURN_CAP, &mut Vec::new())
    }

    /// The events since the agent last looked, with what they did: the log replayed.
    fn since_seen(&self, maps: &Maps) -> Res<Vec<Happened>> {
        let log = self.host.log();
        let mut game = Game::new(&log.setup, maps)?;
        let mut happened = Vec::new();
        for (i, &event) in log.events.iter().enumerate() {
            let effects = game.apply(event)?;
            if i >= self.seen {
                happened.push(Happened { event, effects });
            }
        }
        Ok(happened)
    }
}

/// A seed from the clock: an agent's game needn't be reproducible unless it asks for a seed.
fn fresh_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |since| u64::try_from(since.as_nanos() % u128::from(u64::MAX)).unwrap_or(1))
}

/// A shot to trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceRequest {
    pub shot: Shot,
    pub aim: i16,
    /// Twentieths of a bar, 0 to 80.
    pub power: u8,
    /// A loadout slot to arm first.
    #[serde(default)]
    pub item: Option<u8>,
}

/// Where a traced shot went.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trace {
    /// One line for the agent: what the shot would do, and where it comes down nearest the
    /// nearest enemy.
    pub summary: String,
    /// The way it fires: the way you face.
    pub facing: Facing,
    /// Where each projectile's flight ended, `[x, y]`; off the map when outside it.
    pub ends: Vec<[i32; 2]>,
    /// Every blast, `[x, y]`.
    pub blasts: Vec<[i32; 2]>,
    pub damage: Vec<Hurt>,
    /// Seats that would die.
    pub deaths: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Hurt {
    pub seat: u8,
    pub amount: i32,
    pub enemy: bool,
}

fn trace(game: &Game, you: u8, effects: &[Effect]) -> Trace {
    let players = game.players();
    let me = &players[usize::from(you)];
    let enemy = |seat: u8| players.get(usize::from(seat)).is_some_and(|p| p.team != me.team);
    let mut damage: Vec<Hurt> = Vec::new();
    let (mut ends, mut blasts, mut deaths) = (Vec::new(), Vec::new(), Vec::new());
    for effect in effects.iter().take_while(|e| !matches!(e, Effect::TurnStarted { .. } | Effect::Dropped { .. })) {
        match effect {
            Effect::Fired { legs, .. } | Effect::Beacon { legs, .. } | Effect::Helix { legs, .. } | Effect::Guide { legs, .. } | Effect::Orbit { legs, .. } => {
                if let Some(leg) = legs.last() {
                    ends.push(point(leg.flight.at(leg.to)));
                }
            }
            Effect::Exploded { at, .. } => blasts.push(point(*at)),
            Effect::Damaged { seat, amount } | Effect::Shielded { seat, amount } => match damage.iter_mut().find(|h| h.seat == *seat) {
                Some(hurt) => hurt.amount += amount,
                None => damage.push(Hurt {
                    seat: *seat,
                    amount: *amount,
                    enemy: enemy(*seat),
                }),
            },
            Effect::Died { seat, .. } => deaths.push(*seat),
            _ => {}
        }
    }
    let to_enemies: i32 = damage.iter().filter(|h| h.enemy).map(|h| h.amount).sum();
    let to_friends: i32 = damage.iter().filter(|h| !h.enemy).map(|h| h.amount).sum();
    let mut summary = if to_enemies > 0 {
        format!("Hits the enemy for {to_enemies}.")
    } else {
        "Hurts no enemy.".to_owned()
    };
    // Where it comes down nearest the nearest living enemy: a blast, or a flight that ended on the
    // map. Flights that end outside it left the map.
    let terrain = game.terrain();
    let on_map = |&[x, y]: &[i32; 2]| u32::try_from(x).is_ok_and(|x| x < terrain.width()) && u32::try_from(y).is_ok_and(|y| y < terrain.height());
    let landings: Vec<[i32; 2]> = blasts.iter().chain(ends.iter().filter(|end| on_map(end))).copied().collect();
    let target = (0_u8..)
        .zip(players)
        .filter(|&(seat, p)| p.alive && enemy(seat))
        .min_by_key(|(_, p)| (p.footing.x - me.footing.x).abs());
    let gap = |[x, y]: [i32; 2], p: &Player| -> f64 {
        let center = p.center();
        (f64::from(x) - center.x).hypot(f64::from(y) - center.y)
    };
    match (target, landings.first()) {
        (_, None) => summary.push_str(" Leaves the map."),
        (None, Some(&[x, y])) => {
            let _ = write!(summary, " Comes down at x={x}, y={y}.");
        }
        (Some((seat, p)), Some(_)) => {
            let nearest = landings.iter().copied().min_by(|&a, &b| gap(a, p).total_cmp(&gap(b, p))).unwrap_or_default();
            let [x, y] = nearest;
            let _ = write!(summary, " Comes down at x={x}, y={y}, {:.0} px from seat {seat}.", gap(nearest, p));
        }
    }
    if to_friends > 0 {
        let _ = write!(summary, " Costs your side {to_friends}.");
    }
    if !deaths.is_empty() {
        let _ = write!(summary, " Kills seat {}.", deaths.iter().map(u8::to_string).collect::<Vec<_>>().join(", seat "));
    }
    Trace {
        summary,
        facing: me.facing,
        ends,
        blasts,
        damage,
        deaths,
    }
}

fn point(at: lobshot_sim::units::Vec2) -> [i32; 2] {
    // Map coordinates fit an i32 many times over.
    #[allow(clippy::cast_possible_truncation)]
    [at.x.round() as i32, at.y.round() as i32]
}

/// What the agent sees: what a player sees on the HUD and the map, in numbers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct View {
    /// One line: whose turn it is, or how the game ended.
    pub status: String,
    pub you: u8,
    pub your_turn: bool,
    pub over: bool,
    /// The winning team once it's over; `None` for a draw (or still playing).
    pub winner: Option<Team>,
    /// Turns played so far.
    pub turn: i64,
    /// This game's seed: `new_game` with it plays the same game.
    pub seed: u64,
    pub map: MapView,
    pub wind: WindView,
    pub weather: WeatherView,
    pub sudden_death: Option<SuddenDeathView>,
    pub players: Vec<PlayerView>,
    /// Slot-machine drops still falling.
    pub drops: Vec<DropView>,
    /// What you may do, on your turn.
    pub options: Option<Options>,
    /// What happened since you last looked, in order.
    pub happened: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapView {
    pub name: String,
    pub variant: String,
    pub width: u32,
    pub height: u32,
    /// Every ground top (y, down from the top) in a column, top first, every `surface_step` px from
    /// x = 0: under an overhang, the floor comes after the roof. Empty where a column is open to the
    /// bottom: falling there is death.
    pub surfaces: Vec<Vec<i32>>,
    pub surface_step: u32,
    /// The map as text, top row first; `legend` says what each character is. Each character is
    /// `cell[0]` px wide and `cell[1]` tall.
    pub picture: Vec<String>,
    pub legend: &'static str,
    pub cell: [u32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WindView {
    pub strength: u8,
    /// Where it blows toward, degrees counterclockwise from screen-right: 90 blows up.
    pub direction: u16,
    /// Its push to the right (negative: left) and up (negative: down).
    pub across: f64,
    pub up: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WeatherView {
    /// The weather that came up last; Lock and Balm act while it is theirs.
    pub now: Option<WeatherKind>,
    /// Columns standing now.
    pub columns: Vec<WeatherSpot>,
    /// What comes next, front first; the front arrives in `turns_until_next` turns.
    pub coming: Vec<WeatherSpot>,
    pub turns_until_next: u32,
    /// The satellite that beams down while Skyeye is up.
    pub skyeye: SkyeyeView,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SkyeyeView {
    /// 1 to 6: it levels up with the damage its beams deal, all game.
    pub level: u8,
    pub xp: u32,
    /// Where it hovers (above the map's top, so `y` is negative).
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WeatherSpot {
    pub kind: WeatherKind,
    /// The column's center (no column for Wind, Lock or Balm).
    pub x: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SuddenDeathView {
    pub kind: &'static str,
    pub from_turn: u32,
    pub on: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlayerView {
    pub seat: u8,
    pub team: Team,
    pub mobile: &'static str,
    pub you: bool,
    pub alive: bool,
    /// Where it stands: the ground under it.
    pub x: i32,
    pub y: i32,
    pub facing: Facing,
    pub hp: i32,
    pub max_hp: i32,
    pub shield: i32,
    pub max_shield: i32,
    /// How much more damage Physical shots deal it, in percent: Frostfin's S2 and SS leave it
    /// brittle for the rest of the game.
    pub brittle: u8,
    /// Accumulated delay: the living player with the least moves next.
    pub delay: u32,
    /// Unused items, by loadout slot.
    pub items: Vec<SlotItem>,
    pub ss: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SlotItem {
    pub slot: u8,
    pub item: Item,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct DropView {
    pub kind: lobshot_sim::drops::DropKind,
    pub x: i32,
    /// It lands when the turn count reaches this.
    pub lands_at_turn: i64,
}

/// What the agent may do this turn.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Options {
    pub shots: Vec<ShotOption>,
    /// Items it may use now (one a turn).
    pub items: Vec<SlotItem>,
    /// Pixels it may still walk this turn.
    pub moves_left: u16,
    /// Your mobile's delay every turn, whatever it does.
    pub base_delay: u32,
    /// What a pass adds on top of the base delay.
    pub pass_delay: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShotOption {
    pub shot: Shot,
    pub can_fire: bool,
    /// Degrees above level, toward where it faces (90 is straight up).
    pub aim_min: i16,
    pub aim_max: i16,
    /// Full damage only from these angles; the rest of the range deals less.
    pub true_min: i16,
    pub true_max: i16,
    /// What firing it adds to your delay, before your mobile's base delay and items.
    pub delay: u32,
}

fn view(session: &Session, happened: &[Happened]) -> View {
    let game = session.game();
    let you = session.you;
    let (over, winner) = match game.phase() {
        Phase::Over { winner } => (true, winner),
        Phase::Turn { .. } => (false, None),
    };
    let your_turn = game.phase() == (Phase::Turn { seat: you }) && game.pending_roll().is_none();
    let my_team = game.players()[usize::from(you)].team;
    let status = match game.phase() {
        Phase::Over { winner: Some(team) } if team == my_team => "Game over: your team won.".to_owned(),
        Phase::Over { winner: Some(_) } => "Game over: your team lost.".to_owned(),
        Phase::Over { winner: None } => "Game over: a draw.".to_owned(),
        Phase::Turn { .. } if game.turns() >= TURN_CAP => format!("Stopped at the {TURN_CAP}-turn cap."),
        Phase::Turn { seat } if seat == you => "Your turn.".to_owned(),
        Phase::Turn { seat } => format!("Seat {seat}'s turn."),
    };
    let (across, up) = game.wind().components();
    let weather = game.weather();
    View {
        status,
        you,
        your_turn,
        over,
        winner,
        turn: game.turns(),
        seed: session.seed,
        map: map_view(game, &session.host.log().setup.map),
        wind: WindView {
            strength: game.wind().strength,
            direction: game.wind().direction,
            across: round2(across),
            up: round2(up),
        },
        weather: WeatherView {
            now: weather.now,
            columns: weather.columns.iter().map(|c| WeatherSpot { kind: c.kind, x: c.x }).collect(),
            coming: weather.queue.iter().map(|f| WeatherSpot { kind: f.kind, x: f.x }).collect(),
            turns_until_next: weather.turns_left,
            skyeye: {
                let at = Skyeye::at(game.terrain().width());
                SkyeyeView {
                    level: weather.skyeye.level(),
                    xp: weather.skyeye.xp,
                    x: at.x,
                    y: at.y,
                }
            },
        },
        sudden_death: session.host.log().setup.sudden_death.map(|sd| SuddenDeathView {
            kind: sd.kind.name(),
            from_turn: sd.turn,
            on: game.sudden_death().is_some(),
        }),
        players: (0_u8..).zip(game.players()).map(|(seat, p)| player_view(game, seat, p, you)).collect(),
        drops: game
            .drops()
            .iter()
            .map(|f| DropView {
                kind: f.drop.kind,
                x: f.drop.x,
                lands_at_turn: f.lands,
            })
            .collect(),
        options: your_turn.then(|| options(game, you)),
        happened: narrate(game, you, happened),
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn map_view(game: &Game, map: &MapRef) -> MapView {
    let terrain = game.terrain();
    let (width, height) = (terrain.width(), terrain.height());
    let (name, variant) = match map {
        MapRef::Asset { name, variant, .. } => (name.clone(), variant.clone()),
        MapRef::Flat { .. } => (lobshot_assets::FLAT.to_owned(), "A".to_owned()),
        MapRef::Hills { .. } => ("hills".to_owned(), "A".to_owned()),
    };
    let step = 50;
    let surfaces = (0..width.div_ceil(step))
        .map(|i| {
            let x = i32::try_from(i * step).unwrap_or(i32::MAX);
            (0..i32::try_from(height).unwrap_or(i32::MAX)).filter(|&y| terrain.is_surface(x, y)).collect()
        })
        .collect();
    let cell_w = width.div_ceil(PICTURE_COLUMNS).max(1);
    // Text characters are about twice as tall as wide.
    let cell_h = 2 * cell_w;
    let mut rows: Vec<Vec<char>> = (0..height.div_ceil(cell_h))
        .map(|row| {
            (0..width.div_ceil(cell_w))
                .map(|column| {
                    // Each cell's center, kept on the map at its far edges.
                    let (x, y) = ((column * cell_w + cell_w / 2).min(width - 1), (row * cell_h + cell_h / 2).min(height - 1));
                    let solid = terrain.is_solid(i32::try_from(x).unwrap_or(i32::MAX), i32::try_from(y).unwrap_or(i32::MAX));
                    if solid { '#' } else { '.' }
                })
                .collect()
        })
        .collect();
    let mut mark = |x: i32, y: i32, c: char| {
        let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else { return };
        if let Some(cell) = rows
            .get_mut(usize::try_from(y / cell_h).unwrap_or(usize::MAX))
            .and_then(|r| r.get_mut(usize::try_from(x / cell_w).unwrap_or(usize::MAX)))
        {
            *cell = c;
        }
    };
    for column in &game.weather().columns {
        let glyph = match column.kind {
            WeatherKind::Force => 'F',
            WeatherKind::Mirror => 'M',
            // Wind, Lock, Balm and Skyeye never stand as columns.
            WeatherKind::Tornado | WeatherKind::Wind | WeatherKind::Lock | WeatherKind::Balm | WeatherKind::Skyeye => 'T',
            WeatherKind::Electricity => 'E',
            WeatherKind::Drain => 'D',
            WeatherKind::Random => '?',
        };
        for y in (0..height).step_by(usize::try_from(cell_h).unwrap_or(1)) {
            mark(column.x, i32::try_from(y).unwrap_or(0), glyph);
        }
    }
    for (seat, player) in (0_u8..).zip(game.players()).filter(|(_, p)| p.alive) {
        let center = player.center();
        #[allow(clippy::cast_possible_truncation)]
        mark(center.x.round() as i32, center.y.round() as i32, char::from(b'0' + seat));
    }
    MapView {
        name,
        variant,
        width,
        height,
        surfaces,
        surface_step: step,
        picture: rows.into_iter().map(|row| row.into_iter().collect()).collect(),
        legend: "# ground, . air, 0-7 a living mobile by seat, and weather columns: F Force, M Mirror, T Tornado, E Electricity, D Drain, ? Random.",
        cell: [cell_w, cell_h],
    }
}

fn player_view(game: &Game, seat: u8, player: &Player, you: u8) -> PlayerView {
    let tuning = player.tuning();
    PlayerView {
        seat,
        team: player.team,
        mobile: tuning.name,
        you: seat == you,
        alive: player.alive,
        x: player.footing.x,
        y: player.footing.y,
        facing: player.facing,
        hp: player.hp,
        max_hp: tuning.hp.value,
        shield: player.shield,
        max_shield: tuning.shield.value,
        brittle: player.brittle,
        delay: player.clock.delay,
        items: slot_items(player, |_| true),
        ss: match game.ss_state(seat) {
            Ok(SsState::Ready) => "ready".to_owned(),
            Ok(SsState::Locked(turns)) => format!("locked for {turns} more of its turns"),
            Ok(SsState::Off(kind)) => format!("off for the rest of the game ({} sudden death)", kind.name()),
            Ok(SsState::Armed(item)) => format!("not with {} armed", item.name()),
            Err(_) => "unknown".to_owned(),
        },
    }
}

fn slot_items(player: &Player, keep: impl Fn(u8) -> bool) -> Vec<SlotItem> {
    (0_u8..)
        .zip(player.items)
        .filter_map(|(slot, item)| item.filter(|_| keep(slot)).map(|item| SlotItem { slot, item }))
        .collect()
}

fn options(game: &Game, you: u8) -> Options {
    let player = &game.players()[usize::from(you)];
    let shots = [Shot::S1, Shot::S2, Shot::Ss]
        .into_iter()
        .map(|shot| {
            let tuning = shot.tuning(player.tuning());
            let aim = tuning.aim.value;
            ShotOption {
                shot,
                can_fire: game.can_shoot(you, shot),
                aim_min: aim.min,
                aim_max: aim.max,
                true_min: aim.true_min,
                true_max: aim.true_max,
                delay: tuning.delay.value,
            }
        })
        .collect();
    Options {
        shots,
        items: slot_items(player, |slot| game.can_use_item(you, slot)),
        moves_left: player.moves_left,
        base_delay: player.tuning().base_delay.value,
        pass_delay: tuning::PASS_DELAY.value,
    }
}

/// What happened, a line an event, as a player watching would put it.
fn narrate(game: &Game, you: u8, happened: &[Happened]) -> Vec<String> {
    let names = Names { game, you };
    happened
        .iter()
        .filter_map(|h| {
            let mut line = match h.event {
                Event::Shoot { .. } => shot_line(&names, h),
                Event::Pass { seat, .. } => format!("{} passed.", names.who(seat)),
                Event::Forfeit { seat } => format!("{} forfeited.", names.who(seat)),
                _ => String::new(),
            };
            for effect in &h.effects {
                line.push_str(&effect_line(&names, effect));
            }
            let line = line.trim();
            (!line.is_empty()).then(|| line.to_owned())
        })
        .collect()
}

/// Who's who, as the agent reads it: "You", or a seat with its mobile and team.
struct Names<'a> {
    game: &'a Game,
    you: u8,
}

impl Names<'_> {
    fn who(&self, seat: u8) -> String {
        if seat == self.you {
            return "You".to_owned();
        }
        self.game
            .players()
            .get(usize::from(seat))
            .map_or_else(|| format!("Seat {seat}"), |p| format!("Seat {seat} ({}, team {:?})", p.tuning().name, p.team))
    }

    fn whose(&self, seat: u8) -> String {
        if seat == self.you { "Your".to_owned() } else { format!("{}'s", self.who(seat)) }
    }
}

/// A shot: who fired what, which way, and where it burst.
fn shot_line(names: &Names<'_>, h: &Happened) -> String {
    let Event::Shoot { seat, shot, aim, power, .. } = h.event else { return String::new() };
    let mut line = String::new();
    // Which way it went: the first projectile's start.
    let way = h.effects.iter().find_map(|e| match e {
        Effect::Fired { legs, .. } | Effect::Beacon { legs, .. } | Effect::Helix { legs, .. } | Effect::Orbit { legs, .. } => {
            legs.first().map(|leg| if leg.flight.velocity.x < 0.0 { "left" } else { "right" })
        }
        _ => None,
    });
    let _ = write!(
        line,
        "{} fired {shot:?}{} at {aim} degrees, power {} ({:.2} bars).",
        names.who(seat),
        way.map_or_else(String::new, |way| format!(" to the {way}")),
        power.twentieths(),
        power.bars()
    );
    let mut blasts: Vec<String> = h
        .effects
        .iter()
        .take_while(|e| !matches!(e, Effect::TurnStarted { .. } | Effect::Dropped { .. }))
        .filter_map(|e| match e {
            Effect::Exploded { at, .. } => {
                let [x, y] = point(*at);
                Some(format!("x={x}, y={y}"))
            }
            _ => None,
        })
        .collect();
    // A Dual's two shells often burst on the same spot.
    blasts.dedup();
    match blasts.as_slice() {
        [] => line.push_str(" Nothing burst: it left the map."),
        [one] => {
            let _ = write!(line, " It burst at {one}.");
        }
        [first, second, rest @ ..] => {
            let more = if rest.is_empty() { String::new() } else { format!(" and {} more", rest.len()) };
            let _ = write!(line, " It burst at {first}, {second}{more}.");
        }
    }
    line
}

/// What one effect did, as a clause; empty for the ones a player wouldn't remark on.
fn effect_line(names: &Names<'_>, effect: &Effect) -> String {
    match effect {
        Effect::Walked { seat, to, .. } => format!(" {} walked to x={}.", names.who(*seat), to.x),
        Effect::Damaged { seat, amount } => format!(" {} took {amount} damage.", names.who(*seat)),
        Effect::Shielded { seat, amount } => format!(" {} shield took {amount}.", names.whose(*seat)),
        Effect::Frosted { seat, brittle } => {
            let (is, them) = if *seat == names.you { ("are", "you") } else { ("is", "it") };
            format!(" {} {is} {brittle}% brittle: Physical shots deal {them} that much more.", names.who(*seat))
        }
        Effect::Healed { seat, amount } => format!(" {} healed {amount}.", names.who(*seat)),
        Effect::Died { seat, death } => format!(
            " {} died ({}).",
            names.who(*seat),
            match death {
                Death::Blasted => "out of HP",
                Death::Bunged => "fell off the map",
                Death::Forfeited => "forfeited",
            }
        ),
        Effect::ItemUsed { seat, item, .. } => format!(" {} used {}.", names.who(*seat), item.name()),
        Effect::Teleported { seat, to, .. } => format!(" {} teleported to x={}.", names.who(*seat), to.x),
        Effect::Swapped { seat, with } => format!(" {} swapped places with {}.", names.who(*seat), names.who(*with)),
        Effect::Slid { seat, path, .. } => path.last().map_or_else(String::new, |to| format!(" {} slid to x={}.", names.who(*seat), to.x)),
        Effect::WindChanged(wind) => format!(" The wind is now {} toward {} degrees.", wind.strength, wind.direction),
        Effect::WeatherChanged { columns, now, .. } => match now {
            Some(WeatherKind::Lock) => " Lock came up: no items, and no shields regenerate.".to_owned(),
            Some(WeatherKind::Balm) => " Balm came up: everyone heals as their turn starts.".to_owned(),
            Some(WeatherKind::Skyeye) => " Skyeye came up: its beam follows every projectile that lands.".to_owned(),
            _ if columns.is_empty() => " The weather cleared.".to_owned(),
            _ => {
                let standing: Vec<String> = columns.iter().map(|c| format!("{:?} at x={}", c.kind, c.x)).collect();
                format!(" Weather now: {}.", standing.join(", "))
            }
        },
        Effect::Revealed { kind, x } => format!(" The Random column at x={x} was {kind:?}."),
        #[allow(clippy::cast_possible_truncation)]
        Effect::Beam { to, .. } => format!(" Skyeye's beam struck at x={}, y={}.", to.x.round() as i32, to.y.round() as i32),
        Effect::SkyeyeLevel { level } => format!(" Skyeye reached level {level}."),
        Effect::Spun { seat } => format!(" {} spun the slot machine.", names.who(*seat)),
        Effect::DropRolled { drop, .. } => format!(" It stopped on {}, to fall at x={}.", drop.kind.name(), drop.x),
        Effect::Dropped { drop, .. } => format!(" {} fell at x={}.", drop.kind.name(), drop.x),
        Effect::SuddenDeath { kind } => format!(" Sudden death: {}. No more items.", kind.name()),
        Effect::GameOver { winner } => match winner {
            Some(team) => format!(" Game over: team {team:?} won."),
            None => " Game over: a draw.".to_owned(),
        },
        _ => String::new(),
    }
}

/// How to play, for an agent that has never seen the game.
#[must_use]
pub fn rules() -> String {
    let mut text = RULES
        .replace("{PASS}", &tuning::PASS_DELAY.value.to_string())
        .replace("{BALM}", &tuning::BALM_HEAL_PERCENT.value.to_string());
    text.push_str("\n## Mobiles\n");
    for mobile in MobileKind::ALL {
        let t = mobile.tuning();
        let _ = writeln!(text, "\n### {} (`{}`)\n", t.name, mobile_slug(mobile));
        let _ = writeln!(text, "{}", about(mobile));
        let _ = write!(text, "\nHP {}, {:?} armor", t.hp.value, t.armor.value);
        if t.shield.value > 0 {
            let _ = write!(text, ", shield {} (regains {} a turn)", t.shield.value, t.shield_regen.value);
        }
        let _ = writeln!(
            text,
            "; base delay {}, plus {} a second of turn time; walks {} px a turn.",
            t.base_delay.value, t.delay_per_second.value, t.move_budget.value
        );
        for shot in [Shot::S1, Shot::S2, Shot::Ss] {
            let s = shot.tuning(t);
            let aim = s.aim.value;
            let _ = writeln!(
                text,
                "- {shot:?}: delay {}, aim {} to {} (full damage {} to {}).",
                s.delay.value, aim.min, aim.max, aim.true_min, aim.true_max
            );
        }
    }
    text.push_str("\n## Items\n\n");
    for item in Item::ALL {
        let _ = writeln!(
            text,
            "- {} (`{item:?}`): {} Fills {} slot{}; adds {} delay.",
            item.name(),
            item_about(item),
            item.tuning().slots.value,
            if item.tuning().slots.value == 1 { "" } else { "s" },
            item.tuning().delay.value
        );
    }
    let _ = writeln!(text, "\nBots play at {} level unless the game says otherwise.", level_name(Level::Normal));
    text
}

const RULES: &str = "# lobshot, for agents

Turn-based artillery: two teams of mobiles (tanks) on a 2D map. A mobile dies when its HP runs out
or it falls off the bottom of the map. The last team standing wins.

## Coordinates

World pixels: x grows right, y grows DOWN from the top of the map. A player's x and y are where it
stands on the ground. The view's `picture` is the map as text, and `surface` gives the ground's top
along the map.

## Your turn

On your turn you may walk (it doesn't end the turn), use one item, then shoot or pass. Act with the
wire's own JSON actions:

- `{\"Walk\": {\"facing\": \"Left\", \"steps\": 40}}`: turn to face Left or Right and walk up to
  `steps` px (slopes and your remaining moves may stop you sooner). `steps: 0` just turns.
- `{\"Item\": {\"slot\": 0}}`: use the item in that loadout slot.
- `{\"Shoot\": {\"shot\": \"S1\", \"aim\": 45, \"power\": 48}}`: fire. `shot` is S1, S2 or Ss.
  `aim` is degrees above level toward where you face (90 is straight up), within the shot's range.
  `power` is in twentieths of a bar: 0 to 80 (the power bar is 4 bars long).
- `\"Pass\"`: end the turn without shooting.

Before you shoot, `trace` tells you where a shot would land and whom it would hurt in the wind as
it is now. It knows nothing you can't see. It fires the way you face, as `Shoot` does: to shoot the
other way, turn first with a `Walk` of 0 steps (turning is free). An item keeps its slot number
after the items before it are used.

## Delay: who moves next

Every turn adds to your delay: your mobile's base delay, the shot's delay (or a pass's, {PASS}),
any item's, and time. The living player with the least delay moves next, so a heavy shot or an item
makes you wait longer. Agent turns are charged a fixed time.

## Wind and weather

The wind pushes every shot; `wind.across` and `wind.up` give its push right and up. Weather
columns stand on the map: Force boosts shots through it, Drain halves them, Electricity calls a
bolt down wherever a shot through it lands, Mirror sends them back the way they came, Tornado
flings them back up. A Random column hides one of those five: a shot through it meets what it
hides, and it shows that from then on (`trace` can't see what it hides). A Wind weather rerolls
the wind. Lock and Balm cover the whole map while
they last (`weather.now`): under Lock no one may use an item, shields stop regenerating and SS
comes back twice as fast; under Balm each player heals {BALM}% of max HP as their turn starts
(Bionic mobiles double). While Skyeye is up, a satellite above the middle of the map
(`weather.skyeye`) beams down toward wherever each projectile lands: whoever or whatever is in the
beam's way takes it, you included. Its beams level it up (1 to 6) with the damage they deal, all
game. The view lists what is coming.

## SS, sudden death, the slot machine

SS is each mobile's special shot: after you fire it, it locks for a few of your turns. Sudden death
starts at a set turn: Double fires every S1 and S2 twice, Big Bomb makes bigger craters, SS Death
lets everyone fire SS every turn; items are off from then on. A dead player's slot machine drops
things onto the map (Dynamite, a Hammer, a Bolt, a healing Chest, or the wind dying or turning
around); if you die, it is spun for you.
";

fn about(mobile: MobileKind) -> String {
    match mobile {
        MobileKind::Coilback => "A sturdy mortar. S1 is one heavy round; S2 two warheads, the second carrying on through the first's crater; SS a shell that grows in flight and hits much harder.".to_owned(),
        MobileKind::Lanternwing => {
            "A moth mech with a shield. S1 is a laser bolt; S2 two strands winding about one path, each its own hit; SS a ball that strips every nearby mobile's shield where it lands.".to_owned()
        }
        MobileKind::Hookclaw => {
            "Throws boomerangs that the wind bends hard: one the wind turns around in flight deals more. S1 is one, S2 four in a cluster, SS one that turns golden if it stays up long enough.".to_owned()
        }
        MobileKind::Sparkhorn => format!(
            "Lands a ball that calls lightning onto the impact; ground above a target blocks its bolt. S1's bolt strikes straight down, S2's two come in a V from either side, SS sends one down onto every mobile in reach. An S1 or S2 bolt also splashes a flat {} onto each mobile near its path, once a turn, whatever its armor, the weather or items. Has a shield.",
            tuning::sparkhorn::SPLASH.value
        ),
        MobileKind::Marblemole => {
            "S1 is the weakest shot in the game; S2 four marbles that bounce, roll and go off; SS a ball that passes through mobiles, hurting each the whole time it touches, then blows up on the ground.".to_owned()
        }
        MobileKind::Spoutpot => {
            "A hippo with a teapot water tank. S1 is a stream of water; S2 two streams that spiral about one path, each its own hit: they cross again and again, and after long enough in the air they come together for good, so time a fork or give it airtime; SS a blob that opens into six balls in mid-flight, which fan out and fall: open it just short of the target and all six land. Pays 12 delay a second of turn time, not 10.".to_owned()
        }
        MobileKind::Tuskrack => {
            "Fires spreads of light projectiles, each its own blast, so long shots scatter and close ones pile up. S1 is four missiles leaving the rack one after another, the last two down the same line into one deep crater; S2 a bundle of six grenades that bursts into three pairs a beat after the muzzle (all six go off at once if it strikes first); SS nine missiles, and those that strike a mobile carve nothing. A heavy shot with a narrow aim; it moves far but climbs poorly.".to_owned()
        }
        MobileKind::Sudsotter => format!(
            "Fires sponges that fly through mobiles, stick where they land and walk along the ground, climbing walls and clinging under ledges, then go off {} s after landing wherever they are: S1's walk left on screen, S2's right, whichever way you face (about {:.0} px on level ground). SS is a blob of suds that walks the way it was moving as it landed, hurting every mobile it passes near, most on a small island it circles; a direct hit only pops it. The weather leaves its damage alone.",
            tuning::sudsotter::SPONGE_FUSE_SECONDS.value,
            tuning::sudsotter::WALK_SPEED.value * tuning::sudsotter::SPONGE_FUSE_SECONDS.value
        ),
        MobileKind::Frostfin => format!(
            "A narwhal on an ice sled, with the most HP of any mobile. S1 is an icicle from the cannon on its back that digs a wide hole; S2 a ball of ice from its tusk, a lower muzzle that can jam in a wall the cannon clears; SS a snowflake from the tusk with a much wider blast. Each S2 or SS leaves every mobile it hurts {}% more brittle, up to {}%, for the rest of the game: Physical shots, anyone's, deal it that much more (`brittle` in the view).",
            tuning::frostfin::BRITTLE_PER_HIT.value,
            tuning::frostfin::BRITTLE_MAX.value
        ),
        MobileKind::Herdhound => {
            "Each shot is a warhead that passes through mobiles and opens at the top of its arc: a guide flies on along the shot's path, and missiles loop for a second, then home on it and strike where it lands. Opened too low, they crash first; a warhead that hits the ground before it opens is a dud. S1 opens into two missiles, S2 four, SS four that split into eight, each with a guide of its own fanned about the path.".to_owned()
        }
        MobileKind::Thistleback => {
            "Heavy shots. S1 is one cabbage; S2 a cabbage with two sprouts circling it clockwise, each its own hit (a tornado stops their circling); SS a pumpkin that bursts nine times in a spinning ring where it lands, digging a deep hole, and hurts most when it lands at a target's feet.".to_owned()
        }
        MobileKind::Lodestoad => format!(
            "A toad on a magnet tractor, with a shield. S1 is an electric orb whose discharge deals a flat {} to every mobile within {:.0} px of its blast, once a turn even with Dual. S2 is a magnet ball that flies through mobiles to the ground, digs a hole and pulls every mobile whose middle is within {:.0} px across and {:.0} px up or down of it (yours and you too) toward that spot along the ground: Mechanical ones all the way, Shield ones half, Bionic ones a quarter. It drags them over edges, into its hole, and off the map if nothing is below, and hurts most bursting at a mobile's feet. SS is a pyramid that pushes every mobile whose middle is within {:.0} px across away along the ground to {:.0} px from where it struck (Bionic ones a quarter as far), hurting most at its center and more each one a wall or a cliff's edge stops short; it never pushes anyone over an edge. Walls stop both.",
            tuning::lodestoad::SPLASH.value,
            tuning::lodestoad::SPLASH_REACH.value,
            tuning::lodestoad::PULL_REACH.value.0,
            tuning::lodestoad::PULL_REACH.value.1,
            tuning::lodestoad::PUSH_REACH.value.0,
            tuning::lodestoad::PUSH_TO.value
        ),
    }
}

fn item_about(item: Item) -> String {
    use tuning::items::{BIG_HEAL_PERCENT, BIONIC_HEAL_EXTRA, DAMAGE_BOOST, DIG_CRATER, HEAL_PERCENT, SACRIFICE_PERCENT};
    let heal = |percent: u32| format!("Heals {percent}% of your max HP at once ({}% for a Bionic mobile).", percent + BIONIC_HEAL_EXTRA.value);
    match item {
        Item::Dual => "Your S1 or S2 fires twice.".to_owned(),
        Item::DualPlus => "Your shot, then the other of S1 and S2.".to_owned(),
        Item::Teleport => "The shot hurts no one and carries you to where it lands.".to_owned(),
        Item::PowerUp => format!("This turn's shot deals {}x damage; a bolt's splash stays flat.", DAMAGE_BOOST.value),
        Item::Sacrifice => format!(
            "Pay {}% of your max HP for {}x damage this turn; a bolt's splash stays flat.",
            SACRIFICE_PERCENT.value, DAMAGE_BOOST.value
        ),
        Item::Dig => format!("This turn's shot carves {}x craters.", DIG_CRATER.value),
        Item::Heal => heal(HEAL_PERCENT.value),
        Item::BigHeal => heal(BIG_HEAL_PERCENT.value),
        Item::WindFlip => "The wind turns around, at once.".to_owned(),
        Item::Thunder => "A bolt strikes wherever this turn's shot lands.".to_owned(),
        Item::Swap => "Trade places with your weakest living teammate; it ends your turn.".to_owned(),
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use pretty_assertions::assert_eq;

    use lobshot_sim::units::Centis;

    use super::*;

    fn maps() -> Maps {
        Maps::new(Path::new("no-assets"))
    }

    fn flat(seed: u64) -> NewGame {
        NewGame {
            mobile: Some("coilback".into()),
            against: vec!["sparkhorn".into()],
            map: Some("flat".into()),
            level: Some("hard".into()),
            seed: Some(seed),
            ..NewGame::default()
        }
    }

    /// The shot that hurts the enemy most at aim 45, by tracing every power.
    fn best_power(session: &Session) -> Option<u8> {
        (0..=80)
            .step_by(2)
            .filter_map(|power| {
                let trace = session
                    .trace(&TraceRequest {
                        shot: Shot::S1,
                        aim: 45,
                        power,
                        item: None,
                    })
                    .ok()?;
                let hurt: i32 = trace.damage.iter().filter(|h| h.enemy).map(|h| h.amount).sum();
                (hurt > 0).then_some((hurt, power))
            })
            .max()
            .map(|(_, power)| power)
    }

    /// Faces the nearest enemy.
    fn face(session: &Session) -> Facing {
        let players = session.game().players();
        let me = players[0];
        let enemy = players.iter().filter(|p| p.alive && p.team != me.team).min_by_key(|p| (p.footing.x - me.footing.x).abs()).unwrap();
        if enemy.footing.x > me.footing.x { Facing::Right } else { Facing::Left }
    }

    #[test]
    fn a_new_game_starts_on_the_agents_turn_with_what_it_needs_to_see() {
        let mut session = Session::new(&flat(3), &maps()).unwrap();
        let view = session.view(&maps()).unwrap();
        assert!(view.your_turn && !view.over, "{}", view.status);
        assert_eq!(view.status, "Your turn.");
        assert_eq!(view.seed, 3);
        assert_eq!((view.players.len(), view.players[0].mobile, view.players[1].mobile), (2, "Coilback", "Sparkhorn"));
        assert!(view.players[0].you && !view.players[1].you);
        assert_eq!(view.map.name, "flat");
        assert_eq!(view.map.surfaces[0], [700]);
        assert_eq!(view.map.picture.len(), usize::try_from(900_u32.div_ceil(view.map.cell[1])).unwrap());
        assert!(view.map.picture.iter().any(|row| row.contains('0')) && view.map.picture.iter().any(|row| row.contains('1')));
        assert!(view.map.picture.last().unwrap().chars().all(|c| c == '#'), "the ground fills the bottom");
        let options = view.options.unwrap();
        assert_eq!(options.shots.len(), 3);
        assert!(options.shots[0].can_fire);
        assert_eq!(options.items.len(), 4, "the bot loadout");
        // Looking again tells nothing new.
        assert_eq!(session.view(&maps()).unwrap().happened, Vec::<String>::new());
    }

    #[test]
    fn a_trace_is_what_the_shot_then_does() {
        let mut session = Session::new(&flat(5), &maps()).unwrap();
        let facing = face(&session);
        session.act(Action::Walk { facing, steps: 0 }).unwrap();
        let power = best_power(&session).expect("a flat duel has a shot that hits");
        let traced = session
            .trace(&TraceRequest {
                shot: Shot::S1,
                aim: 45,
                power,
                item: None,
            })
            .unwrap();
        assert!(traced.summary.starts_with("Hits the enemy for"), "{}", traced.summary);
        let traced_hurt: i32 = traced.damage.iter().filter(|h| h.seat == 1).map(|h| h.amount).sum();
        session.seen = session.host.log().events.len();
        session
            .act(Action::Shoot {
                shot: Shot::S1,
                aim: 45,
                power: Power::from_twentieths(power).unwrap(),
            })
            .unwrap();
        let shot = session.since_seen(&maps()).unwrap().into_iter().find(|h| matches!(h.event, Event::Shoot { seat: 0, .. })).unwrap();
        let hurt: i32 = shot
            .effects
            .iter()
            .take_while(|e| !matches!(e, Effect::TurnStarted { .. }))
            .filter_map(|e| match *e {
                Effect::Damaged { seat: 1, amount } | Effect::Shielded { seat: 1, amount } => Some(amount),
                _ => None,
            })
            .sum();
        assert_eq!(hurt, traced_hurt, "the trace said what the shot would do");
        let view = session.view(&maps()).unwrap();
        assert!(
            view.happened.iter().any(|line| line.starts_with("You fired S1 to the ") && line.contains(" at 45 degrees")),
            "{:?}",
            view.happened
        );
    }

    #[test]
    fn traces_change_nothing_and_refuse_what_the_game_would() {
        let session = Session::new(&flat(5), &maps()).unwrap();
        let before = session.game().state_hash().unwrap();
        session
            .trace(&TraceRequest {
                shot: Shot::S1,
                aim: 45,
                power: 40,
                item: Some(0),
            })
            .unwrap();
        assert_eq!(session.game().state_hash().unwrap(), before);
        assert!(
            session
                .trace(&TraceRequest {
                    shot: Shot::S1,
                    aim: 120,
                    power: 40,
                    item: None
                })
                .is_err(),
            "out of range"
        );
        assert!(
            session
                .trace(&TraceRequest {
                    shot: Shot::S1,
                    aim: 45,
                    power: 81,
                    item: None
                })
                .is_err(),
            "above four bars"
        );
    }

    #[test]
    fn an_agent_plays_a_whole_game_and_a_saved_one_picks_up() {
        let mut session = Session::new(&flat(9), &maps()).unwrap();
        let mut view = session.view(&maps()).unwrap();
        let mut told = view.happened.clone();
        let mut acts = 0;
        while !view.over {
            acts += 1;
            assert!(acts < 100, "the game should end");
            let facing = face(&session);
            session.act(Action::Walk { facing, steps: 0 }).unwrap();
            let action = best_power(&session).map_or(Action::Pass, |power| Action::Shoot {
                shot: Shot::S1,
                aim: 45,
                power: Power::from_twentieths(power).unwrap(),
            });
            // Saved and loaded between every move, as the CLI does.
            let saved = serde_json::to_string(&session.save()).unwrap();
            session = Session::load(serde_json::from_str(&saved).unwrap(), &maps()).unwrap();
            session.act(action).unwrap();
            view = session.view(&maps()).unwrap();
            told.extend(view.happened.iter().cloned());
        }
        assert!(view.status.starts_with("Game over"), "{}", view.status);
        assert!(told.last().unwrap().contains("Game over"), "{told:?}");
        assert!(
            told.iter().any(|line| line.starts_with("You fired")) && told.iter().any(|line| line.starts_with("Seat 1 (Sparkhorn, team B) fired")),
            "{told:?}"
        );
        assert!(session.act(Action::Pass).is_err(), "nothing to do once it's over");
    }

    #[test]
    fn the_game_refuses_what_a_player_couldnt_do() {
        let mut session = Session::new(&flat(5), &maps()).unwrap();
        let before = session.save();
        let error = session
            .act(Action::Shoot {
                shot: Shot::S1,
                aim: 120,
                power: Power::MAX,
            })
            .unwrap_err();
        assert!(format!("{error:#}").contains("can't aim"), "{error:#}");
        assert_eq!(session.save(), before, "a refused action changes nothing");
    }

    #[test]
    fn agent_turns_take_the_bots_think_time() {
        let mut session = Session::new(&flat(5), &maps()).unwrap();
        session.act(Action::Pass).unwrap();
        let mine = session.host.log().events.iter().find_map(|e| match *e {
            Event::Pass { seat: 0, time } => Some(time),
            _ => None,
        });
        assert_eq!(mine, Some(tuning::bot::THINK.value));
        assert_ne!(mine, Some(Centis(0)));
    }

    #[test]
    fn setups_are_checked() {
        for bad in [
            NewGame { mobile: Some("pip".into()), ..flat(1) },
            NewGame { map: Some("nowhere".into()), ..flat(1) },
            NewGame { team_size: Some(5), ..flat(1) },
            NewGame { level: Some("great".into()), ..flat(1) },
            NewGame {
                against: vec!["coilback".into(); 2],
                ..flat(1)
            },
        ] {
            assert!(Session::new(&bad, &maps()).is_err(), "{bad:?}");
        }
        let four = Session::new(
            &NewGame {
                team_size: Some(4),
                against: Vec::new(),
                ..flat(1)
            },
            &maps(),
        )
        .unwrap();
        assert_eq!(four.game().players().len(), 8);
        assert!(serde_json::from_str::<NewGame>(r#"{"mobel": "x"}"#).is_err(), "unknown fields are refused");
    }

    #[test]
    fn the_view_shows_floors_under_overhangs_and_what_a_pass_costs() {
        let root = tempfile::TempDir::new().unwrap();
        // Ground from y = 700, and a roof slab over the middle.
        let ledge = lobshot_sim::terrain::Terrain::from_fn(1600, 900, |x, y| y >= 700 || ((600..1000).contains(&x) && (300..340).contains(&y))).unwrap();
        lobshot_assets::tests::write_map_variants(root.path(), "ledge", &[("A", &ledge, &[200, 1400])]).unwrap();
        let maps = Maps::new(root.path());
        let mut session = Session::new(&NewGame { map: Some("ledge".into()), ..flat(2) }, &maps).unwrap();
        let view = session.view(&maps).unwrap();
        assert_eq!(view.map.surfaces[800 / 50], [300, 700], "the roof, then the floor");
        assert_eq!(view.map.surfaces[0], [700]);
        assert!(view.map.legend.contains("M Mirror"));
        let options = view.options.unwrap();
        assert_eq!((options.base_delay, options.pass_delay), (MobileKind::Coilback.tuning().base_delay.value, tuning::PASS_DELAY.value));
    }

    #[test]
    fn traces_fire_the_way_you_face_and_say_how_close_a_miss_came() {
        let session = Session::new(&flat(5), &maps()).unwrap();
        let facing = session.game().players()[0].facing;
        let miss = session
            .trace(&TraceRequest {
                shot: Shot::S1,
                aim: 50,
                power: 8,
                item: None,
            })
            .unwrap();
        assert_eq!(miss.facing, facing);
        assert!(miss.summary.starts_with("Hurts no enemy. Comes down at x="), "{}", miss.summary);
        assert!(miss.summary.contains("px from seat 1."), "{}", miss.summary);
    }

    #[test]
    fn the_story_reads_right_and_says_where_shots_burst() {
        let shot = Happened {
            event: Event::Shoot {
                seat: 1,
                shot: Shot::S1,
                aim: 40,
                power: Power::MAX,
                time: Centis(0),
            },
            effects: vec![
                Effect::Exploded {
                    at: lobshot_sim::units::Vec2::new(410.4, 650.0),
                    radius: 30.0,
                    tick: 3,
                    dirty: None,
                },
                Effect::Shielded { seat: 0, amount: 62 },
                Effect::Damaged { seat: 0, amount: 20 },
            ],
        };
        let session = Session::new(&flat(5), &maps()).unwrap();
        let lines = narrate(session.game(), 0, &[shot]);
        assert_eq!(
            lines,
            ["Seat 1 (Sparkhorn, team B) fired S1 at 40 degrees, power 80 (4.00 bars). It burst at x=410, y=650. Your shield took 62. You took 20 damage."]
        );
    }

    #[test]
    fn the_rules_cover_every_mobile_and_item() {
        let rules = rules();
        for mobile in MobileKind::ALL {
            assert!(rules.contains(&format!("### {}", mobile.tuning().name)), "{mobile:?}");
        }
        for item in Item::ALL {
            assert!(rules.contains(&format!("- {} (`{item:?}`)", item.name())), "{item:?}");
        }
        assert!(rules.contains("- Big Heal (`BigHeal`): Heals 25% of your max HP at once (30% for a Bionic mobile)."), "{rules}");
        assert!(rules.contains("Bionic armor") && rules.contains("a pass's, 200"), "{rules}");
    }

    #[test]
    fn a_frosted_mobile_is_brittle_in_the_log_and_the_view() {
        let mut new = flat(3);
        new.against = vec!["frostfin".into()];
        let session = Session::new(&new, &maps()).unwrap();
        let names = Names { game: session.game(), you: 0 };
        assert_eq!(
            effect_line(&names, &Effect::Frosted { seat: 0, brittle: 10 }),
            " You are 10% brittle: Physical shots deal you that much more."
        );
        assert_eq!(
            effect_line(&names, &Effect::Frosted { seat: 1, brittle: 5 }),
            " Seat 1 (Frostfin, team B) is 5% brittle: Physical shots deal it that much more."
        );
        assert!(rules().contains("5% more brittle, up to 50%"), "{}", rules());
        // The view reports what the player carries (the Frostfin bot may have frosted it already).
        let mut player = session.game().players()[0];
        player.brittle = 0;
        assert_eq!(player_view(session.game(), 0, &player, 0).brittle, 0);
        player.brittle = 15;
        assert_eq!(player_view(session.game(), 0, &player, 0).brittle, 15);
    }

    #[test]
    fn lock_and_balm_are_in_the_rules_and_the_log() {
        assert!(rules().contains("each player heals 3% of max HP as their turn starts"));
        let session = Session::new(&flat(1), &maps()).unwrap();
        let names = Names { game: session.game(), you: 0 };
        let lock = Effect::WeatherChanged {
            columns: Vec::new(),
            wind_owed: false,
            now: Some(WeatherKind::Lock),
        };
        assert_eq!(effect_line(&names, &lock), " Lock came up: no items, and no shields regenerate.");
    }

    #[test]
    fn the_view_shows_skyeye_and_the_log_its_beams() {
        let mut session = Session::new(&flat(1), &maps()).unwrap();
        let view = session.view(&maps()).unwrap();
        assert_eq!((view.weather.skyeye.level, view.weather.skyeye.x), (1, f64::from(session.game().terrain().width()) / 2.0));
        let names = Names { game: session.game(), you: 0 };
        let beam = Effect::Beam {
            from: lobshot_sim::units::Vec2::new(900.0, -100.0),
            to: lobshot_sim::units::Vec2::new(812.4, 455.6),
            tick: 0,
            level: 1,
        };
        assert_eq!(effect_line(&names, &beam), " Skyeye's beam struck at x=812, y=456.");
        assert!(rules().contains("While Skyeye is up"));
    }

    #[test]
    fn the_rules_say_a_bolts_splash_stays_flat() {
        let rules = rules();
        assert!(rules.contains("splashes a flat 76"), "{rules}");
        assert!(rules.contains("- Power Up (`PowerUp`): This turn's shot deals 1.33x damage; a bolt's splash stays flat."), "{rules}");
    }
}
