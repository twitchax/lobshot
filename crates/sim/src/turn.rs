//! GunBound's delay system. Every turn adds delay (the mobile's base, the shot or pass, items,
//! and time spent), and the living player with the least accumulated delay moves next.

use serde::{Deserialize, Serialize};

use crate::{
    tuning::{PASS_DELAY, TIE_GOES_TO_LONGEST_WAITING},
    units::Centis,
};

/// A player's place in the turn order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clock {
    /// Accumulated delay.
    pub delay: u32,
    /// The turn number this player last acted on. Before anyone acts it is negative, ranked by the
    /// server's starting order, so the first in that order counts as waiting longest.
    pub last_turn: i64,
}

/// What a turn ended with, for its delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// A shot with this shot delay (S1, S2 or SS).
    Shot(u32),
    Pass,
    /// An item that ends the turn by itself (Swap): nothing on top of the item's own delay.
    Item,
}

/// Delay for one turn: base + shot (or the pass cost) + items + time at `per_second`.
#[must_use]
pub fn turn_delay(base: u32, ending: Ending, items: u32, per_second: u32, time: Centis) -> u32 {
    let action = match ending {
        Ending::Shot(delay) => delay,
        Ending::Pass => PASS_DELAY.value,
        Ending::Item => 0,
    };
    // Time is charged in whole delay points, rounding down.
    base + action + items + per_second * time.0 / 100
}

/// The player who moves next among `clocks` (index, clock) of living players: least delay, then
/// the tie-break, then seat order.
pub fn next<'a>(clocks: impl IntoIterator<Item = (usize, &'a Clock)>) -> Option<usize> {
    clocks
        .into_iter()
        .min_by_key(|&(seat, clock)| {
            // Longest waiting first means the smallest last turn; otherwise the latest goes again.
            let recency = if TIE_GOES_TO_LONGEST_WAITING.value { clock.last_turn } else { -clock.last_turn };
            (clock.delay, recency, seat)
        })
        .map(|(seat, _)| seat)
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn delay_adds_base_shot_items_and_time() {
        // Armor S1 with no time spent is GunBound's 770 (SW-mds).
        assert_eq!(turn_delay(520, Ending::Shot(250), 0, 10, Centis(0)), 770);
        // A pass is base + 200 (SW-mds's 720), plus time.
        assert_eq!(turn_delay(520, Ending::Pass, 0, 10, Centis(0)), 720);
        assert_eq!(turn_delay(520, Ending::Shot(250), 0, 10, Centis(1234)), 770 + 123);
        assert_eq!(turn_delay(490, Ending::Shot(250), 0, 12, Centis(1000)), 740 + 120, "Turtle pays 12 per second");
        // Dual on an Armor S1: the item's 600 plus Armor's 40 surcharge (SW-mds).
        assert_eq!(turn_delay(520, Ending::Shot(250), 640, 10, Centis(0)), 1410);
        assert_eq!(turn_delay(480, Ending::Item, 50, 10, Centis(0)), 530, "a Swap pays only its item delay");
    }

    #[test]
    fn least_delay_moves_next() {
        let clocks = [Clock { delay: 900, last_turn: 1 }, Clock { delay: 770, last_turn: 2 }, Clock { delay: 800, last_turn: 0 }];
        assert_eq!(next(clocks.iter().enumerate()), Some(1));
    }

    #[test]
    fn ties_go_to_the_longest_waiting_then_the_lower_seat() {
        let clocks = [Clock { delay: 770, last_turn: 5 }, Clock { delay: 770, last_turn: 3 }, Clock { delay: 770, last_turn: 3 }];
        assert_eq!(next(clocks.iter().enumerate()), Some(1));
        assert_eq!(next(clocks.iter().enumerate().filter(|&(seat, _)| seat != 1)), Some(2));
        assert_eq!(next(std::iter::empty()), None);
    }
}
