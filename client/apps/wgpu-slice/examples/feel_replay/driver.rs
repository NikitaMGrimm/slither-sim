//! CPU adapter for production LocalGame. No transport, window or GPU is created.
use serde_json::{Value, json};
use slither_client::{
    feel_trace::Recorder,
    protocol::{self, ElapsedHeader},
};
use slither_wgpu_slice::{
    Result,
    game::{EffectKind, LocalGame},
    render,
};
use std::collections::BTreeSet;

const ENDPOINT: &str = "ws://127.0.0.1:19090/slither";

pub struct Driver {
    pub game: LocalGame,
    pub recorder: Recorder,
    pub effects: Vec<String>,
    pub commands: Vec<Value>,
    pub receipts: Vec<Value>,
    pub acknowledged_pictures: BTreeSet<u64>,
    pub extent: [u32; 2],
    socket: Option<u32>,
    sends: usize,
    controls: bool,
}
impl Driver {
    pub fn new(width: f64, height: f64) -> Result<Self> {
        let recorder = Recorder::default();
        let mut game = LocalGame::new(0.)?;
        game.record_feel(recorder.clone());
        Ok(Self {
            game,
            recorder,
            effects: vec![],
            commands: vec![],
            receipts: vec![],
            acknowledged_pictures: BTreeSet::new(),
            extent: render::backing_size([width, height])?,
            socket: None,
            sends: 0,
            controls: false,
        })
    }
    pub fn play(&mut self) -> Result<()> {
        self.game.play(ENDPOINT, "Replay", 0)
    }
    pub fn drain_effects(&mut self, now: f64) -> Result<()> {
        for _ in 0..64 {
            let Some(e) = self.game.effect() else {
                return Ok(());
            };
            let g = e.generation;
            let kind = match &e.kind {
                EffectKind::Connect(url) => {
                    if self.socket.is_some() || url != ENDPOINT {
                        return Err("Unexpected fake transport connection".into());
                    }
                    self.game.construct(g, e.id, now)?;
                    self.socket = Some(g);
                    self.sends = 0;
                    self.controls = false;
                    self.effects.push(format!("socket-{g}-connect"));
                    "connect"
                }
                EffectKind::Send(bytes) => {
                    if self.socket != Some(g) {
                        return Err("Send without fake socket".into());
                    }
                    if self.controls && safe_command(bytes) {
                        self.commands.push(json!({"effectId": format!("socket-{g}-send-{}", self.sends),
                            "bytes": bytes, "decisionMs": now, "sendMs": now, "ackMs": now, "succeeded": true}));
                    } else {
                        // Negotiation/login bodies never leave the adapter.
                        self.effects
                            .push(format!("socket-{g}-negotiation-{}", self.sends));
                    }
                    self.sends += 1;
                    "send"
                }
                EffectKind::Close => {
                    if self.socket.take().is_some() {
                        self.effects.push(format!("socket-{g}-closed"));
                    }
                    "close"
                }
            };
            self.game.acknowledge(g, e.id, true)?;
            self.receipts
                .push(json!({"generation":g, "id":e.id.to_string(), "kind":kind, "ackMs":now}));
            if kind == "connect" {
                self.effects.push(format!("socket-{g}-opened"));
                self.game.opened(g, now)?;
            }
        }
        Err("Effect drain budget".into())
    }
    pub fn pictures(&mut self) -> Result<()> {
        for _ in 0..16 {
            let Some(_picture) = self.game.take_frame() else {
                return Ok(());
            };
            // The issued candidate is observable before its world is committed.
            let observation = self.game.issued_feel_observation().map(|row| row.sequence);
            if observation.is_none() && !self.recorder.has_failed() {
                return Err("Issued picture has no observation".into());
            }
            self.game.presented(true)?;
            if let Some(sequence) = observation
                && !self.acknowledged_pictures.insert(sequence)
            {
                return Err("Picture observation acknowledged twice".into());
            }
        }
        if self.game.has_frame() {
            Err("Picture drain budget".into())
        } else {
            Ok(())
        }
    }
    pub fn flush(&mut self, now: f64) -> Result<()> {
        self.game.observe_time(now)?;
        self.game.viewport(self.extent, 1.)?;
        self.pictures()?;
        self.drain_effects(now)
    }
    pub fn frame(&mut self, now: f64) -> Result<()> {
        self.flush(now)?;
        self.game.begin_frame(now, self.extent, 1.)?;
        self.pictures()?;
        self.drain_effects(now)?;
        self.flush(now)
    }
    pub fn packet(&mut self, bytes: &[u8], now: f64) -> Result<()> {
        let generation = self.socket.ok_or("Packet before fake socket opened")?;
        let frame =
            protocol::decode_frame(bytes, ElapsedHeader::Absent).map_err(|e| e.to_string())?;
        if frame.commands.iter().any(|c| c[0] == b'a') {
            self.controls = true;
        }
        self.game.receive(generation, bytes, now)?;
        self.drain_effects(now)?;
        self.flush(now)
    }
    pub fn close(&mut self, now: f64) -> Result<()> {
        let g = self.socket.take().ok_or("Close without fake socket")?;
        self.effects.push(format!("socket-{g}-remote-closed"));
        self.game.closed(g);
        self.flush(now)
    }
}
fn safe_command(bytes: &[u8]) -> bool {
    matches!(bytes, [b] if *b <= 254 && *b != 252)
        || matches!(bytes, [252, b] if *b != 0 && *b != 128)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn playing() -> Driver {
        let mut d = Driver::new(960., 540.).unwrap();
        d.play().unwrap();
        d.frame(0.).unwrap();
        let record: Value = serde_json::from_str(include_str!(
            "../../../../tools/feel-replay/browser-synthetic.json"
        ))
        .unwrap();
        for p in record["packets"].as_array().unwrap().iter().take(3) {
            // Test-only decoding of the authored fixture, keeping envelope bytes intact.
            let mut accumulator = 0u32;
            let mut bits = 0;
            let mut bytes = vec![];
            for c in p["body"]
                .as_str()
                .unwrap()
                .bytes()
                .take_while(|c| *c != b'=')
            {
                let unit = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
                    .iter()
                    .position(|b| *b == c)
                    .unwrap() as u32;
                accumulator = (accumulator << 6) | unit;
                bits += 6;
                if bits >= 8 {
                    bits -= 8;
                    bytes.push((accumulator >> bits) as u8);
                }
            }
            d.packet(&bytes, p["at"].as_f64().unwrap()).unwrap();
        }
        d
    }
    #[test]
    fn reset_connect_and_every_effect_are_acknowledged() {
        let mut d = Driver::new(960., 540.).unwrap();
        d.play().unwrap();
        d.frame(0.).unwrap();
        assert_eq!(d.game.feel_pending(), [0, 0]);
        assert_eq!(d.receipts.len(), 3);
        assert_eq!(
            d.effects,
            [
                "socket-1-connect",
                "socket-1-opened",
                "socket-1-negotiation-0",
                "socket-1-negotiation-1"
            ]
        );
        assert!(d.commands.is_empty());
        d.game.stop();
        d.drain_effects(1.).unwrap();
        assert_eq!(d.receipts.last().unwrap()["kind"], "close");
        assert_eq!(d.game.feel_pending(), [0, 0]);
    }
    #[test]
    fn delayed_issue_retry_and_commit_keep_the_exact_staged_snapshot() {
        let mut d = Driver::new(960., 540.).unwrap();
        d.play().unwrap();
        d.game.begin_frame(8., d.extent, 1.).unwrap();
        let committed = d.game.client().clone();
        let picture = d.game.take_frame().unwrap();
        let staged = d.game.issued_feel_observation().unwrap().clone();
        let journal = d.recorder.take();
        assert_eq!(journal.last(), Some(&staged));
        d.game.begin_frame(50., d.extent, 1.).unwrap();
        assert_eq!(d.game.client(), &committed);
        assert!(d.game.take_frame().is_none());
        assert!(d.game.effect().is_none()); // Connect waits for successful reset presentation.
        d.game.retry_frame(picture).unwrap();
        let _picture = d.game.take_frame().unwrap();
        assert_eq!(d.game.issued_feel_observation(), Some(&staged));
        assert!(d.recorder.take().is_empty());
        d.game.presented(true).unwrap();
        assert_eq!(d.game.client().draw_serial(), committed.draw_serial() + 1);
        assert!(d.recorder.take().is_empty());
        d.drain_effects(50.).unwrap();
        assert_eq!(d.game.feel_pending(), [0, 0]);
    }
    #[test]
    fn fake_transport_captures_controls_and_excludes_negotiation() {
        let mut d = playing();
        assert!(d.commands.is_empty());
        d.game.pointer(720., 216., 960., 540., false).unwrap();
        d.game.key("Space", true);
        d.frame(300.).unwrap();
        assert!(!d.commands.is_empty());
        assert!(
            d.commands
                .iter()
                .all(|c| c["bytes"].as_array().unwrap().len() <= 2 && c["succeeded"] == true)
        );
        assert_eq!(d.commands[0]["sendMs"], 300.);
        assert_eq!(d.game.feel_pending(), [0, 0]);
    }
    #[test]
    fn own_candidate_observation_matches_commit_and_retry_does_not_resample() {
        let mut d = playing();
        d.recorder.take();
        let serial = d.game.client().draw_serial();
        d.game.begin_frame(40., d.extent, 1.).unwrap();
        d.drain_effects(40.).unwrap();
        let picture = d.game.take_frame().unwrap();
        let row = d.game.issued_feel_observation().unwrap().clone();
        assert!(row.token.is_some());
        assert_eq!(d.game.client().draw_serial(), serial);
        assert_eq!(d.recorder.take().last(), Some(&row));
        d.game.begin_frame(140., d.extent, 1.).unwrap();
        assert_eq!(d.game.issued_feel_observation(), Some(&row));
        d.game.retry_frame(picture).unwrap();
        let _picture = d.game.take_frame().unwrap();
        assert_eq!(d.game.issued_feel_observation(), Some(&row));
        d.game.presented(true).unwrap();
        assert_eq!(d.game.client().draw_serial(), serial + 1);
        let view = d.game.client().control_view();
        assert_eq!(view.camera.x.to_bits(), row.values[5].to_bits());
        assert_eq!(view.camera.y.to_bits(), row.values[6].to_bits());
        assert_eq!(
            view.own.unwrap().raw_head.x.to_bits(),
            row.values[139].to_bits()
        );
        assert!(d.recorder.take().is_empty());
        assert!(d.game.presented(true).is_err());
    }
}
