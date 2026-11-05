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
    /// `MANEUVER_SECONDS` of turns, straights and zigzags, then the tightest
    /// circle forever.
    Maneuvers,
}

const MANEUVER_SECONDS: f64 = 20.;
/// A gentle turn: about a quarter of an 80-segment snake's maximum rate.
const GENTLE_RATE: f64 = 0.8;
/// Bots this close to the owner's head count toward the smoothness metrics.
const NEAR_RADIUS: f64 = 2000.;
/// A reversal turns the wanted heading back by two of the 251 wire steps
/// from its last extreme; smaller wiggles are not visible jitter.
const REVERSAL: f64 = 2. * TAU / 251.;

impl Drive {
    /// The owner's wanted heading at `time`; `line` is the heading a
    /// straight stretch holds and `turn` accumulates gentle turns.
    fn wanted(self, time: f64, heading: f64, line: &mut f64, turn: &mut f64, dt: f64) -> f64 {
        let tightest = |side: f64| heading + side * PI / 2.;
        let phase = match self {
            Drive::Straight => 0,
            Drive::Tightest => 1,
            Drive::Gentle => 3,
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
        if phase != 3 {
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
            // Zigzag: one radian either side, switching every half second.
            _ if ((time * 2.) as u64).is_multiple_of(2) => *line + 1.,
            _ => *line - 1.,
        }
    }
}

struct Outcome {
    /// Times the owner died to a bot.
    owner_deaths: Vec<f64>,
    /// Deaths after the steady state settled: these break the guarantee.
    late_deaths: usize,
    /// Bots that died on the owner, anywhere along it.
    body_kills: usize,
    food: usize,
    near_seconds: f64,
    reversals: usize,
    escape_ticks: u64,
}

impl Outcome {
    fn report(&self, drive: Drive, avoidance: bool) {
        println!(
            "{drive:?} avoidance={avoidance} owner_deaths={:?} late_deaths={} body_kills={} \
             death_food={} near_bot_seconds={:.0} reversals_per_near_bot_second={:.3} \
             escaping_share={:.4}",
            self.owner_deaths,
            self.late_deaths,
            self.body_kills,
            self.food,
            self.near_seconds,
            self.reversals as f64 / self.near_seconds,
            self.escape_ticks as f64 * physics::LOOP_SECONDS / self.near_seconds,
        );
    }
}

fn scenario(drive: Drive, avoidance: bool, seconds: f64, seed: u32) -> Outcome {
    let mut raw = host(256, BotBehavior::Follow, seed);
    if drive == Drive::Straight {
        // A long straight route that stays inside the wire-compatible arena.
        let mut config = raw.backend.config.clone();
        config.world = Config {
            radius: 32000.,
            dynamic_radius: false,
            size_to_population: false,
            ..config.world
        };
        raw = RawSimulator::new(seed, config, 257).unwrap();
        bots(&mut raw, BotBehavior::Follow);
    }
    let start = Point {
        x: if drive == Drive::Straight {
            -17000.
        } else {
            0.
        },
        y: 0.,
    };
    owner(&mut raw, start, 80);
    raw.backend.in_world_bots.as_mut().unwrap().avoidance = avoidance;
    // Explicitly synthetic initial crowd. Respawns thereafter use only the
    // host's normal sampler, clearance search and owner-path check.
    for slot in 1usize..257 {
        let side = if slot.is_multiple_of(2) { 1. } else { -1. };
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
        let (body, heading) = raw.backend.world.spawn_body(head, heading).unwrap();
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
    };
    let mut seen_deaths = 0;
    let (mut line, mut turn) = (0., 0.);
    // The steady state starts with the run, after the maneuvers, or at a
    // rejoin; it settles after the fit window and the longest body lifetime.
    let mut steady_from = if drive == Drive::Maneuvers {
        MANEUVER_SECONDS
    } else {
        0.
    };
    let mut longest_release = 0f64;
    // Per bot: life, wanted heading at the last extreme, turn direction.
    let mut steering = vec![(0u64, 0f64, 0f64); 257];
    for tick in 0..(seconds / dt).ceil() as usize {
        let time = tick as f64 * dt;
        let heading = raw.backend.world.snakes()[0].heading;
        raw.backend.slots[0].control.heading =
            drive.wanted(time, heading, &mut line, &mut turn, dt);
        raw.advance_ticks(1).unwrap();
        raw.take(0).unwrap();
        let world = &raw.backend.world;
        let owner = world.snakes()[0].head;
        for (slot, entry) in steering.iter_mut().enumerate().skip(1) {
            let bot = &world.snakes()[slot];
            if bot.alive && time >= steady_from {
                let release = crate::client::host_bots::release_time(bot.segments, bot.fam);
                longest_release = longest_release.max(release);
            }
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
            let settle = steady_from + crate::client::host_bots::FIT_SECONDS + longest_release;
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
    outcome.report(drive, avoidance);
    outcome
}

fn assert_safe_and_feeding(drive: Drive, seed: u32) {
    let outcome = scenario(drive, true, 180., seed);
    assert_eq!(
        outcome.late_deaths, 0,
        "{drive:?} owner died after settling"
    );
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

#[test]
fn host_bots_safety_after_maneuvers_and_a_steady_hold() {
    assert_safe_and_feeding(Drive::Maneuvers, 73);
}

#[test]
fn host_bots_sensitivity_control() {
    // Without avoidance, bots chasing the head kill the circling owner: the
    // matched crowd is dangerous, so the safety runs above are meaningful.
    let control = scenario(Drive::Tightest, false, 180., 73);
    assert!(!control.owner_deaths.is_empty());
}

#[test]
#[ignore = "explicit robustness sweep: the four safety runs on more seeds"]
fn host_bots_safety_on_more_seeds() {
    for seed in [11, 29, 47] {
        for drive in [
            Drive::Straight,
            Drive::Tightest,
            Drive::Gentle,
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
