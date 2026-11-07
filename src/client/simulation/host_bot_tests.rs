//! Deterministic, synthetic host-policy checks; no outside servers or clients.
use super::*;
use crate::{Config, client::BotBehavior, physics};
use std::f64::consts::{PI, TAU};

fn host(count: usize, behavior: BotBehavior, seed: u32) -> RawSimulator {
    let mut config = SimulationConfig {
        agents: count + 1,
        world: Config::official((count + 1).min(2281)),
        ..Default::default()
    };
    if count >= 2281 {
        config.world.radius = 32000.;
        config.world.dynamic_radius = false;
        config.world.size_to_population = false;
    }
    let mut raw = RawSimulator::new(seed, config, count + 1).unwrap();
    bots(&mut raw, behavior);
    raw
}

/// Compares a live snake with a state recorded on x86-64 Linux. Other targets'
/// libm rounds differently, so after thousands of ticks coordinates drift by
/// ~1e-13 (macOS CI); numbers match within 1e-6, everything else exactly.
fn assert_recorded(snake: &impl serde::Serialize, recorded: &serde_json::Value) {
    fn close(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value::*;
        match (a, b) {
            (Number(a), Number(b)) => (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-6,
            (Array(a), Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b)),
            (Object(a), Object(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| close(v, w)))
            }
            _ => a == b,
        }
    }
    let live = serde_json::to_value(snake).unwrap();
    assert!(close(&live, recorded), "live {live}\nrecorded {recorded}");
}

/// Slot 0 is the only socket seat; every later slot is a host bot.
fn bots(raw: &mut RawSimulator, behavior: BotBehavior) {
    let profiles = (0..raw.backend.slots.len() - 1)
        .map(crate::client::server::bot_appearance)
        .collect();
    raw.set_in_world_bots(1, behavior, profiles);
}

fn owner(raw: &mut RawSimulator, head: Point, segments: usize) {
    assert!(
        raw.connect(
            0,
            PlayerAppearance {
                owner: true,
                nickname: b"Owner".to_vec(),
                ..Default::default()
            }
        )
        .unwrap()
    );
    raw.backend.world.retire(0).unwrap();
    let body = (0..segments)
        .map(|i| Point {
            x: head.x - i as f64 * physics::ANCHOR_SPACING,
            y: head.y,
        })
        .collect();
    raw.backend.world.respawn_body(0, body, 0.).unwrap();
    raw.backend.slots[0].control.heading = 0.;
}

#[test]
fn host_bots_owner_life_and_newest_admission() {
    let mut raw = RawSimulator::new(
        11,
        SimulationConfig {
            agents: 3,
            world: Config::sandbox(),
            ..Default::default()
        },
        3,
    )
    .unwrap();
    let keyed = || PlayerAppearance {
        owner: true,
        ..Default::default()
    };
    assert!(raw.connect(0, keyed()).unwrap());
    assert_eq!(raw.backend.owner_slot(), Some(0));
    assert!(raw.connect(1, keyed()).unwrap());
    assert_eq!(raw.backend.owner_slot(), Some(1));
    raw.backend.world.retire(1).unwrap();
    assert_eq!(raw.backend.owner_slot(), None);
    assert!(raw.connect(1, PlayerAppearance::default()).unwrap());
    assert_eq!(raw.backend.owner_slot(), None);
    assert!(raw.connect(2, keyed()).unwrap());
    raw.disconnect(2).unwrap();
    assert_eq!(raw.backend.owner_slot(), None);
}

#[test]
fn host_bots_owner_name_stays_private_in_packets_recordings_and_records() {
    let mut config = SimulationConfig {
        agents: 2,
        world: Config::sandbox(),
        local_records: true,
        leaderboard_interval_seconds: physics::LOOP_SECONDS,
        ..Default::default()
    };
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    let mut raw = RawSimulator::new(17, config, 2).unwrap();
    let key = crate::client::OwnerKey::from("bananatest".to_owned());
    let mut login = vec![b's', 11, 0, 16];
    login.extend(b"Name !bananatest");
    login.extend([0, 255]);
    // The marker has already disappeared when the world receives an appearance.
    let profile = crate::client::server::parse_login_with_owner(&login, Some(&key)).unwrap();
    assert_eq!(profile.nickname, b"Name");
    assert!(raw.connect(0, profile).unwrap());
    assert!(raw.connect(1, PlayerAppearance::default()).unwrap());
    for (slot, x) in [(0, 0.), (1, 400.)] {
        let (body, heading) = raw
            .backend
            .world
            .spawn_body(Point { x, y: 0. }, 0.)
            .unwrap();
        raw.backend
            .world
            .correct_pose(slot, body[0], body, heading)
            .unwrap();
        raw.backend.slots[slot].control.heading = heading;
    }
    // A real native observer uses the production recording path, not a copy
    // of the host encoder. The player's raw login never enters that recorder.
    raw.backend.slots[1].remote = false;
    raw.backend.slots[1].records = Some(Vec::new());
    raw.backend.connect(1).unwrap();
    raw.advance_ticks(3).unwrap();
    let records = raw.backend.drain_records(1).unwrap();
    let mut appearance = false;
    let mut leaderboard = false;
    for record in &records {
        assert!(!record.bytes.windows(10).any(|bytes| bytes == b"bananatest"));
        if record.event == WireEvent::Receive {
            for packet in crate::client::protocol::split_frame(&record.bytes).unwrap() {
                if packet.windows(4).any(|bytes| bytes == b"Name") {
                    appearance |= packet[0] == b's';
                    leaderboard |= packet[0] == b'l';
                }
            }
        }
    }
    assert!(appearance && leaderboard);
    assert!(
        !serde_json::to_string(&records)
            .unwrap()
            .contains("bananatest")
    );
    raw.backend.world.retire(0).unwrap();
    raw.advance_ticks(1).unwrap();
    assert_eq!(raw.backend.record.as_ref().unwrap().name, b"Name");
    assert!(!format!("{:?}", raw.backend.record).contains("bananatest"));
}

#[test]
fn host_bots_center_and_ownerless_follow_are_identical_and_never_connect() {
    let mut a = host(36, BotBehavior::Center, 19);
    let mut b = host(36, BotBehavior::Follow, 19);
    for _ in 0..300 {
        a.advance_ticks(1).unwrap();
        b.advance_ticks(1).unwrap();
        assert_eq!(a.backend.world.snakes(), b.backend.world.snakes());
        for s in &a.backend.slots[1..] {
            assert!(!s.connection.open && !s.connection.setup);
            assert!(s.connection.uplink.is_empty() && s.connection.downlink.is_empty());
            assert_eq!(s.state.status(), ClientStatus::Disconnected);
            assert!(s.records.is_none());
        }
    }
    assert_eq!(a.backend.profiles[1].nickname, b"Noodle (bot)");
}

#[test]
fn host_bots_spawn_search_keeps_clearance_and_rejects_owner_route() {
    let mut raw = host(128, BotBehavior::Follow, 29);
    owner(&mut raw, Point::default(), 80);
    let b = &mut raw.backend;
    let mut bots = b.in_world_bots.take().unwrap();
    // A clear body lying across the owner's lane is refused only while the
    // owner is alive and predicted to pass there.
    let (body, heading) = b
        .world
        .spawn_body(Point { x: 200., y: 0. }, PI / 2.)
        .unwrap();
    assert!(b.world.spawn_margin(&body).unwrap() > 0.);
    bots.observe(&b.world, None);
    assert!(bots.spawn_is_safe(&body, heading));
    bots.observe(&b.world, Some(0));
    assert!(!bots.spawn_is_safe(&body, heading));
    for slot in 1..129 {
        let (body, heading) = b
            .spawn_pose_vetoed(slot, 0., &mut |body, heading| {
                bots.spawn_is_safe(body, heading)
            })
            .unwrap();
        assert!(b.world.spawn_margin(&body).unwrap() > 0.);
        assert!(bots.spawn_is_safe(&body, heading));
        b.world.respawn_body(slot, body, heading).unwrap();
    }
}

/// Owner steering scripts. Every wanted heading goes through the engine's
/// quantized steering, as a real client's does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drive {
    /// One heading forever.
    Straight,
    /// The tightest circle forever.
    Tightest,
    /// A gentle constant turn rate forever.
    Gentle,
    /// A very gentle constant turn rate forever: a circle almost as large as
    /// the arena, whose quantized heading steps seconds apart.
    VeryGentle,
    /// `MANEUVER_SECONDS` of turns, straights and zigzags, then the tightest
    /// circle forever.
    Maneuvers,
}

const MANEUVER_SECONDS: f64 = 20.;
/// A gentle turn: about a quarter of an 80-segment snake's maximum rate.
const GENTLE_RATE: f64 = 0.8;
/// About 27,000 units of radius at 80 segments, inside the 32,000 arena.
const VERY_GENTLE_RATE: f64 = 0.007;
/// The largest wire-compatible fixed arena, for the long routes.
const LARGE_ARENA: f64 = 32000.;
/// Bots this close to the owner's head count toward the smoothness metrics.
const NEAR_RADIUS: f64 = 2000.;
/// A reversal turns the wanted heading back by two of the 251 wire steps
/// from its last extreme; smaller wiggles are not visible jitter.
const REVERSAL: f64 = 2. * TAU / 251.;

/// Reduced engine replay: keep the owner's exact steady-circle input phase
/// and idle speed, without later pickups or other actors. The complete bot
/// line is frozen through the existing release estimate, so this checks the
/// fitted witness against engine motion, not future bot motion or growth.
fn gentle_frozen_engine_contacts(
    owner: crate::Snake,
    bot: &crate::Snake,
    mut wanted: f64,
) -> usize {
    let mut config = Config::official(257);
    config.radius = LARGE_ARENA;
    config.dynamic_radius = false;
    config.size_to_population = false;
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.prey.count = 0;
    let dt = config.dt;
    let mut tip = physics::head_tip(owner.head, owner.heading, owner.segments);
    let speed = owner.speed;
    let mut world = crate::Simulator::from_scenario(
        73,
        config,
        crate::Scenario {
            snakes: vec![(
                "owner".into(),
                crate::ReplayLife::from_observed(
                    owner,
                    crate::InitialSpeed::Known(physics::SpeedState::idle(speed)),
                ),
            )],
            ..Default::default()
        },
    )
    .unwrap();
    let line: Vec<_> = [
        physics::head_tip(bot.head, bot.heading, bot.segments),
        bot.head,
    ]
    .into_iter()
    .chain(bot.body.iter().copied())
    .collect();
    let ticks =
        (crate::client::host_bots::release_time(bot.segments, bot.fam) / dt).ceil() as usize;
    let mut contacts = 0;
    for _ in 0..ticks {
        world
            .step(&[Some(crate::Action {
                heading: wanted,
                boost: false,
            })])
            .unwrap();
        let actual = &world.snakes()[0];
        assert!(actual.alive, "solo replay must remain inside the arena");
        let next_tip = physics::head_tip(actual.head, actual.heading, actual.segments);
        contacts += usize::from(physics::strikes(
            tip,
            next_tip,
            actual.heading,
            actual.segments,
            bot.segments,
            line.iter().copied(),
        ));
        tip = next_tip;
        wanted -= GENTLE_RATE * dt;
    }
    contacts
}

#[test]
fn host_bots_gentle_engine_motion_can_reach_the_frozen_line() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/follow-gentle-contacts.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut owner: crate::Snake = serde_json::from_value(fixture["owner"].clone()).unwrap();
        let mut bot: crate::Snake = serde_json::from_value(fixture["bot"].clone()).unwrap();
        owner.head = serde_json::from_value(case["ownerHead"].clone()).unwrap();
        bot.head = serde_json::from_value(case["botHead"].clone()).unwrap();
        bot.heading = case["botHeading"].as_f64().unwrap();
        let tick = case["tick"].as_u64().unwrap();
        let mut wanted = 0f64;
        for _ in 0..=tick {
            wanted -= GENTLE_RATE * physics::LOOP_SECONDS;
        }
        assert_eq!(format!("{:016x}", wanted.to_bits()), case["wantedBits"]);
        let contacts = gentle_frozen_engine_contacts(owner, &bot, wanted);
        println!(
            "Gentle frozen replay tick={tick} fitted_contact_tick={} release_seconds={:.6} frozen_engine_contacts={contacts}",
            case["fittedContactTick"],
            crate::client::host_bots::release_time(bot.segments, bot.fam)
        );
        // Even authoritative owner motion can reach a frozen line. Actual
        // bot motion and death must be replayed before classifying a hit.
        assert!(contacts > 0, "retain the reachable frozen line at {tick}");
    }
}

/// Replay physical host state, retaining hidden world state, all food and
/// actors, planner commitments/history, admission RNG, controls and phases.
/// Packet presentation does not participate in in-world steering or physics.
fn gentle_engine_replay(raw: &RawSimulator, ticks: usize, mut wanted: f64) -> crate::Simulator {
    let source = &raw.backend;
    let mut replay = RawSimulator::new(73, source.config.clone(), source.slots.len()).unwrap();
    let b = &mut replay.backend;
    b.world = source.world.clone();
    b.in_world_bots = source.in_world_bots.clone();
    b.in_world_bots.as_mut().unwrap().witness = false;
    b.spawn_random = source.spawn_random.clone();
    b.owner = source.owner;
    b.tick = source.tick;
    for (slot, original) in b.slots.iter_mut().zip(&source.slots) {
        slot.control = original.control;
        slot.life = original.life;
    }
    let deaths = b.world.deaths().len();
    for _ in 0..ticks {
        b.slots[0].control.heading = wanted;
        b.drive_in_world_bots().unwrap();
        let actions: Vec<_> = b
            .world
            .snakes()
            .iter()
            .zip(&b.slots)
            .map(|(snake, slot)| snake.alive.then_some(slot.control))
            .collect();
        b.world.step(&actions).unwrap();
        b.tick += 1;
        assert!(
            b.world.snakes()[0].alive,
            "real engine owner death in replay: {:?}",
            &b.world.deaths()[deaths..]
        );
        wanted -= GENTLE_RATE * raw.dt();
    }
    b.world.clone()
}

impl Drive {
    /// The owner's wanted heading at `time`; `line` is the heading a
    /// straight stretch holds and `turn` accumulates gentle turns.
    fn wanted(self, time: f64, heading: f64, line: &mut f64, turn: &mut f64, dt: f64) -> f64 {
        let tightest = |side: f64| heading + side * PI / 2.;
        let phase = match self {
            Drive::Straight => 0,
            Drive::Tightest => 1,
            Drive::Gentle => 3,
            Drive::VeryGentle => 5,
            Drive::Maneuvers => match time {
                t if t >= MANEUVER_SECONDS => 1,
                t if t < 4. => 0,
                t if t < 7. => 1,
                t if t < 9. => 0,
                t if t < 12. => 3,
                t if t < 15. => 4,
                t if t < 17. => 2,
                _ => 0,
            },
        };
        // Straights and zigzags keep the heading they started from.
        if phase != 0 && phase != 4 {
            *line = heading;
        }
        if phase != 3 && phase != 5 {
            *turn = heading;
        }
        match phase {
            0 => *line,
            1 => tightest(1.),
            2 => tightest(-1.),
            3 => {
                *turn -= GENTLE_RATE * dt;
                *turn
            }
            5 => {
                *turn -= VERY_GENTLE_RATE * dt;
                *turn
            }
            // Zigzag: one radian either side, switching every half second.
            _ if ((time * 2.) as u64).is_multiple_of(2) => *line + 1.,
            _ => *line - 1.,
        }
    }

    /// Seconds the path fit takes to settle on this drive's steady state:
    /// one fitted turn, or for a lane the longest heading hold of a circle
    /// inside the arena. Tight circles allow for an owner grown to the
    /// slowest turn.
    fn fit_seconds(self) -> f64 {
        use crate::client::host_bots::{FIT_SECONDS, FIT_TURN};
        match self {
            Drive::Straight => TAU / 251. * LARGE_ARENA / physics::base_speed(80),
            Drive::Tightest | Drive::Maneuvers => {
                FIT_TURN / physics::turn_rate(physics::MAX_SEGMENTS)
            }
            Drive::Gentle => FIT_TURN / GENTLE_RATE,
            Drive::VeryGentle => (FIT_TURN / VERY_GENTLE_RATE).min(FIT_SECONDS),
        }
    }
}

struct Outcome {
    /// Times the owner died to a bot.
    owner_deaths: Vec<f64>,
    /// Deaths after the scenario's declared settling threshold.
    late_deaths: usize,
    /// Bots that died on the owner, anywhere along it.
    body_kills: usize,
    food: usize,
    near_seconds: f64,
    reversals: usize,
    escape_ticks: u64,
    /// Fitted owner forward-probe contacts with the complete frozen bot line,
    /// within the estimated release/impact horizon. Diagnostic, not proof of
    /// the actual future body's motion or lifetime.
    conflicts: u64,
    /// Full physical continuations checked after a gentle-circle witness hit.
    engine_replays: usize,
    /// Original sum-of-radii frozen-anchor screen; retain its raw evidence.
    bound_hits: u64,
    /// Bot ticks after settling that dropped a striking plan.
    broken: u64,
    /// The longest bot seen.
    longest_bot: usize,
    checked_ticks: usize,
}

impl Outcome {
    fn report(&self, drive: Drive, seed: u32, avoidance: bool) {
        println!(
            "{drive:?} seed={seed} avoidance={avoidance} owner_deaths={:?} late_deaths={} body_kills={} \
             death_food={} near_bot_seconds={:.0} reversals_per_near_bot_second={:.3} \
             escaping_share={:.4} late_frozen_contacts={} engine_replays={} late_body_bound_hits={} late_broken_strikes={} longest_bot={} checked_ticks={}",
            self.owner_deaths,
            self.late_deaths,
            self.body_kills,
            self.food,
            self.near_seconds,
            self.reversals as f64 / self.near_seconds,
            self.escape_ticks as f64 * physics::LOOP_SECONDS / self.near_seconds,
            self.conflicts,
            self.engine_replays,
            self.bound_hits,
            self.broken,
            self.longest_bot,
            self.checked_ticks,
        );
    }
}

fn scenario(
    drive: Drive,
    seconds: f64,
    seed: u32,
    setup: impl FnOnce(&mut crate::client::host_bots::InWorldBots),
) -> Outcome {
    let mut raw = host(256, BotBehavior::Follow, seed);
    if matches!(drive, Drive::Straight | Drive::VeryGentle) {
        // A long route that stays inside the wire-compatible arena.
        let mut config = raw.backend.config.clone();
        config.world = Config {
            radius: LARGE_ARENA,
            dynamic_radius: false,
            size_to_population: false,
            ..config.world
        };
        if drive == Drive::VeryGentle {
            // Explicitly synthetic: no ambient food, so the owner keeps its
            // speed and with it the circle. Bots still eat death food.
            config.world.food_count = 0;
            config.world.ambient_replenish_per_second = 0.;
        }
        raw = RawSimulator::new(seed, config, 257).unwrap();
        bots(&mut raw, BotBehavior::Follow);
    }
    let start = match drive {
        Drive::Straight => Point { x: -17000., y: 0. },
        // Heading east, turning clockwise around the arena center.
        Drive::VeryGentle => Point {
            x: 0.,
            y: physics::base_speed(80) / VERY_GENTLE_RATE,
        },
        _ => Point::default(),
    };
    owner(&mut raw, start, 80);
    let bots = raw.backend.in_world_bots.as_mut().unwrap();
    setup(bots);
    let avoidance = bots.avoidance;
    // Explicitly synthetic initial crowd. Respawns thereafter use only the
    // host's normal sampler, clearance search and owner-path check.
    for slot in 1usize..257 {
        let side = if slot.is_multiple_of(2) { 1. } else { -1. };
        let (body, heading) = if drive == Drive::VeryGentle {
            // Long bots in lanes beside the owner's circle and ahead of it,
            // parallel to it.
            let lane = slot % 8;
            let radius = start.y + side * (400. + (lane / 2) as f64 * 300.);
            let angle = PI / 2. - 0.04 - (slot / 8) as f64 * 0.07;
            let heading = angle - PI / 2.;
            let body = (0..40)
                .map(|i| {
                    let back = i as f64 * physics::ANCHOR_SPACING;
                    Point {
                        x: radius * angle.cos() - back * heading.cos(),
                        y: radius * angle.sin() - back * heading.sin(),
                    }
                })
                .collect();
            (body, heading)
        } else {
            let head = if drive == Drive::Straight {
                Point {
                    x: start.x - 1600. + (slot / 2) as f64 * 150.,
                    y: side * (300. + (slot % 7) as f64 * 70.),
                }
            } else {
                let angle = slot as f64 * TAU / 256.;
                let radius = 800. + (slot % 11) as f64 * 100.;
                Point {
                    x: angle.cos() * radius,
                    y: angle.sin() * radius,
                }
            };
            let heading = (start.y - head.y).atan2(start.x - head.x);
            raw.backend.world.spawn_body(head, heading).unwrap()
        };
        raw.backend.world.respawn_body(slot, body, heading).unwrap();
        raw.backend.slots[slot].control.heading = heading;
    }
    let dt = raw.dt();
    let mut outcome = Outcome {
        owner_deaths: Vec::new(),
        late_deaths: 0,
        body_kills: 0,
        food: 0,
        near_seconds: 0.,
        reversals: 0,
        escape_ticks: 0,
        conflicts: 0,
        engine_replays: 0,
        bound_hits: 0,
        broken: 0,
        longest_bot: 0,
        checked_ticks: 0,
    };
    let (mut seen_conflicts, mut seen_broken, mut seen_bounds) = (0, 0, 0);
    let mut seen_deaths = 0;
    let (mut line, mut turn) = (0., 0.);
    // The steady state starts with the run, after the maneuvers, or at a
    // rejoin. It has settled once the fit has and the bodies laid before are
    // gone, judged by the longest release seen since.
    let mut steady_from = if drive == Drive::Maneuvers {
        MANEUVER_SECONDS
    } else {
        0.
    };
    let mut longest_release = 0f64;
    // Per bot: life, wanted heading at the last extreme, turn direction.
    let mut steering = vec![(0u64, 0f64, 0f64); 257];
    let recorded: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/follow-gentle-contacts.json"
    ))
    .unwrap();
    let mut replay_matches = Vec::new();
    for tick in 0..(seconds / dt).ceil() as usize {
        let time = tick as f64 * dt;
        let heading = raw.backend.world.snakes()[0].heading;
        raw.backend.slots[0].control.heading =
            drive.wanted(time, heading, &mut line, &mut turn, dt);
        // Reproduce all three recorded pre-step states, then independently
        // continue their full physical worlds through the release horizon.
        let captured = (drive == Drive::Gentle && seed == 73)
            .then(|| {
                recorded["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|case| case["tick"].as_u64().unwrap() == tick as u64)
            })
            .flatten();
        if let Some(case) = captured {
            let mut owner = recorded["owner"].clone();
            let mut bot = recorded["bot"].clone();
            owner["head"] = case["ownerHead"].clone();
            bot["head"] = case["botHead"].clone();
            bot["heading"] = case["botHeading"].clone();
            assert_recorded(&raw.backend.world.snakes()[0], &owner);
            assert_recorded(&raw.backend.world.snakes()[206], &bot);
            let snake = &raw.backend.world.snakes()[206];
            let ticks = (crate::client::host_bots::release_time(snake.segments, snake.fam) / dt)
                .ceil() as usize;
            let replay = gentle_engine_replay(&raw, ticks, raw.backend.slots[0].control.heading);
            println!("Gentle engine replay snapshot_tick={tick} ticks={ticks} owner_alive=true");
            replay_matches.push((tick + ticks, replay));
            outcome.engine_replays += 1;
        }
        raw.advance_ticks(1).unwrap();
        raw.take(0).unwrap();
        replay_matches.retain(|(until, replay)| {
            if *until == tick + 1 {
                assert_eq!(
                    serde_json::to_value(raw.backend.world.checkpoint().unwrap()).unwrap(),
                    serde_json::to_value(replay.checkpoint().unwrap()).unwrap(),
                    "physical replay must match the original host at tick {until}"
                );
                println!("Gentle engine replay matched full host checkpoint at tick {until}");
                false
            } else {
                true
            }
        });
        let world = &raw.backend.world;
        let owner = world.snakes()[0].head;
        for (slot, entry) in steering.iter_mut().enumerate().skip(1) {
            let bot = &world.snakes()[slot];
            if bot.alive && time >= steady_from {
                let release = crate::client::host_bots::release_time(bot.segments, bot.fam);
                longest_release = longest_release.max(release);
            }
            outcome.longest_bot = outcome.longest_bot.max(bot.segments);
            let life = raw.backend.slots[slot].life;
            let wanted = raw.backend.slots[slot].control.heading;
            if !bot.alive || bot.head.distance(owner) > NEAR_RADIUS || entry.0 != life {
                *entry = (life, wanted, 0.);
                continue;
            }
            outcome.near_seconds += dt;
            let change = (wanted - entry.1).sin().atan2((wanted - entry.1).cos());
            if change * entry.2 > 0. {
                entry.1 = wanted;
            } else if change.abs() >= REVERSAL {
                outcome.reversals += usize::from(entry.2 != 0.);
                *entry = (life, wanted, change.signum());
            }
        }
        for death in &world.deaths()[seen_deaths..] {
            if death.killer.as_deref() == Some("0") {
                outcome.body_kills += 1;
                outcome.food += death.food;
            }
        }
        seen_deaths = world.deaths().len();
        let settle = steady_from + drive.fit_seconds() + longest_release;
        let bots = raw.backend.in_world_bots.as_ref().unwrap();
        if time > settle {
            outcome.checked_ticks += 1;
            outcome.conflicts += bots.conflicts - seen_conflicts;
            outcome.bound_hits += bots.bound_hits - seen_bounds;
            outcome.broken += bots.broken - seen_broken;
            if drive == Drive::Gentle && captured.is_none() && bots.conflicts > seen_conflicts {
                // The just-completed step is covered by the hard owner-death
                // check below. Replay the remaining future from this exact
                // physical state, retaining actual bot steering and ecology.
                let ticks = world
                    .snakes()
                    .iter()
                    .skip(1)
                    .filter(|snake| snake.alive)
                    .map(|snake| {
                        (crate::client::host_bots::release_time(snake.segments, snake.fam) / dt)
                            .ceil() as usize
                    })
                    .max()
                    .unwrap_or(0);
                let replay = gentle_engine_replay(
                    &raw,
                    ticks,
                    raw.backend.slots[0].control.heading - GENTLE_RATE * dt,
                );
                println!(
                    "Gentle seed={seed} engine replay after_tick={tick} ticks={ticks} owner_alive=true"
                );
                replay_matches.push((tick + 1 + ticks, replay));
                outcome.engine_replays += 1;
            }
        }
        (seen_conflicts, seen_broken, seen_bounds) = (bots.conflicts, bots.broken, bots.bound_hits);
        if world.snakes()[0].alive {
            continue;
        }
        let death = world
            .deaths()
            .iter()
            .rev()
            .find(|d| d.victim == "0")
            .unwrap();
        println!("{drive:?} avoidance={avoidance} owner death at {time:.3}s: {death:?}");
        if death.killer.is_some() {
            outcome.owner_deaths.push(time);
            if time > settle {
                outcome.late_deaths += 1;
            }
        }
        if !avoidance {
            break;
        }
        // Rejoin through ordinary admission; the steady state restarts.
        let appearance = PlayerAppearance {
            owner: true,
            nickname: b"Owner".to_vec(),
            ..Default::default()
        };
        while !raw.connect(0, appearance.clone()).unwrap() {}
        steady_from = steady_from.max(time);
        longest_release = 0.;
    }
    outcome.escape_ticks = raw.backend.in_world_bots.as_ref().unwrap().escape_ticks;
    outcome.report(drive, seed, avoidance);
    outcome
}

/// Finite regression: no late owner death or real replay death,
/// while bots keep striking. The witness is the review repro for growth after
/// planning: bots near the owner eat death food, and a later pickup keeps a
/// body longer than the release it was planned with; a bot that drops a
/// striking plan leaves its approach body in the path. Before the fix,
/// `Maneuvers` on seed 73 had 84 such bot ticks.
fn assert_safe_and_feeding(drive: Drive, seed: u32) {
    let seconds = if drive == Drive::VeryGentle {
        300.
    } else {
        180.
    };
    let outcome = scenario(drive, seconds, seed, |bots| bots.witness = true);
    assert!(outcome.checked_ticks as f64 * physics::LOOP_SECONDS >= 30.);
    assert_eq!(
        outcome.late_deaths, 0,
        "{drive:?} owner died after settling"
    );
    if drive == Drive::Gentle {
        // The frozen line can be reachable even with engine owner motion;
        // it omits the bot moving/dying before arrival. Keep the count
        // advisory here, with hard full-physics replays at each witness hit
        // and unchanged owner-death assertions. The three retained seed-73
        // replays also must match the original host's final checkpoints.
        if seed == 73 {
            assert!(outcome.engine_replays >= 3);
        }
    } else {
        assert_eq!(
            outcome.conflicts, 0,
            "{drive:?} the fitted probe contacted a frozen bot line after settling"
        );
    }
    assert!(
        outcome.body_kills >= 20,
        "only {} body kills",
        outcome.body_kills
    );
    assert!(outcome.food > 0, "body kills must leave normal death food");
}

#[test]
fn host_bots_safety_straight_forever() {
    assert_safe_and_feeding(Drive::Straight, 73);
}

#[test]
fn host_bots_safety_tightest_circle_forever() {
    assert_safe_and_feeding(Drive::Tightest, 73);
}

#[test]
fn host_bots_safety_gentle_circle_forever() {
    assert_safe_and_feeding(Drive::Gentle, 73);
}

/// Review repro: a turn so gentle that its heading steps seconds apart, with
/// bots long enough to outlast a few seconds of fit window.
#[test]
fn host_bots_safety_very_gentle_circle_with_long_bots() {
    assert_safe_and_feeding(Drive::VeryGentle, 73);
}

#[test]
fn host_bots_safety_after_maneuvers_and_a_steady_hold() {
    assert_safe_and_feeding(Drive::Maneuvers, 73);
}

#[test]
fn host_bots_sensitivity_control() {
    // Without avoidance, bots chasing the head kill the circling owner: the
    // matched crowd is dangerous, so the safety runs above are meaningful.
    let control = scenario(Drive::Tightest, 180., 73, |bots| bots.avoidance = false);
    assert!(!control.owner_deaths.is_empty());
}

#[test]
#[ignore = "explicit robustness sweep: the safety runs on more seeds"]
fn host_bots_safety_on_more_seeds() {
    for seed in [11, 29, 47] {
        for drive in [
            Drive::Straight,
            Drive::Tightest,
            Drive::Gentle,
            Drive::VeryGentle,
            Drive::Maneuvers,
        ] {
            assert_safe_and_feeding(drive, seed);
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "explicit single-thread CPU measurement, not a CI timing assertion"]
fn host_bots_tick_benchmark() {
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanos: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: std::ffi::c_int, value: *mut Timespec) -> std::ffi::c_int;
    }
    fn cpu() -> f64 {
        let mut time = Timespec {
            seconds: 0,
            nanos: 0,
        };
        // Linux CLOCK_THREAD_CPUTIME_ID excludes scheduling/throttling and all
        // other threads. The workload itself has no workers or outside I/O.
        assert_eq!(unsafe { clock_gettime(3, &mut time) }, 0);
        time.seconds as f64 + time.nanos as f64 * 1e-9
    }
    for behavior in [BotBehavior::Center, BotBehavior::Follow] {
        let mut raw = host(3000, behavior, 73);
        // Keep official mechanics/ecology, with the largest encodable fixed
        // arena. Seat-scaled 3,001-player radius exceeds the protocol limit.
        owner(&mut raw, Point::default(), 80);
        let mut owner_deaths = 0;
        let mut samples = Vec::new();
        let mut living = 0usize;
        let dt = raw.dt();
        for tick in 0..(90. / dt) as usize {
            if !raw.backend.world.snakes()[0].alive {
                owner_deaths += 1;
                assert!(
                    raw.connect(
                        0,
                        PlayerAppearance {
                            owner: true,
                            nickname: b"Owner".to_vec(),
                            ..Default::default()
                        }
                    )
                    .unwrap()
                );
            }
            raw.backend.slots[0].control.heading = raw.backend.world.snakes()[0].heading + PI / 2.;
            let start = cpu();
            raw.advance_ticks(1).unwrap();
            let elapsed = cpu() - start;
            raw.take(0).unwrap();
            if tick >= (30. / dt) as usize {
                samples.push(elapsed);
                living += raw
                    .backend
                    .world
                    .snakes()
                    .iter()
                    .skip(1)
                    .filter(|s| s.alive)
                    .count();
            }
        }
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        samples.sort_by(f64::total_cmp);
        let p99 = samples[(samples.len() as f64 * 0.99).ceil() as usize - 1];
        let alive = raw
            .backend
            .world
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .count();
        println!(
            "{behavior:?} slots=3000 owner=1 warmup=30s measured=60s ticks={} CPU_mean_ms={:.6} CPU_p99_ms={:.6} mean_living_bots={:.3} final_alive={alive} owner_deaths={owner_deaths}",
            samples.len(),
            mean * 1000.,
            p99 * 1000.,
            living as f64 / samples.len() as f64
        );
    }
}
