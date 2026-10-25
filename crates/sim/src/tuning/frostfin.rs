//! Frostfin: a narwhal on an ice sled, with GunBound Ice's behavior. Icicles fly from the snow
//! cannon on its back; ice balls and the snowflake leave its tusk, a lower muzzle (`second_barrel`).
//! Geometry (hitbox, probes, cannon, tusk) matches its drawing in
//! `assets/mobiles/frostfin/rig.ron`. Its sources are gathered in `docs/research/frostfin.md`.

use super::{AimRange, ArmorClass, DamageRange, MobileTuning, Rect, SecondBarrel, ShotTuning, Tuned, WeaponType, cited, fitted, picked};
use crate::game::Shot;

pub const MOBILE: MobileTuning = MobileTuning {
    name: "Frostfin",
    behaves_like: "Ice",
    // The most HP of any mobile (SW-mob, GF-ice). GF-lee's 2003 guide says about 1175.
    hp: cited(1200, &["SW-mob", "CR-mob", "DB-mob", "GF-ice"]),
    shield: cited(0, &["CR-mob", "GF-ice"]),
    shield_regen: cited(0, &["CR-mob", "GF-ice"]),
    armor: cited(ArmorClass::Bionic, &["SW-mob", "WP06", "DB-mob"]),
    weapon: cited(WeaponType::Physical, &["SW-mob", "WP06", "DB-mob"]),
    base_delay: cited(490, &["CR-mob", "SW-mds", "SW-mob", "GF-ice"]),
    delay_per_second: super::DELAY_PER_SECOND,
    item_surcharge: picked(0, "Only Armor pays extra for items (SW-mds)."),
    // GF-ice calls all three shots light; the one per-mobile power the charts give is CR-fixed's.
    gravity: fitted(GRAVITY, &["CR-fixed"], "60 degrees lands one screen at 2.33 bars from the cannon, zero wind, flat ground."),
    wind_sensitivity: picked(1.0, "No source sets it apart from the reference shot."),
    move_budget: cited(90, &["OB"]),
    climb_px: super::CLIMB_PX,
    hitbox: picked(
        Rect { x: -32.0, y: -50.0, w: 64.0, h: 50.0 },
        "Frostfin's drawn sled and narwhal, the cannon's mount included. The rider, the tusk, the tail flukes and the sled's curled prow are cosmetic.",
    ),
    ground_probes: picked([-20.0, 18.0], "Frostfin's drawn sled runners, under its rear and front struts."),
    cannon_pivot: picked((5.0, -48.0), "Frostfin's drawn snow cannon, on a swivel on its back, ahead of the rider."),
    barrel_length: picked(25.0, "Frostfin's drawn snow cannon, swivel to mouth."),
    s1: ShotTuning {
        delay: cited(250, &["CR-mob", "SW-mds", "SW-mob", "GF-ice"]),
        aim: cited(
            AimRange {
                min: 20,
                max: 70,
                true_min: 30,
                true_max: 60,
            },
            &["CR-mob", "GF-ice", "OB"],
        ),
    },
    s2: ShotTuning {
        delay: cited(400, &["CR-mob", "SW-mds", "SW-mob", "GF-ice"]),
        aim: cited(
            AimRange {
                min: 20,
                max: 70,
                true_min: 30,
                true_max: 60,
            },
            &["CR-mob", "GF-ice", "OB"],
        ),
    },
    ss: ShotTuning {
        delay: cited(800, &["CR-mob", "SW-mob", "GF-ice"]),
        aim: cited(
            AimRange {
                min: 30,
                max: 60,
                true_min: 30,
                true_max: 60,
            },
            &["CR-mob", "GF-ice", "OB"],
        ),
    },
    // S2 leaves "the trunk", lower than the cannon: it can jam in a wall S1 clears (SW-mob, WP06),
    // and lands about where S1 would in the open (GF-ice). SS leaves it too (GF-lee). GF-lee (2003)
    // has it higher than the cannon (CONTRADICTION).
    second_barrel: Some(SecondBarrel {
        pivot: picked((26.0, -26.0), "Frostfin's drawn tusk socket, on its upper lip: a muzzle lower and farther forward than the cannon's."),
        length: picked(30.0, "Frostfin's drawn tusk, socket to tip."),
        shots: &[Shot::S2, Shot::Ss],
    }),
};

/// Fitted by `cargo test -p lobshot-sim --test formula -- --ignored --nocapture tool_fit`, which
/// rewrites this line.
const GRAVITY: f64 = 558.4;

/// S1: a big icicle from the cannon on its back that "removes a lot of land" (SW-mob): "large,
/// wide, oval holes" (WP06), as much as Trico's S1 (GF-ice). The hole is round here.
pub const S1_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 120, max: 160 }, &["SW-mob", "DB-mob"]);
pub const S1_BLAST: Tuned<f64> = picked(30.0, "Coilback's bunging reach: the icicle's damage is an ordinary shell's.");
pub const S1_CRATER: Tuned<f64> = picked(42.0, "Removes a lot of land (SW-mob): more than any other S1 so far, short of Coilback's expanded SS.");

/// S2: a ball of ice from the tusk. Every mobile it hurts turns brittle (`BRITTLE_PER_HIT`).
pub const S2_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 150, max: 170 }, &["SW-mob", "DB-mob"]);
pub const S2_BLAST: Tuned<f64> = picked(20.0, "The ice crumbles where it hits and doesn't do much bunge (GF-lee).");

/// SS: a snowflake from the tusk that "turns into an enormous snowflake on contact" (WP06) and deals
/// "a little more" (SW-mob, WP06) over a much wider blast. SW-mob and DB-mob give it S2's 150-170
/// base damage; GF-ice and GF-lee average it at 250 (CONTRADICTION), which balance (PRD-0006 T-010)
/// takes: at S2's damage for twice its delay, a 1v1 SS was all cost, and Frostfin, which fired it
/// 1% of its turns, won 41% at Hard. Every mobile it hurts turns brittle, as S2 does: WP06 has it
/// frost "anything in the snowflake's radius"; GF-ice (2007), only a mobile it hits directly
/// (CONTRADICTION).
pub const SS_DAMAGE: Tuned<DamageRange> = cited(DamageRange { min: 230, max: 270 }, &["GF-ice", "GF-lee"]);
pub const SS_BLAST: Tuned<f64> = picked(45.0, "An enormous snowflake (WP06): the widest blast so far.");
pub const SS_CRATER: Tuned<f64> = picked(30.0, "Snow: it spreads wide but digs no more than an ordinary shell.");

/// How much more Physical damage a mobile takes for each S2 or SS that hurts it, in percent of the
/// damage, for the rest of the game (WP06: even after a respawn in Score mode). GunBound's SS took
/// 20 until a patch made it 5, like S2 (WP06, GF-ice).
pub const BRITTLE_PER_HIT: Tuned<u8> = cited(5, &["SW-mob", "WP06", "GF-ice"]);
/// The most brittle a mobile gets. OB caps it at 35 and GF-lee (2003) at 20 (CONTRADICTION).
pub const BRITTLE_MAX: Tuned<u8> = cited(50, &["SW-mob", "WP06", "GF-ice"]);
/// The one weapon type brittleness weakens a mobile against: only Physical ("Hit") shots deal more,
/// whoever fires them, teammates included (WP06, GF-ice). GF-lee says the SS weakens it to Electric
/// (CONTRADICTION, 2003).
pub const BRITTLE_AGAINST: Tuned<WeaponType> = cited(WeaponType::Physical, &["SW-mob", "WP06", "GF-ice"]);
