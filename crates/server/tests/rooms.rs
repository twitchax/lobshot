//! Rooms over real sockets, in-process: the router on `127.0.0.1:0`, driven by WebSocket clients.
//! Bots play at once (`test_config`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::net::SocketAddr;

use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error, Message},
};

use lobshot_protocol::{Action, ClientMsg, RoomOptions, RoomView, ServerMsg, Stage};
use lobshot_server::{
    rate::TOO_FAST,
    tests::{self, Replica},
};
use lobshot_sim::{
    game::{MobileKind, Phase, Shot, Team},
    items::{Item, Loadout},
    log::Log,
    map::NoAssets,
    units::Power,
};

use support::{Client, play_out};

/// A server and a fresh room on it. Holding `_dist` keeps the bundle directory alive.
struct Fixture {
    addr: SocketAddr,
    code: String,
    _dist: TempDir,
}

async fn fixture() -> Fixture {
    let dist = TempDir::new().unwrap();
    tests::write_bundle(dist.path()).unwrap();
    let addr = tests::spawn_test_server(dist.path()).await.unwrap();
    let code = tokio::task::spawn_blocking(move || tests::create_room(addr)).await.unwrap().unwrap();
    Fixture { addr, code, _dist: dist }
}

fn flat() -> ClientMsg {
    ClientMsg::Configure(RoomOptions {
        map: "flat".into(),
        team_size: 1,
        max_wind: 26,
        turn_seconds: 20,
        random_teams: false,
        sudden_death: None,
    })
}

/// A master on team A, seated against a bot, ready and started.
async fn versus_bot(f: &Fixture) -> (Client, lobshot_protocol::MemberId, lobshot_protocol::Token) {
    let mut human = Client::connect(f.addr, &f.code, false).await;
    let (me, token, _) = human.hello("Pip", None).await;
    human.send(&flat()).await;
    human.send(&ClientMsg::AddBot { team: Team::B }).await;
    human.send(&ClientMsg::Ready(true)).await;
    human.send(&ClientMsg::Start).await;
    (human, me, token)
}

#[tokio::test]
async fn sockets_for_unknown_rooms_are_refused() {
    let f = fixture().await;
    for code in ["aaaaaaaa", "nope"] {
        let Err(Error::Http(response)) = connect_async(format!("ws://{}/ws/{code}", f.addr)).await else {
            panic!("{code} opened")
        };
        assert_eq!(response.status(), 404);
    }
}

#[tokio::test]
async fn the_server_answers_in_the_form_the_client_last_used() {
    let f = fixture().await;
    let mut client = Client::connect(f.addr, &f.code, true).await;
    let (_, token, view) = client.hello("Pip", None).await;
    assert_eq!(token.0.len(), 32);
    assert_eq!(view.code.as_str(), f.code);

    client.json = false;
    client
        .send(&ClientMsg::Chat {
            text: "now in postcard".into(),
            team_only: false,
        })
        .await;
    assert!(matches!(client.recv().await, ServerMsg::Chat { text, .. } if text == "now in postcard"));
    client.json = true;
    client.send(&ClientMsg::Ready(true)).await;
    assert!(matches!(client.recv().await, ServerMsg::Room(_)));
}

#[tokio::test]
async fn a_player_plays_a_bot_to_the_end_while_a_spectator_follows() {
    let f = fixture().await;
    let (mut human, me, _) = versus_bot(&f).await;

    // The game can't end before the human plays, so the spectator joins mid-game.
    let mut watcher = Client::connect(f.addr, &f.code, true).await;
    let (watcher_id, _, view) = watcher.hello("Watcher", None).await;
    assert!(matches!(view.stage, Stage::Playing { .. }));
    assert_eq!(view.members.iter().find(|m| m.id == watcher_id).unwrap().team, None);

    let ((played, lobby), (followed, _)) = tokio::join!(play_out(&mut human, me, None, 1), play_out(&mut watcher, watcher_id, Some(&view), 0));
    let game = played.game().unwrap();
    assert!(matches!(game.phase(), Phase::Over { winner: Some(_) }), "{:?}", game.phase());
    assert_eq!(game.state_hash().unwrap(), followed.game().unwrap().state_hash().unwrap());
    assert_eq!(lobby.stage, Stage::Lobby);
    let pip = lobby.members.iter().find(|m| m.id == me).unwrap();
    assert!(!pip.ready, "everyone readies again");
}

#[tokio::test]
async fn a_token_rejoins_its_seat_mid_game() {
    let f = fixture().await;
    let (mut human, me, token) = versus_bot(&f).await;
    human.recv_until(|msg| matches!(msg, ServerMsg::Game { .. })).await;
    drop(human);

    let mut back = Client::connect(f.addr, &f.code, false).await;
    let (again, same, view) = back.hello("Pip", Some(token.clone())).await;
    assert_eq!((again, &same), (me, &token));
    let Stage::Playing { seats } = &view.stage else { panic!("{view:?}") };
    assert!(seats.contains(&me));
    assert!(view.members.iter().find(|m| m.id == me).unwrap().connected);

    // `Game { log }` comes next; the rejoined player finishes the game from it.
    let (played, _) = play_out(&mut back, me, Some(&view), 1).await;
    assert!(matches!(played.game().unwrap().phase(), Phase::Over { winner: Some(_) }));
}

#[tokio::test]
async fn a_player_who_rejoins_mid_game_gets_the_mines_left_on_the_map() {
    let f = fixture().await;
    let mut human = Client::connect(f.addr, &f.code, false).await;
    let (me, token, _) = human.hello("Pip", None).await;
    for msg in [
        flat(),
        ClientMsg::Mobile(Some(MobileKind::Tinbadger)),
        ClientMsg::AddBot { team: Team::B },
        ClientMsg::Ready(true),
        ClientMsg::Start,
    ] {
        human.send(&msg).await;
    }
    // Lay mines with soft lobs on Pip's turns. The spawns and the wind are rolled, so a lob's
    // shells may land on the bot (going off at once) or leave the map: lob again until a mine is
    // down, rather than wait on one lob forever.
    let mut replica = Replica::default();
    let mut seat = None;
    let shot = Action::Shoot {
        shot: Shot::S2,
        aim: 45,
        power: Power::nearest(1.3),
    };
    let mut lobs = 0;
    while replica.game().is_none_or(|game| game.mines().is_empty()) {
        let msg = human.recv().await;
        replica.absorb(&msg).unwrap();
        if let ServerMsg::Room(RoomView { stage: Stage::Playing { seats }, .. }) = &msg {
            seat = seats.iter().position(|&id| id == me).and_then(|seat| u8::try_from(seat).ok());
        }
        let Some(seat) = seat else { continue };
        if replica
            .game()
            .is_some_and(|game| game.phase() == (Phase::Turn { seat }) && game.pending_roll().is_none() && game.mines().is_empty())
        {
            assert!(lobs < 8, "no mine down after {lobs} lobs");
            human.send(&ClientMsg::Act(shot)).await;
            lobs += 1;
            // Wait out the lob: until a mine is down or the turn has passed.
            while replica.game().is_some_and(|game| game.phase() == (Phase::Turn { seat }) && game.mines().is_empty()) {
                replica.absorb(&human.recv().await).unwrap();
            }
        }
    }
    let laid = replica.game().unwrap().mines().to_vec();
    let fired = replica.log().unwrap().events.len();
    drop(human);

    let mut back = Client::connect(f.addr, &f.code, false).await;
    back.hello("Pip", Some(token)).await;
    let ServerMsg::Game { log } = back.recv_until(|msg| matches!(msg, ServerMsg::Game { .. })).await else {
        unreachable!()
    };
    assert!(log.replay(&NoAssets).is_ok(), "the whole game so far replays");
    let then = Log {
        setup: log.setup.clone(),
        events: log.events[..fired].to_vec(),
    };
    assert_eq!(then.replay(&NoAssets).unwrap().mines(), laid, "the mines come back with the log");
}

#[tokio::test]
async fn a_kicked_player_hears_kicked_then_the_socket_closes() {
    let f = fixture().await;
    let mut master = Client::connect(f.addr, &f.code, false).await;
    master.hello("Master", None).await;
    let mut pest = Client::connect(f.addr, &f.code, false).await;
    let (pest_id, token, _) = pest.hello("Pest", None).await;

    master.send(&ClientMsg::Kick(pest_id)).await;
    let rest = pest.until_closed().await;
    assert_eq!(rest.last(), Some(&ServerMsg::Kicked), "{rest:?}");
    let view = master.recv_until(|msg| matches!(msg, ServerMsg::Room(view) if view.members.len() == 1)).await;
    assert!(matches!(view, ServerMsg::Room(_)));

    let mut back = Client::connect(f.addr, &f.code, false).await;
    let (again, _, _) = back.hello("Pest", Some(token)).await;
    assert_ne!(again, pest_id, "the token was revoked; they come back as someone new");
}

/// The binary form of `ClientMsg::Loadout` with three of `item`, then a fourth written in by hand:
/// postcard writes a list as its length, then its items.
fn four_of(item: Item) -> Message {
    let mut bytes = lobshot_protocol::encode(&ClientMsg::Loadout(Loadout::new(&[item; 3]).unwrap())).unwrap();
    let length = bytes.len() - 4;
    let [3, a, b, c] = bytes[length..] else { panic!("{bytes:?}") };
    assert!(a == b && b == c);
    bytes[length] = 4;
    bytes.push(a);
    Message::Binary(bytes.into())
}

/// A `Loadout` can't be built over six slots, so a client sends one only by hand; either form
/// is refused as it decodes, and the member keeps the loadout they had.
#[tokio::test]
async fn a_loadout_over_six_slots_is_refused_in_either_form() {
    let f = fixture().await;
    let mut client = Client::connect(f.addr, &f.code, false).await;
    let (me, _, _) = client.hello("Pip", None).await;
    let items = |view: &RoomView| view.members.iter().find(|m| m.id == me).unwrap().items;

    // Four Heals fill four slots: the hand-written list itself is sound.
    client.send_frame(four_of(Item::Heal)).await;
    let ServerMsg::Room(view) = client.recv().await else { panic!() };
    let heals = Loadout::new(&[Item::Heal; 4]).unwrap();
    assert_eq!(items(&view), heals);
    // Four Duals fill eight. postcard keeps none of the check's words, only that it failed.
    client.send_frame(four_of(Item::Dual)).await;
    let ServerMsg::Rejected { reason } = client.recv().await else { panic!() };
    assert!(reason.starts_with("decoding a message"), "{reason}");

    client.json = true;
    client.send_frame(Message::Text(r#"{"Loadout":["Dual","Dual","Dual","Heal"]}"#.into())).await;
    let ServerMsg::Rejected { reason } = client.recv().await else { panic!() };
    assert!(reason.contains("a loadout fills at most 6 slots, not 7"), "{reason}");

    client.send(&ClientMsg::Ready(true)).await;
    let ServerMsg::Room(view) = client.recv().await else { panic!() };
    assert_eq!(items(&view), heals, "unchanged");
}

#[tokio::test]
async fn messages_past_the_rate_limit_are_refused() {
    let f = fixture().await;
    let mut client = Client::connect(f.addr, &f.code, false).await;
    client.hello("Pip", None).await;
    for i in 0..200 {
        client.send(&ClientMsg::Ready(i % 2 == 0)).await;
    }
    let mut refused = 0;
    for _ in 0..200 {
        match client.recv().await {
            ServerMsg::Rejected { reason } => {
                assert_eq!(reason, TOO_FAST);
                refused += 1;
            }
            ServerMsg::Room(_) => {}
            other => panic!("{other:?}"),
        }
    }
    // The burst is 60, plus whatever refilled while the 200 went out.
    assert!((100..=140).contains(&refused), "{refused} refused");
}
