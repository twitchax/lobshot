//! Your turn's controls. Left/Right (or A/D) walk, Up/Down (or W/S) aim, Tab or 1/2/3 pick the
//! shot, and holding Space charges power until you let go (or it maxes out and fires). F8 or P
//! passes, Enter chats. Walks are predicted: applied at once and sent in 100 ms batches.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{Action, ClientMsg};
use lobshot_sim::{
    game::{Event, Player, Shot},
    units::{Facing, Power},
};

use super::playback::TurnClock;
use crate::{
    model::{ChatLine, Match, Screen, Session},
    net::Socket,
    ui::{Clicked, UiAction},
    web::{TextInput, Typed},
};

/// Power fills 4 bars in 2.5 s while Space is held (OB), so a 0.05-bar step is about 31 ms.
const BARS_PER_SECOND: f32 = 1.6;
/// Walking pace: 60 px/s, one step a frame at GunBound's 60 fps (OB).
const STEPS_PER_SECOND: f32 = 60.0;
/// Aim moves a degree per this long while Up or Down is held.
const AIM_REPEAT: f32 = 1.0 / 15.0;
/// Walks are batched this long before sending.
const WALK_BATCH: f32 = 0.1;

/// This player's controls.
#[derive(Resource)]
pub struct Controls {
    pub shot: Shot,
    pub aim: i16,
    /// Charged power, in bars.
    pub power: f32,
    pub charging: bool,
    pub last_aim: Option<i16>,
    pub last_power: Option<f32>,
    aim_held: f32,
    walk_steps: f32,
    walk_facing: Option<Facing>,
    walk_held: f32,
    /// The turn this player last set up for.
    turn: Option<(u8, f64)>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            shot: Shot::S1,
            aim: 30,
            power: 0.0,
            charging: false,
            last_aim: None,
            last_power: None,
            aim_held: 0.0,
            walk_steps: 0.0,
            walk_facing: None,
            walk_held: 0.0,
            turn: None,
        }
    }
}

/// An action for the referee: the server, or practice.
#[derive(Message, Debug, Clone, Copy)]
pub struct Act(pub Action);

/// Whether it's this player's turn and they can act now.
fn my_turn(session: &Session, clock: &TurnClock, game: &Match) -> Option<u8> {
    let seat = session.my_seat()?;
    (clock.seat == Some(seat) && super::world::current_seat(game) == Some(seat) && game.predicted.pending_roll().is_none()).then_some(seat)
}

/// What decides whether this player may act.
#[derive(SystemParam)]
struct Turn<'w> {
    session: Res<'w, Session>,
    clock: Res<'w, TurnClock>,
    typing: Res<'w, TextInput>,
    time: Res<'w, Time>,
}

fn keys(keyboard: Res<ButtonInput<KeyCode>>, turn: Turn, game: Option<ResMut<Match>>, mut controls: ResMut<Controls>, mut acts: MessageWriter<Act>) {
    let Some(mut game) = game else { return };
    let Some(seat) = my_turn(&turn.session, &turn.clock, &game) else {
        controls.charging = false;
        controls.power = 0.0;
        return;
    };
    let Some(me) = game.predicted.players().get(usize::from(seat)).copied() else { return };
    start_turn(&mut controls, seat, turn.clock.started, &me);
    if turn.typing.is_open() {
        return;
    }
    let dt = turn.time.delta_secs();
    pick_shot(&keyboard, &mut controls);
    aim(&keyboard, &mut controls, &me, dt);
    let mut hands = Hands {
        controls: &mut controls,
        game: &mut game,
        seat,
        predict: !turn.session.practice,
        acts: &mut acts,
    };
    hands.walk(&keyboard, &me, dt);
    hands.charge(&keyboard, dt);
    if keyboard.any_just_pressed([KeyCode::F8, KeyCode::KeyP]) {
        acts.write(Act(Action::Pass));
    }
}

/// A new turn: start from last turn's aim (or the middle of the true range) and no power.
fn start_turn(controls: &mut Controls, seat: u8, started: f64, me: &Player) {
    if controls.turn == Some((seat, started)) {
        return;
    }
    let range = controls.shot.tuning(me.tuning()).aim.value;
    controls.turn = Some((seat, started));
    controls.aim = controls.last_aim.unwrap_or(i16::midpoint(range.true_min, range.true_max)).clamp(range.min, range.max);
    controls.power = 0.0;
    controls.charging = false;
}

fn pick_shot(keyboard: &ButtonInput<KeyCode>, controls: &mut Controls) {
    if keyboard.just_pressed(KeyCode::Tab) {
        controls.shot = match controls.shot {
            Shot::S1 => Shot::S2,
            Shot::S2 => Shot::Ss,
            Shot::Ss => Shot::S1,
        };
    }
    for (key, shot) in [(KeyCode::Digit1, Shot::S1), (KeyCode::Digit2, Shot::S2), (KeyCode::Digit3, Shot::Ss)] {
        if keyboard.just_pressed(key) {
            controls.shot = shot;
        }
    }
}

/// A degree per press, repeating while held, kept inside the shot's range.
fn aim(keyboard: &ButtonInput<KeyCode>, controls: &mut Controls, me: &Player, dt: f32) {
    let up = keyboard.any_pressed([KeyCode::ArrowUp, KeyCode::KeyW]);
    let down = keyboard.any_pressed([KeyCode::ArrowDown, KeyCode::KeyS]);
    if up == down {
        controls.aim_held = 0.0;
    } else {
        let pressed = keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW, KeyCode::ArrowDown, KeyCode::KeyS]);
        controls.aim_held += dt;
        if pressed || controls.aim_held >= AIM_REPEAT {
            controls.aim_held = 0.0;
            controls.aim += if up { 1 } else { -1 };
        }
    }
    let range = controls.shot.tuning(me.tuning()).aim.value;
    controls.aim = controls.aim.clamp(range.min, range.max);
}

/// This turn's moving parts: the controls, the game to predict on, and where actions go.
struct Hands<'a, 'w> {
    controls: &'a mut Controls,
    game: &'a mut Match,
    seat: u8,
    /// Online walks are predicted; practice's referee applies them straight away.
    predict: bool,
    acts: &'a mut MessageWriter<'w, Act>,
}

impl Hands<'_, '_> {
    /// Walking at GunBound's pace, not while charging a shot.
    fn walk(&mut self, keyboard: &ButtonInput<KeyCode>, me: &Player, dt: f32) {
        if self.controls.charging {
            return;
        }
        let left = keyboard.any_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]);
        let right = keyboard.any_pressed([KeyCode::ArrowRight, KeyCode::KeyD]);
        let facing = match (left, right) {
            (true, false) => Some(Facing::Left),
            (false, true) => Some(Facing::Right),
            _ => None,
        };
        if facing.is_some() && self.controls.walk_facing != facing {
            self.flush_walk();
            self.controls.walk_facing = facing;
        }
        let Some(facing) = facing else {
            self.flush_walk();
            self.controls.walk_facing = None;
            return;
        };
        self.controls.walk_steps += STEPS_PER_SECOND * dt;
        self.controls.walk_held += dt;
        // A tap the other way at least turns the mobile around.
        if me.facing != facing && self.controls.walk_steps < 1.0 {
            self.controls.walk_steps = 1.0;
        }
        if self.controls.walk_held >= WALK_BATCH {
            self.flush_walk();
        }
    }

    /// Power: hold Space to charge (it only goes up), release to fire; full power fires itself.
    fn charge(&mut self, keyboard: &ButtonInput<KeyCode>, dt: f32) {
        if keyboard.just_pressed(KeyCode::Space) {
            self.flush_walk();
            self.controls.charging = true;
            self.controls.power = 0.0;
        }
        if self.controls.charging {
            self.controls.power = (self.controls.power + BARS_PER_SECOND * dt).min(4.0);
            if keyboard.just_released(KeyCode::Space) || self.controls.power >= 4.0 {
                self.fire();
            }
        }
    }

    fn fire(&mut self) {
        let power = Power::nearest(f64::from(self.controls.power));
        self.controls.charging = false;
        // Power steps are twentieths of a bar, at most 80.
        self.controls.last_power = Some(f32::from(power.twentieths()) / 20.0);
        self.controls.last_aim = Some(self.controls.aim);
        self.acts.write(Act(Action::Shoot {
            shot: self.controls.shot,
            aim: self.controls.aim,
            power,
        }));
    }

    /// Sends the walking done since the last batch.
    fn flush_walk(&mut self) {
        self.controls.walk_held = 0.0;
        let Some(facing) = self.controls.walk_facing else { return };
        // A tenth of a second of walking: a handful of steps.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = self.controls.walk_steps.floor() as u16;
        if steps == 0 {
            return;
        }
        self.controls.walk_steps -= f32::from(steps);
        if !self.predict || self.game.predict(Event::Walk { seat: self.seat, facing, steps }) {
            self.acts.write(Act(Action::Walk { facing, steps }));
        }
    }
}

/// Buttons on the game screen.
fn buttons(mut clicked: MessageReader<Clicked>, mut controls: ResMut<Controls>, mut acts: MessageWriter<Act>, typing: Res<TextInput>) {
    for Clicked(action) in clicked.read() {
        match action {
            UiAction::Shot(shot) => controls.shot = *shot,
            UiAction::Pass => {
                acts.write(Act(Action::Pass));
            }
            UiAction::Chat => typing.open("Say something (start with /t for your team)", lobshot_protocol::MAX_CHAT, ""),
            _ => {}
        }
    }
}

/// Enter opens chat; whatever's typed goes to the room (or, in practice, just the log).
fn chat(keyboard: Res<ButtonInput<KeyCode>>, typing: Res<TextInput>, mut session: ResMut<Session>, mut socket: NonSendMut<Socket>) {
    if keyboard.just_pressed(KeyCode::Enter) && !typing.is_open() {
        typing.open("Say something (start with /t for your team)", lobshot_protocol::MAX_CHAT, "");
    }
    for typed in typing.take() {
        let Typed::Submitted(text) = typed else { continue };
        let (team_only, text) = text.strip_prefix("/t ").map_or((false, text.trim().to_owned()), |rest| (true, rest.trim().to_owned()));
        if text.is_empty() {
            continue;
        }
        if session.practice {
            session.chat.push(ChatLine { name: "You".to_owned(), text, team_only });
        } else {
            socket.send(&ClientMsg::Chat { text, team_only });
        }
    }
}

/// Online, actions go to the server (walks were already predicted).
fn send(mut acts: MessageReader<Act>, session: Res<Session>, mut socket: NonSendMut<Socket>) {
    if session.practice {
        return;
    }
    for Act(action) in acts.read() {
        socket.send(&ClientMsg::Act(*action));
    }
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>()
            .add_message::<Act>()
            .add_systems(Update, (keys, buttons, chat, send).chain().run_if(in_state(Screen::Game)));
    }
}
