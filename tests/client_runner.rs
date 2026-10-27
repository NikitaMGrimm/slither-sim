#![cfg(feature = "networking")]

use slither_sim::{
    Action,
    client::{Backend, Bot, BotRunner, CONTRACT, ClientFrame, ClientStatus},
};
use std::collections::VecDeque;

struct FakeBackend {
    frames: VecDeque<ClientFrame>,
    actions: Vec<Action>,
    processing: Vec<f64>,
}

impl Backend for FakeBackend {
    fn next_frame(&mut self) -> Result<ClientFrame, String> {
        self.frames.pop_front().ok_or("No more frames".into())
    }
    fn send_action(&mut self, action: Action) -> Result<(), String> {
        self.actions.push(action);
        Ok(())
    }
    fn reconnect(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        self.processing.push(seconds);
        Ok(())
    }
}

#[derive(Default)]
struct CountingBot {
    resets: usize,
    decisions: usize,
}
impl Bot for CountingBot {
    fn reset(&mut self) {
        self.resets += 1;
    }
    fn decide(&mut self, _: &ClientFrame) -> Option<Action> {
        self.decisions += 1;
        Some(Action {
            heading: 1.,
            boost: false,
        })
    }
}

fn frame(status: ClientStatus, generation: u64, life: u64) -> ClientFrame {
    let received = slither_sim::client::ReceivedMetadata {
        connection_generation: generation,
        ..Default::default()
    };
    ClientFrame {
        rules: None,
        contract: CONTRACT.into(),
        time: 0.,
        status,
        life,
        settings: None,
        observation: None,
        received,
        minimap: None,
        timing: Default::default(),
        hud: Default::default(),
    }
}

#[test]
fn recurrent_state_resets_on_life_connection_and_departure_from_play() {
    use ClientStatus::*;
    let frames = [
        (Waiting, 1, 0),
        (Playing, 1, 1),
        (Playing, 1, 1),
        (Dead, 1, 1),
        (Dead, 1, 1),
        (Playing, 1, 2),
        (Disconnected, 1, 2),
        (Waiting, 2, 2),
        (Playing, 2, 3),
    ];
    let backend = FakeBackend {
        frames: frames.into_iter().map(|(s, g, l)| frame(s, g, l)).collect(),
        actions: vec![],
        processing: vec![],
    };
    let mut runner = BotRunner::new(backend);
    let mut bot = CountingBot::default();
    for _ in frames {
        runner.step(&mut bot).unwrap();
    }
    assert_eq!(bot.resets, 7);
    assert_eq!(bot.decisions, 4);
    assert_eq!(runner.backend_mut().actions.len(), 4);
    assert!(
        runner
            .backend_mut()
            .processing
            .iter()
            .all(|s| s.is_finite() && *s >= 0.)
    );
}

#[test]
fn incompatible_contract_never_reaches_bot_or_action_encoder() {
    assert_eq!(CONTRACT, "slither-client-v2");
    let mut previous = frame(ClientStatus::Playing, 1, 1);
    previous.contract = "slither-client-v1".into();
    let mut incompatible = frame(ClientStatus::Playing, 1, 1);
    incompatible.contract = "future-incompatible".into();
    let backend = FakeBackend {
        frames: [previous, incompatible].into(),
        actions: vec![],
        processing: vec![],
    };
    let mut runner = BotRunner::new(backend);
    let mut bot = CountingBot::default();
    assert!(runner.step(&mut bot).is_err());
    assert!(runner.step(&mut bot).is_err());
    assert_eq!(bot.decisions, 0);
    assert!(runner.backend_mut().actions.is_empty());
}

#[test]
fn invalid_bot_actions_do_not_record_processing() {
    struct InvalidBot(f64);
    impl Bot for InvalidBot {
        fn decide(&mut self, _: &ClientFrame) -> Option<Action> {
            Some(Action {
                heading: self.0,
                boost: false,
            })
        }
    }
    for heading in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let backend = FakeBackend {
            frames: [
                frame(ClientStatus::Playing, 1, 1),
                frame(ClientStatus::Playing, 1, 1),
            ]
            .into(),
            actions: vec![],
            processing: vec![],
        };
        let mut runner = BotRunner::new(backend);
        assert_eq!(
            runner.step(&mut InvalidBot(heading)).unwrap_err(),
            "Action heading must be finite"
        );
        assert!(runner.backend_mut().processing.is_empty());
        assert!(runner.backend_mut().actions.is_empty());
        runner.step(&mut CountingBot::default()).unwrap();
        assert_eq!(runner.backend_mut().processing.len(), 1);
        assert_eq!(runner.backend_mut().actions.len(), 1);
    }
}
