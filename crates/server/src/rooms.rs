//! The room registry: live rooms by code. Creating a room mints an unguessable code and starts its
//! task; a room that ends removes itself. In memory only (`DESIGN.md`, "Rooms"): a restart drops
//! every room.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, PoisonError, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};

use lobshot_protocol::RoomCode;

use crate::{
    maps::Maps,
    room::{
        ConnId, Room, Timing,
        task::{self, RoomHandle},
    },
};

/// Every live room. Cheap to clone: clones share the registry.
#[derive(Debug, Clone)]
pub struct Rooms(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    rooms: Mutex<HashMap<RoomCode, RoomHandle>>,
    maps: Arc<Maps>,
    timing: Timing,
    next_conn: AtomicU64,
}

impl Rooms {
    #[must_use]
    pub fn new(maps: Arc<Maps>, timing: Timing) -> Self {
        Self(Arc::new(Inner {
            rooms: Mutex::new(HashMap::new()),
            maps,
            timing,
            next_conn: AtomicU64::new(0),
        }))
    }

    /// Opens a room under a fresh code and starts its task. Must run inside a tokio runtime.
    #[must_use]
    pub fn create(&self) -> RoomCode {
        let mut rng = rand::rng();
        let mut rooms = self.lock();
        let code = loop {
            let code = RoomCode::from_indices(std::array::from_fn(|_| rng.random_range(0..RoomCode::ALPHABET.len()).try_into().unwrap_or(0)));
            if !rooms.contains_key(&code) {
                break code;
            }
        };
        let room = Room::new(code.clone(), Arc::clone(&self.0.maps), self.0.timing, StdRng::from_rng(&mut rng), Instant::now());
        // Weak, so a room's task doesn't keep the registry alive. Codes are unique among live
        // rooms, so the entry under this code is this room's until it removes it.
        let registry = Arc::downgrade(&self.0);
        let key = code.clone();
        let handle = task::spawn(room, move || remove(&registry, &key));
        rooms.insert(code.clone(), handle);
        tracing::info!(room = code.as_str(), live = rooms.len(), "room created");
        code
    }

    /// The live room under `code`.
    #[must_use]
    pub fn get(&self, code: &RoomCode) -> Option<RoomHandle> {
        self.lock().get(code).cloned()
    }

    /// How many rooms are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A fresh socket id.
    #[must_use]
    pub fn next_conn(&self) -> ConnId {
        ConnId(self.0.next_conn.fetch_add(1, Ordering::Relaxed))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<RoomCode, RoomHandle>> {
        // The map holds only handles; one a panicking thread left behind is still consistent.
        self.0.rooms.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn remove(registry: &Weak<Inner>, code: &RoomCode) {
    if let Some(inner) = registry.upgrade() {
        inner.rooms.lock().unwrap_or_else(PoisonError::into_inner).remove(code);
        tracing::info!(room = code.as_str(), "room ended");
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::{collections::HashSet, path::Path, time::Duration};

    use pretty_assertions::assert_eq;

    use super::*;

    fn rooms(timing: Timing) -> Rooms {
        Rooms::new(Arc::new(Maps::new(Path::new("no-such-dist"))), timing)
    }

    #[tokio::test]
    async fn created_rooms_get_distinct_valid_codes() {
        let rooms = rooms(Timing::new(Duration::ZERO));
        let codes: HashSet<RoomCode> = (0..100).map(|_| rooms.create()).collect();
        assert_eq!(codes.len(), 100);
        assert_eq!(rooms.len(), 100);
        for code in &codes {
            assert_eq!(&RoomCode::parse(code.as_str()).unwrap(), code);
            assert!(rooms.get(code).is_some());
        }
        assert!(rooms.get(&RoomCode::parse("aaaaaaaa").unwrap()).is_none());
    }

    #[tokio::test]
    async fn an_idle_room_leaves_the_registry() {
        let rooms = rooms(Timing {
            idle: Duration::from_millis(50),
            ..Timing::new(Duration::ZERO)
        });
        let code = rooms.create();
        assert!(rooms.get(&code).is_some());
        let gone = async {
            while rooms.get(&code).is_some() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        tokio::time::timeout(Duration::from_secs(5), gone).await.expect("the room ends once idle");
        assert!(rooms.is_empty());
    }

    #[test]
    fn socket_ids_are_unique() {
        let rooms = rooms(Timing::new(Duration::ZERO));
        assert_ne!(rooms.next_conn(), rooms.next_conn());
    }
}
