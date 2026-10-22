//! Sudsotter (J.Frog): sponges that fly through mobiles, stick where they land and walk the edge of
//! the ground (`crawl`), S1 left and S2 right, going off three seconds later wherever they are; and
//! an SS blob of suds that walks the way it was moving, hurting every mobile it passes near, but
//! pops at once, weakly, on a direct hit. The weather leaves their damage alone (`WEATHER_PROOF`).

use super::{
    Gun,
    crawl::{self, Sense, Walk, Walked},
};
use crate::{
    ballistics::Flight,
    body::Pose,
    damage,
    game::{Contact, Effect, Game, Impact, MAX_SEATS, Senses, Shot, Track},
    tuning::{
        Rect,
        sudsotter::{
            CLIMB_SHARE, SPONGE_BLAST, SPONGE_DAMAGE, SPONGE_FUSE_SECONDS, SUDS_BLAST, SUDS_BLAST_DAMAGE, SUDS_DAMAGE, SUDS_FUSE_SECONDS, SUDS_REACH, SUDS_TOUCH_PER_SECOND, UNDER_MOBILE_SHARE,
            WALK_SPEED, WEATHER_PROOF,
        },
    },
    units::{Facing, Vec2, seconds, ticks},
    weather::Marks,
};

/// What a sponge's flight senses: it passes through mobiles (SW-mob, DB-mob).
const THROUGH_MOBILES: Senses = Senses { mobiles: false, terrain: true };

pub(super) fn fire(game: &mut Game, gun: &Gun, shot: Shot, effects: &mut Vec<Effect>) {
    let Some(flight) = gun.launch(game, 0, 0.0) else { return };
    match shot {
        Shot::S1 | Shot::S2 => {
            let track = game.track_sensing(flight, 0, 0, THROUGH_MOBILES);
            let Some(walked) = walk_on(game, &track, sponge_sense(shot), SPONGE_FUSE_SECONDS.value) else {
                effects.push(Effect::Fired { seat: gun.seat, legs: track.legs });
                return;
            };
            let marks = felt(track.marks);
            let blast = gun.blast(walked.end.at, SPONGE_BLAST.value, SPONGE_DAMAGE.value, marks.boost());
            effects.push(Effect::Fired {
                seat: gun.seat,
                legs: track.legs.into_iter().chain(walked.legs).collect(),
            });
            gun.land(game, &blast, SPONGE_BLAST.value, &walked.end, marks, effects);
        }
        Shot::Ss => {
            let track = game.track(flight, 0, 0);
            let marks = felt(track.marks);
            let facing = game.players()[usize::from(gun.seat)].facing;
            let walked = walk_on(game, &track, suds_sense(&track, facing), SUDS_FUSE_SECONDS.value);
            let Some(walked) = walked else {
                // A direct hit pops it at once; off the map, nothing.
                let blast = gun.blast(track.impact.at, SUDS_BLAST.value, SUDS_BLAST_DAMAGE.value, marks.boost());
                effects.push(Effect::Fired { seat: gun.seat, legs: track.legs });
                gun.land(game, &blast, SUDS_BLAST.value, &track.impact, marks, effects);
                return;
            };
            effects.push(Effect::Fired {
                seat: gun.seat,
                legs: track.legs.into_iter().chain(walked.legs.iter().copied()).collect(),
            });
            touch(game, gun, &walked, marks, effects);
            let blast = gun.blast(walked.end.at, SUDS_BLAST.value, SUDS_BLAST_DAMAGE.value, marks.boost());
            gun.land(game, &blast, SUDS_BLAST.value, &walked.end, marks, effects);
        }
    }
}

/// S1 walks left (counterclockwise round the ground), S2 right, on screen (WP06, SW-mob, CR-frog).
fn sponge_sense(shot: Shot) -> Sense {
    if shot == Shot::S1 { Sense::Counterclockwise } else { Sense::Clockwise }
}

/// The suds walk the way they were moving as they landed: right (clockwise) if heading right, so a
/// wind that blew them back turns their walk round too (SW-mob); straight down, the way the
/// shooter faces.
fn suds_sense(track: &Track, facing: Facing) -> Sense {
    let across = track.last.flight.velocity_at(track.last_tick).x;
    let rightward = if across == 0.0 { facing == Facing::Right } else { across > 0.0 };
    if rightward { Sense::Clockwise } else { Sense::Counterclockwise }
}

/// What the columns a shot passed through do to its damage: nothing, when it is weather-proof.
fn felt(marks: Marks) -> Marks {
    if WEATHER_PROOF.value { Marks::default() } else { marks }
}

/// The walk of a shot that landed on the ground at the end of `track`, for `fuse` seconds; `None`
/// if it didn't land there.
fn walk_on(game: &Game, track: &Track, sense: Sense, fuse: f64) -> Option<Walked> {
    if track.impact.contact != Contact::Terrain {
        return None;
    }
    let rules = Walk {
        speed: WALK_SPEED.value,
        climb_share: CLIMB_SHARE.value,
        under_mobile_share: UNDER_MOBILE_SHARE.value,
        fuse: ticks(fuse),
    };
    // Where it was a tick before it touched: in the open.
    let open = track.last.flight.at(track.last_tick.saturating_sub(1).max(track.last.from));
    Some(crawl::walk(game.terrain(), open, track.impact.at, track.impact.tick, sense, &rules, &game.hit_frames()))
}

/// One stretch of the suds' walk within reach of a mobile: its first and last ticks.
struct Touch {
    seat: u8,
    from: u32,
    to: u32,
}

/// Hurts each mobile the suds came within `SUDS_REACH` of, for as long as they stayed: each
/// stretch deals `SUDS_TOUCH_PER_SECOND` while it lasts, up to `SUDS_DAMAGE`'s touch share for the
/// whole walk, as it ends.
fn touch(game: &mut Game, gun: &Gun, walked: &Walked, marks: Marks, effects: &mut Vec<Effect>) {
    let targets: Vec<(u8, Pose, Rect)> = (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive)
        .map(|(seat, player)| (seat, player.pose(), player.tuning().hitbox.value))
        .collect();
    let mut touches: Vec<Touch> = Vec::new();
    for &(seat, pose, hitbox) in &targets {
        let mut open: Option<Touch> = None;
        for stay in &walked.stays {
            if pose.gap(hitbox, stay.at) <= SUDS_REACH.value {
                let touch = open.get_or_insert(Touch { seat, from: stay.from, to: stay.to });
                touch.to = stay.to;
            } else if let Some(done) = open.take() {
                touches.push(done);
            }
        }
        touches.extend(open);
    }
    touches.sort_by_key(|touch| (touch.to, touch.seat));
    let range = SUDS_DAMAGE.value;
    let most = f64::from(range.max - range.min);
    let mut dealt = [0.0; MAX_SEATS];
    let boost = marks.boost() * gun.salvo.boost;
    for touch in touches {
        let player = game.players()[usize::from(touch.seat)];
        let so_far = &mut dealt[usize::from(touch.seat)];
        let base = (SUDS_TOUCH_PER_SECOND.value * seconds(touch.to - touch.from)).min(most - *so_far);
        if !player.alive || base <= 0.0 {
            continue;
        }
        *so_far += base;
        effects.push(Effect::Pierced {
            seat: touch.seat,
            from: touch.from,
            to: touch.to,
        });
        let amount = damage::scale(base, gun.tuning.weapon.value, player.tuning().armor.value, gun.false_angle, boost);
        game.hurt(touch.seat, amount, effects);
    }
}

/// Where a shot from `seat` on `flight` does its work, for SearchBot's aim pass: where a sponge's
/// walk takes it before it goes off; for the suds, the point of their walk nearest `target` (on a
/// mobile, if one is within reach there). A direct hit with the suds counts as a near miss, since
/// it only pops.
pub(super) fn aim_point(game: &Game, seat: u8, shot: Shot, flight: Flight, target: Vec2) -> Impact {
    if shot != Shot::Ss {
        let track = game.track_sensing(flight, 0, 0, THROUGH_MOBILES);
        return walk_on(game, &track, sponge_sense(shot), SPONGE_FUSE_SECONDS.value).map_or(track.impact, |walked| walked.end);
    }
    let track = game.track(flight, 0, 0);
    let facing = game.players().get(usize::from(seat)).map_or(Facing::Right, |player| player.facing);
    let walked = walk_on(game, &track, suds_sense(&track, facing), SUDS_FUSE_SECONDS.value);
    let Some(nearest) = walked
        .as_ref()
        .and_then(|walked| walked.stays.iter().min_by(|a, b| a.at.distance(target).total_cmp(&b.at.distance(target))))
    else {
        let contact = if matches!(track.impact.contact, Contact::Mobile(_)) {
            Contact::Terrain
        } else {
            track.impact.contact
        };
        return Impact { contact, ..track.impact };
    };
    let near = (0_u8..)
        .zip(game.players())
        .filter(|(_, player)| player.alive)
        .map(|(seat, player)| (player.pose().gap(player.tuning().hitbox.value, nearest.at), seat))
        .filter(|&(gap, _)| gap <= SUDS_REACH.value)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    Impact {
        tick: nearest.from,
        at: nearest.at,
        contact: near.map_or(Contact::Terrain, |(_, seat)| Contact::Mobile(seat)),
    }
}

#[cfg(test)]
mod unit {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        game::{
            Leg, MobileKind, Team,
            testing::{damage_to, direct_hit_power, duel_of, game_of, game_on, seat, shoot},
        },
        terrain::Terrain,
        units::{Power, TICKS_PER_SECOND, Wind},
        weather::{Column, WeatherKind},
    };

    /// A Sudsotter at x = 300 facing right, and a Coilback `distance` px away, on flat ground.
    fn duel(distance: i32) -> Game {
        duel_of(MobileKind::Sudsotter, MobileKind::Coilback, distance)
    }

    fn fired(effects: &[Effect]) -> &[Leg] {
        effects.iter().find_map(|e| if let Effect::Fired { legs, .. } = e { Some(legs.as_slice()) } else { None }).unwrap()
    }

    /// Where and when the shot stuck to the ground: the start of its first walking leg.
    fn stuck(effects: &[Effect]) -> (Vec2, u32) {
        let leg = fired(effects).iter().find(|leg| leg.flight.accel == Vec2::ZERO).unwrap();
        (leg.flight.at(leg.from), leg.offset)
    }

    /// Where and when the shot went off.
    fn blast(effects: &[Effect]) -> (Vec2, u32) {
        effects.iter().find_map(|e| if let Effect::Exploded { at, tick, .. } = e { Some((*at, *tick)) } else { None }).unwrap()
    }

    /// The power step whose `shot` from seat 0 at `aim` does its work on seat 1 (`aim_point`),
    /// closest to its center.
    fn work_power(game: &Game, shot: Shot, aim: i16) -> Power {
        let center = game.players()[1].center();
        (0..=Power::MAX.twentieths())
            .filter_map(|twentieths| {
                let power = Power::from_twentieths(twentieths).ok()?;
                let point = aim_point(game, 0, shot, game.launch(0, aim, power.bars())?, center);
                (point.contact == Contact::Mobile(1)).then(|| (point.at.distance(center), power))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, power)| power)
            .expect("some power works")
    }

    #[test]
    fn s1_walks_left_and_s2_right_whichever_way_it_faces_then_goes_off_three_seconds_on() {
        let fuse = 3 * TICKS_PER_SECOND;
        // 3 s on level ground: 4 parts of a 30-part screen (CR-frog), give or take a pixel a step.
        let (least, most) = (100.0, 110.0);
        for facing in [Facing::Right, Facing::Left] {
            // In the middle of the map, with room to shoot either way.
            let game = game_of(vec![seat(Team::A, MobileKind::Sudsotter, 1200, facing), seat(Team::B, MobileKind::Coilback, 2300, Facing::Left)]);
            for (shot, sign) in [(Shot::S1, -1.0), (Shot::S2, 1.0)] {
                let effects = shoot(&mut game.clone(), 0, shot, 45, Power::nearest(1.6));
                let ((from, landed), (to, tick)) = (stuck(&effects), blast(&effects));
                let walked = (to.x - from.x) * sign;
                assert!((least..=most).contains(&walked), "{facing:?} {shot:?}: {walked} px");
                assert!((to.y - from.y).abs() < 1.0, "level ground");
                assert_eq!(tick, landed + fuse, "{facing:?} {shot:?}");
            }
        }
    }

    #[test]
    fn sponges_fly_through_mobiles_and_go_off_inside_one_they_walk_into() {
        let mut game = duel(800);
        let through = direct_hit_power(&game, 45);
        let effects = shoot(&mut game.clone(), 0, Shot::S1, 45, through);
        let (landed, _) = stuck(&effects);
        assert!(landed.x > game.players()[1].center().x, "it flew on through the target: {landed:?}");

        // S2 landed short walks right, into it.
        let power = work_power(&game, Shot::S2, 45);
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        assert!(stuck(&effects).0.x < 1100.0 - 30.0, "it landed short");
        let dealt = damage_to(&effects, 1);
        // 150-230, Laser against Mechanical; off center, from the hitbox's foot.
        assert!((150..=230).contains(&dealt), "{dealt}: {effects:?}");
    }

    #[test]
    fn a_sponge_climbs_a_wall_and_walks_on_over_the_top() {
        // Level ground at 500, and a 20 px block on it from x = 700 to 760.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 500 || ((700..760).contains(&x) && y >= 480)).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Sudsotter, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1500, Facing::Left)];
        let mut game = game_on(terrain, seats);
        let power = (20..=60)
            .map(|t| Power::from_twentieths(t).unwrap())
            .find(|power| {
                let track = game.track_sensing(game.launch(0, 45, power.bars()).unwrap(), 0, 0, THROUGH_MOBILES);
                (640.0..690.0).contains(&track.impact.at.x) && track.impact.at.y >= 499.0
            })
            .unwrap();
        let effects = shoot(&mut game, 0, Shot::S2, 45, power);
        let legs = fired(&effects);
        assert!(legs.iter().any(|leg| leg.flight.velocity.x == 0.0 && leg.flight.velocity.y < 0.0), "up the block's face: {legs:?}");
        let (at, _) = blast(&effects);
        assert!(at.y < 480.0 || at.x > 760.0, "over the top: {at:?}");
    }

    #[test]
    fn once_down_a_sponge_walks_through_a_tornado() {
        let calm = duel(1500);
        let effects = shoot(&mut calm.clone(), 0, Shot::S2, 45, Power::nearest(1.6));
        let (landed, _) = stuck(&effects);
        let mut stormy = calm.clone();
        // A column over its walk, past where it landed.
        #[allow(clippy::cast_possible_truncation)]
        let x = landed.x as i32 + 60;
        stormy.set_columns(vec![Column::new(WeatherKind::Tornado, x)]);
        assert_eq!(blast(&shoot(&mut stormy, 0, Shot::S2, 45, Power::nearest(1.6))), blast(&effects));
    }

    #[test]
    fn the_weather_leaves_its_damage_alone_but_still_turns_it() {
        let game = duel(800);
        let power = work_power(&game, Shot::S2, 45);
        let calm = damage_to(&shoot(&mut game.clone(), 0, Shot::S2, 45, power), 1);
        let x = game.players()[0].footing.x + 300;
        for kind in [WeatherKind::Force, WeatherKind::Drain, WeatherKind::Electricity] {
            let mut weathered = game.clone();
            weathered.set_columns(vec![Column::new(kind, x)]);
            let effects = shoot(&mut weathered, 0, Shot::S2, 45, power);
            assert_eq!(damage_to(&effects, 1), calm, "{kind:?}");
            assert!(!effects.iter().any(|e| matches!(e, Effect::Bolt { .. })), "{kind:?}");
        }
        let mut mirrored = game.clone();
        mirrored.set_columns(vec![Column::new(WeatherKind::Mirror, x)]);
        let effects = shoot(&mut mirrored, 0, Shot::S2, 45, power);
        assert!(stuck(&effects).0.x < f64::from(x), "sent back: {effects:?}");
    }

    #[test]
    fn ss_pops_weakly_on_a_direct_hit() {
        let mut game = duel(800);
        let power = direct_hit_power(&game, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Pierced { .. })), "{effects:?}");
        let dealt = damage_to(&effects, 1);
        assert!((70..=100).contains(&dealt), "{dealt}");
    }

    #[test]
    fn ss_walks_through_a_mobile_hurting_it_all_the_while() {
        let mut game = duel(800);
        let power = work_power(&game, Shot::Ss, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let Some(&Effect::Pierced { seat: 1, from, to }) = effects.iter().find(|e| matches!(e, Effect::Pierced { .. })) else {
            panic!("no touch: {effects:?}")
        };
        // Under it at half speed, and within reach either side: seconds of it.
        assert!(to - from > TICKS_PER_SECOND, "{from}..{to}");
        let dealt = damage_to(&effects, 1);
        assert!((150..=700).contains(&dealt), "{dealt}: {effects:?}");
        let pierced = effects.iter().position(|e| matches!(e, Effect::Pierced { .. })).unwrap();
        let popped = effects.iter().position(|e| matches!(e, Effect::Exploded { .. })).unwrap();
        assert!(pierced < popped, "it touches, then pops: {effects:?}");
    }

    #[test]
    fn ss_walks_the_way_the_wind_turned_it() {
        // Lobbed right off a cliff into a gale blowing left: it lands below, moving left, and walks
        // left, back up the cliff (SW-mob). In calm air the same shot walks right.
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 550 || (x < 400 && y >= 300)).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Sudsotter, 370, Facing::Right), seat(Team::B, MobileKind::Coilback, 1500, Facing::Left)];
        let calm = game_on(terrain, seats);
        let mut windy = calm.clone();
        windy.wind = Wind::new(26, 180).unwrap();
        let power = (4..=30)
            .map(|t| Power::from_twentieths(t).unwrap())
            .find(|power| {
                let track = windy.track(windy.launch(0, 60, power.bars()).unwrap(), 0, 0);
                track.impact.at.y >= 549.0 && track.impact.at.x >= 420.0 && track.last.flight.velocity_at(track.last_tick).x < 0.0
            })
            .expect("a lob the wind turns round");
        let walks = |game: &mut Game| {
            let effects = shoot(game, 0, Shot::Ss, 60, power);
            fired(&effects)
                .iter()
                .filter(|leg| leg.flight.accel == Vec2::ZERO && leg.flight.velocity.x != 0.0)
                .map(|leg| leg.flight.velocity.x.signum())
                .collect::<Vec<f64>>()
        };
        let (windward, calmly) = (walks(&mut windy), walks(&mut calm.clone()));
        assert!(!windward.is_empty() && windward.iter().all(|&x| x < 0.0), "{windward:?}");
        assert!(!calmly.is_empty() && calmly.iter().all(|&x| x > 0.0), "{calmly:?}");
    }

    #[test]
    fn ss_round_a_speck_of_land_hurts_most() {
        // The target stands on a floating speck hardly wider than itself: the suds land on its
        // near end and walk round and round it, touching the target the whole time (SW-mob,
        // CR-frog).
        let terrain = Terrain::from_fn(2400, 600, |x, y| y >= 560 || ((1050..1120).contains(&x) && (400..410).contains(&y))).unwrap();
        let seats = vec![seat(Team::A, MobileKind::Sudsotter, 300, Facing::Right), seat(Team::B, MobileKind::Coilback, 1100, Facing::Left)];
        let mut game = game_on(terrain, seats);
        let center = game.players()[1].center();
        let (aim, power) = (30..=60)
            .flat_map(|aim| (0..=Power::MAX.twentieths()).map(move |t| (aim, Power::from_twentieths(t).unwrap())))
            .find(|&(aim, power)| {
                let track = game.track(game.launch(0, aim, power.bars()).unwrap(), 0, 0);
                track.impact.contact == Contact::Terrain && track.impact.at.distance(center) < 60.0
            })
            .expect("a shot onto the speck");
        let effects = shoot(&mut game, 0, Shot::Ss, aim, power);
        let dealt = damage_to(&effects, 1);
        assert!(dealt >= 400, "{dealt}: {effects:?}");
        let mut flat = duel(800);
        let power = work_power(&flat, Shot::Ss, 45);
        let passing = damage_to(&shoot(&mut flat, 0, Shot::Ss, 45, power), 1);
        assert!(passing < dealt, "a pass on flat ground deals less: {passing}");
    }

    #[test]
    fn the_suds_touches_and_pop_come_in_tick_order() {
        let mut game = duel(800);
        let power = work_power(&game, Shot::Ss, 45);
        let effects = shoot(&mut game, 0, Shot::Ss, 45, power);
        let ticks: Vec<u32> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Pierced { to, .. } | Effect::Exploded { tick: to, .. } => Some(*to),
                _ => None,
            })
            .collect();
        assert!(ticks.windows(2).all(|w| w[0] <= w[1]), "{ticks:?}");
    }
}
