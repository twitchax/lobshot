//! A mobile's pose in the world, and the geometry that hangs off it: hitbox, muzzle, launch
//! direction. Body frame: origin at the center of the ground contact, `+x` forward, `y` down.

use serde::{Deserialize, Serialize};

use crate::{
    game::Shot,
    tuning::{MobileTuning, Rect},
    units::{Facing, Vec2, to_radians},
};

/// How far `aim` points above the body's horizon on the side its shot leaves: the aim itself, or for
/// one over the back (past straight up, `AimRange::rear`), its angle above the way the body faces
/// away from. What a player reads and GunBound's angle rules (High Angle) mean.
#[must_use]
pub fn elevation(aim: i16) -> i16 {
    if aim > 90 { 180 - aim } else { aim }
}

/// Where a mobile stands and which way it leans.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// The ground contact point, in world pixels.
    pub foot: Vec2,
    pub facing: Facing,
    /// The ground's slope under the mobile, radians counterclockwise on screen (rising to the
    /// right is positive), whichever way the mobile faces.
    pub tilt: f64,
}

impl Pose {
    /// A body-frame point in world coordinates: mirror for facing, rotate by the slope, translate.
    #[must_use]
    pub fn to_world(&self, body: Vec2) -> Vec2 {
        self.foot + Vec2::new(body.x * self.facing.signum(), body.y).rotated(self.tilt)
    }

    /// A world point in the body frame.
    #[must_use]
    pub fn to_body(&self, world: Vec2) -> Vec2 {
        let local = (world - self.foot).rotated(-self.tilt);
        Vec2::new(local.x * self.facing.signum(), local.y)
    }

    /// The unit vector a shot leaves along at `aim` degrees above the body's forward direction.
    #[must_use]
    pub fn aim_direction(&self, aim: i16) -> Vec2 {
        let aim = to_radians(f64::from(aim));
        Vec2::new(libm::cos(aim) * self.facing.signum(), -libm::sin(aim)).rotated(self.tilt)
    }

    /// Where `shot` leaves its barrel at `aim`.
    #[must_use]
    pub fn muzzle(&self, mobile: &MobileTuning, shot: Shot, aim: i16) -> Vec2 {
        let barrel = mobile.barrel(shot);
        let (px, py) = barrel.pivot;
        let aim = to_radians(f64::from(aim));
        self.to_world(Vec2::new(px + barrel.length * libm::cos(aim), py - barrel.length * libm::sin(aim)))
    }

    /// Whether a world point is inside the hitbox.
    #[must_use]
    pub fn hit(&self, hitbox: Rect, world: Vec2) -> bool {
        HitFrame::new(self, hitbox).contains(world)
    }

    /// Distance from a world point to the hitbox (0 inside).
    #[must_use]
    pub fn gap(&self, hitbox: Rect, world: Vec2) -> f64 {
        let p = self.to_body(world);
        let nearest = Vec2::new(p.x.clamp(hitbox.x, hitbox.x + hitbox.w), p.y.clamp(hitbox.y, hitbox.y + hitbox.h));
        p.distance(nearest)
    }

    /// The hitbox's center in world coordinates.
    #[must_use]
    pub fn center(&self, hitbox: Rect) -> Vec2 {
        self.to_world(Vec2::new(hitbox.x + hitbox.w / 2.0, hitbox.y + hitbox.h / 2.0))
    }
}

/// A pose's hitbox with its rotation worked out once, for the per-tick collision loop. The math is
/// `Pose::to_body`'s, term for term, so it agrees with it to the bit.
#[derive(Debug, Clone, Copy)]
pub struct HitFrame {
    foot: Vec2,
    sign: f64,
    sin: f64,
    cos: f64,
    rect: Rect,
    /// No point farther than this from the foot can be inside.
    reach_squared: f64,
}

impl HitFrame {
    #[must_use]
    pub fn new(pose: &Pose, rect: Rect) -> Self {
        let corners = [(rect.x, rect.y), (rect.x + rect.w, rect.y), (rect.x, rect.y + rect.h), (rect.x + rect.w, rect.y + rect.h)];
        let reach_squared = corners.iter().map(|&(x, y)| x * x + y * y).fold(0.0, f64::max);
        Self {
            foot: pose.foot,
            sign: pose.facing.signum(),
            sin: libm::sin(-pose.tilt),
            cos: libm::cos(-pose.tilt),
            rect,
            reach_squared,
        }
    }

    #[must_use]
    pub fn contains(&self, world: Vec2) -> bool {
        let d = world - self.foot;
        if d.x * d.x + d.y * d.y > self.reach_squared {
            return false;
        }
        let local = Vec2::new(d.x * self.cos + d.y * self.sin, d.y * self.cos - d.x * self.sin);
        let (x, y) = (local.x * self.sign, local.y);
        x >= self.rect.x && x <= self.rect.x + self.rect.w && y >= self.rect.y && y <= self.rect.y + self.rect.h
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::{
        game::MobileKind,
        tuning::{coilback, frostfin},
    };

    fn pose(facing: Facing, tilt_degrees: f64) -> Pose {
        Pose {
            foot: Vec2::new(100.0, 200.0),
            facing,
            tilt: to_radians(tilt_degrees),
        }
    }

    fn close(a: Vec2, b: Vec2) -> bool {
        a.distance(b) < 1e-9
    }

    #[test]
    fn facing_left_mirrors_the_body() {
        let right = pose(Facing::Right, 0.0);
        let left = pose(Facing::Left, 0.0);
        assert!(close(right.to_world(Vec2::new(10.0, -5.0)), Vec2::new(110.0, 195.0)));
        assert!(close(left.to_world(Vec2::new(10.0, -5.0)), Vec2::new(90.0, 195.0)));
    }

    #[test]
    fn body_and_world_round_trip() {
        for facing in [Facing::Left, Facing::Right] {
            let pose = pose(facing, 17.0);
            let body = Vec2::new(12.0, -30.0);
            assert!(close(pose.to_body(pose.to_world(body)), body));
        }
    }

    #[test]
    fn aim_is_relative_to_the_slope() {
        // On a slope rising to the right, a right-facing mobile's 30 degrees is 40 in the world.
        let direction = pose(Facing::Right, 10.0).aim_direction(30);
        assert!(close(direction, Vec2::new(libm::cos(to_radians(40.0)), -libm::sin(to_radians(40.0)))));
        // Facing left on the same slope, the mobile leans back: 30 degrees is 20 in the world.
        let direction = pose(Facing::Left, 10.0).aim_direction(30);
        assert!(close(direction, Vec2::new(-libm::cos(to_radians(20.0)), -libm::sin(to_radians(20.0)))));
    }

    #[test]
    fn an_aim_over_the_back_leaves_behind_at_its_elevation() {
        // 150 degrees is 30 above the way a right-facing mobile faces away from: up and to the left.
        let direction = pose(Facing::Right, 0.0).aim_direction(150);
        assert!(close(direction, Vec2::new(-libm::cos(to_radians(30.0)), -libm::sin(to_radians(30.0)))));
        assert_eq!(elevation(150), 30);
        assert_eq!((elevation(30), elevation(90)), (30, 90), "a forward aim is its own elevation");
        // On a slope rising to the right, a left-facing mobile's back points up the slope: 150 is 40
        // above the world's horizon, to the right.
        let direction = pose(Facing::Left, 10.0).aim_direction(150);
        assert!(close(direction, Vec2::new(libm::cos(to_radians(40.0)), -libm::sin(to_radians(40.0)))));
    }

    #[test]
    fn the_muzzle_clears_the_hitbox_at_every_aim() {
        let pose = pose(Facing::Right, 0.0);
        for kind in MobileKind::ALL {
            let mobile = kind.tuning();
            for shot in [Shot::S1, Shot::S2, Shot::Ss] {
                let range = shot.tuning(mobile).aim.value;
                for aim in range.min..=range.max {
                    assert!(!pose.hit(mobile.hitbox.value, pose.muzzle(mobile, shot, aim)), "{kind:?} {shot:?} at {aim}");
                }
            }
        }
    }

    #[test]
    fn a_shot_leaves_its_own_barrel() {
        let pose = pose(Facing::Right, 0.0);
        let frostfin = frostfin::MOBILE;
        let tusk = frostfin.second_barrel.unwrap();
        // At aim 0 a barrel points straight ahead, so its muzzle is its pivot plus its length.
        let (px, py) = tusk.pivot.value;
        assert!(close(pose.muzzle(&frostfin, Shot::S2, 0), pose.to_world(Vec2::new(px + tusk.length.value, py))));
        assert!(close(pose.muzzle(&frostfin, Shot::Ss, 0), pose.muzzle(&frostfin, Shot::S2, 0)));
        let (cx, cy) = frostfin.cannon_pivot.value;
        assert!(close(pose.muzzle(&frostfin, Shot::S1, 0), pose.to_world(Vec2::new(cx + frostfin.barrel_length.value, cy))));
        // The tusk is the lower muzzle at every aim the two share.
        for aim in 20..=70 {
            assert!(pose.muzzle(&frostfin, Shot::S2, aim).y > pose.muzzle(&frostfin, Shot::S1, aim).y + 10.0, "at {aim}");
        }
        // Every other mobile fires all three from its cannon.
        let coilback = coilback::MOBILE;
        assert!(close(pose.muzzle(&coilback, Shot::Ss, 30), pose.muzzle(&coilback, Shot::S1, 30)));
    }

    #[test]
    fn the_hit_frame_agrees_with_the_pose() {
        let hitbox = coilback::MOBILE.hitbox.value;
        for facing in [Facing::Left, Facing::Right] {
            let pose = pose(facing, 23.0);
            let frame = HitFrame::new(&pose, hitbox);
            for i in -40..40 {
                for j in -40..10 {
                    let world = pose.foot + Vec2::new(f64::from(i) * 1.7, f64::from(j) * 1.9);
                    let local = pose.to_body(world);
                    let by_pose = local.x >= hitbox.x && local.x <= hitbox.x + hitbox.w && local.y >= hitbox.y && local.y <= hitbox.y + hitbox.h;
                    assert_eq!(frame.contains(world), by_pose, "{facing:?} {world:?}");
                }
            }
        }
    }

    #[test]
    fn gap_is_zero_inside_and_the_distance_outside() {
        let hitbox = coilback::MOBILE.hitbox.value;
        let pose = pose(Facing::Right, 0.0);
        assert!(pose.hit(hitbox, pose.center(hitbox)));
        assert!(pose.gap(hitbox, pose.center(hitbox)).abs() < 1e-9);
        // 10 px past the front edge (body x = 34).
        assert!((pose.gap(hitbox, Vec2::new(100.0 + 44.0, 180.0)) - 10.0).abs() < 1e-9);
    }
}
