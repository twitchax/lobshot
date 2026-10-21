//! The bonus tally's thresholds: GunBound's bonuses (SW-gp; mechanics.md section 4), counted
//! without their gold.

use super::{Tuned, cited};

/// Kills in one shot for a multi-kill (GunBound's Shot of God: 2 or more).
pub const MULTI_KILL: Tuned<u32> = cited(2, &["SW-gp"]);

/// Damage to enemies in one shot for the three hit tiers. DragonBound's are 100/200/400
/// (DB-play; CONTRADICTION).
pub const HIT_TIERS: Tuned<[i32; 3]> = cited([150, 250, 500], &["SW-gp"]);

/// The damage to enemies a trick shot (tornado, mirror, high angle, boomerang, backshot) must deal
/// to count. GunBound states none.
pub const TRICK_DAMAGE: Tuned<i32> = cited(50, &["DB-play"]);

/// A high-angle shot is aimed at least this high and flies at least as long as the thresholds
/// below. DragonBound's English guide says 2 s and 3 s (DB-play; CONTRADICTION); its Spanish guide
/// matches GunBound.
pub const HIGH_ANGLE_AIM: Tuned<i16> = cited(70, &["SW-gp", "DB-play"]);
pub const HIGH_ANGLE_SECONDS: Tuned<f64> = cited(2.5, &["SW-gp", "DB-es-play"]);
pub const ULTRA_HIGH_ANGLE_SECONDS: Tuned<f64> = cited(4.0, &["SW-gp", "DB-es-play"]);
