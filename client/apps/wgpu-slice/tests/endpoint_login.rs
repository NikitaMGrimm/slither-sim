#![cfg(not(target_arch = "wasm32"))]
#[path = "support/announcement_tape.rs"]
mod support;

use slither_wgpu_slice::{endpoint_login, game::LocalGame};
use std::{
    net::TcpListener,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use support::{bundle, challenge, effects, pump};
use tungstenite::Message;

#[test]
fn direct_server_login_keeps_the_fixed_full_packet() {
    // Pin src/client/network.rs::login_packet on main (d9dd6498), not a downloaded fixture.
    let mut expected = vec![
        115, 30, 1, 35, 54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208,
        136, 213, 140, 111, 9, 5,
    ];
    expected.extend(b"Fixed");
    expected.extend([0, 255]);
    // The production interactive helper does not distinguish loopback from remote hosts.
    // All effects are acknowledged in memory; no socket is constructed here.
    for server in ["192.0.2.1:445", "game.example:80", "[2001:db8::1]:446"] {
        let endpoint = slither_wgpu_slice::official::explicit_endpoint(server).unwrap();
        for appearance in [false, true] {
            let mut game = LocalGame::new(0.).unwrap();
            let version = if appearance {
                endpoint_login::play_appearance(
                    &mut game,
                    &endpoint,
                    "Fixed",
                    &slither_client::skin_studio::AppearanceSelection {
                        skin: 9,
                        custom_bytes: vec![],
                    },
                )
                .unwrap()
            } else {
                endpoint_login::play(&mut game, &endpoint, "Fixed", 9).unwrap()
            };
            assert_eq!(version, 291);
            pump(&mut game, 1.);
            effects(&mut game);
            game.opened(game.generation(), 2.).unwrap();
            assert_eq!(effects(&mut game), [vec![1], vec![99, 0]]);
            game.receive(game.generation(), &bundle(&[challenge()]), 3.)
                .unwrap();
            let packets = effects(&mut game);
            assert_eq!(packets.len(), 2);
            assert_eq!(packets[1], expected);
        }
    }
}

struct Probe(Child);
impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn e2e_binary_still_sends_zero_profile_to_a_synthetic_loopback_peer() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = Probe(
        Command::new(env!("CARGO_BIN_EXE_slither-client-e2e"))
            .args(["--server", &address.to_string(), "--seconds", "5"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(5));
            }
            result => panic!("synthetic e2e peer accept: {result:?}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut socket = tungstenite::accept(stream).unwrap();
    assert_eq!(socket.read().unwrap(), Message::Binary(vec![1].into()));
    assert_eq!(socket.read().unwrap(), Message::Binary(vec![99, 0].into()));
    socket
        .send(Message::Binary(bundle(&[challenge()]).into()))
        .unwrap();
    assert!(matches!(socket.read().unwrap(), Message::Binary(_)));
    let Message::Binary(login) = socket.read().unwrap() else {
        panic!("expected login")
    };
    assert_eq!(
        login.as_ref(),
        slither_client::protocol::encode_login(0, &[0; 20], "Local E2E", 0, None).unwrap()
    );
    // This peer ends after inspecting login; it deliberately supplies no Setup/world.
    socket.close(None).unwrap();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert_eq!(status.code(), Some(1));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "e2e did not exit after synthetic peer close"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
