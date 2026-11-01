//! Authored loopback peers; no GPU, window or outside network.
use super::{bundle, challenge, setup, snake};
use slither_client::session_policy::Phase;
use slither_wgpu_slice::{
    game::{EffectKind, LocalGame},
    native::{Event, Transport},
};
use std::{
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};
use tungstenite::{
    Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};
#[path = "../../../../tests/support/protocol13.rs"]
mod wire13;

fn connected(protocol: u8, setup_picture: bool) -> LocalGame {
    let mut game = super::negotiating();
    let packets = match protocol {
        13 => vec![wire13::setup(), wire13::snake(7), wire13::snake(8)],
        19 => {
            let mut rival = snake(0);
            rival[2] = 8;
            vec![setup(), snake(0), rival]
        }
        _ => unreachable!(),
    };
    game.receive(game.generation(), &bundle(&packets), 3.)
        .unwrap();
    if !setup_picture {
        super::pictures(&mut game);
        super::pump(&mut game, 20.);
    }
    game
}

#[test]
fn own_death_removal_without_game_over_is_quiet_on_both_protocols() {
    for protocol in [13, 19] {
        let mut game = connected(protocol, false);
        let generation = game.generation();
        game.receive(generation, &bundle(&[vec![b's', 0, 7, 1]]), 30.)
            .unwrap();
        assert!(game.client().control_view().own.unwrap().dead);
        assert!(game.champion_session().outcome.is_none());
        game.transport_failed(generation, "synthetic reset after own death")
            .unwrap();
        assert_eq!(game.phase(), Phase::Disconnected);
        assert!(
            game.failure.is_none(),
            "protocol {protocol}: {:?}",
            game.failure
        );
        // Removal is sufficient to classify the close, but does not invent v's result.
        assert!(game.champion_session().outcome.is_none());
        super::effects(&mut game);
        game.play("ws://127.0.0.1:19491/slither", "Player", 0)
            .unwrap();
        game.transport_failed(generation, "stale reset").unwrap();
        assert!(game.failure.is_none());
        game.transport_failed(game.generation(), "new connection failed")
            .unwrap();
        assert_eq!(game.failure.as_deref(), Some("new connection failed"));
    }
}

#[test]
fn rival_death_and_own_non_death_removal_keep_errors_on_both_protocols() {
    for protocol in [13, 19] {
        for removal in [vec![b's', 0, 8, 1], vec![b's', 0, 7, 0]] {
            let mut game = connected(protocol, false);
            let generation = game.generation();
            game.receive(generation, &bundle(&[removal]), 30.).unwrap();
            assert!(!game.client().control_view().own.unwrap().dead);
            game.transport_failed(generation, "synthetic midgame reset")
                .unwrap();
            assert_eq!(game.failure.as_deref(), Some("synthetic midgame reset"));
        }
    }
}

#[test]
fn deferred_game_over_survives_terminal_send_and_presentation_fences() {
    for protocol in [13, 19] {
        for fence in ["setup", "send", "prepared", "issued"] {
            for terminal in [vec![b'v', 0], vec![b'v', 1], vec![b's', 0, 7, 1]] {
                let outcome = (terminal[0] == b'v').then(|| {
                    if terminal[1] == 0 {
                        Phase::Dead
                    } else {
                        Phase::Victory
                    }
                });
                let mut game = connected(protocol, fence == "setup");
                let generation = game.generation();
                let mut issued = None;
                match fence {
                    "setup" => assert!(game.has_frame()),
                    "send" => {
                        game.begin_frame(300., [960, 540], 1.).unwrap();
                        assert!(matches!(game.effect().unwrap().kind, EffectKind::Send(_)));
                    }
                    "prepared" | "issued" => {
                        game.begin_frame(40., [960, 540], 1.).unwrap();
                        super::effects(&mut game);
                        assert!(game.has_frame());
                        if fence == "issued" {
                            issued = Some(game.take_frame().unwrap());
                        }
                    }
                    _ => unreachable!(),
                }
                let now = if fence == "send" { 301. } else { 41. };
                game.receive(generation, &bundle(&[terminal]), now).unwrap();
                assert!(game.champion_session().outcome.is_none());
                game.transport_failed(generation, "synthetic terminal transport")
                    .unwrap();
                assert!(
                    game.failure.is_none(),
                    "protocol {protocol}, {fence}, {outcome:?}: {:?}",
                    game.failure
                );
                assert_eq!(game.champion_session().outcome, outcome);
                assert_eq!(game.login_final().is_some(), outcome.is_some());
                assert_eq!(game.phase(), Phase::Disconnected);
                assert!(game.effect().is_none(), "terminal ingress must not send");
                if issued.is_some() {
                    assert!(
                        game.presented(true).is_err(),
                        "terminal event cancelled the issued frame"
                    );
                }
            }
        }
    }
}

fn disconnect(death: Option<u8>, close: Option<CloseCode>) -> LocalGame {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
    let peer = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("accept: {e}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        for expected in [vec![1], vec![99, 0]] {
            assert!(matches!(socket.read().unwrap(), Message::Binary(b) if b.as_ref() == expected));
        }
        socket
            .send(Message::Binary(bundle(&[challenge()]).into()))
            .unwrap();
        assert!(matches!(socket.read().unwrap(), Message::Binary(_)));
        assert!(matches!(socket.read().unwrap(), Message::Binary(b) if b[0] == b's'));
        socket
            .send(Message::Binary(bundle(&[setup(), snake(0)]).into()))
            .unwrap();
        if let Some(kind) = death {
            socket
                .send(Message::Binary(bundle(&[vec![b'v', kind]]).into()))
                .unwrap();
        }
        if let Some(code) = close {
            socket
                .close(Some(CloseFrame {
                    code,
                    reason: "synthetic close".into(),
                }))
                .unwrap();
        }
        // Deliberately drop TCP, including the missing-handshake regression.
    });
    let started = Instant::now();
    let mut game = LocalGame::new(0.).unwrap();
    let mut transport = Transport::default();
    game.play(&endpoint, "Player", 0).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(game.phase(), Phase::Disconnected | Phase::Failed) {
        assert!(Instant::now() < deadline, "disconnect deadline");
        let now = started.elapsed().as_secs_f64() * 1000.;
        for event in transport.poll() {
            match event {
                Event::Opened(g) => game.opened(g, now).unwrap(),
                Event::Binary(g, b) => game.receive(g, &b, now).unwrap(),
                Event::Closed(g) => game
                    .transport_failed(g, "Connection closed before game-over")
                    .unwrap(),
                Event::Failed(g, e) => game.transport_failed(g, &e).unwrap(),
            }
        }
        game.begin_redraw(now, [960, 540], 1.).unwrap();
        for _ in 0..64 {
            let Some(effect) = game.effect() else { break };
            if matches!(effect.kind, EffectKind::Connect(_)) {
                game.construct(effect.generation, effect.id, now).unwrap();
            }
            if !transport.execute_pending(&effect, Instant::now()).unwrap() {
                break;
            }
            game.acknowledge(effect.generation, effect.id, true)
                .unwrap();
        }
        while game.take_frame().is_some() {
            game.presented(true).unwrap();
        }
        thread::sleep(Duration::from_millis(1));
    }
    peer.join().unwrap();
    game
}

#[test]
fn death_then_tcp_drop_or_close_is_quiet() {
    for kind in [0, 1] {
        for close in [None, Some(CloseCode::Normal), Some(CloseCode::Error)] {
            let game = disconnect(Some(kind), close);
            assert_eq!(game.phase(), Phase::Disconnected);
            assert!(game.failure.is_none(), "{:?}", game.failure);
            assert!(game.champion_session().outcome.is_some());
        }
    }
}

#[test]
fn disconnect_before_death_keeps_the_error() {
    for close in [None, Some(CloseCode::Normal), Some(CloseCode::Error)] {
        let game = disconnect(None, close);
        assert_eq!(game.phase(), Phase::Failed);
        assert!(game.failure.is_some());
    }
}

#[test]
fn refused_connection_is_still_an_error() {
    // Reserve an ephemeral loopback endpoint, then close its listener.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
    drop(listener);
    let mut game = LocalGame::new(0.).unwrap();
    let mut transport = Transport::default();
    game.play(&endpoint, "Player", 0).unwrap();
    super::pump(&mut game, 1.);
    let effect = game.effect().expect("Connect after reset picture");
    game.construct(effect.generation, effect.id, 1.).unwrap();
    transport.execute_pending(&effect, Instant::now()).unwrap();
    game.acknowledge(effect.generation, effect.id, true)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < deadline, "refused connection deadline");
        if let Some(Event::Failed(g, error)) = transport.poll().into_iter().next() {
            game.transport_failed(g, &error).unwrap();
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(game.phase(), Phase::Failed);
    assert!(game.failure.is_some());
}
