#![cfg(feature = "networking")]

use slither_sim::{
    Config,
    client::{
        Bot, CONTRACT, ClientState, ClientStatus, ReferenceBot, SimulationConfig, SimulatorBackend,
        WireEvent, replay_record,
    },
    reference,
};
use std::{net::TcpListener, sync::mpsc, thread, time::Duration};
use tungstenite::Message;

#[test]
fn adapter_checks_lifecycle_and_reset_has_no_cross_life_state() {
    let mut backend = SimulatorBackend::new(7, SimulationConfig::default()).unwrap();
    backend.advance_ticks(6).unwrap();
    let mut frame = backend.frame().unwrap();
    assert_eq!(frame.status, ClientStatus::Playing);
    let mut bot = ReferenceBot;
    let expected = reference::decide(frame.observation.as_ref().unwrap());
    assert_eq!(bot.decide(&frame), Some(expected));
    let mut changed = frame.clone();
    changed.observation.as_mut().unwrap().food.clear();
    bot.decide(&changed);
    bot.reset();
    assert_eq!(bot.decide(&frame), ReferenceBot.decide(&frame));
    assert_eq!(std::mem::size_of::<ReferenceBot>(), 0);
    for status in [
        ClientStatus::Waiting,
        ClientStatus::Dead,
        ClientStatus::Disconnected,
    ] {
        frame.status = status;
        assert_eq!(bot.decide(&frame), None);
    }
    frame.status = ClientStatus::Playing;
    frame.contract = "incompatible".into();
    assert_eq!(bot.decide(&frame), None);
    frame.contract = CONTRACT.into();
    frame.observation = None;
    assert_eq!(bot.decide(&frame), None);
}

#[test]
fn local_and_websocket_delivered_frames_choose_identical_actions() {
    // Real loopback WebSocket payloads, with retained virtual receipt timestamps:
    // OS scheduling must not change the input tape being compared.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    thread::scope(|scope| {
        let server = scope.spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            stream.set_nodelay(true).unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            for bytes in receiver {
                socket.send(Message::Binary(bytes.into())).unwrap();
            }
        });
        let (mut socket, _) = tungstenite::connect(format!("ws://{address}/slither")).unwrap();
        let config = SimulationConfig {
            agents: 4,
            world: Config::official(4),
            record_wire: true,
            ..Default::default()
        };
        let mut backend = SimulatorBackend::new(7, config).unwrap();
        let mut decoded = ClientState::new();
        let mut local_bot = ReferenceBot;
        let mut wire_bot = ReferenceBot;
        let mut playing = 0;
        for step in 0..160 {
            if step == 80 {
                backend.reconnect_slot(0).unwrap();
                local_bot.reset();
                wire_bot.reset();
            }
            for mut record in backend.drain_records(0).unwrap() {
                if record.event == WireEvent::Receive {
                    sender.send(record.bytes.clone()).unwrap();
                    record.bytes = socket.read().unwrap().into_data().to_vec();
                }
                replay_record(&mut decoded, &record).unwrap();
            }
            let local = backend.frame_for(0).unwrap();
            let wire = decoded.frame(backend.time()).unwrap();
            assert_eq!(local, wire);
            assert_eq!(local_bot.decide(&local), wire_bot.decide(&wire));
            playing += usize::from(local.status == ClientStatus::Playing);
            let actions = (0..backend.slots())
                .map(|slot| ReferenceBot.decide(&backend.frame_for(slot).unwrap()))
                .collect::<Vec<_>>();
            backend.step_all(&actions).unwrap();
        }
        assert!(playing > 100);
        drop(sender);
        server.join().unwrap();
    });
}
