//! Practice against SearchBot, with no server: the client referees itself, answering the sim's
//! rolls, stamping turn times, timing turns out, and playing the bots' turns (and their spins). You
//! play with your picks; 1v1 against a bot, or 2v2 with a bot beside you.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{Action, Member, MemberId, RoomCode, RoomOptions, RoomView, Stage};
use lobshot_sim::{
    bot::{self, Level},
    drops::{Drop, DropKind},
    game::{Event, Game, MobileKind, Roll, Seat, Setup, Team},
    log::Log,
    map::{MapRef, NoAssets},
    units::{Centis, Facing, Wind},
    weather::{Forecast, WeatherKind},
};

use super::{
    input::Act,
    playback::{Timeline, TurnClock},
};
use crate::{
    model::{ApplyEvents, Match, Played, Screen, Session},
    picks::Picks,
};

const ME: MemberId = MemberId(1);
/// The bot's think time, so its turn reads as a turn.
const BOT_THINK: f64 = 1.2;

/// The practice referee's own dice.
#[derive(Resource)]
struct Referee {
    rng: u64,
}

impl Referee {
    fn roll(&mut self, below: u32) -> u32 {
        // xorshift64; a game's dice need no more.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        u32::try_from(self.rng % u64::from(below.max(1))).unwrap_or(0)
    }
}

/// The practice field's spawn columns by team, nearest the middle last.
const SPAWNS: [[i32; 2]; 2] = [[360, 700], [1440, 1100]];

/// Sets up a practice game on rolling hills: you (seat 0) and any bot teammate on team A, bots on
/// team B. You play your picks; the bots pick their own mobiles and bring SearchBot's items, as the
/// server's do.
fn start(mut session: ResMut<Session>, picks: Res<Picks>, mut commands: Commands) {
    if !session.practice {
        return;
    }
    // Seeded from the browser's dice; practice needs no reproducibility.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let seed = (js_sys::Math::random() * 9.0e15) as u64 | 1;
    let mut referee = Referee { rng: seed };
    let size = session.practice_team_size.clamp(1, 2);
    let name = crate::web::load("name").unwrap_or_else(|| "You".to_owned());

    let mut seats = Vec::new();
    let mut members = Vec::new();
    for (team, spawns, facing) in [(Team::A, SPAWNS[0], Facing::Right), (Team::B, SPAWNS[1], Facing::Left)] {
        for &spawn_x in &spawns[..usize::from(size)] {
            let id = MemberId(u32::try_from(members.len()).unwrap_or(0) + 1);
            let mine = id == ME;
            let mobile = match picks.mobile {
                Some(mobile) if mine => mobile,
                _ => MobileKind::ALL[referee.roll(u32::try_from(MobileKind::ALL.len()).unwrap_or(1)) as usize],
            };
            let items = if mine { picks.loadout } else { bot::LOADOUT };
            seats.push(Seat { team, mobile, spawn_x, facing, items });
            members.push(Member {
                id,
                name: match (mine, size) {
                    (true, _) => name.clone(),
                    (false, 1) => "SearchBot".to_owned(),
                    (false, _) => format!("SearchBot {}", id.0 - 1),
                },
                team: Some(team),
                mobile: Some(mobile),
                items,
                rider: "pip".to_owned(),
                ready: true,
                bot: !mine,
                connected: true,
            });
        }
    }
    let ids: Vec<MemberId> = members.iter().map(|member| member.id).collect();
    let options = RoomOptions {
        map: "hills".to_owned(),
        team_size: size,
        ..RoomOptions::default()
    };
    session.me = Some(ME);
    session.seats.clone_from(&ids);
    session.room = Some(RoomView {
        code: RoomCode::from_indices([0; RoomCode::LEN]),
        master: ME,
        options: options.clone(),
        members,
        stage: Stage::Playing { seats: ids },
    });

    let map = MapRef::Hills {
        width: 1800,
        height: 1000,
        ground: 840,
        // Slopes up to about 23 degrees: GunBound ground a mobile can fight from.
        amplitude: 120,
        period: 900,
    };
    let order = order(&mut referee, size);
    let setup = Setup {
        map,
        seats,
        order,
        wind: wind(&mut referee, options.max_wind),
        sudden_death: options.sudden_death,
    };
    let Ok(game) = Game::new(&setup, &NoAssets) else {
        error!("the practice setup is invalid");
        return;
    };
    commands.insert_resource(Match::new(Log { setup, events: Vec::new() }, game));
    commands.insert_resource(referee);
}

/// A starting order with the teams alternating, as a server rolls it: a random team first, each
/// team's seats shuffled. Team A holds seats `0..size`, team B the rest.
fn order(referee: &mut Referee, size: u8) -> Vec<u8> {
    let mut teams: [Vec<u8>; 2] = [(0..size).collect(), (size..2 * size).collect()];
    for team in &mut teams {
        // Fisher-Yates.
        for i in (1..team.len()).rev() {
            let j = referee.roll(u32::try_from(i + 1).unwrap_or(1)) as usize;
            team.swap(i, j);
        }
    }
    if referee.roll(2) == 1 {
        teams.swap(0, 1);
    }
    (0..usize::from(size)).flat_map(|round| [teams[0][round], teams[1][round]]).collect()
}

fn wind(referee: &mut Referee, max: u8) -> Wind {
    let strength = u8::try_from(referee.roll(u32::from(max) + 1)).unwrap_or(0);
    let direction = u16::try_from(referee.roll(360)).unwrap_or(0);
    Wind { strength, direction }
}

/// Applies an event as the referee and plays its effects.
fn decide(game: &mut Match, event: Event, played: &mut MessageWriter<Played>) {
    match game.referee(event) {
        Ok(effects) => {
            played.write(Played { event, effects });
        }
        Err(error) => warn!("practice refused {event:?}: {error:#}"),
    }
}

/// When things happen.
#[derive(SystemParam)]
struct Clocks<'w> {
    timeline: Res<'w, Timeline>,
    clock: Res<'w, TurnClock>,
    time: Res<'w, Time>,
}

fn referee(session: Res<Session>, referee: Option<ResMut<Referee>>, game: Option<ResMut<Match>>, clocks: Clocks, mut acts: MessageReader<Act>, mut played: MessageWriter<Played>) {
    let Clocks { timeline, clock, time } = clocks;
    let (true, Some(mut referee), Some(mut game)) = (session.practice, referee, game) else { return };
    // Answer whatever the sim owes first.
    while let Some(roll) = game.confirmed.pending_roll() {
        let event = match roll {
            Roll::Weather { width } => {
                let kind = WeatherKind::ALL[referee.roll(4) as usize];
                Event::Weather(Forecast {
                    kind,
                    x: i32::try_from(referee.roll(width)).unwrap_or(0),
                })
            }
            Roll::Wind => Event::Wind(wind(&mut referee, 26)),
            Roll::Drop { width } => Event::Drop(Drop {
                kind: DropKind::ALL[referee.roll(u32::try_from(DropKind::ALL.len()).unwrap_or(1)) as usize],
                x: i32::try_from(referee.roll(width)).unwrap_or(0),
            }),
        };
        decide(&mut game, event, &mut played);
    }
    let (Some(seat), Some(mine)) = (super::world::current_seat(&game), session.my_seat()) else {
        return;
    };
    let elapsed = time.elapsed_secs_f64() - clock.started;
    // Turn time in hundredths, as a server would stamp it.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let centis = Centis((elapsed.max(0.0) * 100.0) as u32);

    for Act(action) in acts.read() {
        let event = match *action {
            // The slot machine works on anyone's turn.
            Action::Spin => Event::Spin { seat: mine },
            _ if mine != seat => continue,
            Action::Walk { facing, steps } => Event::Walk { seat, facing, steps },
            Action::Shoot { shot, aim, power } => Event::Shoot { seat, shot, aim, power, time: centis },
            Action::Pass => Event::Pass { seat, time: centis },
            Action::Item { slot } => Event::UseItem { seat, slot },
        };
        decide(&mut game, event, &mut played);
    }
    if timeline.busy() {
        return;
    }
    // Dead bots spin whenever the sim lets them, as the server's do.
    for bot in (0..u8::try_from(game.confirmed.players().len()).unwrap_or(0)).filter(|&bot| bot != mine) {
        if game.confirmed.can_spin(bot) {
            decide(&mut game, Event::Spin { seat: bot }, &mut played);
        }
    }

    if seat == mine {
        if clock.seat == Some(seat) && elapsed >= 20.0 {
            decide(&mut game, Event::Pass { seat, time: Centis(2000) }, &mut played);
        }
        return;
    }
    // A bot's turn, once the last shot has played out and it has "thought".
    if clock.seat != Some(seat) || elapsed < BOT_THINK {
        return;
    }
    let Some(events) = bot::plan(&game.confirmed, seat, Level::Normal, u64::from(referee.roll(u32::MAX))) else {
        return;
    };
    info!("SearchBot plays {events:?}");
    for event in events {
        let event = if let Event::Shoot { seat, shot, aim, power, .. } = event {
            Event::Shoot { seat, shot, aim, power, time: centis }
        } else {
            event
        };
        decide(&mut game, event, &mut played);
    }
}

fn finish(mut session: ResMut<Session>, mut commands: Commands) {
    if session.practice {
        session.practice = false;
        session.room = None;
        commands.remove_resource::<Referee>();
        commands.remove_resource::<Match>();
    }
}

pub struct PracticePlugin;

impl Plugin for PracticePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Game), start)
            .add_systems(OnExit(Screen::Game), finish)
            .add_systems(Update, referee.in_set(ApplyEvents).run_if(in_state(Screen::Game)));
    }
}
