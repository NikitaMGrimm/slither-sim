#![cfg(feature = "networking")]

use slither_sim::{
    Action,
    client::network::{Handshake, login_packet},
    client::{
        Backend, ClientState, ClientStatus, NetworkBackend, NetworkConfig, WireEvent, replay_record,
    },
};
use std::{
    io::Write,
    net::TcpListener,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tungstenite::{
    Message, accept_hdr,
    handshake::server::{Request, Response},
};

fn setup() -> Vec<u8> {
    vec![
        97, 0, 127, 38, 1, 174, 1, 44, 0, 102, 48, 2, 27, 0, 40, 5, 120, 0, 33, 0, 29, 1, 174, 19,
        42, 34, 198, 0, 30, 246,
    ]
}

fn u24(n: u32) -> [u8; 3] {
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}

fn appearance() -> Vec<u8> {
    // Synthetic protocol-19 appearance: one head, two body anchors.
    let mut p = vec![b's', 0, 1, 0, 0, 0, b'1', 0, 0, 0, 15, 160, 0, 0, 0, 0];
    p.extend(u24(32550 * 5));
    p.extend(u24(32550 * 5));
    p.extend([0, 0, 0]);
    p.extend(u24(32466 * 5));
    p.extend(u24(32550 * 5));
    p.extend([0, 0]);
    p
}

fn framed(packet: &[u8]) -> Message {
    let mut p = vec![0, 0];
    p.extend(packet);
    Message::Binary(p.into())
}

#[test]
#[allow(
    clippy::result_large_err,
    reason = "tungstenite's header callback fixes the error type"
)]
fn loopback_uses_binary_actions_measured_ping_and_replayable_receipts() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sent, received) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut socket = accept_hdr(stream, |request: &Request, response: Response| {
            assert_eq!(request.uri().path(), "/slither");
            assert_eq!(request.headers()["Origin"], "https://local.example");
            Ok(response)
        })
        .unwrap();
        assert_eq!(socket.read().unwrap().into_data().as_ref(), [4]);
        let mut expected_login = login_packet("Portable", 2).unwrap();
        expected_login.extend([3, 1, 4]);
        assert_eq!(socket.read().unwrap().into_data().as_ref(), expected_login);
        socket.send(framed(&setup())).unwrap();
        socket.send(framed(&appearance())).unwrap();
        while let Ok(message) = socket.read() {
            match message {
                Message::Binary(bytes) if bytes.as_ref() == [251] => {
                    socket.send(framed(b"p")).unwrap();
                }
                Message::Binary(bytes) => {
                    sent.send(bytes.to_vec()).unwrap();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    let mut backend = NetworkBackend::connect(NetworkConfig {
        url: format!("ws://{address}/slither"),
        origin: Some("https://local.example".into()),
        nickname: "Portable".into(),
        skin: 2,
        custom_skin: vec![3, 1, 4],
        request_session_id: true,
        handshake: Handshake::Direct,
        record_wire: true,
        ..NetworkConfig::default()
    })
    .unwrap();
    // Setup and the pong can arrive on different decision polls, particularly
    // when the peer's small writes are delayed by TCP buffering.
    let deadline = Instant::now() + Duration::from_secs(2);
    let frame = loop {
        let frame = backend.next_frame().unwrap();
        if frame.status == ClientStatus::Playing && frame.timing.ping_seconds.is_some() {
            break frame;
        }
        assert!(
            Instant::now() < deadline,
            "No playing frame with measured ping"
        );
    };
    assert_eq!(frame.status, ClientStatus::Playing);
    assert_eq!(frame.observation.as_ref().unwrap().me.id, "1");
    assert!(frame.timing.ping_seconds.is_some());
    backend.set_processing_seconds(0.003).unwrap();
    backend
        .send_action(Action {
            heading: std::f64::consts::FRAC_PI_2,
            boost: true,
        })
        .unwrap();
    assert_eq!(received.recv_timeout(Duration::from_secs(3)).unwrap(), [62]);
    assert_eq!(
        received.recv_timeout(Duration::from_secs(3)).unwrap(),
        [253]
    );
    let poll = backend.frame().unwrap();
    assert_eq!(poll.timing.processing_seconds, 0.003);
    let original = frame.received.snake("1").unwrap();
    let later = poll.received.snake("1").unwrap();
    assert!(later.body.age_seconds >= original.body.age_seconds);
    assert_eq!(later.incarnation, original.incarnation);
    let records = backend.drain_records();
    assert!(
        records
            .iter()
            .any(|r| r.event == WireEvent::Send && r.bytes == [251])
    );
    let mut replay = ClientState::new();
    for record in &records {
        replay_record(&mut replay, record).unwrap();
    }
    replay.set_processing_seconds(0.003).unwrap();
    let expected = replay.frame(poll.time).unwrap();
    assert_eq!(poll, expected);
    backend.disconnect();
    assert_eq!(backend.frame().unwrap().status, ClientStatus::Disconnected);
    server.join().unwrap();
}

#[test]
fn invalid_network_configuration_fails_before_connecting() {
    for url in [
        "",
        "https://localhost/slither",
        "ws://user:secret@localhost/slither",
    ] {
        assert!(
            NetworkConfig {
                url: url.into(),
                ..NetworkConfig::default()
            }
            .validate()
            .is_err()
        );
    }
    for duration in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(
            NetworkConfig {
                url: "ws://localhost/slither".into(),
                decision_interval_seconds: duration,
                ..NetworkConfig::default()
            }
            .validate()
            .is_err()
        );
    }
    assert!(login_packet("😊", 0).is_err());
    assert!(login_packet(&"a".repeat(25), 0).is_err());
    assert!(
        NetworkConfig {
            url: "ws://localhost/slither".into(),
            custom_skin: vec![0; 256],
            ..NetworkConfig::default()
        }
        .validate()
        .is_err()
    );
    let login = login_packet("ä", 7).unwrap();
    assert_eq!(&login[24..], &[7, 1, 228, 0, 255]);
}

#[test]
fn unsupported_setup_closes_the_connection_instead_of_producing_an_observation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        let mut packet = setup();
        packet[23] = 11;
        socket.send(framed(&packet)).unwrap();
        let _ = socket.read();
    });
    let mut backend = NetworkBackend::connect(NetworkConfig {
        url: format!("ws://{address}/slither"),
        handshake: Handshake::Direct,
        ..NetworkConfig::default()
    })
    .unwrap();
    assert!(backend.next_frame().is_err());
    let frame = backend.frame().unwrap();
    assert_eq!(frame.status, ClientStatus::Disconnected);
    assert!(frame.observation.is_none());
    server.join().unwrap();
}

#[test]
fn captured_challenge_and_unframed_reconnect_clear_connection_knowledge() {
    // Exact payload/reply from coil bot0 records 4/5; the rest of this server is synthetic.
    let challenge = b"euLxuGXMjVNCwXDSLcWXFbjNuKZXhYQPBNffoPvMhWQcTpdrOHROCcmItebmheOLZwfSnCyLBxlKwRCBjGSmxOgfRIZZIulatqGwnZWizOLFQRYmgfrLwWEUMHutfZJjRrZXmhtNbQjWrdyVHBonUvfrJmVjDwLxUgzPjjRGZPFTqetQFZJHSreb";
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for connection in 0..2 {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            assert_eq!(socket.read().unwrap().into_data().as_ref(), [99, 0]);
            let mut packet = vec![b'6'];
            packet.extend(challenge);
            socket.send(Message::Binary(packet.into())).unwrap();
            assert_eq!(
                socket.read().unwrap().into_data().as_ref(),
                b"LzLIOvqUYaFMrAJWShKPlPEiOBC"
            );
            assert_eq!(
                socket.read().unwrap().into_data().as_ref(),
                login_packet("SlitherSim", 0).unwrap()
            );
            if connection == 0 {
                socket.send(Message::Binary(setup().into())).unwrap();
                socket.send(Message::Binary(appearance().into())).unwrap();
            } else {
                let mut bundle = (setup().len() as u16).to_be_bytes().to_vec();
                bundle.extend(setup());
                bundle.push(32 + appearance().len() as u8);
                bundle.extend(appearance());
                socket.send(Message::Binary(bundle.into())).unwrap();
            }
            while let Ok(message) = socket.read() {
                match message {
                    Message::Binary(bytes) if bytes.as_ref() == [251] => {
                        socket.send(Message::Binary(vec![b'p'].into())).unwrap();
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    });
    let mut backend = NetworkBackend::connect(NetworkConfig {
        url: format!("ws://{address}/slither"),
        ..NetworkConfig::default()
    })
    .unwrap();
    let first = backend.next_frame().unwrap();
    assert_eq!(first.status, ClientStatus::Playing);
    backend
        .send_action(Action {
            heading: 0.,
            boost: true,
        })
        .unwrap();
    assert!(!backend.frame().unwrap().timing.sent_commands.is_empty());
    backend.reconnect().unwrap();
    let reset = backend.frame().unwrap();
    assert!(reset.observation.is_none());
    assert!(reset.timing.sent_commands.is_empty());
    assert!(reset.timing.ping_seconds.is_none());
    assert!(reset.received.connection_generation > first.received.connection_generation);
    let second = backend.next_frame().unwrap();
    assert_eq!(second.status, ClientStatus::Playing);
    assert_eq!(second.observation.as_ref().unwrap().me.id, "1");
    backend.disconnect();
    server.join().unwrap();
}

#[test]
fn direct_login_accepts_timing_header_with_challenge_high_byte() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        let mut frame = vec![b'6', 0];
        frame.extend(setup());
        socket.send(Message::Binary(frame.into())).unwrap();
        socket.send(framed(&appearance())).unwrap();
        while let Ok(message) = socket.read() {
            if matches!(message, Message::Close(_)) {
                break;
            }
        }
    });
    let mut backend = NetworkBackend::connect(NetworkConfig {
        url: format!("ws://{address}/slither"),
        handshake: Handshake::Direct,
        ..NetworkConfig::default()
    })
    .unwrap();
    assert_eq!(backend.next_frame().unwrap().status, ClientStatus::Playing);
    backend.disconnect();
    server.join().unwrap();
}

#[test]
fn partial_messages_obey_deadlines_and_resume_from_buffered_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (begin, ready) = mpsc::channel();
    let (polled, next_poll) = mpsc::channel();
    let (written, write_done) = mpsc::channel();
    let (trickle, start_trickle) = mpsc::channel();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.read().unwrap();
        socket.send(framed(&setup())).unwrap();
        socket.send(framed(&appearance())).unwrap();
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        // Send one small valid message over several client decision polls,
        // one byte per poll: fixed sleeps overslept by up to 40 ms on macOS runners.
        let mut payload = vec![0, 7, b'z'];
        payload.extend(u24(7900));
        let stream = socket.get_mut();
        stream.write_all(&[0x82, payload.len() as u8]).unwrap();
        for byte in payload {
            next_poll.recv_timeout(Duration::from_secs(2)).unwrap();
            stream.write_all(&[byte]).unwrap();
            written.send(()).unwrap();
        }
        start_trickle.recv_timeout(Duration::from_secs(2)).unwrap();
        // No complete game message arrives before idle timeout. Frequent bytes
        // must not renew the absolute operation deadline inside socket.read().
        let mut payload = vec![0, 7, b'q'];
        payload.resize(120, b'x');
        stream.write_all(&[0x82, payload.len() as u8]).unwrap();
        for byte in payload {
            if stream.write_all(&[byte]).is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(4));
        }
    });
    let mut backend = NetworkBackend::connect(NetworkConfig {
        url: format!("ws://{address}/slither"),
        handshake: Handshake::Direct,
        decision_interval_seconds: 0.015,
        // Above the first message's seven polls; below the second's 480 ms trickle.
        idle_timeout_seconds: 0.3,
        ..NetworkConfig::default()
    })
    .unwrap();
    assert_eq!(backend.next_frame().unwrap().status, ClientStatus::Playing);
    begin.send(()).unwrap();
    let started = Instant::now();
    let first = backend.next_frame().unwrap();
    assert!(started.elapsed() < Duration::from_millis(150));
    assert_eq!(first.observation.unwrap().arena.radius, 7926.);
    let mut radius = 7926.;
    for _ in 0..12 {
        polled.send(()).unwrap();
        write_done.recv_timeout(Duration::from_secs(2)).unwrap();
        radius = backend
            .next_frame()
            .unwrap()
            .observation
            .unwrap()
            .arena
            .radius;
        if radius == 7900. {
            break;
        }
    }
    assert_eq!(
        radius, 7900.,
        "partial WebSocket message must survive decision timeouts"
    );
    trickle.send(()).unwrap();
    let started = Instant::now();
    let mut timed_out = false;
    for _ in 0..40 {
        if backend.next_frame().is_err() {
            timed_out = true;
            break;
        }
    }
    assert!(timed_out, "trickled bytes must not bypass the idle timeout");
    assert!(started.elapsed() < Duration::from_millis(450));
    assert_eq!(backend.frame().unwrap().status, ClientStatus::Disconnected);
    backend.disconnect();
    server.join().unwrap();
}
