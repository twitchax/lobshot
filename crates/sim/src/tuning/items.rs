//! Battle items: the slots each fills (WP06, AK), the delay it adds (SW-gp first), and what it
//! does (SW-gp first, OpenBound where GunBound's sources give no number).

use super::{DamageRange, Tuned, WeaponType, cited, picked};

/// A loadout holds six small items, or three big ones.
pub const LOADOUT_SLOTS: Tuned<u8> = cited(6, &["WP06", "AK"]);

// Shares of max HP are whole percents, worked in integers: as floats, a Bionic mobile's 12% + 5%
// Heal came to 169.99999 of 1000 HP, and floored to 169.

/// Heal's share of max HP, in percent. SW-gp says only "small heal".
pub const HEAL_PERCENT: Tuned<u32> = cited(12, &["OB"]);

/// Big Heal's share of max HP, in percent. OpenBound heals 30% (CONTRADICTION).
pub const BIG_HEAL_PERCENT: Tuned<u32> = cited(25, &["SW-gp"]);

/// The percent Bionic mobiles gain on top of either heal's share (Big Heal: 25% becomes 30%).
/// OpenBound gives 3% (CONTRADICTION).
pub const BIONIC_HEAL_EXTRA: Tuned<u32> = cited(5, &["SW-gp"]);

/// Sacrifice's cost, in percent of max HP; it never leaves its user below 1 HP.
pub const SACRIFICE_PERCENT: Tuned<u32> = cited(8, &["SW-gp", "OB"]);

/// The damage multiplier on the turn's shot from Power Up or Sacrifice.
pub const DAMAGE_BOOST: Tuned<f64> = cited(1.33, &["SW-gp"]);

/// Dig's crater multiplier (+25% land damage); damage reach is unchanged.
pub const DIG_CRATER: Tuned<f64> = cited(1.25, &["SW-gp"]);

/// Thunder's bolt, wherever each projectile lands. GunBound gives no number, and OpenBound's
/// Electricity bolt deals a flat 30.
pub const THUNDER_DAMAGE: Tuned<DamageRange> = picked(
    DamageRange { min: 76, max: 80 },
    "The one GunBound bolt with a stated damage is the slot machine's (WP06), so Thunder strikes as hard.",
);
pub const THUNDER_WEAPON: Tuned<WeaponType> = picked(WeaponType::Electric, "Lightning is electric.");

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
