//! The end-of-game scoreboard (PRD-0004 T-008): each player's damage to enemies, kills and bonuses,
//! tallied once from the game's log by `lobshot_sim::score` when the result shows. Cosmetic:
//! nothing reads it back.

use bevy::{picking::Pickable, prelude::*};
use lobshot_protocol::MAX_NAME;
use lobshot_sim::{
    game::{MobileKind, Team},
    score::{self, Bonus, Tally},
};

use super::{
    GameRoot,
    playback::{Ending, Outcome},
};
use crate::{
    assets::Masks,
    model::{Match, Screen, Session},
    picks,
    ui::{panel, team_color, text, theme},
};

#[derive(Component)]
struct Board;

/// A tally's bonuses as a line, in the order `Bonus::ALL` lists them: each label once, counted when
/// earned more than once ("Finisher x2, Nice hit").
fn bonus_line(tally: &Tally) -> String {
    Bonus::ALL
        .iter()
        .filter_map(|&bonus| match tally.count(bonus) {
            0 => None,
            1 => Some(bonus.label().to_owned()),
            times => Some(format!("{} x{times}", bonus.label())),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The seats in the order the board lists them: the winning team first, then by damage dealt,
/// then by seat.
fn standings(teams: &[Team], tallies: &[Tally], winner: Option<Team>) -> Vec<usize> {
    let mut seats: Vec<usize> = (0..teams.len().min(tallies.len())).collect();
    seats.sort_by_key(|&seat| (Some(teams[seat]) != winner, std::cmp::Reverse(tallies[seat].damage), seat));
    seats
}

/// The board's text size, and one character's width at it: Bevy's default font (Fira Mono) is
/// monospace, 0.6 em to a character.
const SIZE: f32 = 15.0;
const CHARACTER: f32 = SIZE * 0.6;
/// Space between columns, in UI pixels.
const GAP: f32 = 16.0;
/// The bonuses take what's left, wrapping past this, in UI pixels.
const BONUSES: f32 = 300.0;

/// A fixed-width column: room for this many characters, and whether it holds numbers, which line
/// up on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Column {
    characters: u16,
    numbers: bool,
}

impl Column {
    /// Its width in UI pixels: the characters and the gap after them.
    fn width(self) -> f32 {
        f32::from(self.characters) * CHARACTER + GAP
    }
}

/// The columns before the bonuses: player, mobile, damage and kills, each as wide as its longest
/// value (a full-length name, the longest mobile's name, five-digit damage under its heading).
fn columns() -> [Column; 4] {
    let longest_mobile = MobileKind::ALL.iter().map(|&mobile| picks::mobile_name(Some(mobile)).len()).max().unwrap_or(0);
    let characters = |count: usize| u16::try_from(count).unwrap_or(u16::MAX);
    [
        Column {
            characters: characters(MAX_NAME),
            numbers: false,
        },
        Column {
            characters: characters(longest_mobile),
            numbers: false,
        },
        Column {
            characters: characters("Damage".len()),
            numbers: true,
        },
        Column {
            characters: characters("Kills".len()),
            numbers: true,
        },
    ]
}

/// One line of the board: the player's cell in `first`'s color, the rest in `rest`'s.
fn row(board: &mut ChildSpawnerCommands, cells: [String; 5], (first, rest): (Color, Color), size: f32) {
    let columns = columns();
    board.spawn(Node::default()).with_children(|line| {
        for (index, cell) in cells.into_iter().enumerate() {
            let column = columns.get(index).copied();
            let node = Node {
                width: column.map_or(Val::Auto, |column| Val::Px(column.width())),
                max_width: Val::Px(BONUSES),
                // Half the gap trails the text, so a right-aligned number keeps clear of the next
                // column and a long value still has room.
                padding: UiRect::right(Val::Px(GAP / 2.0)),
                justify_content: if column.is_some_and(|column| column.numbers) {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                },
                ..default()
            };
            line.spawn((node, children![text(cell, size, if index == 0 { first } else { rest })]));
        }
    });
}

/// Puts the board up once the result shows (the tally replays the log: once a game, not a frame).
fn show(game: Option<Res<Match>>, outcome: Res<Outcome>, session: Res<Session>, masks: Masks, mut done: Local<bool>, mut commands: Commands) {
    let Some(game) = game else { return };
    let Some(ending) = outcome.0 else {
        *done = false;
        return;
    };
    if *done {
        return;
    }
    *done = true;
    let tallies = match score::tally(&game.log, &masks) {
        Ok(tallies) => tallies,
        Err(error) => {
            warn!("no scoreboard this game: {error:#}");
            return;
        }
    };
    let teams: Vec<Team> = game.log.setup.seats.iter().map(|seat| seat.team).collect();
    let winner = match ending {
        Ending::Winner(team) => Some(team),
        Ending::Draw => None,
    };
    let root = commands
        .spawn((
            GameRoot,
            Board,
            // Under the top bar, clear of the result's banner; the board takes no clicks.
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(118.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).with_children(|overlay| {
        overlay
            .spawn((
                panel(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(3.0),
                    ..default()
                }),
                Pickable::IGNORE,
            ))
            .with_children(|board| {
                board.spawn(text("Scoreboard", 20.0, theme::ACCENT));
                row(board, ["Player", "Mobile", "Damage", "Kills", "Bonuses"].map(str::to_owned), (theme::MUTED, theme::MUTED), 12.0);
                for seat in standings(&teams, &tallies, winner) {
                    let (tally, seat_index) = (&tallies[seat], u8::try_from(seat).unwrap_or(u8::MAX));
                    let mobile = game.log.setup.seats.get(seat).map_or("", |seat| picks::mobile_name(Some(seat.mobile)));
                    let cells = [session.seat_name(seat_index), mobile.to_owned(), tally.damage.to_string(), tally.kills.to_string(), bonus_line(tally)];
                    row(board, cells, (team_color(teams[seat]), theme::TEXT), SIZE);
                }
            });
    });
}

pub struct ScoreboardPlugin;

impl Plugin for ScoreboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.run_if(in_state(Screen::Game)));
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    // The runner only runs `wasm_bindgen_test`s; this makes `#[test]` one.
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    fn tally(damage: i32, bonuses: &[Bonus]) -> Tally {
        Tally {
            damage,
            kills: 0,
            bonuses: bonuses.to_vec(),
        }
    }

    #[test]
    fn bonuses_read_in_order_counted() {
        let earned = tally(0, &[Bonus::NiceHit, Bonus::Finisher, Bonus::NiceHit, Bonus::FriendlyFire]);
        assert_eq!(bonus_line(&earned), "Finisher, Nice hit x2, Friendly fire");
        assert_eq!(bonus_line(&tally(0, &[])), "");
    }

    #[test]
    fn the_columns_fit_their_longest_values_and_numbers_line_up_right() {
        let [player, mobile, damage, kills] = columns();
        assert_eq!(usize::from(player.characters), MAX_NAME);
        for each in MobileKind::ALL {
            let name = picks::mobile_name(Some(each));
            assert!(name.len() <= usize::from(mobile.characters), "{name}");
        }
        assert!(usize::from(damage.characters) >= "99999".len());
        assert_eq!([player.numbers, mobile.numbers, damage.numbers, kills.numbers], [false, false, true, true]);
        // A row of the widest values, and the bonuses, fits a 1024 px window.
        let width: f32 = columns().iter().map(|column| column.width()).sum::<f32>() + BONUSES;
        assert!(width < 1024.0, "{width}");
    }

    #[test]
    fn the_winners_lead_then_the_damage() {
        let teams = [Team::A, Team::B, Team::A, Team::B];
        let tallies = [tally(100, &[]), tally(900, &[]), tally(300, &[]), tally(100, &[])];
        assert_eq!(standings(&teams, &tallies, Some(Team::A)), [2, 0, 1, 3]);
        assert_eq!(standings(&teams, &tallies, None), [1, 2, 0, 3], "a draw: damage alone");
    }
}
