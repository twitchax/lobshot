//! The dead players' slot machine (WP06, the only source; mechanics.md section 5.2): when a spin
//! may come, how long a drop takes to fall, and what each drop does when it lands.

use super::{DamageRange, Tuned, WeaponType, cited, picked};

/// A drop lands at the end of the fourth turn to finish after its spin (the turn it was spun in
/// counts as the first).
pub const FALL_TURNS: Tuned<i64> = cited(4, &["WP06"]);

/// Rounds (one turn per seat, as weather counts them) between one seat's spins.
pub const RECHARGE_ROUNDS: Tuned<i64> = picked(1, "WP06 gives no rate; once a round keeps the dead in the game without burying the living in drops.");

/// Gravity on a falling drop, px/s².
pub const GRAVITY: Tuned<f64> = picked(600.0, "Close to the shots' (Coilback's 622): a drop falls like anything else.");

/// Dynamite: blows up where it lands; the wind carries it as it falls.
pub const DYNAMITE_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 50, max: 60 }, &["WP06"]);
pub const DYNAMITE_BLAST: Tuned<f64> = picked(25.0, "A small stick of dynamite: a little under Coilback's S1.");
pub const DYNAMITE_WEAPON: Tuned<WeaponType> = picked(WeaponType::Explosion, "Dynamite explodes.");
/// How strongly the wind pushes dynamite, against a shot's 1.0: WP06 says only "wind-affected".
pub const DYNAMITE_WIND: Tuned<f64> = picked(1.0, "WP06 gives no strength, so the reference shot's.");

/// The hammer: lands hard on whatever is under it, straight down.
pub const HAMMER_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 50, max: 50 }, &["WP06"]);
pub const HAMMER_BLAST: Tuned<f64> = picked(15.0, "A hammer dents; it doesn't blast.");
pub const HAMMER_WEAPON: Tuned<WeaponType> = picked(WeaponType::Physical, "A hammer hits.");

/// The bolt: strikes straight down its column at once, so the wind can't move it.
pub const BOLT_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 76, max: 80 }, &["WP06"]);
pub const BOLT_WEAPON: Tuned<WeaponType> = picked(WeaponType::Electric, "Lightning is electric.");

/// The chest heals whoever it lands on by this share of their max HP. WP06 lists treasure chests
/// with no effect.
pub const CHEST_HEAL: Tuned<f64> = picked(0.12, "No gold in lobshot, so a small heal: the Heal item's share.");
