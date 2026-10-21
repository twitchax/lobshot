//! How much an explosion hurts each mobile. No public source gives GunBound's formula (docs/research
//! /mechanics.md, section 4), so this is our pick, shaped by what the sources do say: a direct hit
//! deals the shot's listed range by how central it is, a near miss deals less the farther the blast
//! was, and armor class and false angle scale the result.

use crate::{
    body::Pose,
    tuning::{ArmorClass, DamageRange, FALSE_ANGLE_DAMAGE, Rect, WeaponType},
    units::Vec2,
};

/// One explosion, as the damage model sees it.
#[derive(Debug, Clone, Copy)]
pub struct Blast {
    pub center: Vec2,
    pub radius: f64,
    pub damage: DamageRange,
    pub weapon: WeaponType,
    /// Whether the shot was fired outside the shooter's true-angle range.
    pub false_angle: bool,
    /// Weather's multiplier on the shot (a Force beam), 1.0 otherwise.
    pub boost: f64,
}

/// A mobile in the blast's way.
#[derive(Debug, Clone, Copy)]
pub struct Target {
    pub pose: Pose,
    pub hitbox: Rect,
    pub armor: ArmorClass,
}

/// Damage to `target`. `direct` means the projectile struck this mobile's hitbox.
///
/// - Direct: `max` dead center, falling linearly to `min` at the hitbox's corner.
/// - Otherwise: `min` touching the hitbox, falling linearly to 0 at the blast's edge.
#[must_use]
pub fn damage(blast: &Blast, target: &Target, direct: bool) -> i32 {
    let DamageRange { min, max } = blast.damage;
    let (min, max) = (f64::from(min), f64::from(max));
    let base = if direct {
        let half_diagonal = (target.hitbox.w * target.hitbox.w + target.hitbox.h * target.hitbox.h).sqrt() / 2.0;
        let off_center = (blast.center.distance(target.pose.center(target.hitbox)) / half_diagonal).clamp(0.0, 1.0);
        max - (max - min) * off_center
    } else {
        let gap = target.pose.gap(target.hitbox, blast.center);
        if gap >= blast.radius {
            return 0;
        }
        min * (1.0 - gap / blast.radius)
    };
    scale(base, blast.weapon, target.armor, blast.false_angle, blast.boost)
}

/// `base` damage from a `weapon` against `armor`, scaled for false angle and `boost` (weather,
/// items), rounding down.
#[must_use]
pub fn scale(base: f64, weapon: WeaponType, armor: ArmorClass, false_angle: bool, boost: f64) -> i32 {
    let angle = if false_angle { FALSE_ANGLE_DAMAGE.value } else { 1.0 };
    let damage = (base * weapon.against(armor) * angle * boost).floor();
    // Bounded by a few times the shot's max damage, far inside i32.
    #[allow(clippy::cast_possible_truncation)]
    let damage = damage as i32;
    damage
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::units::Facing;

    fn target() -> Target {
        Target {
            pose: Pose {
                foot: Vec2::new(500.0, 400.0),
                facing: Facing::Left,
                tilt: 0.0,
            },
            hitbox: Rect { x: -30.0, y: -60.0, w: 60.0, h: 60.0 },
            armor: ArmorClass::Mechanical,
        }
    }

    fn blast(center: Vec2) -> Blast {
        Blast {
            center,
            radius: 30.0,
            damage: DamageRange { min: 120, max: 165 },
            weapon: WeaponType::Explosion,
            false_angle: false,
            boost: 1.0,
        }
    }

    #[test]
    fn a_dead_center_hit_deals_max_and_an_edge_hit_min() {
        let target = target();
        assert_eq!(damage(&blast(Vec2::new(500.0, 370.0)), &target, true), 165);
        assert_eq!(damage(&blast(Vec2::new(470.0, 340.0)), &target, true), 120, "the corner");
    }

    #[test]
    fn near_misses_fall_off_to_nothing_at_the_blast_edge() {
        let target = target();
        // Hitbox spans x 470..530; 15 px past its edge is half the 30 px radius.
        assert_eq!(damage(&blast(Vec2::new(545.0, 370.0)), &target, false), 60);
        assert_eq!(damage(&blast(Vec2::new(560.0, 370.0)), &target, false), 0);
        assert_eq!(damage(&blast(Vec2::new(531.0, 370.0)), &target, false), 116);
    }

    #[test]
    fn false_angle_and_armor_scale_damage() {
        let target = target();
        let mut shot = blast(Vec2::new(500.0, 370.0));
        shot.false_angle = true;
        assert_eq!(damage(&shot, &target, true), 115, "165 x 0.70");
        shot.false_angle = false;
        shot.weapon = WeaponType::Electric;
        assert_eq!(damage(&shot, &target, true), 171, "165 x 1.04 against Mechanical");
        shot.weapon = WeaponType::Explosion;
        shot.boost = 1.33;
        assert_eq!(damage(&shot, &target, true), 219, "165 x 1.33 through Force");
    }
}
