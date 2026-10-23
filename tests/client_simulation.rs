#![cfg(feature = "networking")]

use slither_sim::{
    Action,
    client::{
        Backend, ClientFrame, ClientState, ClientStatus, HostConfig, SimulationConfig,
        SimulatorBackend, WireEvent, WireRecord, latency::LatencyConfig, protocol::split_frame,
        replay_record,
    },
    physics,
};
use std::f64::consts::TAU;

const DT: f64 = physics::LOOP_SECONDS;

fn config(agents: usize, latency: LatencyConfig) -> SimulationConfig {
    let mut config = SimulationConfig {
        agents,
        latency,
        record_wire: true,
        ..Default::default()
    };
    config.world.radius = 1500.;
    config.world.vision = 900.;
    config.world.food_count = 200;
    config
}

fn playing(backend: &mut SimulatorBackend, slot: usize) -> ClientFrame {
    for _ in 0..200 {
        let frame = backend.frame_for(slot).unwrap();
        if frame.status == ClientStatus::Playing {
            return frame;
        }
        backend.advance_ticks(1).unwrap();
    }
    panic!("slot {slot} never started playing");
}

fn packets(record: &WireRecord) -> Vec<Vec<u8>> {
    split_frame(&record.bytes)
        .unwrap()
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

fn appearances(records: &[WireRecord]) -> Vec<u16> {
    records
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .flat_map(packets)
        .filter(|p| p[0] == b's' && p.len() > 7)
        .map(|p| u16::from_be_bytes([p[1], p[2]]))
        .collect()
}

#[test]
fn self_appears_first_and_nothing_is_observed_before_it() {
    // The setup's server number (1) is deliberately not the controlled snake id.
    let mut config = config(3, LatencyConfig::fixed(0.2));
    config.controlled_slot = 2;
    let mut backend = SimulatorBackend::new(3, config).unwrap();
    let mut waited = false;
    while backend.frame().unwrap().status == ClientStatus::Waiting {
        let frame = backend.frame().unwrap();
        assert!(frame.observation.is_none());
        assert!(frame.received.snakes.is_empty());
        waited = true;
        backend.advance_ticks(1).unwrap();
    }
    assert!(waited && backend.time() >= 0.2);
    let frame = backend.frame().unwrap();
    assert_eq!(frame.status, ClientStatus::Playing);
    let me = &frame.observation.unwrap().me;
    assert_eq!(me.id, "2");
    let actual = &backend.world().snakes[2];
    assert!(me.body[0].distance(actual.body[0]) < 1.);
    let records = backend.drain_records(2).unwrap();
    let first = records
        .iter()
        .find(|r| r.event == WireEvent::Receive)
        .unwrap();
    let setup = &packets(first)[0];
    assert_eq!(
        (setup[0], u16::from_be_bytes([setup[25], setup[26]])),
        (b'a', 1)
    );
    assert_eq!(appearances(&records)[0], 2);
}

#[test]
fn commands_wait_for_the_uplink_and_frames_for_the_downlink() {
    let mut backend =
        SimulatorBackend::new(1, config(1, LatencyConfig::one_way(0.1, 0.05))).unwrap();
    playing(&mut backend, 0);
    let receipts: Vec<_> = backend
        .drain_records(0)
        .unwrap()
        .into_iter()
        .filter(|r| r.event == WireEvent::Receive)
        .map(|r| r.time)
        .collect();
    // Connect arrives after 0.1 s; setup is emitted at that tick, delivered 0.05 s later.
    assert!(receipts[0] >= 0.15 - 1e-9 && receipts[0] < 0.15 + DT);
    for &at in &receipts[1..] {
        let emitted = at - 0.05;
        assert!((emitted / DT - (emitted / DT).round()).abs() < 1e-6);
    }

    let heading = backend.world().snakes[0].heading;
    let sent_at = backend.time();
    backend
        .send_to(
            0,
            Action {
                heading: heading + 2.,
                boost: false,
            },
        )
        .unwrap();
    let mut turned = None;
    for _ in 0..40 {
        backend.advance_ticks(1).unwrap();
        if turned.is_none() && backend.world().snakes[0].heading != heading {
            turned = Some(backend.time());
        }
    }
    let turned = turned.expect("command never applied");
    assert!(turned - sent_at >= 0.1 && turned - sent_at <= 0.1 + 2. * DT);
}

#[test]
fn jitter_and_congestion_never_reorder_either_stream() {
    let mut latency = LatencyConfig::fixed(0.12);
    latency.jitter_seconds = 0.05;
    latency.congestion.rate_per_second = 1.;
    latency.congestion.duration_seconds = 0.5;
    latency.congestion.extra_seconds = [0.05, 0.3];
    let mut backend = SimulatorBackend::new(9, config(2, latency)).unwrap();
    playing(&mut backend, 0);
    let mut last = 0;
    for step in 0..120 {
        if !backend.decision_pending(0).unwrap() {
            last = step % 200;
            let heading = f64::from(last) * TAU / 251. + 0.001;
            backend
                .send_to(
                    0,
                    Action {
                        heading,
                        boost: false,
                    },
                )
                .unwrap();
        }
        backend.advance_ticks(6).unwrap();
    }
    backend.advance_ticks(200).unwrap();
    // The last command in send order is the one in force.
    let wanted = backend
        .frame()
        .unwrap()
        .received
        .snake("0")
        .unwrap()
        .wanted_heading;
    assert!(wanted.valid);
    let records = backend.drain_records(0).unwrap();
    let receipts: Vec<_> = records
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .collect();
    assert!(receipts.windows(2).all(|w| w[0].time <= w[1].time));
    // Server clock from headers increases in delivery order while transit varies.
    let mut server = 0.;
    let mut transit = Vec::new();
    for (index, record) in receipts.iter().enumerate() {
        let header = f64::from(u16::from_be_bytes([record.bytes[0], record.bytes[1]]));
        if index > 0 {
            server += header / 1000.;
        }
        transit.push(record.time - server);
    }
    let spread = transit.iter().copied().fold(f64::MIN, f64::max)
        - transit.iter().copied().fold(f64::MAX, f64::min);
    assert!(spread > 0.02);
    let snake = &backend.world().snakes[0];
    let target = (f64::from(last) * TAU / 251.).rem_euclid(TAU);
    let error = (snake.heading - target)
        .sin()
        .atan2((snake.heading - target).cos());
    assert!(error.abs() < 0.03, "{error}");
}

#[test]
fn ping_is_measured_from_delivered_pongs_not_configured() {
    let mut latency = LatencyConfig::fixed(0.1);
    let mut backend = SimulatorBackend::new(2, config(1, latency.clone())).unwrap();
    backend.advance_ticks(400).unwrap();
    let timing = backend.frame().unwrap().timing;
    let ping = timing.ping_seconds.unwrap();
    // The server answers at its first tick after arrival.
    assert!((0.1..=0.1 + DT + 1e-9).contains(&ping), "{ping}");
    let records = backend.drain_records(0).unwrap();
    assert!(
        records
            .iter()
            .filter(|r| r.event == WireEvent::Send)
            .all(|r| r.bytes == [251])
    );

    latency.jitter_seconds = 0.03;
    let mut backend = SimulatorBackend::new(2, config(1, latency)).unwrap();
    backend.advance_ticks(1000).unwrap();
    let frame = backend.frame().unwrap();
    let [low, high] = frame.timing.ping_range_seconds.unwrap();
    assert!(low < high && low >= 0.04 && high <= 0.16 + DT);
    let json = serde_json::to_string(&frame).unwrap();
    for hidden in ["roundTrip", "jitter", "uplink", "downlink", "congestion"] {
        assert!(!json.contains(hidden));
    }
}

#[test]
fn processing_delays_departure_and_is_known_to_the_client() {
    let mut latency = LatencyConfig::fixed(0.);
    latency.processing_seconds = [0.05, 0.05];
    let mut backend = SimulatorBackend::new(4, config(1, latency)).unwrap();
    playing(&mut backend, 0);
    backend.drain_records(0).unwrap();
    let action = Action {
        heading: 1.,
        boost: false,
    };
    let start = backend.time();
    backend.send_to(0, action).unwrap();
    assert!(backend.decision_pending(0).unwrap());
    assert!(backend.send_to(0, action).is_err());
    assert!(backend.step_all(&[Some(action)]).is_err());
    let sends = |backend: &mut SimulatorBackend| -> Vec<f64> {
        let records = backend.drain_records(0).unwrap();
        records
            .iter()
            .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
            .map(|r| r.time)
            .collect()
    };
    backend.step_all(&[None]).unwrap();
    backend.advance_ticks(2).unwrap();
    assert!(!backend.decision_pending(0).unwrap());
    let departures = sends(&mut backend);
    assert!(!departures.is_empty());
    assert!(
        departures
            .iter()
            .all(|&at| (at - start - 0.05).abs() < 1e-9)
    );
    assert_eq!(backend.frame().unwrap().timing.processing_seconds, 0.05);

    // A measured reply time replaces the configured draw for one decision.
    backend.set_processing_seconds(0.08).unwrap();
    assert_eq!(backend.frame().unwrap().timing.processing_seconds, 0.08);
    let start = backend.time();
    backend.send_to(0, action).unwrap();
    backend.advance_ticks(20).unwrap();
    let departures = sends(&mut backend);
    assert!(
        departures
            .iter()
            .all(|&at| (at - start - 0.08).abs() < 1e-9)
    );
}

#[test]
fn next_frame_waits_for_virtual_processing_longer_than_a_decision() {
    let mut backend = SimulatorBackend::new(4, config(1, LatencyConfig::fixed(0.02))).unwrap();
    while backend.next_frame().unwrap().status != ClientStatus::Playing {}
    backend.set_processing_seconds(0.1).unwrap();
    let start = backend.time();
    backend
        .send_action(Action {
            heading: 2.,
            boost: false,
        })
        .unwrap();
    let frame = backend.next_frame().unwrap();
    assert!(frame.time - start >= 0.1 - 1e-9);
    assert!(!backend.decision_pending(0).unwrap());
    assert_eq!(frame.timing.processing_seconds, 0.1);
    assert_eq!(frame.timing.sent_commands.len(), 1);
}

#[test]
fn wire_records_replay_to_identical_frames() {
    let mut latency = LatencyConfig::fixed(0.08);
    latency.jitter_seconds = 0.02;
    let mut config = config(3, latency);
    config.world.prey.count = 4;
    let mut backend = SimulatorBackend::new(11, config).unwrap();
    let mut replays: Vec<ClientState> = (0..3).map(|_| ClientState::new()).collect();
    for step in 0..150 {
        let actions: Vec<_> = (0..3)
            .map(|slot| {
                let playing = backend.frame_for(slot).unwrap().status == ClientStatus::Playing;
                (playing && step % 3 == slot).then_some(Action {
                    heading: step as f64 * 0.37 + slot as f64,
                    boost: false,
                })
            })
            .collect();
        let frames = backend.step_all(&actions).unwrap();
        if step == 80 {
            backend.reconnect_slot(1).unwrap();
        }
        for (slot, replay) in replays.iter_mut().enumerate() {
            for record in backend.drain_records(slot).unwrap() {
                replay_record(replay, &record).unwrap();
            }
            let now = backend.time();
            let expected = backend.frame_for(slot).unwrap();
            assert_eq!(
                replay.frame(now).unwrap(),
                expected,
                "slot {slot} step {step}"
            );
            if step != 80 || slot != 1 {
                assert_eq!(frames[slot], expected);
            }
        }
    }
    let frame = backend.frame_for(0).unwrap();
    assert!(frame.timing.ping_seconds.is_some());
    assert!(!frame.timing.sent_commands.is_empty());
    assert_eq!(
        backend.frame_for(1).unwrap().received.connection_generation,
        2
    );
}

#[test]
fn received_state_hides_private_metadata() {
    let mut config = config(3, LatencyConfig::fixed(0.05));
    config.world.vision = 3000.;
    let mut backend = SimulatorBackend::new(6, config).unwrap();
    for _ in 0..80 {
        backend.next_frame().unwrap();
    }
    let frame = backend.frame().unwrap();
    let observation = frame.observation.clone().unwrap();
    assert!(!observation.opponents.is_empty());
    for snake in std::iter::once(&observation.me).chain(&observation.opponents) {
        assert_eq!(snake.kills, 0);
        assert_eq!(snake.score, physics::score(snake.segments, snake.fam));
    }
    assert!(observation.food.iter().all(|food| food.origin.is_none()));
    assert_eq!(observation.arena.center.x, 0.);
    let json = serde_json::to_string(&frame).unwrap();
    assert!(!json.contains("origin") && !json.contains("ambient"));
}

#[test]
fn appearances_only_on_entry_and_unreported_fields_keep_aging() {
    let mut config = config(2, LatencyConfig::fixed(0.02));
    config.world.vision = 3000.;
    config.world.food_count = 0;
    let mut backend = SimulatorBackend::new(8, config).unwrap();
    playing(&mut backend, 0);
    backend.advance_ticks(140).unwrap();
    let records = backend.drain_records(0).unwrap();
    assert_eq!(appearances(&records), [0, 1]);
    // Straight cruise: anchors refresh the body, but no heading report is sent.
    let frame = backend.frame().unwrap();
    let me = frame.received.snake("0").unwrap();
    assert!(me.heading.age_seconds > 0.5, "{}", me.heading.age_seconds);
    assert!(me.body.age_seconds < 0.5);
    let observed = frame.observation.unwrap().me;
    let actual = &backend.world().snakes[0];
    assert_eq!(observed.body.len(), actual.body.len());
    for (a, b) in observed.body.iter().zip(&actual.body) {
        assert!(a.distance(*b) < 0.5);
    }
}

#[test]
fn dead_slots_stop_and_reconnect_resets_history() {
    let mut config = config(2, LatencyConfig::fixed(0.02));
    config.world.radius = 600.;
    config.world.vision = 3000.;
    config.world.boundary_mode = slither_sim::BoundaryMode::Hard;
    let mut backend = SimulatorBackend::new(5, config).unwrap();
    playing(&mut backend, 0);
    playing(&mut backend, 1);
    // Slot 1 starts on the ring at angle pi facing the center; turn it into the
    // rim while slot 0 turns aside.
    let outward = Action {
        heading: std::f64::consts::PI,
        boost: false,
    };
    let aside = Action {
        heading: std::f64::consts::FRAC_PI_2,
        boost: false,
    };
    let mut sent = false;
    for _ in 0..200 {
        let actions = [(!sent).then_some(aside), (!sent).then_some(outward)];
        sent = true;
        let frames = backend.step_all(&actions).unwrap();
        if frames[1].status == ClientStatus::Dead {
            break;
        }
    }
    assert!(!backend.world().snakes[1].alive);
    let frame = backend.frame_for(1).unwrap();
    assert_eq!(frame.status, ClientStatus::Dead);
    assert!(backend.send_to(1, outward).is_err());
    backend.step_all(&[None, None]).unwrap();
    let opponents = &backend.frame_for(0).unwrap().observation.unwrap().opponents;
    assert!(opponents.is_empty());

    let generation = frame.received.connection_generation;
    backend.reconnect_slot(1).unwrap();
    let frame = backend.frame_for(1).unwrap();
    assert_ne!(frame.status, ClientStatus::Playing);
    assert!(frame.received.snakes.is_empty() && frame.timing.sent_commands.is_empty());
    assert!(frame.timing.ping_seconds.is_none());
    assert_eq!(frame.received.connection_generation, generation + 1);
    assert!(backend.world().snakes[1].alive);
    let frame = playing(&mut backend, 1);
    let front = frame.observation.unwrap().me.body[0];
    assert!(front.distance(backend.world().snakes[1].body[0]) < 1.);
    let records = backend.drain_records(1).unwrap();
    let disconnect = records
        .iter()
        .position(|r| r.event == WireEvent::Disconnect)
        .unwrap();
    assert_eq!(records[disconnect + 1].event, WireEvent::Connect);
}

#[test]
fn background_slots_steer_from_their_own_frames() {
    let mut config = config(3, LatencyConfig::fixed(0.04));
    config.world.food_count = 400;
    let mut backend = SimulatorBackend::new(12, config).unwrap();
    for _ in 0..100 {
        backend.next_frame().unwrap();
    }
    for slot in 1..3 {
        let records = backend.drain_records(slot).unwrap();
        assert!(
            records
                .iter()
                .any(|r| r.event == WireEvent::Send && r.bytes[0] <= 250)
        );
    }
}

#[test]
fn unsupported_configurations_are_rejected() {
    let reject = |edit: fn(&mut SimulationConfig)| {
        let mut config = SimulationConfig::default();
        edit(&mut config);
        assert!(SimulatorBackend::new(1, config).is_err());
    };
    reject(|c| c.world.radius = 40_000.);
    reject(|c| c.agents = 0);
    reject(|c| c.controlled_slot = 1);
    reject(|c| c.broadcast_ticks = 40);
    reject(|c| c.minimap_side = 513);
    reject(|c| c.leaderboard_interval_seconds = 0.);
    reject(|c| c.leaderboard_interval_seconds = -1.);
    reject(|c| c.leaderboard_interval_seconds = f64::NAN);
    reject(|c| c.leaderboard_interval_seconds = f64::INFINITY);
    reject(|c| c.latency.round_trip_weights = vec![f64::MAX, f64::MAX]);
    assert!(serde_json::from_str::<SimulationConfig>(r#"{"agents":2,"unknown":1}"#).is_err());
    let config: SimulationConfig =
        serde_json::from_str(r#"{"agents":2,"latency":{"roundTripSeconds":[0.1]}}"#).unwrap();
    assert_eq!(config.broadcast_ticks, 6);
    assert_eq!(config.leaderboard_interval_seconds, 3.);
    let config: SimulationConfig =
        serde_json::from_str(r#"{"leaderboardIntervalSeconds":1.5}"#).unwrap();
    assert_eq!(config.leaderboard_interval_seconds, 1.5);
    assert!(!config.record_wire);
    let mut backend = SimulatorBackend::new(1, SimulationConfig::default()).unwrap();
    assert!(backend.drain_records(0).is_err());
}

#[test]
fn prey_speed_validation_matches_the_wire_rounding_boundary() {
    let mut config = SimulationConfig::default();
    for (speed, accepted) in [
        (65535. / 32., true),
        (65535.49 / 32., true),
        (65535.5 / 32., false),
    ] {
        config.world.prey.cruise_speed = speed;
        config.world.prey.escape_speed = speed;
        config.world.prey.sprint_speed = speed;
        // The core simulator retains its broader configuration range.
        assert!(config.world.validate().is_ok());
        assert_eq!(config.validate().is_ok(), accepted);
    }
}

#[test]
fn prey_size_validation_checks_attainable_wire_sizes() {
    let mut config = SimulationConfig::default();
    for (size, max_size, accepted) in [
        (51., 51., true),
        (51., 51.01, true),
        (51., f64::from_bits(51.2_f64.to_bits() - 1), true),
        (51., 51.2, false),
        (51.2, 51.2, false),
        (60., 60., false),
    ] {
        config.world.prey.size = size;
        config.world.prey.max_size = max_size;
        // The core simulator retains its broader configuration range.
        assert!(config.world.validate().is_ok());
        assert_eq!(config.validate().is_ok(), accepted, "{size}..={max_size}");
        let host = HostConfig {
            simulation: config.clone(),
            ..Default::default()
        };
        assert_eq!(
            host.validate().is_ok(),
            accepted,
            "host {size}..={max_size}"
        );
    }
}

#[test]
fn accepted_prey_size_matches_the_received_appearance() {
    for (size, max_size, expected) in [(0.2, 0.2, 0.2), (51., 51., 51.), (51., 51.01, 51.)] {
        let mut config = SimulationConfig::default();
        config.world.food_count = 0;
        config.world.vision = 3000.;
        config.world.prey.count = 1;
        config.world.prey.size = size;
        config.world.prey.max_size = max_size;
        let mut backend = SimulatorBackend::new(7, config).unwrap();
        backend.advance_ticks(6).unwrap();
        let authoritative = &backend.world().prey;
        let received = backend.frame().unwrap().observation.unwrap().prey;
        assert_eq!(authoritative.len(), 1);
        assert_eq!(received.len(), 1);
        assert_eq!(authoritative[0].size, expected);
        assert_eq!(received[0].size, authoritative[0].size);
    }
}

#[test]
fn measured_processing_without_an_action_still_occupies_the_decision() {
    let mut backend = SimulatorBackend::new(4, config(1, LatencyConfig::fixed(0.02))).unwrap();
    while backend.next_frame().unwrap().status != ClientStatus::Playing {}
    backend.drain_records(0).unwrap();
    let commands = |backend: &mut SimulatorBackend| -> Vec<f64> {
        let records = backend.drain_records(0).unwrap();
        records
            .iter()
            .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
            .map(|r| r.time)
            .collect()
    };

    // The bot decided nothing, but deciding took 0.12 s of its virtual CPU.
    backend.set_processing_seconds(0.12).unwrap();
    let start = backend.time();
    let frame = backend.next_frame().unwrap();
    assert!(frame.time - start >= 0.12 - 1e-9);
    assert_eq!(frame.timing.processing_seconds, 0.12);
    assert!(!backend.decision_pending(0).unwrap());
    assert!(commands(&mut backend).is_empty());

    // The measured duration of a decision with an action delays that action.
    backend.set_processing_seconds(0.03).unwrap();
    let start = backend.time();
    let action = Action {
        heading: 1.,
        boost: false,
    };
    backend.send_action(action).unwrap();
    backend.next_frame().unwrap();
    let sent = commands(&mut backend);
    assert!(!sent.is_empty() && sent.iter().all(|&at| (at - start - 0.03).abs() < 1e-9));

    // Explicit multi-slot stepping keeps a busy slot pending across decisions.
    backend.set_slot_processing_seconds(0, 0.12).unwrap();
    let start = backend.time();
    backend.step_all(&[None]).unwrap();
    assert!(backend.decision_pending(0).unwrap());
    assert!(backend.step_all(&[Some(action)]).is_err());
    assert!(backend.set_slot_processing_seconds(0, 0.01).is_err());
    while backend.decision_pending(0).unwrap() {
        backend.step_all(&[None]).unwrap();
    }
    assert!(backend.time() - start >= 0.12 - 1e-9);
    assert!(commands(&mut backend).is_empty());
    backend.step_all(&[Some(action)]).unwrap();
    assert!(!commands(&mut backend).is_empty());
}

#[test]
fn header_clock_tracks_server_time_with_fractional_ticks() {
    for dt in [0.0071, 0.00725] {
        let mut config = config(1, LatencyConfig::default());
        config.world.dt = dt;
        let mut backend = SimulatorBackend::new(2, config).unwrap();
        backend.advance_ticks((12. / dt) as usize).unwrap();
        let receipts: Vec<_> = backend
            .drain_records(0)
            .unwrap()
            .into_iter()
            .filter(|r| r.event == WireEvent::Receive)
            .collect();
        assert_eq!(packets(&receipts[0])[0][0], b'a');
        assert!(receipts.iter().any(|r| r.bytes[2..] == *b"p"));
        // Without latency, receipt time is emission time.
        let setup = receipts[0].time;
        let mut server_ms = 0u64;
        for record in &receipts[1..] {
            server_ms += u64::from(u16::from_be_bytes([record.bytes[0], record.bytes[1]]));
            let actual = (record.time - setup) * 1000.;
            assert!(
                (actual - server_ms as f64).abs() <= 1.,
                "dt {dt}: {actual} vs {server_ms}"
            );
        }
        assert!(server_ms > 11_000);
    }
}

/// Fields of the stock `l` handler: own row, rank, population and rows of
/// `(segments, fam, name)`.
fn leaderboard(p: &[u8]) -> (u8, usize, usize, Vec<(usize, f64, String)>) {
    let u16_at = |m: usize| usize::from(u16::from_be_bytes([p[m], p[m + 1]]));
    let mut rows = Vec::new();
    let mut m = 6;
    while m < p.len() {
        let fam = f64::from(u32::from_be_bytes([0, p[m + 2], p[m + 3], p[m + 4]])) / 16777215.;
        let n = usize::from(p[m + 6]);
        let name = String::from_utf8(p[m + 7..m + 7 + n].to_vec()).unwrap();
        rows.push((u16_at(m), fam, name));
        m += 7 + n;
    }
    (p[1], u16_at(2), u16_at(4), rows)
}

#[test]
fn native_leaderboards_follow_own_appearance_with_ranked_rows() {
    let mut backend = SimulatorBackend::new(8, config(3, LatencyConfig::default())).unwrap();
    while backend.time() < 7.5 {
        backend.next_frame().unwrap();
    }
    let received: Vec<(f64, Vec<u8>)> = backend
        .drain_records(0)
        .unwrap()
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .flat_map(|r| packets(r).into_iter().map(move |p| (r.time, p)))
        .collect();
    let own = received
        .iter()
        .position(|(_, p)| p[0] == b's' && p.len() > 7 && p[1..3] == [0, 0])
        .unwrap();
    let boards: Vec<_> = received
        .iter()
        .enumerate()
        .filter(|(_, (_, p))| p[0] == b'l')
        .collect();
    assert!(boards[0].0 > own);
    assert_eq!(boards[0].1.0, received[own].0);
    assert_eq!(boards.len(), 3);
    for pair in boards.windows(2) {
        let gap = pair[1].1.0 - pair[0].1.0;
        assert!((3. - 1e-9..3. + 6. * DT).contains(&gap), "{gap}");
    }
    for (_, (_, packet)) in boards {
        let (position, rank, count, rows) = leaderboard(packet);
        assert!((1..=3).contains(&count) && rows.len() == count);
        // Native slots are named by slot; rows descend by the stock display
        // score of their wire mass, ties by slot.
        let keys: Vec<_> = rows
            .iter()
            .map(|(segments, fam, name)| {
                let slot: usize = name.parse().unwrap();
                (-physics::score(*segments, *fam), slot)
            })
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        if position > 0 {
            assert_eq!(usize::from(position), rank);
            assert_eq!(rows[rank - 1].2, "0");
        }
    }
}
