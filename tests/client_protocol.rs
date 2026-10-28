#![cfg(feature = "networking")]

use serde_json::Value;
use slither_sim::{
    client::{
        ClientFrame, ClientState, ClientStatus, MAX_FRAME_MINIMAP_CELLS,
        protocol::{action_packets, split_frame},
    },
    physics, *,
};
use std::f64::consts::TAU;

const GRID: f64 = 21600.;
// Same archived bot0 challenge as client_challenge.rs, record 4.
const RAW_CHALLENGE: &[u8] = b"6euLxuGXMjVNCwXDSLcWXFbjNuKZXhYQPBNffoPvMhWQcTpdrOHROCcmItebmheOLZwfSnCyLBxlKwRCBjGSmxOgfRIZZIulatqGwnZWizOLFQRYmgfrLwWEUMHutfZJjRrZXmhtNbQjWrdyVHBonUvfrJmVjDwLxUgzPjjRGZPFTqetQFZJHSreb";

#[test]
fn alphabetic_timed_session_report_does_not_select_bare_framing() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state.receive_frame(b"6ASAAAAAA", 0.).unwrap();
    let report = state.frame(0.).unwrap();
    assert!(report.hud.session.is_some());
    let mut frame = vec![0, 0];
    frame.extend(setup());
    state.receive_frame(&frame, 0.1).unwrap();
    assert!(state.frame(0.1).unwrap().settings.is_some());
}

fn bundled(packets: &[&[u8]]) -> Vec<u8> {
    let mut frame = vec![0, 0];
    for packet in packets {
        frame.extend((packet.len() as u16).to_be_bytes());
        frame.extend(*packet);
    }
    frame
}

#[test]
fn retained_body_limits_reject_whole_frames_and_reuse_removed_slots() {
    let mut client = Client::with_setup(&setup_with(1, 0, 430));
    client.send(&straight(9, 0.));
    let grow = [b'N', 0, 0, 0];
    let growth = bundled(&vec![grow.as_slice(); 4093]);
    client.state.receive_frame(&growth, client.time).unwrap();
    assert_eq!(client.me().body.len(), 4096);
    let before = client.frame();
    let error = client
        .state
        .receive_frame(&bundled(&[&[b'e', 64], &grow]), client.time)
        .unwrap_err();
    assert!(error.contains("snake body limit"));
    assert_eq!(client.frame(), before);

    let mut client = Client::new();
    let points = vec![Point::default(); 4094];
    for id in 1..=16 {
        client.send(&appearance(id, 0, 5000, Point::default(), &points, Some(0)));
    }
    let before = client.frame();
    let next = appearance(
        17,
        0,
        5000,
        Point::default(),
        &[Point::default(); 16],
        Some(0),
    );
    assert!(
        client
            .state
            .receive_packet(&next, client.server, client.time)
            .unwrap_err()
            .contains("total body limit")
    );
    assert_eq!(client.frame(), before);
    // Removed records still retain their bodies; replacement reuses that budget.
    client.send(&[b's', 0, 2]);
    client.send(&next);
    assert_eq!(client.frame().received.snakes.len(), 16);
}

#[test]
fn retained_food_limit_counts_unique_ids_and_releases_cleared_sectors() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    for sx in 0..2 {
        let packet = food_sector(sx, 0, 65_536);
        client.send(&packet);
        client.send(&packet); // Replacement packets do not add retained entities.
    }
    assert_eq!(client.frame().observation.unwrap().food.len(), 131_072);
    let before = client.frame();
    let pellet = [b'f', 2, 0, 0, 0, 5];
    assert!(
        client
            .state
            .receive_frame(&bundled(&[&[b'e', 64], &pellet]), client.time)
            .unwrap_err()
            .contains("food limit")
    );
    assert_eq!(client.frame(), before);
    client
        .state
        .receive_frame(&bundled(&[&[b'w', 0, 0], &pellet]), client.time)
        .unwrap();
    assert_eq!(client.frame().observation.unwrap().food.len(), 65_537);
    client.state.new_connection(client.time).unwrap();
    client.send(&setup());
    client.send(&straight(9, 0.));
    client.send(&pellet);
    assert_eq!(client.frame().observation.unwrap().food.len(), 1);
}

// Synthetic protocol-19 food IDs, ordered by their sector-local coordinates.
fn food_sector(sx: u8, sy: u8, count: usize) -> Vec<u8> {
    let mut packet = vec![b'F', sx, sy];
    for id in 0..count {
        packet.extend([0, (id >> 8) as u8, id as u8, 5]);
    }
    packet
}

#[test]
fn repeated_sector_clears_preserve_unrelated_food_at_the_retention_limit() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    for sx in 0..2 {
        client.send(&food_sector(sx, 0, 65_536));
    }
    let before = client.frame();
    let empty_clear = [b'w', 2, 0];
    let replacement = [b'f', 0, 0, 0, 0, 5];
    let mut packets = vec![empty_clear.as_slice(); 65_536];
    // The replacement also exercises food-budget preflight for the whole frame.
    packets.push(&replacement);
    let frame = bundled(&packets);
    assert!(frame.len() < 1 << 20);
    client.state.receive_frame(&frame, client.time).unwrap();
    assert_eq!(client.frame(), before);

    let occupied_clear = [b'w', 0, 0];
    let addition = [b'f', 2, 0, 0, 0, 5];
    let mut packets = vec![occupied_clear.as_slice(); 65_536];
    packets.push(&addition);
    client
        .state
        .receive_frame(&bundled(&packets), client.time)
        .unwrap();
    let food = client.frame().observation.unwrap().food;
    assert_eq!(food.len(), 65_537);
    assert_eq!(food[..65_536], before.observation.unwrap().food[65_536..]);
}

#[test]
fn sector_clear_preflight_preserves_packet_order_and_failure_atomicity() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&food_sector(0, 0, 65_536));
    client.send(&food_sector(255, 254, 65_534));
    client.send(&[b'F', 255, 255, 0, 255, 254, 5, 0, 255, 255, 5]);
    let before = client.frame().observation.unwrap().food;
    assert_eq!(before.len(), 131_072);

    let clear = [b'w', 255, 255];
    let first = [b'f', 255, 255, 255, 254, 25];
    let last = [b'f', 255, 255, 255, 255, 15];
    let packets: &[&[u8]] = &[
        &[b'c', 255, 255, 255, 254],     // Overlay false before the first clear.
        &[b'f', 255, 255, 255, 255, 10], // Overlay true replaces a base pellet.
        &clear,
        &clear,
        &last,
        &[b'f', 255, 254, 20], // Short addition uses the previous food sector.
        &[b'C', 255, 254],     // Short removal uses the previous eaten sector.
        &clear,
        &first,
        &last,
    ];
    client
        .state
        .receive_frame(&bundled(packets), client.time)
        .unwrap();
    let after = client.frame().observation.unwrap().food;
    assert_eq!(after.len(), 131_072);
    assert_eq!(after[..131_070], before[..131_070]);
    assert_eq!(after[131_070].size, Some(5.));
    assert_eq!(after[131_071].size, Some(3.));

    let before = client.frame();
    let overflow = bundled(&[&[b'e', 64], &clear, &first, &last, &[b'f', 1, 0, 0, 0, 5]]);
    let error = client
        .state
        .receive_frame(&overflow, client.time)
        .unwrap_err();
    assert!(error.contains("food limit"));
    assert_eq!(client.frame(), before);
}

fn u24(value: u32) -> [u8; 3] {
    let [_, a, b, c] = value.to_be_bytes();
    [a, b, c]
}
fn coordinate(centered: f64) -> [u8; 3] {
    u24(((centered + GRID) * 5.).round() as u32)
}

/// Protocol-19 setup with an explicit server number, contraction and max segments.
fn setup_with(server_number: u16, cst: u16, mscps: u16) -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend(u24(GRID as u32));
    p.extend(mscps.to_be_bytes());
    p.extend(300u16.to_be_bytes());
    p.extend(144u16.to_be_bytes());
    p.push(48);
    p.extend([0; 10]);
    p.extend(cst.to_be_bytes());
    p.extend([19, 42]);
    p.extend(server_number.to_be_bytes());
    p.extend(u24(21000));
    assert_eq!(p.len(), 30);
    p
}
fn setup() -> Vec<u8> {
    setup_with(1, 430, 430)
}

#[test]
fn unsupported_contraction_cannot_poison_received_geometry() {
    for contraction in [1001, u16::MAX] {
        let mut state = ClientState::new();
        state.new_connection(0.).unwrap();
        let before = state.frame(0.).unwrap();
        assert!(
            state
                .receive_packet(&setup_with(1, contraction, 430), 0., 0.1)
                .is_err()
        );
        assert_eq!(state.frame(0.).unwrap(), before);
        state
            .receive_packet(&setup_with(1, 1000, 430), 0., 0.1)
            .unwrap();
        assert_eq!(
            state.frame(0.1).unwrap().settings.unwrap().body_contraction,
            1.
        );
    }
}

/// Full `s` appearance. Points are centered, tail first; the last uses `iang`.
fn appearance(
    id: u16,
    heading_u24: u32,
    sp: u16,
    head: Point,
    tail: &[Point],
    iang: Option<u16>,
) -> Vec<u8> {
    let mut p = vec![b's'];
    p.extend(id.to_be_bytes());
    p.extend(u24(heading_u24));
    p.push(48);
    p.extend(u24(heading_u24));
    p.extend(sp.to_be_bytes());
    p.extend(u24(8_388_608));
    p.push(3);
    p.extend(coordinate(head.x));
    p.extend(coordinate(head.y));
    p.extend([2, b'o', b'k', 0, 255]);
    let mut previous: Option<Point> = None;
    for &point in tail {
        match previous {
            None => {
                p.extend(coordinate(point.x));
                p.extend(coordinate(point.y));
            }
            Some(last) => {
                p.push((127. + (point.x - last.x) * 2.) as u8);
                p.push((127. + (point.y - last.y) * 2.) as u8);
            }
        }
        previous = Some(point);
    }
    if let Some(iang) = iang {
        p.extend(iang.to_be_bytes());
    }
    p
}
fn pt(x: f64, y: f64) -> Point {
    Point { x, y }
}
/// A snake whose wire body is three anchors along +x ending in an iang anchor.
fn straight(id: u16, x: f64) -> Vec<u8> {
    appearance(
        id,
        0,
        5000,
        pt(x + 50., 0.),
        &[pt(x - 84., 0.), pt(x - 42., 0.)],
        Some(0),
    )
}

struct Client {
    state: ClientState,
    server: f64,
    time: f64,
}
impl Client {
    fn new() -> Self {
        Self::with_setup(&setup())
    }
    fn with_setup(setup: &[u8]) -> Self {
        let mut state = ClientState::new();
        state.new_connection(1.).unwrap();
        state.receive_packet(setup, 0., 1.).unwrap();
        Self {
            state,
            server: 0.,
            time: 1.,
        }
    }
    fn at(&mut self, dt: f64, packet: &[u8]) -> Result<(), String> {
        self.server += dt;
        self.time += dt;
        self.state.receive_packet(packet, self.server, self.time)
    }
    fn send(&mut self, packet: &[u8]) {
        self.at(0., packet).unwrap();
    }
    fn frame(&self) -> ClientFrame {
        self.state.frame(self.time).unwrap()
    }
    fn me(&self) -> Snake {
        self.frame().observation.unwrap().me
    }
    fn opponent(&self, id: &str) -> Option<Snake> {
        self.frame()
            .observation
            .unwrap()
            .opponents
            .into_iter()
            .find(|snake| snake.id == id)
    }
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn split_frame_uses_elapsed_header_and_length_prefixes() {
    let frame = [0, 7, 0, 3, 101, 1, 2, 35, 104, 0, 9];
    assert_eq!(
        split_frame(&frame).unwrap(),
        vec![&[101, 1, 2][..], &[104, 0, 9][..]]
    );
    assert_eq!(split_frame(&[0, 7, 112]).unwrap(), vec![&[112][..]]);
    for bad in [
        &b""[..],
        b"\x00\x07",
        b"\x00\x07\x00",
        b"\x00\x07\x00\x00",
        b"\x00\x07\x00\x03e",
        b"\x00\x07\x00\x01p\x23e",
    ] {
        assert!(split_frame(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn actions_use_the_251_step_heading_grid_and_send_boost_only_on_change() {
    let action = |heading, boost| Action { heading, boost };
    assert_eq!(
        action_packets(action(0., false), None).unwrap(),
        vec![vec![0], vec![254]]
    );
    assert_eq!(
        action_packets(action(0., true), Some(false)).unwrap(),
        vec![vec![0], vec![253]]
    );
    assert_eq!(
        action_packets(action(0., true), Some(true)).unwrap(),
        vec![vec![0]]
    );
    assert_eq!(
        action_packets(action(TAU - 1e-12, true), Some(true)).unwrap(),
        vec![vec![250]]
    );
    assert_eq!(
        action_packets(action(-1e-9, true), Some(true)).unwrap(),
        vec![vec![250]]
    );
    assert!(action_packets(action(f64::NAN, true), None).is_err());

    let mut client = Client::new();
    for k in 0..400 {
        let heading = k as f64 * 0.0731 - 7.;
        for packet in action_packets(action(heading, k % 3 == 0), None).unwrap() {
            client.state.record_sent(&packet, client.time).unwrap();
        }
        let sent = &client.frame().timing.sent_commands[0].action;
        assert_eq!(sent.heading, physics::quantize_heading(heading));
        assert_eq!(sent.boost, k % 3 == 0);
        client.time += 0.01;
    }
}

#[test]
fn sent_commands_need_both_components_and_keep_the_newest_64() {
    let mut client = Client::new();
    client.state.record_sent(&[10], 1.).unwrap();
    // Handshake and other multi-byte messages are not commands.
    client.state.record_sent(&[115, 30, 1], 1.).unwrap();
    client.state.record_sent(&[252, 140], 1.).unwrap();
    assert!(client.frame().timing.sent_commands.is_empty());
    client.state.record_sent(&[253], 1.).unwrap();
    for k in 0..70u8 {
        client
            .state
            .record_sent(&[k], 1. + f64::from(k) * 0.01)
            .unwrap();
    }
    client.time = 2.;
    let sent = client.frame().timing.sent_commands;
    assert_eq!(sent.len(), 64);
    assert_eq!(sent[0].action.heading, 69. * TAU / 251.);
    assert!(sent[0].action.boost);
    assert!(close(sent[0].seconds_ago, 2. - 1.69, 1e-12));
    assert!(
        sent.windows(2)
            .all(|w| w[0].seconds_ago <= w[1].seconds_ago)
    );
    // Heading and boost bytes sent at one instant form one command.
    client.state.record_sent(&[100], 2.).unwrap();
    client.state.record_sent(&[254], 2.).unwrap();
    let sent = client.frame().timing.sent_commands;
    assert_eq!(
        sent[0].action,
        Action {
            heading: 100. * TAU / 251.,
            boost: false
        }
    );
    assert_eq!(sent[1].action.heading, 69. * TAU / 251.);
}

#[test]
fn self_is_the_first_full_appearance_not_the_setup_server_number() {
    let mut client = Client::with_setup(&setup_with(7, 430, 430));
    assert_eq!(client.frame().status, ClientStatus::Waiting);
    // A removal is not an appearance.
    client.send(&[b's', 0, 5, 0]);
    assert_eq!(client.frame().status, ClientStatus::Waiting);
    assert!(client.frame().observation.is_none());
    client.send(&straight(9, 0.));
    client.send(&straight(7, 500.));
    let frame = client.frame();
    assert_eq!(frame.status, ClientStatus::Playing);
    assert_eq!(frame.life, 1);
    let observation = frame.observation.unwrap();
    assert_eq!(observation.me.id, "9");
    assert_eq!(observation.opponents.len(), 1);
    assert_eq!(observation.opponents[0].id, "7");
    // Implicit own updates follow the first appearance.
    client.send(&[b'e', 64]);
    assert_eq!(client.me().heading, TAU / 4.);
    assert_eq!(client.opponent("7").unwrap().heading, 0.);
}

#[test]
fn lifecycle_status_matches_frames_without_advancing_receipts() {
    let check = |client: &Client, expected| {
        let before = client.frame();
        let later = client.state.frame(client.time + 1.).unwrap();
        assert_eq!(client.state.status(), expected);
        assert_eq!(client.state.status(), before.status);
        assert_eq!(client.frame(), before);
        assert_eq!(client.state.frame(client.time + 1.).unwrap(), later);
    };
    let mut client = Client::new();
    check(&client, ClientStatus::Waiting);
    client.at(0.1, &straight(9, 0.)).unwrap();
    client.at(0.1, &[b'e', 64]).unwrap();
    check(&client, ClientStatus::Playing);
    client.at(0.1, &[b's', 0, 9, 1]).unwrap();
    check(&client, ClientStatus::Dead);
    client.state.disconnect();
    check(&client, ClientStatus::Disconnected);
    client.time += 1.;
    client.state.new_connection(client.time).unwrap();
    check(&client, ClientStatus::Waiting);
}

#[test]
fn lives_status_and_reconnects_reset_known_state() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&straight(4, 300.));
    client.send(&[b'F', 72, 72, 0, 10, 10, 25]);
    let first = client.frame();
    assert_eq!(
        (
            first.status,
            first.life,
            first.received.connection_generation
        ),
        (ClientStatus::Playing, 1, 1)
    );
    assert_eq!(first.received.snakes.len(), 2);
    client.send(&[b's', 0, 9, 1]);
    let dead = client.frame();
    assert_eq!(dead.status, ClientStatus::Dead);
    assert!(dead.observation.is_none());
    assert_eq!(dead.received.snakes.len(), 1);
    // Implicit own packets after death are ignored, not applied to an opponent.
    client.send(&[b'e', 64]);

    client.time += 1.;
    client.state.new_connection(client.time).unwrap();
    client.server = 0.;
    let fresh = client.frame();
    assert_eq!(
        (
            fresh.status,
            fresh.life,
            fresh.received.connection_generation
        ),
        (ClientStatus::Waiting, 1, 2)
    );
    assert!(
        fresh.received.snakes.is_empty() && fresh.settings.is_none() && fresh.minimap.is_none()
    );
    assert!(!fresh.received.food.valid);
    client.at(0.1, &setup()).unwrap();
    client.send(&straight(9, 0.));
    let again = client.frame();
    assert_eq!((again.status, again.life), (ClientStatus::Playing, 2));
    assert!(again.observation.as_ref().unwrap().food.is_empty());
    // The same wire id in a new life is a new incarnation.
    assert_eq!(again.received.snakes[0].incarnation, 2);
    client.send(&[b'v', 0]);
    assert_eq!(client.frame().status, ClientStatus::Dead);
    client.state.disconnect();
    assert_eq!(client.frame().status, ClientStatus::Disconnected);
    assert!(client.state.receive_packet(b"p", 1., client.time).is_err());
    assert!(!client.state.ping_due(client.time + 1.).unwrap());
}

#[test]
fn heads_extrapolate_between_server_clocks_and_polling_is_pure() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    let speed = 5. / 0.032;
    let frame = client.frame();
    let me = &frame.observation.as_ref().unwrap().me;
    assert_eq!(me.head, pt(50., 0.));
    assert_eq!(me.speed, speed);
    assert_eq!(me.body, vec![pt(0., 0.), pt(-42., 0.), pt(-84., 0.)]);
    assert!(!frame.received.snakes[0].position.predicted);
    // Own heading changes to pi/2 after 0.1 s; another packet arrives 0.1 s later.
    client.at(0.1, &[b'e', 64]).unwrap();
    client.at(0.1, b"p").unwrap();
    let frame = client.frame();
    let head = frame.observation.as_ref().unwrap().me.head;
    assert!(close(head.x, 50. + speed * 0.1, 1e-9) && close(head.y, speed * 0.1, 1e-9));
    assert_eq!(frame.observation.as_ref().unwrap().time, client.server);
    let row = &frame.received.snakes[0];
    assert!(row.position.predicted && row.position.valid);
    assert!(close(row.position.age_seconds, 0.2, 1e-12));
    assert!(close(row.heading.age_seconds, 0.1, 1e-12));
    // Polling reads only: the same instant reproduces the frame; later only ages grow.
    assert_eq!(client.state.frame(client.time).unwrap(), frame);
    let later = client.state.frame(client.time + 1.).unwrap();
    assert_eq!(later.observation, frame.observation);
    assert!(close(
        later.received.snakes[0].position.age_seconds,
        1.2,
        1e-12
    ));
    assert!(close(later.time - frame.time, 1., 1e-12));
    // A new anchor is an exact position again.
    client.at(0.05, b"G").unwrap();
    let frame = client.frame();
    assert!(!frame.received.snakes[0].position.predicted);
    assert_eq!(frame.observation.unwrap().me.head, pt(42., 0.));
    frame.received.validate().unwrap();
}

#[test]
fn anchors_follow_iang_absolute_growth_and_tail_reports() {
    // Zero contraction makes anchor positions exact.
    let mut client = Client::with_setup(&setup_with(1, 0, 430));
    client.send(&straight(9, 0.));
    client.send(&straight(4, 1000.));
    // Implicit own anchor with a new direction (pi/2).
    client.send(&[b'G', 0x40, 0]);
    let me = client.me();
    assert_eq!(me.body.len(), 3);
    assert!(close(me.body[0].x, 0., 1e-9) && close(me.body[0].y, 42., 1e-9));
    assert_eq!(me.body[1], pt(0., 0.));
    // Growing own anchor reuses that direction.
    client.send(&[b'N', 0x80, 0, 0]);
    let me = client.me();
    assert_eq!((me.segments, me.body.len()), (4, 4));
    assert!(close(me.fam, 0.5, 1e-7));
    assert!(close(me.body[0].y, 84., 1e-9));
    // Absolute own anchor in world coordinates.
    client.send(&[b'=', 0x00, 0x00, 0x54, 0x60, 0x54, 0x60]);
    assert_eq!(client.me().body[0], pt(0., 0.));
    // Opponent forms: absolute growth and directed anchors.
    let x = (GRID as u16 + 1000).to_be_bytes();
    client.send(&[b'+', 0, 4, 0, 0, x[0], x[1], 0x54, 0x60, 0x40, 0, 0]);
    let other = client.opponent("4").unwrap();
    assert_eq!((other.segments, other.body[0]), (4, pt(1000., 0.)));
    assert!(close(other.fam, 0.25, 1e-7));
    client.send(&[b'g', 0, 4, 0, 0]);
    assert_eq!(client.opponent("4").unwrap().body[0], pt(1042., 0.));
    client.send(&[b'n', 0, 4, 0xc0, 0, 0x80, 0, 0]);
    let other = client.opponent("4").unwrap();
    assert_eq!(other.segments, 5);
    assert!(close(other.body[0].x, 1042., 1e-9) && close(other.body[0].y, -42., 1e-9));
    // Tail removal and mass reports.
    client.send(&[b'r', 0, 4, 0x20, 0, 0]);
    let other = client.opponent("4").unwrap();
    assert_eq!((other.segments, other.body.len()), (4, 4));
    assert!(close(other.fam, 0.125, 1e-7));
    client.send(&[b'h', 0, 4, 0, 0, 0]);
    assert_eq!(client.opponent("4").unwrap().fam, 0.);
    // Updates for unknown snakes are ignored.
    client.send(&[b'g', 0, 77]);
    client.send(&[b'h', 0, 77, 0, 0, 0]);
    client.frame().received.validate().unwrap();
}

#[test]
fn setup_contraction_is_applied_before_insertion() {
    let body: Vec<Point> = (0..8).map(|k| pt(-42. * f64::from(k), 0.)).collect();
    let wire = appearance(
        9,
        0,
        5000,
        pt(10., 0.),
        &body.iter().rev().take(7).copied().collect::<Vec<_>>(),
        Some(0),
    );
    for cst in [430u16, 500] {
        let mut client = Client::with_setup(&setup_with(1, cst, 430));
        client.send(&wire);
        let before = client.me().body;
        assert_eq!(before.len(), 8);
        client.send(&[b'G', 0x40, 0]);
        let mut expected = before.clone();
        let c = f64::from(cst) / 1000.;
        for i in 3..expected.len() {
            let weight = c * ((i - 2) as f64 / 4.).min(1.);
            expected[i].x += (expected[i - 1].x - expected[i].x) * weight;
            expected[i].y += (expected[i - 1].y - expected[i].y) * weight;
        }
        expected.insert(0, pt(before[0].x, before[0].y + 42.));
        expected.pop();
        let after = client.me().body;
        for (a, b) in after.iter().zip(&expected) {
            assert!(
                close(a.x, b.x, 1e-9) && close(a.y, b.y, 1e-9),
                "{cst}: {a:?} {b:?}"
            );
        }
    }
}

#[test]
fn an_anchor_without_known_direction_leaves_the_body_unknown() {
    let mut client = Client::new();
    client.send(&appearance(9, 0, 5000, pt(0., 0.), &[pt(-42., 0.)], None));
    assert_eq!(client.me().body, vec![pt(-42., 0.)]);
    client.send(b"G");
    let frame = client.frame();
    assert!(frame.observation.unwrap().me.body.is_empty());
    assert!(!frame.received.snakes[0].body.valid);
    frame.received.validate().unwrap();
    // A later appearance restores a known body.
    client.send(&straight(9, 0.));
    assert!(client.frame().received.snakes[0].body.valid);
}

#[test]
fn motion_packet_forms_report_only_their_fields() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&straight(4, 300.));
    let speed = |byte: f64| byte / 18. / 0.032;
    client.send(&[b'e', 0, 4, 32, 64, 90]);
    let other = client.opponent("4").unwrap();
    assert_eq!((other.heading, other.speed), (TAU / 8., speed(90.)));
    client.send(&[b'e', 0, 4, 16, 72]);
    let other = client.opponent("4").unwrap();
    assert_eq!((other.heading, other.speed), (TAU / 16., speed(72.)));
    client.send(&[b'3', 0, 4, 54]);
    assert_eq!(client.opponent("4").unwrap().speed, speed(54.));
    client.send(&[b'5', 0, 4, 128, 0]);
    assert_eq!(client.opponent("4").unwrap().heading, TAU / 2.);
    client.send(&[b'4', 0, 4, 0]);
    let wanted_age = client
        .frame()
        .received
        .snake("4")
        .unwrap()
        .wanted_heading
        .age_seconds;
    assert_eq!(wanted_age, 0.);
    // Own implicit forms, including d/7 with all three fields.
    client.send(&[b'd', 192, 0, 36]);
    let me = client.me();
    assert_eq!((me.heading, me.speed), (3. * TAU / 4., speed(36.)));
    client.send(&[b'7', 64, 0, 18]);
    assert_eq!(client.me().speed, speed(18.));
    client.send(&[b'3', 90]);
    assert_eq!(client.me().speed, speed(90.));
    client.send(&[b'E', 10, 20]);
    assert_eq!(client.me().speed, speed(20.));
    let row = client.frame().received.snake("9").cloned().unwrap();
    assert!(!row.wanted_history.is_empty());
}

#[test]
fn food_sectors_pellets_eating_and_clears_are_centered() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    // Sector (72, 72) starts at the grid center: 72 * 300 = 21600.
    client.send(&[b'F', 72, 72, 0, 0, 0, 25, 0, 128, 64, 50]);
    let food = client.frame().observation.unwrap().food;
    assert_eq!(food.len(), 2);
    assert_eq!((food[0].x, food[0].y, food[0].size), (0., 0., Some(5.)));
    assert_eq!((food[1].x, food[1].y, food[1].size), (150., 75., Some(10.)));
    assert_eq!(food[1].value, physics::nutrition(10., 2.));
    assert!(food.iter().all(|f| f.origin.is_none()));
    // Long boost pellet with color, then a short natural pellet in the same sector.
    client.send(&[b'b', 73, 72, 0, 0, 7, 20]);
    client.send(&[b'f', 1, 0, 30]);
    let food = client.frame().observation.unwrap().food;
    assert_eq!(food.len(), 4);
    assert!(
        food.iter()
            .any(|f| (f.x, f.y, f.size) == (300., 0., Some(4.)))
    );
    assert!(
        food.iter()
            .any(|f| f.size == Some(6.) && close(f.x, 301.171875, 1e-12))
    );
    // Long eat, short eat by last eater, eat with eater id.
    client.send(&[b'c', 72, 72, 0, 0]);
    client.send(&[b'C', 128, 64]);
    client.send(&[b'<', 73, 72, 0, 0, 0, 9]);
    assert_eq!(client.frame().observation.unwrap().food.len(), 1);
    client.send(&[b'w', 73, 72]);
    let frame = client.frame();
    assert!(frame.observation.unwrap().food.is_empty());
    assert!(frame.received.food.valid && !frame.received.food.complete);
}

#[test]
fn warmed_food_frames_match_fresh_replay_after_every_edit() {
    let packets = [
        straight(9, 0.),
        vec![b'F', 72, 72, 0, 0, 0, 25, 0, 128, 64, 50],
        vec![b'b', 73, 72, 0, 0, 7, 20],
        vec![b'f', 1, 0, 30],
        // Replacement at an existing key changes value without changing length.
        vec![b'b', 73, 72, 0, 0, 7, 60],
        vec![b'c', 72, 72, 0, 0],
        vec![b'C', 128, 64],
        vec![b'<', 73, 72, 0, 0, 0, 9],
        vec![b'w', 73, 72],
        vec![b'w', 73, 72],
        vec![b'F', 72, 72, 0, 0, 0, 25],
    ];
    let mut warm = Client::new();
    for (index, packet) in packets.iter().enumerate() {
        warm.send(packet);
        let mut cold = Client::new();
        for packet in &packets[..=index] {
            cold.send(packet);
        }
        // Only the warm decoder has been polled between edits. Compare the full
        // frame, not just food, and keep an old owned frame across the next edit.
        let expected = cold.frame();
        let actual = warm.frame();
        assert_eq!(actual, expected, "packet {index}");
        assert_eq!(warm.frame(), expected);
        assert_eq!(warm.state.clone().frame(warm.time).unwrap(), expected);
        let mut held = actual;
        held.observation.as_mut().unwrap().food.clear();
        assert_eq!(warm.frame(), expected);
        // Failed bundled edits must leave the warmed snapshot and clocks intact.
        assert!(
            warm.state
                .receive_frame(&bundled(&[packet, b"w"]), warm.time)
                .is_err()
        );
        assert_eq!(warm.frame(), expected);
    }
    warm.state.new_connection(warm.time).unwrap();
    warm.send(&setup());
    warm.send(&straight(9, 0.));
    assert!(warm.me().body.len() > 1);
    assert!(warm.frame().observation.unwrap().food.is_empty());
}

#[test]
fn short_pellets_before_any_sector_are_not_invented() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&[b'f', 1, 0, 30]);
    client.send(&[b'c', 1, 0]);
    let frame = client.frame();
    assert!(frame.observation.unwrap().food.is_empty());
    assert!(frame.received.food.valid);
}

#[test]
fn prey_appear_move_and_disappear() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    let mut y = vec![b'y', 0, 3, 5];
    y.extend(coordinate(30.));
    y.extend(coordinate(-20.));
    y.extend([40, 48]);
    y.extend(u24(4_194_304)); // wanted
    y.extend(u24(8_388_608)); // heading
    y.extend(4000u16.to_be_bytes());
    client.send(&y);
    let prey = client.frame().observation.unwrap().prey;
    assert_eq!(prey.len(), 1);
    assert_eq!(prey[0].id, 3);
    assert!(close(prey[0].position.x, 30., 1e-9) && close(prey[0].position.y, -20., 1e-9));
    assert_eq!(prey[0].size, 8.);
    assert!(close(
        prey[0].heading,
        TAU * 8_388_608. / 16_777_215.,
        1e-12
    ));
    assert_eq!(prey[0].speed, 4. / 0.032);
    // Position plus speed: x = 1 + 7200 * 3 = 21601.
    client.send(&[b'j', 0, 3, 0x1c, 0x20, 0x1c, 0x20, 0x0b, 0xb8]);
    let prey = &client.frame().observation.unwrap().prey[0];
    assert_eq!((prey.position, prey.speed), (pt(1., 1.), 3. / 0.032));
    client.send(&[
        b'j', 0, 3, 0x1c, 0x20, 0x1c, 0x20, 48, 0, 0, 0, 0x80, 0, 0, 0x03, 0xe8,
    ]);
    let prey = &client.frame().observation.unwrap().prey[0];
    assert_eq!((prey.heading, prey.speed), (0., 1. / 0.032));
    // Unknown prey moves are ignored; eaten prey disappears.
    client.send(&[b'j', 0, 8, 0, 0, 0, 0]);
    client.send(&[b'y', 0, 3, 0, 9]);
    let frame = client.frame();
    assert!(frame.observation.unwrap().prey.is_empty());
    assert!(frame.received.prey.valid);
}

#[test]
fn minimap_snapshots_and_deltas_are_transactional() {
    let mut client = Client::new();
    let before = client.frame();
    assert!(client.at(0.01, &[b'V', 0x40]).is_err());
    assert_eq!(client.state.frame(before.time).unwrap(), before);
    client.send(&[b'M', 0, 2, 0x50]);
    let map = client.frame().minimap.unwrap();
    assert_eq!((map.side, map.cells.clone()), (2, vec![0, 1, 0, 1]));
    client.at(0.5, &[b'V', 0x60]).unwrap();
    let frame = client.state.frame(client.time + 0.25).unwrap();
    let map = frame.minimap.unwrap();
    assert_eq!(map.cells, vec![0, 1, 1, 0]);
    assert!(close(map.age_seconds, 0.25, 1e-12));
    assert!(frame.received.minimap.valid);
    assert_eq!(frame.received.minimap.age_seconds, map.age_seconds);
}

#[test]
fn minimap_frame_budget_covers_every_family_and_resets_per_frame() {
    assert_eq!(MAX_FRAME_MINIMAP_CELLS, 5 * 512 * 512);
    let fill64 = [b'M', 0, 64];
    let fill32 = [b'U', 0, 32];
    let extra_cell = [b'L', 0, 0, 1];
    for (packet, cells) in [
        (&[b'M', 2, 0][..], 512 * 512),
        // U clamps 600 to 512 before charging.
        (&[b'U', 2, 88][..], 512 * 512),
        (&b"V"[..], 512 * 512),
        (&b"u"[..], 80 * 80),
        (&[b'L', 0, 2, 0][..], 512 * 512),
        (&[b'L', 4, 2, 0][..], 5 * 512 * 512),
    ] {
        let mut client = Client::new();
        client.send(&straight(9, 0.));
        client.send(&[b'M', 2, 0, 0x40]);
        let mut packets = vec![packet; MAX_FRAME_MINIMAP_CELLS / cells];
        if packet[0] == b'u' {
            // 204 legacy maps leave 5,120 cells: 64² + 32² fills the budget.
            assert_eq!(MAX_FRAME_MINIMAP_CELLS % cells, 64 * 64 + 32 * 32);
            packets.extend([&fill64[..], &fill32[..]]);
        } else {
            assert_eq!(MAX_FRAME_MINIMAP_CELLS % cells, 0);
        }
        let frame = bundled(&packets);
        for _ in 0..2 {
            client.state.receive_frame(&frame, client.time).unwrap();
        }
        let before = client.frame();
        packets.insert(0, &[b'h', 0, 9, 0x40, 0, 0]);
        packets.push(&extra_cell);
        let error = client
            .state
            .receive_frame(&bundled(&packets), client.time + 0.1)
            .unwrap_err();
        assert!(error.contains("cell work limit"), "{packet:?}: {error}");
        assert_eq!(client.frame(), before, "{packet:?}");
    }
}

#[test]
fn repeated_minimap_snapshots_reject_the_whole_frame() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&[b'L', 2, 0, 2, 0x40, 0x20]);
    let before = client.frame();
    let snapshots = vec![&[b'M', 2, 0][..]; 8];
    let error = client
        .state
        .receive_frame(&bundled(&snapshots), client.time + 0.1)
        .unwrap_err();
    assert!(error.contains("cell work limit"), "{error}");
    assert_eq!(client.frame(), before);
    // A failed admission does not consume the next frame's budget.
    client
        .state
        .receive_frame(&bundled(&snapshots[..5]), client.time)
        .unwrap();
    assert_eq!(client.frame().minimap.unwrap().side, 512);
}

#[test]
fn mixed_minimap_families_keep_packet_order_and_late_error_atomicity() {
    let packets: &[&[u8]] = &[
        &[b'M', 0, 2, 0x40],
        &[b'U', 0, 2, 0x20],
        &[b'V', 0x40],
        &[b'u', 0x40],
        &[b'V', 0x40],
        &[b'L', 2, 0, 2, 0x40, 0x20],
        &[b'V', 0x40],
        &[b'V', 0x40],
    ];
    let mut sequential = Client::new();
    let mut together = Client::new();
    for packet in packets {
        sequential.send(packet);
    }
    together
        .state
        .receive_frame(&bundled(packets), together.time)
        .unwrap();
    let before = together.frame();
    assert_eq!(before, sequential.frame());
    assert_eq!(before.minimap.as_ref().unwrap().cells, [0, 0, 1, 0]);
    let teams = before.hud.minimap_teams.as_ref().unwrap();
    assert_eq!(teams.layers, [vec![0, 0, 0, 1], vec![0, 0, 1, 0]]);
    let appearance = straight(9, 0.);
    let error = together
        .state
        .receive_frame(
            &bundled(&[&appearance, &[b'M', 0, 2, 0x40], &[b'V', 255]]),
            together.time + 0.1,
        )
        .unwrap_err();
    assert!(error.contains("Truncated minimap run"), "{error}");
    assert_eq!(together.frame(), before);
}

#[test]
fn scores_use_transmitted_mass_setup_mscps_and_own_credit_only() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&straight(4, 300.));
    let me = client.me();
    assert_eq!(me.score, physics::score(3, me.fam));
    assert_eq!(me.kills, 0);
    client.send(&[b'R', 5]);
    assert_eq!(client.me().score, physics::score(8, me.fam));
    // A later mass report keeps the own credit.
    client.send(&[b'h', 0, 9, 0x40, 0, 0]);
    let me = client.me();
    assert!(close(me.fam, 0.25, 1e-7));
    assert_eq!(me.score, physics::score(8, me.fam));
    let other = client.opponent("4").unwrap();
    assert_eq!(
        (other.score, other.kills),
        (physics::score(3, other.fam), 0)
    );

    let mut small = Client::with_setup(&setup_with(1, 430, 100));
    small.send(&straight(9, 0.));
    let table = |segments: usize, fam: f64| {
        let weight = |i: usize| (1. - i.min(99) as f64 / 100.).powf(2.25);
        let sum: f64 = (0..segments.min(100)).map(|i| 1. / weight(i)).sum();
        ((sum + fam / weight(segments.min(100)) - 1.) * 15. - 5.).floor()
    };
    let me = small.me();
    assert_eq!(me.score, table(3, me.fam));
    assert_ne!(me.score, physics::score(3, me.fam));
}

#[test]
fn pings_track_pending_measured_and_recent_range() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    assert!(!state.ping_due(0.).unwrap());
    state.receive_frame(&[0, 0, b'6', 1, 2], 0.).unwrap();
    assert!(!state.ping_due(0.).unwrap());
    let mut frame = vec![0, 0, 0, 30];
    frame.extend(setup());
    state.receive_frame(&frame, 0.1).unwrap();
    assert!(state.ping_due(0.1).unwrap());
    assert!(state.frame(0.1).unwrap().timing.ping_seconds.is_none());
    state.record_sent(&[251], 0.2).unwrap();
    assert!(!state.ping_due(1.).unwrap());
    let timing = state.frame(0.25).unwrap().timing;
    assert!(close(timing.ping_wait_seconds.unwrap(), 0.05, 1e-12));
    state.receive_frame(&[0, 40, b'p'], 0.26).unwrap();
    let timing = state.frame(0.3).unwrap().timing;
    assert!(close(timing.ping_seconds.unwrap(), 0.06, 1e-12));
    assert_eq!(timing.ping_wait_seconds, None);
    assert!(!state.ping_due(0.44).unwrap());
    assert!(state.ping_due(0.46).unwrap());
    state.record_sent(&[251], 3.).unwrap();
    state.receive_frame(&[0, 40, b'p'], 3.1).unwrap();
    let range = state.frame(3.1).unwrap().timing.ping_range_seconds.unwrap();
    assert!(close(range[0], 0.06, 1e-12) && close(range[1], 0.1, 1e-12));
    let range = state.frame(5.5).unwrap().timing.ping_range_seconds.unwrap();
    assert!(close(range[0], 0.1, 1e-12) && close(range[1], 0.1, 1e-12));
    // A slow pong is still a measurement; an unmatched one is not.
    state.record_sent(&[251], 6.).unwrap();
    state.receive_frame(&[0, 40, b'p'], 8.5).unwrap();
    assert!(close(
        state.frame(8.5).unwrap().timing.ping_seconds.unwrap(),
        2.5,
        1e-12
    ));
    state.receive_frame(&[0, 40, b'p'], 8.6).unwrap();
    assert!(close(
        state.frame(8.6).unwrap().timing.ping_seconds.unwrap(),
        2.5,
        1e-12
    ));
    state.set_processing_seconds(0.004).unwrap();
    assert_eq!(state.frame(8.6).unwrap().timing.processing_seconds, 0.004);
    assert!(state.set_processing_seconds(-1.).is_err());
}

#[test]
fn raw_challenge_selects_unframed_messages() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state.receive_frame(RAW_CHALLENGE, 0.).unwrap();
    state.receive_frame(&setup(), 1.).unwrap();
    state.receive_frame(&straight(9, 0.), 1.).unwrap();
    state.receive_frame(&[b'e', 64], 1.5).unwrap();
    let observation = state.frame(1.5).unwrap().observation.unwrap();
    // Without a server header, time is local receipt time since setup.
    assert_eq!(observation.time, 0.5);
    assert_eq!(observation.me.heading, TAU / 4.);
}

#[test]
fn timing_header_high_byte_six_does_not_select_bare_framing() {
    for low in [0, b'a', 255] {
        let mut state = ClientState::new();
        state.new_connection(0.).unwrap();
        let mut frame = vec![b'6', low];
        frame.extend(setup());
        state.receive_frame(&frame, 0.).unwrap();
        let mut frame = vec![0, 7];
        frame.extend(straight(9, 0.));
        state.receive_frame(&frame, 0.1).unwrap();
        let frame = state.frame(0.1).unwrap();
        assert_eq!(frame.status, ClientStatus::Playing);
        assert_eq!(frame.observation.unwrap().time, 0.007);
    }
}

#[test]
fn header_free_messages_accept_bundles_and_reject_them_atomically() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state.receive_frame(RAW_CHALLENGE, 0.).unwrap();
    let snake = straight(9, 0.);
    let mut bundle = vec![0, setup().len() as u8];
    bundle.extend(setup());
    bundle.push(32 + snake.len() as u8);
    bundle.extend(snake);
    state.receive_frame(&bundle, 1.).unwrap();
    assert_eq!(state.frame(1.).unwrap().status, ClientStatus::Playing);
    let before = state.frame(1.).unwrap();
    assert!(
        state
            .receive_frame(&[0, 2, b'e', 64, 0, 3, b'e'], 1.5)
            .is_err()
    );
    assert_eq!(state.frame(1.).unwrap(), before);
    state
        .receive_frame(&[0, 2, b'e', 64, 33, b'p'], 1.5)
        .unwrap();
    let observation = state.frame(1.5).unwrap().observation.unwrap();
    assert_eq!(observation.me.heading, TAU / 4.);
    assert_eq!(observation.time, 0.5);
}

#[test]
fn header_clock_starts_at_setup_and_accumulates_elapsed_ms() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state.receive_frame(&[0, 0, b'6', 1], 0.).unwrap();
    let mut frame = vec![0xff, 0xff, 0, 30];
    frame.extend(setup());
    let s = straight(9, 0.);
    frame.extend([0, s.len() as u8]);
    frame.extend(&s);
    state.receive_frame(&frame, 0.1).unwrap();
    state.receive_frame(&[0, 7, b'e', 64], 0.2).unwrap();
    // A leading byte below 32 selects length-prefixed packets.
    state
        .receive_frame(&[0, 9, 0, 2, b'e', 0, 32 + 1, b'p'], 0.3)
        .unwrap();
    assert_eq!(state.frame(0.3).unwrap().observation.unwrap().time, 0.016);
}

#[test]
fn malformed_input_and_invalid_clocks_leave_state_unchanged() {
    let mut client = Client::new();
    client.send(&straight(9, 0.));
    client.send(&straight(4, 300.));
    client.send(&[b'M', 0, 2, 0x50]);
    let before = client.frame();
    let bad: &[&[u8]] = &[
        &[],
        &[b's', 0],
        &[b's', 0, 4, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        &[b'g', 0, 4, 1],
        &[b'G', 1, 2, 3],
        &[b'n', 0, 4, 0, 0],
        &[b'=', 0, 4, 0, 0, 0, 0, 0],
        &[b'e', 0, 4, 1, 2, 3, 4],
        &[b'5', 0, 4, 1],
        &[b'd', 1],
        &[b'h', 0, 4, 1],
        &[b'r', 0, 4, 1],
        b"R",
        &[b'F', 1, 2, 3],
        &[b'b', 1, 2],
        &[b'c', 1, 2, 3],
        &[b'<', 1, 2, 3],
        &[b'w', 1],
        &[b'y', 0, 1, 2],
        &[b'j', 0, 1, 2, 3, 4, 5, 6],
        &[b'z', 0, 0],
        &[b'M', 3, 0],
        &[b'V', 255],
    ];
    for packet in bad {
        assert!(client.at(0.01, packet).is_err(), "{packet:?}");
        client.server -= 0.01;
        client.time -= 0.01;
        assert_eq!(client.frame(), before, "{packet:?}");
    }
    // A valid packet before a malformed one in the same frame is not applied.
    let frame = [0, 5, 0, 2, b'e', 64, 0, 2, b'h', 0];
    assert!(client.state.receive_frame(&frame, client.time).is_err());
    assert_eq!(client.frame(), before);
    // Invalid clocks.
    for (server, receipt) in [
        (f64::NAN, client.time),
        (client.server, f64::NAN),
        (client.server - 1., client.time),
        (client.server, client.time - 1.),
        (-1., client.time),
    ] {
        assert!(client.state.receive_packet(b"p", server, receipt).is_err());
    }
    assert!(client.state.frame(client.time - 0.5).is_err());
    assert!(client.state.frame(f64::INFINITY).is_err());
    assert!(client.state.record_sent(&[1], client.time - 1.).is_err());
    assert_eq!(client.frame(), before);
}

#[test]
fn setup_order_and_protocol_are_enforced() {
    let mut state = ClientState::new();
    assert!(state.receive_packet(&setup(), 0., 0.).is_err());
    assert_eq!(state.frame(0.).unwrap().status, ClientStatus::Disconnected);
    state.new_connection(0.).unwrap();
    let error = state.receive_packet(&straight(9, 0.), 0., 0.).unwrap_err();
    assert!(error.contains("before setup"), "{error}");
    let mut old = setup();
    old[23] = 11;
    assert!(
        state
            .receive_packet(&old, 0., 0.)
            .unwrap_err()
            .contains("Unsupported protocol")
    );
    assert!(state.receive_packet(&setup()[..24], 0., 0.).is_err());
    assert!(state.receive_packet(&setup()[..26], 0., 0.).is_err());
    state.receive_packet(&setup()[..25], 0., 0.).unwrap();
    assert!(
        state
            .receive_packet(&setup(), 0., 0.)
            .unwrap_err()
            .contains("Repeated setup")
    );
    // Without the optional radius the stock client uses 98% of the grid center.
    state.receive_packet(&straight(9, 0.), 0., 0.).unwrap();
    let frame = state.frame(0.).unwrap();
    assert_eq!(frame.observation.unwrap().arena.radius, GRID * 0.98);
    let settings = frame.settings.unwrap();
    assert_eq!(
        (
            settings.protocol,
            settings.grid_center,
            settings.sector_size
        ),
        (19, 21600, 300)
    );
    assert_eq!(
        (
            settings.max_segments,
            settings.body_spacing,
            settings.body_contraction
        ),
        (430, 42., 0.43)
    );
    state
        .receive_packet(&[b'z', 0, 0x1f, 0x40], 1., 1.)
        .unwrap();
    assert_eq!(
        state.frame(1.).unwrap().observation.unwrap().arena.radius,
        8000.
    );
}

#[test]
fn captured_legacy_login_setup_remains_protocol_13_and_is_rejected() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-legacy-login.json")).unwrap();
    let prefix = hex(fixture["joinedSession"]["setup"]["commandPrefixHex"]
        .as_str()
        .unwrap());
    assert_eq!(fixture["wireProtocol"], 13);
    assert_eq!(prefix.len(), 25);
    assert_eq!(prefix[23], 13);
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    let before = state.frame(0.).unwrap();
    assert_eq!(
        state.receive_packet(&prefix, 0., 0.).unwrap_err(),
        "Unsupported protocol version 13; expected 19"
    );
    assert_eq!(state.frame(0.).unwrap(), before);
}

#[test]
fn observed_wsl_setup_prefix_decodes_server_number_separately() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-protocol-paired-coil.json")).unwrap();
    let wsl = &fixture["wslSetup"];
    let bytes = hex(wsl["setupPrefixHex"].as_str().unwrap());
    let expected = &wsl["expected"];
    assert_eq!(
        u16::from_be_bytes([bytes[25], bytes[26]]),
        expected["serverNumber"].as_u64().unwrap() as u16
    );
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state.receive_packet(&bytes, 0., 0.).unwrap();
    let frame = state.frame(0.).unwrap();
    assert_eq!(frame.status, ClientStatus::Waiting);
    assert!(frame.observation.is_none());
    let settings = frame.settings.unwrap();
    assert_eq!(
        u64::from(settings.grid_center),
        expected["grd"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(settings.max_segments),
        expected["mscps"].as_u64().unwrap()
    );
    assert_eq!(
        u64::from(settings.sector_size),
        expected["sectorSize"].as_u64().unwrap()
    );
    assert!(close(
        settings.body_contraction * 1000.,
        expected["cstWire"].as_f64().unwrap(),
        1e-9
    ));
    assert_eq!(settings.body_spacing, expected["msl"].as_f64().unwrap());
    assert_eq!(
        settings.server_number.map(u64::from),
        expected["serverNumber"].as_u64()
    );
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn scheduler_witness(kind: &str) -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/scheduler-near-server-witnesses.json"
    ))
    .unwrap();
    assert_eq!(fixture["wireProtocol"], 19);
    assert_eq!(fixture["clientContract"], "slither-client-v2");
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["kind"] == kind)
        .unwrap()
        .clone()
}

fn witness_client(case: &Value) -> Client {
    let mut client = Client::with_setup(&hex(case["setupHex"].as_str().unwrap()));
    if case["kind"] == "prey" {
        // Synthetic own-snake context makes the captured prey observable.
        client.send(&straight(9, 0.));
    }
    client.send(&hex(case["appearanceHex"].as_str().unwrap()));
    client
}

fn witness_pair(client: &mut Client, case: &Value) -> ClientFrame {
    let first = hex(case["first"]["packetHex"].as_str().unwrap());
    let last = hex(case["last"]["packetHex"].as_str().unwrap());
    // Selected logical packets, reframed for this test: unrelated intervening
    // traffic is omitted, preserving the recorded cumulative clock difference.
    client
        .state
        .receive_frame(&bundled(&[&first]), client.time)
        .unwrap();
    let before = client.frame();
    let gap = case["serverGapMs"].as_u64().unwrap();
    let mut frame = bundled(&[&last]);
    frame[..2].copy_from_slice(&(gap as u16).to_be_bytes());
    client.time += case["receiptGapMs"].as_f64().unwrap() / 1000.;
    client.state.receive_frame(&frame, client.time).unwrap();
    before
}

#[test]
fn clean_server_motion_and_mass_exceptions_update_receipt_state() {
    // #111's clean server-clock minima contradict universal 42/105 ms gates.
    // These are decoder contracts, not changes to the simulator release model.
    for (kind, gap) in [("motion", 24), ("mass", 84)] {
        let case = scheduler_witness(kind);
        assert_eq!(case["serverGapMs"], gap);
        let mut client = witness_client(&case);
        let before = witness_pair(&mut client, &case);
        let after = client.frame();
        let id = case["entity"].as_u64().unwrap().to_string();
        let receipt = after.received.snake(&id).unwrap();
        if kind == "motion" {
            assert_ne!(before.observation.unwrap().me.heading, client.me().heading);
            assert!(close(client.me().heading, 242. * TAU / 256., 1e-12));
            assert_eq!(receipt.heading.age_seconds, 0.);
        } else {
            assert_ne!(before.observation.unwrap().me.fam, client.me().fam);
            assert_eq!(client.me().fam, 0xba309f as f64 / 16_777_215.);
            assert_eq!(receipt.mass.age_seconds, 0.);
        }
        after.received.validate().unwrap();
    }
}

#[test]
fn clean_server_same_frame_motion_before_mass_is_accepted() {
    let case = scheduler_witness("reverse");
    let mut client = witness_client(&case);
    let packets: Vec<_> = case["packetHex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|packet| hex(packet.as_str().unwrap()))
        .collect();
    assert_eq!(packets[0][0], b'3');
    assert_eq!(packets[1][0], b'h');
    let frame = bundled(&[&packets[0], &packets[1]]);
    assert_eq!(
        split_frame(&frame).unwrap(),
        vec![packets[0].as_slice(), packets[1].as_slice()]
    );
    client.state.receive_frame(&frame, client.time).unwrap();
    let me = client.me();
    assert_eq!(me.heading, 7. * TAU / 256.);
    assert_eq!(me.fam, 0xaccc18 as f64 / 16_777_215.);
    let received = client.frame().received;
    let row = received.snake("221").unwrap();
    assert_eq!(row.heading.age_seconds, 0.);
    assert_eq!(row.mass.age_seconds, 0.);
    received.validate().unwrap();
}

#[test]
fn clean_server_long_prey_silence_does_not_expire_or_reject_correction() {
    let case = scheduler_witness("prey");
    assert_eq!(case["serverGapMs"], 3044);
    assert_eq!(case["interveningFrames"], 374);
    let mut client = witness_client(&case);
    let before = witness_pair(&mut client, &case);
    let before = before.observation.unwrap();
    let first = &before.prey[0];
    let after = client.frame();
    let last = &after.observation.as_ref().unwrap().prey[0];
    assert_eq!(last.id, 27656);
    assert_eq!(last.position.x - first.position.x, -861.);
    assert_eq!(last.position.y - first.position.y, -432.);
    assert_eq!(after.received.prey.age_seconds, 0.);
    // A later silence retains the last received pose and reports its age.
    let later = client.state.frame(client.time + 3.044).unwrap();
    assert_eq!(later.observation.unwrap().prey[0].position, last.position);
    assert!(close(later.received.prey.age_seconds, 3.044, 1e-12));
}

fn point(value: &Value) -> Point {
    pt(value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

fn check_snake(snake: &Snake, expected: &Value, own: bool) {
    let id = &snake.id;
    let f = |key: &str| expected[key].as_f64().unwrap();
    assert_eq!(
        snake.segments as u64,
        expected["segments"].as_u64().unwrap(),
        "{id}"
    );
    assert!(close(snake.fam, f("fam"), 1e-9), "{id}");
    assert!(close(snake.heading, f("heading"), 1e-9), "{id}");
    assert!(close(snake.speed, f("speed"), 1e-6), "{id}");
    let head = point(&expected["head"]);
    assert!(
        close(snake.head.x, head.x, 1e-6) && close(snake.head.y, head.y, 1e-6),
        "{id} head {:?} {head:?}",
        snake.head
    );
    assert_eq!(
        snake.body.len() as u64,
        expected["bodyLength"].as_u64().unwrap(),
        "{id}"
    );
    for (actual, key) in [
        (snake.body.first(), "bodyFirst"),
        (snake.body.last(), "bodyLast"),
    ] {
        let want = point(&expected[key]);
        let actual = actual.unwrap();
        assert!(
            close(actual.x, want.x, 1e-6) && close(actual.y, want.y, 1e-6),
            "{id} {key}"
        );
    }
    let sum = snake
        .body
        .iter()
        .fold(pt(0., 0.), |a, p| pt(a.x + p.x, a.y + p.y));
    let want = point(&expected["bodySum"]);
    assert!(
        close(sum.x, want.x, 1e-4) && close(sum.y, want.y, 1e-4),
        "{id} body sum"
    );
    assert_eq!(snake.kills, 0);
    // Opponent scores come from transmitted mass; the own score may add `R` credit.
    let mass_score = physics::score(snake.segments, snake.fam);
    if own {
        assert!(snake.score >= mass_score, "{id}");
    } else {
        assert_eq!(snake.score, mass_score, "{id}");
    }
}

#[test]
fn real_capture_replay_matches_reference_decoders() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-protocol-paired-coil.json")).unwrap();
    let records = fixture["records"].as_array().unwrap();
    let mut checks = fixture["checks"].as_array().unwrap().iter().peekable();
    let identity = &fixture["identity"];
    let mut state = ClientState::new();
    let mut checked = 0;
    for (index, record) in records.iter().enumerate() {
        let seconds = record[1].as_f64().unwrap() / 1000.;
        match record[0].as_str().unwrap() {
            "c" => state.new_connection(seconds).unwrap(),
            "i" => {
                let bytes = hex(record[2].as_str().unwrap());
                for packet in split_frame(&bytes).unwrap() {
                    if packet[0] == b'a' {
                        let server_number = u16::from_be_bytes([packet[25], packet[26]]);
                        assert_eq!(
                            u64::from(server_number),
                            identity["setupServerNumber"].as_u64().unwrap()
                        );
                    }
                }
                state
                    .receive_frame(&bytes, seconds)
                    .unwrap_or_else(|e| panic!("record {index}: {e}"));
            }
            "o" => state
                .record_sent(&hex(record[2].as_str().unwrap()), seconds)
                .unwrap(),
            other => panic!("unknown record {other}"),
        }
        let Some(check) =
            checks.next_if(|check| check["afterRecord"].as_u64() == Some(index as u64))
        else {
            continue;
        };
        checked += 1;
        let frame = state.frame(seconds).unwrap();
        frame.received.validate().unwrap();
        assert_eq!(
            (
                frame.status,
                frame.life,
                frame.received.connection_generation
            ),
            (ClientStatus::Playing, 1, 1)
        );
        let observation = frame.observation.as_ref().unwrap();
        assert_eq!(
            observation.me.id,
            check["selfId"].as_u64().unwrap().to_string()
        );
        assert_eq!(
            observation.me.id,
            identity["firstFullAppearance"]
                .as_u64()
                .unwrap()
                .to_string()
        );
        assert!(close(
            observation.time,
            check["serverSeconds"].as_f64().unwrap(),
            1e-9
        ));
        assert_eq!(observation.arena.radius, check["radius"].as_f64().unwrap());
        let snakes: Vec<&Snake> = std::iter::once(&observation.me)
            .chain(&observation.opponents)
            .collect();
        let expected = check["snakes"].as_array().unwrap();
        assert_eq!(snakes.len(), expected.len());
        for want in expected {
            let id = want["id"].as_u64().unwrap().to_string();
            let snake = snakes
                .iter()
                .find(|s| s.id == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            check_snake(snake, want, *snake == &observation.me);
            assert!(frame.received.snake(&id).is_some());
        }
        let food = &check["food"];
        assert_eq!(
            observation.food.len() as u64,
            food["count"].as_u64().unwrap()
        );
        let sum = |f: fn(&Food) -> f64| observation.food.iter().map(f).sum::<f64>();
        assert!(close(sum(|f| f.x), food["sumX"].as_f64().unwrap(), 1e-6));
        assert!(close(sum(|f| f.y), food["sumY"].as_f64().unwrap(), 1e-6));
        assert!(close(
            sum(|f| f.size.unwrap()),
            food["sumSize"].as_f64().unwrap(),
            1e-6
        ));
        let prey = check["prey"].as_array().unwrap();
        assert_eq!(observation.prey.len(), prey.len());
        for (actual, want) in observation.prey.iter().zip(prey) {
            assert_eq!(actual.id, want["id"].as_u64().unwrap());
            let position = point(&want["position"]);
            assert!(
                close(actual.position.x, position.x, 1e-6)
                    && close(actual.position.y, position.y, 1e-6)
            );
            for (value, key) in [
                (actual.heading, "heading"),
                (actual.target_heading, "targetHeading"),
                (actual.speed, "speed"),
                (actual.size, "size"),
            ] {
                assert!(
                    close(value, want[key].as_f64().unwrap(), 1e-6),
                    "prey {key}"
                );
            }
        }
        let map = frame.minimap.as_ref().unwrap();
        let want = &check["minimap"];
        assert_eq!(map.side as u64, want["side"].as_u64().unwrap());
        let occupied: Vec<u64> = map
            .cells
            .iter()
            .enumerate()
            .filter(|(_, v)| **v == 1)
            .map(|(i, _)| i as u64)
            .collect();
        assert_eq!(occupied.len() as u64, want["occupied"].as_u64().unwrap());
        assert_eq!(
            occupied.iter().sum::<u64>(),
            want["indexSum"].as_u64().unwrap()
        );
        assert_eq!(
            occupied.iter().map(|i| i * i).sum::<u64>(),
            want["indexSquareSum"].as_u64().unwrap()
        );
        assert!(close(
            frame.timing.ping_seconds.unwrap(),
            check["pingSeconds"].as_f64().unwrap(),
            1e-9
        ));
        // This capture never sent a boost byte, so no full command is known.
        assert!(frame.timing.sent_commands.is_empty());
        assert!(
            frame.received.bodies.valid
                && frame.received.food.valid
                && frame.received.minimap.valid
        );
    }
    assert_eq!(checked, 3);
}
