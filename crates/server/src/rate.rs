//! Rate limiting: the one path room creation and every message go through (`DESIGN.md`, "Scope"),
//! so abuse features (filters, bans) are cheap to add later. A token bucket per key: per client IP
//! for room creation, per socket for messages.

use std::{
    collections::HashMap,
    hash::Hash,
    sync::{Mutex, PoisonError},
    time::Instant,
};

/// Why a request over the limit was refused.
pub const TOO_FAST: &str = "slow down: too many requests";

/// A bucket's size and refill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rate {
    /// Tokens a full bucket holds: the burst allowed after a quiet spell.
    pub burst: f64,
    /// Tokens added back per second.
    pub per_second: f64,
}

impl Rate {
    /// Messages, per socket: generous enough for a walk streamed step by step.
    pub const MESSAGES: Self = Self { burst: 60.0, per_second: 30.0 };
    /// Room creation, per client IP: about 10 a minute.
    pub const ROOMS: Self = Self { burst: 10.0, per_second: 10.0 / 60.0 };
}

/// A token bucket. Starts full.
#[derive(Debug, Clone)]
pub struct Bucket {
    rate: Rate,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    #[must_use]
    pub fn new(rate: Rate, now: Instant) -> Self {
        Self { rate, tokens: rate.burst, last: now }
    }

    /// Takes one token if there is one. `false` means over the limit: refuse the request.
    pub fn take(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens < 1.0 {
            return false;
        }
        self.tokens -= 1.0;
        true
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.rate.per_second).min(self.rate.burst);
        self.last = now;
    }

    /// Whether it has refilled completely, so dropping it loses nothing.
    fn is_full(&mut self, now: Instant) -> bool {
        self.refill(now);
        self.tokens >= self.rate.burst
    }
}

/// One bucket per key, shared across tasks.
#[derive(Debug)]
pub struct Limiter<K> {
    rate: Rate,
    buckets: Mutex<HashMap<K, Bucket>>,
}

/// Past this many keys, full buckets are dropped: they carry no state a new bucket wouldn't.
const PRUNE_AT: usize = 4096;

impl<K: Eq + Hash> Limiter<K> {
    #[must_use]
    pub fn new(rate: Rate) -> Self {
        Self {
            rate,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Takes a token from `key`'s bucket.
    pub fn take(&self, key: K, now: Instant) -> bool {
        // A panic while holding the lock leaves only token counts behind, which are safe to reuse.
        let mut buckets = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        if buckets.len() >= PRUNE_AT {
            buckets.retain(|_, bucket| !bucket.is_full(now));
        }
        buckets.entry(key).or_insert_with(|| Bucket::new(self.rate, now)).take(now)
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::time::Duration;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_bucket_allows_its_burst_then_refills_at_its_rate() {
        let start = Instant::now();
        let mut bucket = Bucket::new(Rate { burst: 3.0, per_second: 2.0 }, start);
        assert_eq!((0..4).map(|_| bucket.take(start)).collect::<Vec<_>>(), [true, true, true, false]);
        assert!(!bucket.take(start + Duration::from_millis(400)), "0.8 of a token");
        assert!(bucket.take(start + Duration::from_millis(500)));
        assert!(!bucket.take(start + Duration::from_millis(500)));
        // A long quiet spell refills only up to the burst.
        let later = start + Duration::from_secs(60);
        assert_eq!((0..4).filter(|_| bucket.take(later)).count(), 3);
    }

    #[test]
    fn messages_allow_a_streamed_walk() {
        let start = Instant::now();
        let mut bucket = Bucket::new(Rate::MESSAGES, start);
        // Ten seconds of walking at 30 steps a second never runs dry.
        let walked = (0..300).all(|step| bucket.take(start + Duration::from_millis(step * 1000 / 30)));
        assert!(walked);
    }

    #[test]
    fn room_creation_allows_about_ten_a_minute() {
        let start = Instant::now();
        let limiter = Limiter::new(Rate::ROOMS);
        assert_eq!((0..11).filter(|_| limiter.take("1.2.3.4", start)).count(), 10);
        assert!(limiter.take("5.6.7.8", start), "each key has its own bucket");
        assert!(!limiter.take("1.2.3.4", start + Duration::from_secs(5)));
        assert!(limiter.take("1.2.3.4", start + Duration::from_secs(7)));
    }

    #[test]
    fn full_buckets_are_pruned_once_there_are_many() {
        let start = Instant::now();
        let limiter = Limiter::new(Rate::ROOMS);
        for key in 0..PRUNE_AT {
            limiter.take(key, start);
        }
        // An hour later every bucket is full again, so the next take prunes them all.
        limiter.take(usize::MAX, start + Duration::from_secs(3600));
        assert_eq!(limiter.buckets.lock().unwrap().len(), 1);
    }
}
