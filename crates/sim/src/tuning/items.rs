//! Battle items: the slots each fills (WP06, AK) and the delay it adds (SW-gp first). What each
//! item does, and its effect numbers, land with the item rules.

use super::{Tuned, cited};

/// A loadout holds six small items, or three big ones.
pub const LOADOUT_SLOTS: Tuned<u8> = cited(6, &["WP06", "AK"]);

/// What an item costs to carry and to use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ItemTuning {
    pub slots: Tuned<u8>,
    /// Added to the turn's delay when it is used.
    pub delay: Tuned<u32>,
}

pub const DUAL: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(600, &["SW-gp", "WP06"]),
};

/// OpenBound charges 450 and DragonBound 400 (CONTRADICTION); GunBound's is 250.
pub const DUAL_PLUS: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(250, &["SW-gp"]),
};

/// WP06 and DragonBound charge 100 (CONTRADICTION); SW-gp says 150.
pub const TELEPORT: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(150, &["SW-gp"]),
};

pub const POWER_UP: ItemTuning = ItemTuning {
    slots: cited(1, &["WP06", "AK"]),
    delay: cited(150, &["SW-gp"]),
};

/// GunBound's Blood.
pub const SACRIFICE: ItemTuning = ItemTuning {
    slots: cited(1, &["WP06", "AK"]),
    delay: cited(0, &["SW-gp"]),
};

/// GunBound's Bunge Shot. OpenBound charges 80 (CONTRADICTION).
pub const DIG: ItemTuning = ItemTuning {
    slots: cited(1, &["WP06", "AK"]),
    delay: cited(50, &["SW-gp"]),
};

/// GunBound's Energy Up.
pub const HEAL: ItemTuning = ItemTuning {
    slots: cited(1, &["WP06", "AK"]),
    delay: cited(100, &["SW-gp"]),
};

/// GunBound's Energy Up 2. OpenBound charges 350 (CONTRADICTION).
pub const BIG_HEAL: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(300, &["SW-gp"]),
};

/// GunBound's Change Wind. OpenBound charges 120 (CONTRADICTION).
pub const WIND_FLIP: ItemTuning = ItemTuning {
    slots: cited(1, &["WP06", "AK"]),
    delay: cited(150, &["SW-gp"]),
};

/// WP06 charges 100 (CONTRADICTION); SW-gp says 200.
pub const THUNDER: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(200, &["SW-gp"]),
};

/// GunBound's Team Teleport.
pub const SWAP: ItemTuning = ItemTuning {
    slots: cited(2, &["WP06", "AK"]),
    delay: cited(50, &["SW-gp"]),
};
