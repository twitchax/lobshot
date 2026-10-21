//! Battle items, GunBound's eleven under plain names, and the loadout a player brings into a game:
//! six slots, each item filling one or two (SW-gp). An item is used once, on its owner's turn, one
//! a turn. What each does lives in `game`; its numbers live in `tuning::items`.

use anyhow::ensure;
use serde::{Deserialize, Serialize};

use crate::{
    base::Res,
    tuning::items::{self, ItemTuning, LOADOUT_SLOTS},
};

/// The most items a loadout holds: every item fills at least one slot.
pub const MAX_ITEMS: usize = 6;

/// A battle item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Item {
    /// The selected shot (S1 or S2, never SS) fires twice.
    Dual,
    /// The selected shot, then the other of S1 and S2.
    DualPlus,
    /// The shot hurts no one and carries its shooter to where it lands.
    Teleport,
    /// This turn's shot hits harder.
    PowerUp,
    /// Pay some HP for a harder-hitting shot this turn.
    Sacrifice,
    /// This turn's shot carves bigger craters.
    Dig,
    /// A small heal.
    Heal,
    /// A big heal.
    BigHeal,
    /// The wind turns around.
    WindFlip,
    /// A bolt strikes wherever this turn's shot lands.
    Thunder,
    /// Trade places with your weakest living teammate, ending the turn.
    Swap,
}

impl Item {
    /// Every item, in the order the loadout screen lists them.
    pub const ALL: [Self; 11] = [
        Self::Dual,
        Self::DualPlus,
        Self::Teleport,
        Self::PowerUp,
        Self::Sacrifice,
        Self::Dig,
        Self::Heal,
        Self::BigHeal,
        Self::WindFlip,
        Self::Thunder,
        Self::Swap,
    ];

    /// The player-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dual => "Dual",
            Self::DualPlus => "Dual+",
            Self::Teleport => "Teleport",
            Self::PowerUp => "Power Up",
            Self::Sacrifice => "Sacrifice",
            Self::Dig => "Dig",
            Self::Heal => "Heal",
            Self::BigHeal => "Big Heal",
            Self::WindFlip => "Wind Flip",
            Self::Thunder => "Thunder",
            Self::Swap => "Swap",
        }
    }

    #[must_use]
    pub const fn tuning(self) -> &'static ItemTuning {
        match self {
            Self::Dual => &items::DUAL,
            Self::DualPlus => &items::DUAL_PLUS,
            Self::Teleport => &items::TELEPORT,
            Self::PowerUp => &items::POWER_UP,
            Self::Sacrifice => &items::SACRIFICE,
            Self::Dig => &items::DIG,
            Self::Heal => &items::HEAL,
            Self::BigHeal => &items::BIG_HEAL,
            Self::WindFlip => &items::WIND_FLIP,
            Self::Thunder => &items::THUNDER,
            Self::Swap => &items::SWAP,
        }
    }

    /// How many loadout slots it fills.
    #[must_use]
    pub const fn slots(self) -> u8 {
        self.tuning().slots.value
    }
}

/// The items a player brings into a game, in slot order, filling at most `LOADOUT_SLOTS` slots.
/// Checked whenever one is built or read, so a loadout from the wire or a log is always legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "Vec<Item>", into = "Vec<Item>")]
pub struct Loadout([Option<Item>; MAX_ITEMS]);

impl Loadout {
    /// # Errors
    ///
    /// Fails when the items fill more than `LOADOUT_SLOTS` slots.
    pub fn new(items: &[Item]) -> Res<Self> {
        let slots: u32 = items.iter().map(|item| u32::from(item.slots())).sum();
        ensure!(slots <= u32::from(LOADOUT_SLOTS.value), "a loadout fills at most {} slots, not {slots}", LOADOUT_SLOTS.value);
        let mut picked = [None; MAX_ITEMS];
        for (slot, &item) in picked.iter_mut().zip(items) {
            *slot = Some(item);
        }
        Ok(Self(picked))
    }

    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.0.iter().flatten().copied()
    }

    /// How many slots the items fill.
    #[must_use]
    pub fn slots(&self) -> u8 {
        self.items().map(Item::slots).sum()
    }

    /// The items by position, as a player's unused items start out.
    #[must_use]
    pub fn positions(&self) -> [Option<Item>; MAX_ITEMS] {
        self.0
    }
}

impl TryFrom<Vec<Item>> for Loadout {
    type Error = anyhow::Error;

    fn try_from(items: Vec<Item>) -> Res<Self> {
        Self::new(&items)
    }
}

impl From<Loadout> for Vec<Item> {
    fn from(loadout: Loadout) -> Self {
        loadout.items().collect()
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn a_loadout_fills_six_slots_with_one_and_two_slot_items() {
        let full = Loadout::new(&[Item::Dual, Item::PowerUp, Item::Heal, Item::Dig, Item::WindFlip]).unwrap();
        assert_eq!(full.slots(), 6);
        assert_eq!(full.items().count(), 5);
        assert!(Loadout::new(&[Item::Dual, Item::Teleport, Item::Thunder]).is_ok(), "three big items");
        assert!(Loadout::new(&[Item::Dual, Item::Teleport, Item::Thunder, Item::Heal]).is_err(), "seven slots");
        assert!(Loadout::new(&[Item::Heal; 7]).is_err());
        assert_eq!(Loadout::default().slots(), 0);
    }

    #[test]
    fn a_loadout_off_the_wire_is_checked_too() {
        let loadout = Loadout::new(&[Item::Swap, Item::Sacrifice]).unwrap();
        let json = serde_json::to_string(&loadout).unwrap();
        assert_eq!(json, r#"["Swap","Sacrifice"]"#);
        assert_eq!(serde_json::from_str::<Loadout>(&json).unwrap(), loadout);
        assert_eq!(postcard::from_bytes::<Loadout>(&postcard::to_allocvec(&loadout).unwrap()).unwrap(), loadout);
        assert!(serde_json::from_str::<Loadout>(r#"["Dual","Dual","Dual","Heal"]"#).is_err());
    }

    #[test]
    fn every_item_fills_one_or_two_slots() {
        for item in Item::ALL {
            assert!((1..=2).contains(&item.slots()), "{item:?}");
        }
    }
}
