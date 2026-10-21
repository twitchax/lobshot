//! Your turn's controls. Left/Right (or A/D) walk, Up/Down (or W/S) aim, Tab or 1/2/3 pick the
//! shot (SS only while it's ready), 4 to 9 use the loadout's items, and holding Space charges
//! power until you let go (or it maxes out and fires). F8 or P passes, Enter chats. Walks are
//! predicted: applied at once and sent in 100 ms batches. A dead player's Spin works on anyone's
//! turn.

use bevy::{ecs::system::SystemParam, prelude::*};
use lobshot_protocol::{Action, ClientMsg};
use lobshot_sim::{
    game::{Event, Player, Shot, SsState},
    items::MAX_ITEMS,
    units::{Facing, Power},
};

use super::playback::TurnClock;
use crate::{
    model::{ChatLine, Match, Screen, Session},
    net::Socket,
    ui::{Clicked, UiAction},
    web::{TextInput, Typed},
};

/// The keys that use the loadout's items, by position, and how the item bar labels them: the
/// number row after the shots' 1, 2 and 3. GunBound used F1 to F6, but browsers keep those (F5
/// reloads the page).
const ITEM_KEYS: [(KeyCode, &str); MAX_ITEMS] = [
    (KeyCode::Digit4, "4"),
    (KeyCode::Digit5, "5"),
    (KeyCode::Digit6, "6"),
    (KeyCode::Digit7, "7"),
    (KeyCode::Digit8, "8"),
    (KeyCode::Digit9, "9"),
];

/// The loadout position a key uses, if it's an item key.
#[must_use]
pub fn item_for_key(key: KeyCode) -> Option<u8> {
    ITEM_KEYS.iter().position(|&(item_key, _)| item_key == key).and_then(|position| u8::try_from(position).ok())
}

/// The key that uses the item at `position`, as the item bar shows it.
#[must_use]
pub fn item_key_label(position: u8) -> Option<&'static str> {
    ITEM_KEYS.get(usize::from(position)).map(|&(_, label)| label)
}

/// The shot Tab moves to from `shot`: S1, S2, then SS if it's ready.
#[must_use]
pub fn next_shot(shot: Shot, ss_ready: bool) -> Shot {
    match shot {
        Shot::S1 => Shot::S2,
        Shot::S2 if ss_ready => Shot::Ss,
        Shot::S2 | Shot::Ss => Shot::S1,
    }
}

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
pub fn my_turn(session: &Session, clock: &TurnClock, game: &Match) -> Option<u8> {
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
    let Some(me) = game.player(seat).copied() else { return };
    let ss_ready = game.predicted.ss_state(seat).is_ok_and(|ss| ss == SsState::Ready);
    start_turn(&mut controls, seat, turn.clock.started, &me);
    // A locked SS can't stay picked (it locks once fired; sudden death can take it away).
    if controls.shot == Shot::Ss && !ss_ready {
        controls.shot = Shot::S1;
    }
    if turn.typing.is_open() {
        return;
    }
    let dt = turn.time.delta_secs();
    pick_shot(&keyboard, &mut controls, ss_ready);
    for position in keyboard.get_just_pressed().filter_map(|&key| item_for_key(key)) {
        if game.predicted.can_use_item(seat, position) {
            acts.write(Act(Action::Item { slot: position }));
        }
    }
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

fn pick_shot(keyboard: &ButtonInput<KeyCode>, controls: &mut Controls, ss_ready: bool) {
    if keyboard.just_pressed(KeyCode::Tab) {
        controls.shot = next_shot(controls.shot, ss_ready);
    }
    for (key, shot) in [(KeyCode::Digit1, Shot::S1), (KeyCode::Digit2, Shot::S2), (KeyCode::Digit3, Shot::Ss)] {
        if keyboard.just_pressed(key) && (shot != Shot::Ss || ss_ready) {
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

/// Buttons on the game screen. The HUD disables SS while it's locked, and items that can't be used
/// now, so a click here is one the player could make.
fn buttons(mut clicked: MessageReader<Clicked>, mut controls: ResMut<Controls>, mut acts: MessageWriter<Act>, typing: Res<TextInput>) {
    for Clicked(action) in clicked.read() {
        match action {
            UiAction::Shot(shot) => controls.shot = *shot,
            UiAction::Item(slot) => {
                acts.write(Act(Action::Item { slot: *slot }));
            }
            UiAction::Spin => {
                acts.write(Act(Action::Spin));
            }
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

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    #[test]
    fn items_sit_on_the_number_row_after_the_shots() {
        assert_eq!(item_for_key(KeyCode::Digit4), Some(0));
        assert_eq!(item_for_key(KeyCode::Digit9), Some(5));
        for key in [KeyCode::Digit1, KeyCode::Digit3, KeyCode::F5, KeyCode::KeyP] {
            assert_eq!(item_for_key(key), None, "{key:?}");
        }
        assert_eq!(item_key_label(0), Some("4"));
        assert_eq!(item_key_label(5), Some("9"));
        assert_eq!(item_key_label(6), None);
        for position in 0..6 {
            let (key, label) = ITEM_KEYS[usize::from(position)];
            assert_eq!((item_for_key(key), item_key_label(position)), (Some(position), Some(label)));
        }
    }

    #[test]
    fn tab_skips_ss_unless_it_is_ready() {
        assert_eq!(next_shot(Shot::S1, true), Shot::S2);
        assert_eq!(next_shot(Shot::S2, true), Shot::Ss);
        assert_eq!(next_shot(Shot::Ss, true), Shot::S1);
        assert_eq!(next_shot(Shot::S2, false), Shot::S1, "locked or off");
    }
}
