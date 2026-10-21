//! The messages between the server and its clients, over one WebSocket per client at
//! `/ws/{code}` (relative to the page). Binary frames carry a version byte then postcard; text
//! frames carry the same types as JSON, for debugging and LLM agents. The server answers in
//! whichever form the client last used.
//!
//! A room's game is never sent as state: clients get the event log (`ServerMsg::Game`) and then
//! each accepted event (`ServerMsg::Event`), and replay them with `lobshot_sim`. The server sends
//! its state hash with every event, so a client that diverged knows at once.

pub mod pacing;

use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use lobshot_sim::{
    base::Res,
    game::{Event, MobileKind, Shot, SuddenDeath, SuddenDeathKind, Team},
    items::Loadout,
    log::Log,
    tuning::SUDDEN_DEATH_TURNS,
    units::{Facing, Power},
};

/// Bumped whenever a message from an older build would decode differently.
pub const PROTOCOL_VERSION: u8 = 2;

/// The longest a player name may be, in characters.
pub const MAX_NAME: usize = 16;

/// The longest a chat message may be, in characters.
pub const MAX_CHAT: usize = 200;

/// A room's code: 8 characters from an alphabet without look-alikes (no 0/o, 1/l/i), about 39.6
/// bits. Unguessable enough behind the per-IP rate limit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RoomCode(String);

impl RoomCode {
    pub const ALPHABET: &'static [u8; 31] = b"abcdefghjkmnpqrstuvwxyz23456789";
    pub const LEN: usize = 8;

    /// Validates a code from a URL.
    ///
    /// # Errors
    ///
    /// Fails unless it is 8 characters from `ALPHABET`.
    pub fn parse(code: &str) -> Res<Self> {
        ensure!(code.len() == Self::LEN && code.bytes().all(|b| Self::ALPHABET.contains(&b)), "{code:?} is not a room code");
        Ok(Self(code.to_owned()))
    }

    /// A code from 8 random indices into `ALPHABET` (the caller rolls each in `0..ALPHABET.len()`;
    /// larger ones wrap).
    #[must_use]
    pub fn from_indices(indices: [u8; Self::LEN]) -> Self {
        Self(indices.iter().map(|&i| char::from(Self::ALPHABET[usize::from(i) % Self::ALPHABET.len()])).collect())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A secret that lets a player take their seat back after a disconnect. Kept in `localStorage`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token(pub String);

/// A room member's id, unique within the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemberId(pub u32);

/// Quick reactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Emote {
    Laugh,
    Wow,
    Cry,
    Angry,
    Cool,
    GoodGame,
}

/// What the room master sets before a game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomOptions {
    /// A drawn map's name (`tidewall`), or `flat` for the synthetic test map.
    pub map: String,
    /// Players per team, 1 to 4.
    pub team_size: u8,
    /// The strongest wind the Wind weather may roll: 0, 12 or 26.
    pub max_wind: u8,
    /// The turn timer: 5, 10 or 20 seconds.
    pub turn_seconds: u8,
    /// Shuffle teams at the start instead of keeping members' picks.
    pub random_teams: bool,
    /// What sudden death does and when it starts (one of `SUDDEN_DEATH_TURNS`), or `None`.
    pub sudden_death: Option<SuddenDeath>,
}

impl Default for RoomOptions {
    fn default() -> Self {
        Self {
            map: "tidewall".to_owned(),
            team_size: 1,
            max_wind: 26,
            turn_seconds: 20,
            random_teams: false,
            // A private server's changelog gives Double at 40 turns as the default (RD-origins snippet).
            sudden_death: Some(SuddenDeath {
                kind: SuddenDeathKind::Double,
                turn: SUDDEN_DEATH_TURNS.value[0],
            }),
        }
    }
}

impl RoomOptions {
    /// Checks the ranges.
    ///
    /// # Errors
    ///
    /// Fails on a value the room can't use.
    pub fn validate(&self) -> Res<()> {
        ensure!((1..=4).contains(&self.team_size), "teams are 1 to 4 players, not {}", self.team_size);
        ensure!([0, 12, 26].contains(&self.max_wind), "max wind is 0, 12 or 26, not {}", self.max_wind);
        ensure!([5, 10, 20].contains(&self.turn_seconds), "turns are 5, 10 or 20 seconds, not {}", self.turn_seconds);
        ensure!(!self.map.is_empty(), "pick a map");
        if let Some(SuddenDeath { turn, .. }) = self.sudden_death {
            let turns = SUDDEN_DEATH_TURNS.value;
            ensure!(turns.contains(&turn), "sudden death starts at turn {turns:?}, not {turn}");
        }
        Ok(())
    }
}

/// Someone in the room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub id: MemberId,
    pub name: String,
    /// The team they'll play on, or `None` to spectate.
    pub team: Option<Team>,
    /// `None` is Random: the server picks when the game starts.
    pub mobile: Option<MobileKind>,
    pub items: Loadout,
    /// A cosmetic rider, by asset name.
    pub rider: String,
    pub ready: bool,
    pub bot: bool,
    /// Whether their socket is open (a disconnected player keeps their seat for a while).
    pub connected: bool,
}

/// Where the room is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// Picking teams and mobiles.
    Lobby,
    /// A game is on; `seats[i]` plays seat `i`.
    Playing { seats: Vec<MemberId> },
}

/// Everything a client shows outside the game itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomView {
    pub code: RoomCode,
    pub master: MemberId,
    pub options: RoomOptions,
    pub members: Vec<Member>,
    pub stage: Stage,
}

/// A move in the game, from the player whose turn it is (or, for `Spin`, a dead player). The
/// server turns it into a sim `Event` (stamping a shot's turn time itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Walk {
        facing: Facing,
        steps: u16,
    },
    Shoot {
        shot: Shot,
        aim: i16,
        power: Power,
    },
    Pass,
    /// Use the item at this loadout position.
    Item {
        slot: u8,
    },
    /// A dead player spins their slot machine; it works on anyone's turn.
    Spin,
}

/// Client to server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// The first message on a socket: join, or rejoin with the token from an earlier `Welcome`.
    Hello {
        name: String,
        token: Option<Token>,
        rider: String,
    },
    Chat {
        text: String,
        team_only: bool,
    },
    Emote(Emote),
    /// Play on a team, or `None` to spectate.
    Team(Option<Team>),
    /// A mobile, or `None` for Random.
    Mobile(Option<MobileKind>),
    Loadout(Loadout),
    Rider(String),
    Ready(bool),
    /// Master only.
    Configure(RoomOptions),
    /// Master only: seat a bot.
    AddBot {
        team: Team,
    },
    /// Master only.
    Kick(MemberId),
    /// Master only, once everyone seated is ready.
    Start,
    Act(Action),
}

/// Server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Answers `Hello`: who you are, and the token to rejoin as yourself.
    Welcome {
        you: MemberId,
        token: Token,
    },
    /// The room changed: members, options, stage.
    Room(RoomView),
    Chat {
        from: MemberId,
        name: String,
        text: String,
        team_only: bool,
    },
    Emote {
        from: MemberId,
        emote: Emote,
    },
    /// A game started, or you joined one in progress: replay `log` to catch up.
    Game {
        log: Log,
    },
    /// One more accepted event, the `index`th in the log, and the state hash after it.
    Event {
        index: u32,
        event: Event,
        hash: u64,
    },
    /// Your last message was refused (not your turn, not the master, too fast, ...).
    Rejected {
        reason: String,
    },
    /// The master removed you. The socket closes next.
    Kicked,
}

/// The binary form: the version byte, then postcard.
///
/// # Errors
///
/// Fails only if serialization does.
pub fn encode<T: Serialize>(message: &T) -> Res<Vec<u8>> {
    let mut bytes = vec![PROTOCOL_VERSION];
    bytes.extend(postcard::to_allocvec(message).context("encoding a message")?);
    Ok(bytes)
}

/// Reads the binary form.
///
/// # Errors
///
/// Fails on an empty frame, another version, or malformed bytes.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Res<T> {
    let (&version, body) = bytes.split_first().context("an empty frame")?;
    ensure!(version == PROTOCOL_VERSION, "protocol version {version}, but this build speaks {PROTOCOL_VERSION}");
    postcard::from_bytes(body).context("decoding a message")
}

/// The JSON form.
///
/// # Errors
///
/// Fails only if serialization does.
pub fn to_json<T: Serialize>(message: &T) -> Res<String> {
    serde_json::to_string(message).context("writing a message as JSON")
}

/// Reads the JSON form.
///
/// # Errors
///
/// Fails on malformed JSON.
pub fn from_json<T: DeserializeOwned>(json: &str) -> Res<T> {
    serde_json::from_str(json).context("reading a message from JSON")
}

/// Trims a player name and checks its length.
///
/// # Errors
///
/// Fails when it is empty or longer than `MAX_NAME` characters.
pub fn clean_name(name: &str) -> Res<String> {
    let name = name.trim();
    ensure!(!name.is_empty(), "a name can't be empty");
    ensure!(name.chars().count() <= MAX_NAME, "names are at most {MAX_NAME} characters");
    ensure!(!name.chars().any(char::is_control), "names can't hold control characters");
    Ok(name.to_owned())
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn room_codes_are_eight_characters_from_the_alphabet() {
        let code = RoomCode::from_indices([0, 1, 2, 3, 27, 28, 29, 30]);
        assert_eq!(code.as_str(), "abcd6789");
        assert_eq!(RoomCode::parse(code.as_str()).unwrap(), code);
        for bad in ["", "abcdefg", "abcdefghj", "ABCDEFGH", "abcdefg0", "abcdefgl"] {
            assert!(RoomCode::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn messages_round_trip_in_both_forms() {
        let hello = ClientMsg::Hello {
            name: "Pip".into(),
            token: Some(Token("t0k".into())),
            rider: "pip".into(),
        };
        assert_eq!(decode::<ClientMsg>(&encode(&hello).unwrap()).unwrap(), hello);
        assert_eq!(from_json::<ClientMsg>(&to_json(&hello).unwrap()).unwrap(), hello);

        let act = ClientMsg::Act(Action::Shoot {
            shot: Shot::S2,
            aim: 33,
            power: Power::nearest(2.45),
        });
        assert_eq!(from_json::<ClientMsg>(&to_json(&act).unwrap()).unwrap(), act);
    }

    #[test]
    fn refuses_other_versions() {
        let mut bytes = encode(&ClientMsg::Ready(true)).unwrap();
        bytes[0] = PROTOCOL_VERSION + 1;
        assert!(decode::<ClientMsg>(&bytes).is_err());
        assert!(decode::<ClientMsg>(&[]).is_err());
    }

    #[test]
    fn options_and_names_are_checked() {
        assert!(RoomOptions::default().validate().is_ok());
        assert!(RoomOptions { team_size: 5, ..RoomOptions::default() }.validate().is_err());
        assert!(RoomOptions { max_wind: 13, ..RoomOptions::default() }.validate().is_err());
        assert!(
            RoomOptions {
                turn_seconds: 30,
                ..RoomOptions::default()
            }
            .validate()
            .is_err()
        );
        assert_eq!(clean_name("  Pip ").unwrap(), "Pip");
        assert!(clean_name("   ").is_err());
        assert!(clean_name("a name that is far too long").is_err());
        assert!(clean_name("bad\u{7}").is_err());
    }
}
