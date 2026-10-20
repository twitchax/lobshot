//! Practice against SearchBot, with no server: the client referees itself, answering the sim's
//! rolls, stamping turn times, timing turns out, and playing the bot's turns.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{Action, Member, MemberId, RoomCode, RoomOptions, RoomView, Stage};
use lobshot_sim::{
    bot::{self, Level},
    game::{Event, Game, MobileKind, Roll, Seat, Setup, Team},
    map::{MapRef, NoAssets},
    units::{Centis, Facing, Wind},
    weather::{Forecast, WeatherKind},
};

use super::{
    input::Act,
    playback::{Timeline, TurnClock},
};
use crate::model::{ApplyEvents, Match, Played, Screen, Session};

const ME: MemberId = MemberId(1);
const BOT: MemberId = MemberId(2);
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

fn member(id: MemberId, name: &str, team: Team, bot: bool) -> Member {
    Member {
        id,
        name: name.to_owned(),
        team: Some(team),
        mobile: MobileKind::Coilback,
        rider: "pip".to_owned(),
        ready: true,
        bot,
        connected: true,
    }
}

/// Sets up a practice game: you against SearchBot on rolling hills.
fn start(mut session: ResMut<Session>, mut commands: Commands) {
    if !session.practice {
        return;
    }
    // Seeded from the browser's dice; practice needs no reproducibility.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let seed = (js_sys::Math::random() * 9.0e15) as u64 | 1;
    let mut referee = Referee { rng: seed };
    let name = crate::web::load("name").unwrap_or_else(|| "You".to_owned());
    session.me = Some(ME);
    session.seats = vec![ME, BOT];
    session.room = Some(RoomView {
        code: RoomCode::from_indices([0; RoomCode::LEN]),
        master: ME,
        options: RoomOptions {
            map: "hills".to_owned(),
            ..RoomOptions::default()
        },
        members: vec![member(ME, &name, Team::A, false), member(BOT, "SearchBot", Team::B, true)],
        stage: Stage::Playing { seats: vec![ME, BOT] },
    });

    let map = MapRef::Hills {
        width: 1800,
        height: 1000,
        ground: 840,
        // Slopes up to about 23 degrees: GunBound ground a mobile can fight from.
        amplitude: 120,
        period: 900,
    };
    let setup = Setup {
        map: map.clone(),
        seats: vec![
            Seat {
                team: Team::A,
                mobile: MobileKind::Coilback,
                spawn_x: 360,
                facing: Facing::Right,
            },
            Seat {
                team: Team::B,
                mobile: MobileKind::Coilback,
                spawn_x: 1440,
                facing: Facing::Left,
            },
        ],
        order: if referee.roll(2) == 0 { vec![0, 1] } else { vec![1, 0] },
        wind: wind(&mut referee, 26),
    };
    let Ok(game) = Game::new(&setup, &NoAssets) else {
        error!("the practice setup is invalid");
        return;
    };
    commands.insert_resource(Match::new(map, game, 0));
    commands.insert_resource(referee);
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
        };
        decide(&mut game, event, &mut played);
    }
    let Some(seat) = super::world::current_seat(&game) else { return };
    let elapsed = time.elapsed_secs_f64() - clock.started;
    // Turn time in hundredths, as a server would stamp it.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let centis = Centis((elapsed.max(0.0) * 100.0) as u32);

    if Some(seat) == session.my_seat() {
        for Act(action) in acts.read() {
            let event = match *action {
                Action::Walk { facing, steps } => Event::Walk { seat, facing, steps },
                Action::Shoot { shot, aim, power } => Event::Shoot { seat, shot, aim, power, time: centis },
                Action::Pass => Event::Pass { seat, time: centis },
            };
            decide(&mut game, event, &mut played);
        }
        if clock.seat == Some(seat) && elapsed >= 20.0 && !timeline.busy() {
            decide(&mut game, Event::Pass { seat, time: Centis(2000) }, &mut played);
        }
        return;
    }
    acts.clear();
    // The bot's turn, once the last shot has played out and it has "thought".
    if clock.seat != Some(seat) || timeline.busy() || elapsed < BOT_THINK {
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
