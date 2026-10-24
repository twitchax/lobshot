//! Practice against SearchBot, with no server: the client referees itself, answering the sim's
//! rolls, stamping turn times, timing turns out, and playing the bots' turns (and, dead, their spins
//! or drop points). You play with your picks; 1v1 against a bot, or 2v2 with a bot beside you, in
//! the mode the title set.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{Action, Member, MemberId, RoomCode, RoomOptions, RoomView, Stage};
use lobshot_sim::{
    bot::{self, Level},
    game::{Event, Game, MobileKind, Seat, Setup, Team},
    log::Log,
    map::{MapRef, NoAssets},
    referee::{self, Dice, XorShift},
    tuning,
    units::{Centis, Facing},
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

/// The practice referee: its own dice, and the room's max wind for the rolls it answers.
#[derive(Resource)]
struct Referee {
    dice: XorShift,
    max_wind: u8,
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
    let mut dice = XorShift::new(seed);
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
                _ => MobileKind::ALL[dice.below(u32::try_from(MobileKind::ALL.len()).unwrap_or(1)) as usize],
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
        mode: session.practice_mode,
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
    let teams: Vec<Team> = seats.iter().map(|seat| seat.team).collect();
    let setup = Setup {
        map,
        seats,
        order: referee::turn_order(&teams, &mut dice),
        wind: referee::wind(options.max_wind, &mut dice),
        sudden_death: options.sudden_death,
        mode: options.mode,
    };
    let Ok(game) = Game::new(&setup, &NoAssets) else {
        error!("the practice setup is invalid");
        return;
    };
    commands.insert_resource(Match::new(Log { setup, events: Vec::new() }, game));
    commands.insert_resource(Referee { dice, max_wind: options.max_wind });
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
        let event = referee::answer(roll, referee.max_wind, &mut referee.dice);
        decide(&mut game, event, &mut played);
    }
    let (Some(seat), Some(mine)) = (super::world::current_seat(&game), session.my_seat()) else {
        return;
    };
    let elapsed = time.elapsed_secs_f64() - clock.started;
    // Turn time in hundredths, as a server would stamp it.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let centis = Centis((elapsed.max(0.0) * 100.0) as u32);

    for &Act(action) in acts.read() {
        // The dead spin, or pick where to drop back in, on anyone's turn; everything else waits for
        // yours.
        if !matches!(action, Action::Spin | Action::Respawn { .. }) && mine != seat {
            continue;
        }
        decide(&mut game, action.event(mine, centis), &mut played);
    }
    if timeline.busy() {
        return;
    }
    // Dead bots act whenever the sim lets them, as the server's do.
    for bot in (0..u8::try_from(game.confirmed.players().len()).unwrap_or(0)).filter(|&bot| bot != mine) {
        if let Some(event) = bot::while_dead(&game.confirmed, bot) {
            decide(&mut game, event, &mut played);
        }
    }

    if seat == mine {
        if clock.seat == Some(seat) && elapsed >= 20.0 {
            decide(&mut game, Event::Pass { seat, time: Centis(2000) }, &mut played);
        }
        return;
    }
    // A bot's turn, once the last shot has played out and it has "thought".
    if clock.seat != Some(seat) || elapsed < f64::from(tuning::bot::THINK.value.0) / 100.0 {
        return;
    }
    let Some(events) = bot::plan(&game.confirmed, seat, Level::Normal, referee.dice.next_u64()) else {
        return;
    };
    info!("SearchBot plays {events:?}");
    for event in events {
        // Stamped as the server would: the time the bot's turn took.
        decide(&mut game, referee::stamp(event, centis), &mut played);
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
