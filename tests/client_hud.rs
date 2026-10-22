//! HUD reports and minimap families decoded with the stock client's handlers.
//!
//! The leaderboard fixture holds exact bytes from a retained live capture. Every
//! other packet here is synthetic, built to exercise one stock-parser rule.
use serde_json::Value;
use slither_sim::client::{
    ClientFrame, ClientHud, ClientState, MAX_TEAM_MINIMAP_CELLS, WireEvent, WireRecord,
    replay_record,
};
use std::f64::consts::TAU;

const GRID: u32 = 21600;

fn u24(value: u32) -> [u8; 3] {
    let [_, a, b, c] = value.to_be_bytes();
    [a, b, c]
}

/// Setup through `real_sid` and radius, plus optional `game_mode`/extra bytes.
fn setup(mscps: u16, tail: &[u8]) -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend(u24(GRID));
    p.extend(mscps.to_be_bytes());
    p.extend(300u16.to_be_bytes());
    p.extend(144u16.to_be_bytes());
    p.push(48);
    for v in [539u16, 40, 1400, 33, 29, 430] {
        p.extend(v.to_be_bytes());
    }
    p.extend([19, 42]);
    p.extend(7u16.to_be_bytes());
    p.extend(u24(21000));
    p.extend(tail);
    p
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Full appearance with a transmitted profile and a two-anchor body.
fn appearance(id: u16, skin: u8, name: &[u8], custom: &[u8], cosmetic: u8) -> Vec<u8> {
    let mut p = vec![b's'];
    p.extend(id.to_be_bytes());
    p.extend(u24(0));
    p.push(48);
    p.extend(u24(0));
    p.extend(5000u16.to_be_bytes());
    p.extend(u24(8_388_608));
    p.push(skin);
    let x = (GRID + 100 * u32::from(id)) * 5;
    p.extend(u24(x));
    p.extend(u24(GRID * 5));
    p.push(name.len() as u8);
    p.extend(name);
    p.push(custom.len() as u8);
    p.extend(custom);
    p.push(cosmetic);
    p.extend(u24(x - 300));
    p.extend(u24(GRID * 5));
    p.extend([127 + 84, 127]);
    p
}

fn kills(id: u16, count: u32) -> Vec<u8> {
    let mut p = vec![b'k'];
    p.extend(id.to_be_bytes());
    p.extend(u24(count));
    p
}

fn record(segments: u32, fam: u32, name: &[u8], message: &[u8]) -> Vec<u8> {
    let mut p = vec![b'm'];
    p.extend(u24(segments));
    p.extend(u24(fam));
    p.push(name.len() as u8);
    p.extend(name);
    p.extend(message);
    p
}

fn teams(pairs: &[[i32; 2]]) -> Vec<u8> {
    let mut p = vec![b'o'];
    for pair in pairs {
        p.extend(pair[0].to_be_bytes());
        p.extend(pair[1].to_be_bytes());
    }
    p
}

/// Elapsed-ms framed bundle: a two-byte first length marks bundling, then
/// one-byte lengths offset by 32.
fn bundle(ms: u16, packets: &[&[u8]]) -> Vec<u8> {
    let mut frame = ms.to_be_bytes().to_vec();
    for (i, packet) in packets.iter().enumerate() {
        assert!(packet.len() < 224);
        if i == 0 {
            frame.extend([0, packet.len() as u8]);
        } else {
            frame.push(packet.len() as u8 + 32);
        }
        frame.extend(*packet);
    }
    frame
}

/// The stock `setMscps` tables and leaderboard/record display expression.
fn stock_score(mscps: usize, sct: usize, fam: f64) -> Option<f64> {
    let (mut fmlts, mut fpsls) = (Vec::new(), Vec::new());
    for i in 0..=mscps {
        fmlts.push(if i >= mscps {
            fmlts[i - 1]
        } else {
            (1. - i as f64 / mscps as f64).powf(2.25)
        });
        fpsls.push(if i == 0 {
            0.
        } else {
            fpsls[i - 1] + 1. / fmlts[i - 1]
        });
    }
    let (fmlt, fpsl) = (fmlts[mscps], fpsls[mscps]);
    fmlts.extend([fmlt; 2048]);
    fpsls.extend([fpsl; 2048]);
    Some(((fpsls.get(sct)? + fam / fmlts.get(sct)? - 1.) * 15. - 5.).floor())
}

struct Client {
    state: ClientState,
    time: f64,
}
impl Client {
    fn new(setup_packet: &[u8]) -> Self {
        let mut state = ClientState::new();
        state.new_connection(1.).unwrap();
        state.receive_packet(setup_packet, 0., 1.).unwrap();
        Self { state, time: 1. }
    }
    fn at(&mut self, dt: f64, packet: &[u8]) -> Result<(), String> {
        self.time += dt;
        self.state.receive_packet(packet, self.time - 1., self.time)
    }
    fn send(&mut self, packet: &[u8]) {
        self.at(0., packet).unwrap();
    }
    fn frame(&self) -> ClientFrame {
        self.state.frame(self.time).unwrap()
    }
    fn hud(&self) -> ClientHud {
        self.frame().hud
    }
    /// A packet that must fail without changing anything observable.
    fn reject(&mut self, packet: &[u8]) {
        let before = self.frame();
        assert!(self.at(0., packet).is_err(), "{packet:?}");
        assert_eq!(self.frame(), before, "{packet:?}");
    }
}

#[test]
fn retained_leaderboards_match_the_stock_handler() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-hud-leaderboard.json")).unwrap();
    let setup = &fixture["setup"];
    let expected = &setup["expected"];
    let mut client = Client::new(&hex(setup["hex"].as_str().unwrap()));
    let settings = client.frame().settings.unwrap();
    let int = |v: &Value| v.as_u64().unwrap();
    assert_eq!(
        u64::from(settings.grid_center),
        int(&expected["gridCenter"])
    );
    assert_eq!(
        u64::from(settings.max_segments),
        int(&expected["maxSegments"])
    );
    assert_eq!(
        u64::from(settings.sector_count_along_edge),
        int(&expected["sectorCountAlongEdge"])
    );
    assert_eq!(
        settings.spangdv,
        int(&expected["spangdvTenths"]) as f64 / 10.
    );
    let nsp: Vec<f64> = expected["nspHundredths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| int(v) as f64 / 100.)
        .collect();
    assert_eq!(vec![settings.nsp1, settings.nsp2, settings.nsp3], nsp);
    let mamu = &expected["mamuThousandths"];
    assert_eq!(settings.mamu, int(&mamu[0]) as f64 / 1000.);
    assert_eq!(settings.mamu2, int(&mamu[1]) as f64 / 1000.);
    assert_eq!(
        settings.server_number.map(u64::from),
        Some(int(&expected["serverNumber"]))
    );
    assert_eq!(
        settings.game_mode.map(u64::from),
        Some(int(&expected["gameMode"]))
    );
    assert_eq!(settings.team_val, None);

    let mscps = settings.max_segments as usize;
    for board in fixture["leaderboards"].as_array().unwrap() {
        let bytes = hex(board["hex"].as_str().unwrap());
        client.at(0.5, &bytes).unwrap();
        let want = &board["expected"];
        let got = client.hud().leaderboard.unwrap();
        assert_eq!(int(&want["ownPosition"]), 0);
        assert_eq!(got.own_position, None);
        assert_eq!(u64::from(got.rank), int(&want["rank"]));
        assert_eq!(u64::from(got.player_count), int(&want["playerCount"]));
        assert_eq!(got.age_seconds, 0.);
        let rows = want["entries"].as_array().unwrap();
        assert_eq!(got.entries.len(), rows.len());
        for (entry, row) in got.entries.iter().zip(rows) {
            let fam = int(&row["famU24"]) as f64 / 16_777_215.;
            assert_eq!(u64::from(entry.segments), int(&row["segments"]));
            assert_eq!(entry.fam, fam);
            assert_eq!(entry.score, Some(row["score"].as_f64().unwrap()));
            assert_eq!(
                entry.score,
                stock_score(mscps, usize::from(entry.segments), fam)
            );
            assert_eq!(u64::from(entry.color), int(&row["color"]));
            assert_eq!(entry.name, row["name"].as_str().unwrap());
            assert!(!entry.own);
        }
    }
    // Ages advance with polling time; polling never refreshes the receipt.
    let later = client.state.frame(client.time + 1.25).unwrap();
    assert_eq!(later.hud.leaderboard.as_ref().unwrap().age_seconds, 1.25);
    assert_eq!(client.state.frame(client.time + 1.25).unwrap(), later);
}

#[test]
fn leaderboard_marks_own_row_and_keeps_wire_bytes_as_latin1() {
    let mut client = Client::new(&setup(430, &[]));
    let mut p = vec![b'l', 2, 0, 5, 0, 9];
    for (segments, color, name) in [
        (20u16, 12u8, "Caf\u{e9}".as_bytes()),
        (12, 3, &[b'C', b'a', b'f', 0xe9][..]),
    ] {
        p.extend(segments.to_be_bytes());
        p.extend(u24(8_388_607));
        p.extend([color, name.len() as u8]);
        p.extend(name);
    }
    client.send(&p);
    let board = client.hud().leaderboard.unwrap();
    assert_eq!(
        (board.own_position, board.rank, board.player_count),
        (Some(2), 5, 9)
    );
    // UTF-8 bytes appear one character per byte, as `String.fromCharCode` does.
    assert_eq!(board.entries[0].name, "Caf\u{c3}\u{a9}");
    assert_eq!(board.entries[1].name, "Caf\u{e9}");
    assert_eq!(board.entries[0].color, 12);
    assert_eq!(
        board.entries.iter().map(|e| e.own).collect::<Vec<_>>(),
        [false, true]
    );
    assert_eq!(
        board.entries[0].score,
        stock_score(430, 20, 8_388_607. / 16_777_215.)
    );
    // An empty list is a valid report.
    client.send(&[b'l', 0, 0, 1, 0, 1]);
    assert!(client.hud().leaderboard.unwrap().entries.is_empty());
}

#[test]
fn record_scores_use_the_setup_tables_and_their_stock_extension() {
    let mut client = Client::new(&setup(10, &[]));
    client.send(&record(5, 8_388_608, b"Bo", &[b'g', 0xe9, 0xc3, 0xa9]));
    let longest = client.hud().record.unwrap();
    assert_eq!((longest.segments, longest.name.as_str()), (5, "Bo"));
    assert_eq!(longest.message, "g\u{e9}\u{c3}\u{a9}");
    assert_eq!(longest.score, stock_score(10, 5, longest.fam));
    assert!(longest.score.unwrap() > 0.);
    // The stock tables hold mscps + 1 entries plus 2048 repeats of the last.
    client.send(&record(10 + 2048, 0, b"", b""));
    let longest = client.hud().record.unwrap();
    assert_eq!(longest.score, stock_score(10, 2058, 0.));
    assert!(longest.score.is_some());
    client.send(&record(10 + 2049, 0, b"", b""));
    assert_eq!(client.hud().record.unwrap().score, None);
    assert_eq!(stock_score(10, 2059, 0.), None);
}

#[test]
fn kill_counts_are_known_only_after_a_report_for_the_current_appearance() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&appearance(9, 3, b"me", &[], 255));
    client.send(&appearance(4, 7, &[b'Z', 0xf6], &[1, 2, 3], 17));
    let frame = client.frame();
    let observation = frame.observation.as_ref().unwrap();
    assert_eq!(observation.me.kills, 0);
    let profiles = &frame.hud.snakes;
    assert_eq!(
        profiles
            .iter()
            .map(|p| p.snake_id.as_str())
            .collect::<Vec<_>>(),
        ["4", "9"]
    );
    let other = &profiles[0];
    assert_eq!(
        (other.name.as_str(), other.skin, other.cosmetic),
        ("Z\u{f6}", 7, 17)
    );
    assert_eq!(other.custom_skin, [1, 2, 3]);
    assert!(profiles.iter().all(|p| p.kills.is_none()));

    // A reported zero differs from an unknown count.
    client.at(0.5, &kills(4, 0)).unwrap();
    client.at(0.5, &kills(9, 70_000)).unwrap();
    client.send(&kills(77, 5));
    let frame = client.frame();
    assert_eq!(frame.observation.as_ref().unwrap().me.kills, 70_000);
    let opponent = &frame.observation.as_ref().unwrap().opponents[0];
    assert_eq!(opponent.kills, 0);
    let hud = frame.hud;
    assert_eq!(hud.snakes.len(), 2);
    let report = hud.snakes[0].kills.as_ref().unwrap();
    assert_eq!((report.count, report.age_seconds), (0, 0.5));
    assert_eq!(hud.snakes[1].kills.as_ref().unwrap().count, 70_000);

    // Leaving view drops the profile; a new appearance starts unreported.
    client.at(0.25, &[b's', 0, 4, 0]).unwrap();
    client.send(&[b's', 0, 4, 1]);
    let hud = client.hud();
    assert_eq!(hud.snakes.len(), 1);
    assert_eq!(hud.removals.len(), 1);
    assert_eq!(
        (hud.removals[0].snake_id.as_str(), hud.removals[0].killed),
        ("4", Some(false))
    );
    client.send(&appearance(4, 7, b"Z", &[], 0));
    client.at(0.25, &[b's', 0, 4, 1]).unwrap();
    client.send(&appearance(4, 7, b"Z", &[], 0));
    client.send(&[b's', 0, 4]);
    let hud = client.hud();
    assert_eq!(
        hud.removals.iter().map(|r| r.killed).collect::<Vec<_>>(),
        [None, Some(true), Some(false)]
    );
    assert_eq!(hud.removals[2].age_seconds, 0.25);
    assert!(hud.snakes.iter().all(|p| p.snake_id == "9"));

    // Removal history is bounded to recent known snakes.
    for _ in 0..70 {
        client.send(&appearance(4, 7, b"Z", &[], 0));
        client.send(&[b's', 0, 4, 0]);
    }
    assert_eq!(client.hud().removals.len(), 64);
}

#[test]
fn session_and_team_reports_need_no_setup_but_leaderboards_do() {
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    for early in [&kills(9, 1)[..], &[b'S', 0, 5, 0xde, 0xad, 0xbe, 0xef]] {
        state.receive_packet(early, 0., 0.1).unwrap();
    }
    state.receive_packet(&teams(&[[3, -1]]), 0., 0.1).unwrap();
    let before = state.frame(0.2).unwrap();
    for needs_setup in [
        &[b'l', 0, 0, 1, 0, 1][..],
        &record(1, 0, b"", b""),
        &[b'M', 0, 1, 0x40],
    ] {
        assert!(state.receive_packet(needs_setup, 0., 0.2).is_err());
        assert_eq!(state.frame(0.2).unwrap(), before);
    }
    let session = before.hud.session.clone().unwrap();
    assert_eq!(
        (session.snake_id.as_str(), session.session_id),
        ("5", 0xdead_beef)
    );
    assert!(close(session.age_seconds, 0.1));
    assert!(before.hud.snakes.is_empty());

    // The session report's snake id does not select self.
    state.receive_packet(&setup(430, &[]), 0., 0.3).unwrap();
    state
        .receive_packet(&appearance(9, 0, b"", &[], 0), 0., 0.3)
        .unwrap();
    let frame = state.frame(0.3).unwrap();
    assert_eq!(frame.observation.unwrap().me.id, "9");
    assert_eq!(frame.hud.session.unwrap().snake_id, "5");
    assert_eq!(frame.hud.team_scores.unwrap().history, [[3, -1]]);
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12
}

#[test]
fn team_scores_follow_the_stock_ring_buffer() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&teams(&[[1, 10], [2, 20], [3, 30]]));
    let scores = client.hud().team_scores.unwrap();
    assert_eq!((scores.team1, scores.team2), (1, 10));
    assert_eq!(scores.history, [[1, 10], [2, 20], [3, 30]]);
    // A single pair overwrites the oldest entry and becomes current.
    client.at(0.5, &teams(&[[-1, i32::MAX]])).unwrap();
    let scores = client.hud().team_scores.unwrap();
    assert_eq!((scores.team1, scores.team2), (-1, i32::MAX));
    assert_eq!(scores.history, [[-1, i32::MAX], [1, 10], [2, 20]]);
    // A later list is prepended to the stock array; its last element stays current.
    client.send(&teams(&[[7, 70], [8, 80]]));
    let scores = client.hud().team_scores.unwrap();
    assert_eq!(
        scores.history,
        [[1, 10], [2, 20], [-1, i32::MAX], [7, 70], [8, 80]]
    );
    assert_eq!((scores.team1, scores.age_seconds), (1, 0.));
    client.time += 0.75;
    assert_eq!(client.hud().team_scores.unwrap().age_seconds, 0.75);
    for bad in [&b"o"[..], &[b'o', 0, 0, 0, 1], &teams(&[[1, 2]])[..8]] {
        client.reject(bad);
    }
}

#[test]
fn reconnect_clears_every_hud_report_and_polling_is_pure() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&appearance(9, 3, b"me", &[], 255));
    client.send(&kills(9, 2));
    client.send(&[b'l', 1, 0, 1, 0, 1, 0, 3, 0, 0, 0, 0, 0]);
    client.send(&record(4, 0, b"x", b"y"));
    client.send(&[b'S', 0, 9, 0, 0, 0, 1]);
    client.send(&teams(&[[1, 2]]));
    client.send(&[b'L', 1, 0, 1, 0x40]);
    client.send(&appearance(4, 3, b"o", &[], 255));
    client.send(&[b's', 0, 4, 1]);
    let first = client.state.frame(3.).unwrap();
    let hud = &first.hud;
    assert!(
        hud.leaderboard.is_some()
            && hud.record.is_some()
            && hud.session.is_some()
            && hud.team_scores.is_some()
            && hud.minimap_teams.is_some()
            && hud.minimap_command.as_deref() == Some("L")
            && hud.snakes.len() == 1
            && hud.removals.len() == 1
    );
    assert!(hud.leaderboard.as_ref().unwrap().entries[0].own);
    assert_eq!(hud.leaderboard.as_ref().unwrap().age_seconds, 2.);
    assert_eq!(client.state.frame(3.).unwrap(), first);
    let later = client.state.frame(4.).unwrap();
    assert_eq!(later.hud.record.unwrap().age_seconds, 3.);
    assert_eq!(later.hud.snakes[0].kills.as_ref().unwrap().age_seconds, 3.);

    client.state.new_connection(5.).unwrap();
    let fresh = client.state.frame(5.).unwrap();
    assert_eq!(fresh.hud, ClientHud::default());
    assert!(fresh.minimap.is_none() && fresh.settings.is_none());
}

#[test]
fn truncated_and_malformed_hud_packets_are_rejected_without_effect() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&appearance(9, 3, b"me", &[], 255));
    client.send(&[b'l', 0, 0, 1, 0, 1]);
    let mut full = appearance(4, 1, b"abc", &[5, 6], 2);
    full.truncate(22 + 1 + 3 + 1 + 1);
    for bad in [
        &[b'l', 0, 0, 1, 0][..],
        &[b'l', 0, 0, 1, 0, 1, 0, 3, 0, 0, 0, 0, 2, b'x'],
        &[b'l', 0, 0, 1, 0, 1, 0, 3, 0, 0],
        &record(1, 0, b"ab", b"")[..9],
        &[b'm', 0, 0, 1, 0, 0, 0],
        &[b'S', 0, 9, 0, 0, 0],
        &[b'S', 0, 9, 0, 0, 0, 1, 2],
        &kills(9, 1)[..5],
        &[b'k', 0, 9, 0, 0, 1, 0],
        &[b'o', 0, 0, 0, 1, 0, 0, 0, 2, 3],
        &full,
    ] {
        client.reject(bad);
    }
    // One invalid packet rejects its whole bundled message.
    let before = client.frame();
    let message = bundle(
        5,
        &[&kills(9, 3), &[b'S', 0, 9, 0, 0, 0, 1], &[b'l', 0, 0, 2, 0]],
    );
    assert!(client.state.receive_frame(&message, client.time).is_err());
    assert_eq!(client.frame(), before);
}

#[test]
fn sent_bytes_before_setup_are_negotiation_not_commands() {
    let records = [
        (0., WireEvent::Connect, vec![]),
        (0.01, WireEvent::Send, vec![4]),
        (0.01, WireEvent::Send, vec![1]),
        (0.01, WireEvent::Send, vec![251]),
        (0.01, WireEvent::Send, vec![99, 0]),
        (
            0.05,
            WireEvent::Receive,
            [vec![0, 0], setup(430, &[])].concat(),
        ),
        (0.06, WireEvent::Send, vec![254]),
    ]
    .map(|(time, event, bytes)| WireRecord { time, event, bytes });
    let mut state = ClientState::new();
    for record in &records {
        replay_record(&mut state, record).unwrap();
    }
    let frame = state.frame(0.07).unwrap();
    // A boost alone after [4] does not form a command, and [251] was no ping.
    assert!(frame.timing.sent_commands.is_empty());
    assert_eq!(frame.timing.ping_wait_seconds, None);
    replay_record(
        &mut state,
        &WireRecord {
            time: 0.08,
            event: WireEvent::Send,
            bytes: vec![10],
        },
    )
    .unwrap();
    let sent = state.frame(0.08).unwrap().timing.sent_commands;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].action.heading, 10. * TAU / 251.);
    assert!(!sent[0].action.boost);

    // The same records applied directly produce the same frame.
    let mut direct = ClientState::new();
    direct.new_connection(0.).unwrap();
    for bytes in [&[4][..], &[1], &[251], &[99, 0]] {
        direct.record_sent(bytes, 0.01).unwrap();
    }
    direct.receive_frame(&records[5].bytes, 0.05).unwrap();
    direct.record_sent(&[254], 0.06).unwrap();
    direct.record_sent(&[10], 0.08).unwrap();
    assert_eq!(direct.frame(0.08).unwrap(), state.frame(0.08).unwrap());
}

#[test]
fn setup_optional_mode_bytes_default_to_unknown() {
    let settings = |packet: &[u8]| Client::new(packet).frame().settings.unwrap();
    let base = settings(&setup(430, &[]));
    assert_eq!(base.server_number, Some(7));
    assert_eq!((base.game_mode, base.team_val), (None, None));
    assert_eq!(base.sector_count_along_edge, 144);
    assert_eq!(
        (base.spangdv, base.nsp1, base.nsp2, base.nsp3),
        (4.8, 5.39, 0.4, 14.)
    );
    assert_eq!((base.mamu, base.mamu2), (0.033, 0.029));
    let short = settings(&setup(430, &[])[..25]);
    assert_eq!((short.server_number, short.game_mode), (None, None));
    let team = settings(&setup(430, &[2]));
    assert_eq!((team.game_mode, team.team_val), (Some(2), None));
    let team = settings(&setup(430, &[2, 1]));
    assert_eq!((team.game_mode, team.team_val), (Some(2), Some(1)));
    // Outside team mode the stock client does not read the extra byte as a team.
    let normal = settings(&setup(430, &[1, 1, 9, 9]));
    assert_eq!((normal.game_mode, normal.team_val), (Some(1), None));
}

fn cells(frame: &ClientFrame) -> Vec<u8> {
    frame.minimap.as_ref().unwrap().cells.clone()
}
fn command(client: &Client) -> String {
    client.hud().minimap_command.unwrap()
}

#[test]
fn u_and_legacy_u_snapshots_use_their_stock_orders_and_runs() {
    let mut client = Client::new(&setup(430, &[]));
    // U fills from the last cell; 255 is an ordinary 127-cell skip without a count.
    client.send(&[b'U', 0, 3, 0b100_0001, 129, 0b100_0000]);
    let map = client.frame().minimap.unwrap();
    assert_eq!(map.side, 3);
    assert_eq!(map.cells, [1, 0, 1, 0, 0, 0, 0, 0, 1]);
    assert_eq!(command(&client), "U");
    client.send(&[b'U', 0, 12, 255, 0b111_1111]);
    let map = client.frame().minimap.unwrap();
    assert_eq!(map.cells.iter().filter(|&&c| c == 1).count(), 7);
    assert!(map.cells[10..17].iter().all(|&c| c == 1));
    // Sizes above 512 are clamped by the stock handler.
    client.send(&[b'U', 2, 88, 0b100_0000]);
    let map = client.frame().minimap.unwrap();
    assert_eq!(map.side, 512);
    assert_eq!(map.cells[512 * 512 - 1], 1);

    // Legacy u is a fixed 80-cell map filled from the first cell.
    client.send(&[b'u', 0b110_0000, 128 + 77, 0b000_0001]);
    let map = client.frame().minimap.unwrap();
    assert_eq!(map.side, 80);
    assert_eq!(&map.cells[..3], [1, 1, 0]);
    assert_eq!(map.cells[7 + 77 + 6], 1);
    assert_eq!(map.cells.iter().filter(|&&c| c == 1).count(), 3);
    assert_eq!(command(&client), "u");
    client.reject(&[b'U', 0, 0]);
    client.reject(&[b'U', 0]);
}

#[test]
fn team_layers_keep_wire_order_and_the_minimap_holds_their_union() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&[b'L', 2, 0, 2, 0b100_0000, 0b110_0000]);
    let frame = client.frame();
    assert_eq!(cells(&frame), [0, 0, 1, 1]);
    let teams = frame.hud.minimap_teams.unwrap();
    assert_eq!(teams.side, 2);
    assert_eq!(teams.layers, [vec![0, 0, 0, 1], vec![0, 0, 1, 1]]);
    // L supports the extended 255 skip; an exhausted payload leaves later layers empty.
    client
        .at(0.5, &[b'L', 3, 0, 12, 255, 1, 0b100_0000])
        .unwrap();
    let teams = client.hud().minimap_teams.unwrap();
    assert_eq!(teams.layers.len(), 3);
    assert_eq!(teams.layers[0][144 - 1 - 126], 1);
    assert!(teams.layers[1..].iter().all(|l| l.iter().all(|&c| c == 0)));
    // A delta keeps the layers with their own receipt age; a full snapshot drops them.
    client.at(0.5, &[b'V', 0b100_0000]).unwrap();
    let hud = client.hud();
    assert_eq!(hud.minimap_teams.unwrap().age_seconds, 0.5);
    assert_eq!(hud.minimap_command.as_deref(), Some("V"));
    client.send(&[b'M', 0, 2, 0]);
    assert!(client.hud().minimap_teams.is_none());
    client.reject(&[b'L', 1, 0]);
    client.reject(&[b'L', 1, 0, 0]);
    client.reject(&[b'L', 1, 0, 2, 255]);
}

#[test]
fn team_layers_have_a_bounded_cell_budget() {
    let mut client = Client::new(&setup(430, &[]));
    assert_eq!(MAX_TEAM_MINIMAP_CELLS, 4 * 512 * 512);
    // Four maximum-size layers fit; the evidenced two-team mode is within this.
    client.send(&[b'L', 4, 2, 0, 0b100_0000]);
    let teams = client.hud().minimap_teams.unwrap();
    assert_eq!((teams.side, teams.layers.len()), (512, 4));
    let before = client.frame();
    for count in [5, 255] {
        let error = client.at(0., &[b'L', count, 2, 0, 128]).unwrap_err();
        assert!(error.contains("Unsupported team minimap"), "{error}");
        assert_eq!(client.frame(), before);
    }
}

#[test]
fn deltas_toggle_the_stock_base_after_every_full_snapshot_family() {
    // V applies after a first U, L or u: each allocates a zero base of its side.
    for snapshot in [&[b'U', 0, 2, 0b100_0000][..], &[b'L', 1, 0, 2, 0b100_0000]] {
        let mut client = Client::new(&setup(430, &[]));
        client.send(snapshot);
        client.send(&[b'V', 0b010_0000]);
        // The delta paints its toggled base cell; untouched shown cells remain.
        assert_eq!(cells(&client.frame()), [0, 0, 1, 1]);
        client.send(&[b'V', 0b110_0000]);
        assert_eq!(cells(&client.frame()), [0, 0, 0, 1]);
    }
    let mut client = Client::new(&setup(430, &[]));
    client.send(&[b'u', 0b100_0000]);
    client.send(&[b'V', 0b100_0000]);
    let map = cells(&client.frame());
    assert_eq!((map[0], map[6399]), (0, 1));

    // A same-side u keeps the M base and modern target: V returns to that
    // target, clears the M cell and drops u's separate legacy picture.
    let mut client = Client::new(&setup(430, &[]));
    let mut m = vec![b'M', 0, 80, 0b100_0000];
    client.send(&m);
    client.send(&[b'u', 0b100_0000]);
    let map = cells(&client.frame());
    assert_eq!((map[0], map[6399]), (1, 0));
    client.send(&[b'V', 0b100_0000]);
    let map = cells(&client.frame());
    assert_eq!((map[0], map[6399]), (0, 0));
    assert_eq!(map.iter().filter(|&&c| c == 1).count(), 0);

    // Changing to u's 80-cell side clears both modern canvases and its base.
    client.send(&[b'M', 0, 2, 0b100_0000]);
    client.send(&[b'u', 0b100_0000]);
    client.send(&[b'V', 0b100_0000]);
    let map = cells(&client.frame());
    assert_eq!(map.len(), 6400);
    assert_eq!((map[0], map[6399]), (0, 1));

    // A same-side U keeps the M base too; a different side resets it to zeros.
    m = vec![b'M', 0, 2, 0b100_0000];
    client.send(&m);
    client.send(&[b'U', 0, 2, 0]);
    client.send(&[b'V', 0b100_0000]);
    assert_eq!(cells(&client.frame()), [0, 0, 0, 0]);
    client.send(&m);
    client.send(&[b'U', 0, 3, 0]);
    client.send(&[b'U', 0, 2, 0]);
    client.send(&[b'V', 0b100_0000]);
    assert_eq!(cells(&client.frame()), [0, 0, 0, 1]);

    // Without any full snapshot the delta still has no known base.
    let mut client = Client::new(&setup(430, &[]));
    client.reject(&[b'V', 0b100_0000]);
    // Existing M limits are unchanged.
    client.reject(&[b'M', 2, 1]);
}

#[test]
fn malformed_minimap_bundles_roll_back_every_family() {
    let mut client = Client::new(&setup(430, &[]));
    client.send(&[b'M', 0, 2, 0b101_0000]);
    let before = client.frame();
    let prefix: [&[u8]; 4] = [
        &[b'U', 0, 2, 0b111_1111],
        &[b'L', 2, 0, 2, 0b100_0000, 0],
        &[b'u', 0],
        &[b'V', 0b100_0000],
    ];
    for bad in [
        &[b'V', 255][..],
        &[b'L', 1, 0, 2, 255],
        &[b'L', 5, 2, 0],
        &[b'U', 0],
    ] {
        let message = bundle(5, &[prefix[0], prefix[1], prefix[2], prefix[3], bad]);
        assert!(
            client.state.receive_frame(&message, client.time).is_err(),
            "{bad:?}"
        );
        assert_eq!(client.frame(), before, "{bad:?}");
        assert_eq!(command(&client), "M");
    }
    // The same valid prefix commits in order once nothing in its message fails.
    let message = bundle(5, &prefix);
    client.state.receive_frame(&message, client.time).unwrap();
    assert_eq!(command(&client), "V");
    assert_eq!(cells(&client.frame()).len(), 6400);
    assert!(client.hud().minimap_teams.is_none());
}
