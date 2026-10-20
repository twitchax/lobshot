//! The async shell around a `Room`: one tokio task per room. Sockets send it commands over an
//! mpsc; it hands the room each input with the time, sleeps until the room's next deadline, and
//! carries out what the room asks: messages into each socket's own outbound channel, sockets to
//! close, and bot turns to plan on `spawn_blocking`.

use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::mpsc;

use lobshot_protocol::{ClientMsg, ServerMsg};
use lobshot_sim::game::Event;

use super::{ConnId, Output, Room};

/// Commands a room can queue before senders wait.
const INBOX: usize = 1024;

/// Messages a socket may have queued before it counts as stalled and is dropped. A client that far
/// behind has missed too much of the game to follow it; it rejoins with its token and replays
/// the log.
pub const OUTBOX: usize = 1024;

/// What a socket sends its room's task.
#[derive(Debug)]
pub enum Command {
    /// A socket opened; the room sends it messages through `outbox`.
    Open {
        conn: ConnId,
        outbox: mpsc::Sender<Arc<ServerMsg>>,
    },
    Message(ConnId, ClientMsg),
    Closed(ConnId),
    /// A bot's plan came back from `spawn_blocking`.
    Planned {
        turn: u64,
        events: Option<Vec<Event>>,
    },
}

/// A way into a running room.
#[derive(Debug, Clone)]
pub struct RoomHandle(mpsc::Sender<Command>);

impl RoomHandle {
    /// Queues a command. `false` means the room has ended.
    pub async fn send(&self, command: Command) -> bool {
        self.0.send(command).await.is_ok()
    }
}

/// Runs `room` on its own task until it ends, then calls `on_end`.
pub fn spawn(room: Room, on_end: impl FnOnce() + Send + 'static) -> RoomHandle {
    let (tx, rx) = mpsc::channel(INBOX);
    tokio::spawn(run(room, rx, tx.clone(), on_end));
    RoomHandle(tx)
}

async fn run(mut room: Room, mut inbox: mpsc::Receiver<Command>, me: mpsc::Sender<Command>, on_end: impl FnOnce()) {
    let mut outboxes: HashMap<ConnId, mpsc::Sender<Arc<ServerMsg>>> = HashMap::new();
    while !room.ended() {
        let deadline = room.deadline();
        // A disabled `select!` branch still builds its future, so it needs some instant.
        let wake = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now).into());
        tokio::select! {
            command = inbox.recv() => {
                // The task holds a sender itself (for bot plans), so the inbox never closes.
                let Some(command) = command else { break };
                let now = Instant::now();
                match command {
                    Command::Open { conn, outbox } => {
                        outboxes.insert(conn, outbox);
                        room.open(conn);
                    }
                    Command::Message(conn, msg) => room.message(conn, msg, now),
                    Command::Closed(conn) => {
                        outboxes.remove(&conn);
                        room.closed(conn, now);
                    }
                    Command::Planned { turn, events } => room.planned(turn, events, now),
                }
            }
            () = wake, if deadline.is_some() => room.tick(Instant::now()),
        }
        deliver(&mut room, &mut outboxes, &me);
    }
    // Dropping the outboxes closes every socket still open.
    drop(outboxes);
    on_end();
}

/// Carries out everything the room asked for, including what dropping a stalled socket asks.
fn deliver(room: &mut Room, outboxes: &mut HashMap<ConnId, mpsc::Sender<Arc<ServerMsg>>>, me: &mpsc::Sender<Command>) {
    loop {
        let outputs = room.take_output();
        if outputs.is_empty() {
            return;
        }
        for output in outputs {
            match output {
                Output::Send(conn, msg) => {
                    let Some(outbox) = outboxes.get(&conn) else { continue };
                    if outbox.try_send(msg).is_err() {
                        tracing::warn!(conn = conn.0, "dropping a socket that fell behind");
                        outboxes.remove(&conn);
                        room.closed(conn, Instant::now());
                    }
                }
                Output::Close(conn) => {
                    outboxes.remove(&conn);
                }
                Output::Plan(turn) => {
                    let me = me.clone();
                    tokio::spawn(async move {
                        let serial = turn.turn;
                        // A panicking search reads as no plan: the room passes for the bot.
                        let events = tokio::task::spawn_blocking(move || turn.plan()).await.ok().flatten();
                        // Fails only once the room has ended, when nobody needs the plan.
                        let _ = me.send(Command::Planned { turn: serial, events }).await;
                    });
                }
            }
        }
    }
}
