//! Deterministic, synthetic host-policy checks; no outside servers or clients.
use super::*;
use crate::{Config, client::BotBehavior, physics};
use std::f64::consts::PI;

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
fn host_bots_follow_aims_at_the_owners_current_head_every_tick() {
    let mut raw = host(4, BotBehavior::Follow, 29);
    owner(&mut raw, Point::default(), 20);
    for (slot, (x, y)) in [(500., 500.), (-500., 700.), (500., -500.), (-500., -700.)]
        .into_iter()
        .enumerate()
    {
        let (body, heading) = raw.backend.world.spawn_body(Point { x, y }, 0.).unwrap();
        raw.backend
            .world
            .respawn_body(slot + 1, body, heading)
            .unwrap();
    }
    raw.backend.slots[0].control.heading = PI / 2.;
    for _ in 0..30 {
        let snakes = raw.backend.world.snakes();
        let head = snakes[0].head;
        let expected: Vec<_> = snakes[1..]
            .iter()
            .map(|snake| (head.y - snake.head.y).atan2(head.x - snake.head.x))
            .collect();
        raw.advance_ticks(1).unwrap();
        for (slot, heading) in raw.backend.slots[1..].iter().zip(expected) {
            assert_eq!(slot.control.heading, heading);
            assert!(!slot.control.boost);
        }
        assert!(raw.backend.world.snakes().iter().all(|snake| snake.alive));
    }
    // Losing the owner returns to ordinary staggered center steering.
    raw.disconnect(0).unwrap();
    assert_eq!(raw.backend.owner_slot(), None);
    for _ in 0..6 {
        let expected: Vec<_> = raw.backend.world.snakes()[1..]
            .iter()
            .zip(&raw.backend.slots[1..])
            .enumerate()
            .map(|(index, (snake, slot))| {
                if (raw.backend.tick + index as u64).is_multiple_of(6) {
                    (-snake.head.y).atan2(-snake.head.x)
                } else {
                    slot.control.heading
                }
            })
            .collect();
        raw.advance_ticks(1).unwrap();
        for (slot, heading) in raw.backend.slots[1..].iter().zip(expected) {
            assert_eq!(slot.control.heading, heading);
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
