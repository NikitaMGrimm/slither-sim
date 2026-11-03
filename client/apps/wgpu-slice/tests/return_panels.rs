//! Authored packet and widget replay; no windows, sockets or external assets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{
    login_controls::{NICKNAME, SETTINGS},
    menu_layout::MenuElement as E,
    start_page::{MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::{game::LocalGame, login_native::NativeLogin};

struct Trial {
    game: LocalGame,
    login: NativeLogin,
    now: u32,
    intents: Vec<StartMenuIntent>,
}
impl Trial {
    fn new() -> Self {
        let mut game = tape::playing();
        let mut login = NativeLogin::new("Replay").unwrap();
        login.step(0., true, false).unwrap();
        login.activate(true);
        login.step(16., true, false).unwrap();
        tape::pump(&mut game, 600.);
        login.observe_game(&game).unwrap();
        login.step(600., false, false).unwrap();
        game.receive(game.generation(), &tape::bundle(&[vec![b'v', 0]]), 700.)
            .unwrap();
        login.observe_game(&game).unwrap();
        Self {
            game,
            login,
            now: 700,
            intents: Vec::new(),
        }
    }
    fn frame(&mut self) {
        self.now += 16;
        let now = f64::from(self.now);
        self.game
            .receive(self.game.generation(), &tape::bundle(&[vec![b'p']]), now)
            .unwrap();
        tape::effects(&mut self.game);
        let admission = self.game.can_play();
        self.intents
            .extend(self.login.step(now, admission, false).unwrap().0);
        self.game.begin_redraw(now, [960, 720], 1.).unwrap();
        tape::effects(&mut self.game);
        self.login.observe_game(&self.game).unwrap();
        let picture = self.game.take_frame().unwrap();
        self.login
            .prepare_host([960, 720], 1., admission, None)
            .unwrap();
        self.game.presented(true).unwrap();
        self.game.bank.recycle_frame(picture);
        self.login.sync_return_focus(self.game.can_play());
    }
    fn until(&mut self, time: u32) {
        while self.now < time {
            self.frame();
        }
    }
    fn click(&mut self, button: E) {
        let r = self.login.layout.viewport_rect(button);
        let point = [r.x + r.width / 2., r.y + r.height / 2.];
        self.login.pointer(point, Some(true), self.game.can_play());
        self.login.pointer(point, Some(false), self.game.can_play());
    }
    fn focused(&self) -> Option<egui::Id> {
        self.login.egui.context().unwrap().memory(|m| m.focused())
    }
    fn plays(&self) -> usize {
        self.intents
            .iter()
            .filter(|i| matches!(i, StartMenuIntent::Play { .. }))
            .count()
    }
    fn key(&mut self, key: egui::Key) {
        self.login.settings_input(egui::RawInput {
            events: [true, false]
                .into_iter()
                .map(|pressed| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                })
                .collect(),
            ..Default::default()
        });
    }
    fn request_close(&mut self) -> slither_wgpu_slice::game::Effect {
        self.key(egui::Key::Escape);
        assert!(self.login.take_cancel_request());
        self.login.cancel_game(&mut self.game);
        assert!(self.game.stopping());
        let close = self.game.effect().unwrap();
        assert_eq!(close.kind, slither_wgpu_slice::game::EffectKind::Close);
        close
    }
    fn pending_close_frame(&mut self, close: &slither_wgpu_slice::game::Effect) {
        self.now += 16;
        let now = f64::from(self.now);
        self.intents
            .extend(self.login.step(now, false, false).unwrap().0);
        self.game.begin_redraw(now, [960, 720], 1.).unwrap();
        self.login.observe_game(&self.game).unwrap();
        let picture = self.game.take_frame().unwrap();
        self.login
            .prepare_host([960, 720], 1., false, None)
            .unwrap();
        self.game.presented(true).unwrap();
        self.game.bank.recycle_frame(picture);
        assert_eq!(self.game.effect().unwrap().id, close.id);
    }
    fn close_panel(&mut self, panel: E) {
        if panel == E::Server {
            self.login.picker.policy.close();
        } else {
            self.login.close_settings();
        }
    }
    fn panel_visible(&self, panel: E) -> bool {
        if panel == E::Server {
            self.login.picker.policy.visible()
        } else {
            self.login.state.settings.visible()
        }
    }
    fn service_frame(&mut self) {
        // Running::service observes Close completion before the next menu step.
        self.login.observe_game(&self.game).unwrap();
        self.frame();
    }
    fn wait_for_play(&mut self) {
        let deadline = self.now + 10_000;
        loop {
            self.now += 16;
            let now = f64::from(self.now);
            self.login.observe_game(&self.game).unwrap();
            let admission = self.game.can_play();
            let intents = self.login.step(now, admission, false).unwrap().0;
            let play = intents
                .iter()
                .any(|i| matches!(i, StartMenuIntent::Play { .. }));
            self.intents.extend(intents);
            if play {
                // Running executes Play here, before observing the old game again.
                return;
            }
            assert!(self.now < deadline, "buffered Play must not be lost");
            self.game.begin_redraw(now, [960, 720], 1.).unwrap();
            tape::effects(&mut self.game);
            self.login.observe_game(&self.game).unwrap();
            let picture = self.game.take_frame().unwrap();
            self.login
                .prepare_host([960, 720], 1., admission, None)
                .unwrap();
            self.game.presented(true).unwrap();
            self.game.bank.recycle_frame(picture);
            self.login.sync_return_focus(self.game.can_play());
        }
    }
}

#[test]
fn accepted_play_consumes_close_replay_across_failed_generations() {
    use slither_wgpu_slice::game::EffectKind;
    let mut results = Vec::new();
    for panel in [E::Settings, E::Server] {
        for replacement in [false, true] {
            let mut trial = Trial::new();
            trial.until(3308);
            let close = trial.request_close();
            trial.key(egui::Key::Enter);
            // Rejected admission must leave the deliberate buffered request intact.
            assert!(trial.login.begin_play_attempt(&mut trial.game).is_err());
            trial.click(panel);
            assert!(trial.panel_visible(panel));
            for _ in 0..26 {
                trial.pending_close_frame(&close);
            }
            trial.close_panel(panel);
            while trial.panel_visible(panel) {
                trial.pending_close_frame(&close);
            }
            if replacement {
                trial.click(E::Play);
                assert_eq!(trial.login.state.view(false).phase, MenuPhase::Connecting);
            }
            trial
                .game
                .acknowledge(close.generation, close.id, true)
                .unwrap();
            trial.wait_for_play();
            assert_eq!(trial.plays(), 1);
            let old_generation = trial.game.generation();
            trial.login.begin_play_attempt(&mut trial.game).unwrap();
            trial
                .game
                .play("ws://127.0.0.1:19491/slither", "Replay", 0)
                .unwrap();
            let now = f64::from(trial.now);
            trial.game.begin_redraw(now + 1., [960, 720], 1.).unwrap();
            tape::pictures(&mut trial.game);
            let connect = trial.game.effect().unwrap();
            assert!(matches!(connect.kind, EffectKind::Connect(_)));
            assert_eq!(connect.generation, old_generation + 1);
            trial
                .game
                .construct(connect.generation, connect.id, now + 1.)
                .unwrap();
            trial
                .game
                .acknowledge(connect.generation, connect.id, true)
                .unwrap();
            // Exercise the real failure/Close owners with in-memory receipts.
            trial
                .game
                .transport_failed(connect.generation, "Authored refusal")
                .unwrap();
            let failed_close = trial.game.effect().unwrap();
            assert_eq!(failed_close.kind, EffectKind::Close);
            trial
                .game
                .acknowledge(failed_close.generation, failed_close.id, true)
                .unwrap();
            for offset in 2..=4 {
                trial.login.observe_game(&trial.game).unwrap();
                trial.intents.extend(
                    trial
                        .login
                        .step(now + f64::from(offset), trial.game.can_play(), false)
                        .unwrap()
                        .0,
                );
            }
            assert!(trial.game.can_play());
            println!(
                "{panel:?}, replacement={replacement}: total Play intents={}",
                trial.plays()
            );
            results.push(trial.plays());
        }
    }
    assert_eq!(
        results,
        [1, 1, 1, 1],
        "accepted Play must consume any older Close replay"
    );
}

#[test]
fn closing_panel_enter_and_space_never_create_a_replay_request() {
    for panel in [E::Server, E::Settings] {
        for key in [egui::Key::Space, egui::Key::Enter] {
            let mut trial = Trial::new();
            trial.until(3308);
            let close = trial.request_close();
            trial.click(panel);
            assert!(trial.panel_visible(panel));
            for _ in 0..26 {
                trial.pending_close_frame(&close);
            }
            let quality = trial.login.state.settings.preferences().quality;
            trial.key(key);
            if panel == E::Settings {
                assert_ne!(trial.login.state.settings.preferences().quality, quality);
            }
            trial
                .game
                .acknowledge(close.generation, close.id, true)
                .unwrap();
            while trial.now < 5000 {
                trial.service_frame();
            }
            assert_eq!(trial.plays(), 0);
            trial.close_panel(panel);
            assert!(trial.panel_visible(panel), "exit fade remains modal");
            while trial.panel_visible(panel) {
                trial.key(key);
                trial.service_frame();
                assert_eq!(trial.plays(), 0, "{panel:?} {key:?} cannot queue replay");
            }
            trial.service_frame();
            assert_eq!(
                trial.plays(),
                0,
                "panel input must not become Play after close"
            );
        }
    }
}

#[test]
fn closing_panel_exit_keys_never_create_a_replay_request() {
    for panel in [E::Server, E::Settings] {
        for key in [egui::Key::Space, egui::Key::Enter] {
            let mut trial = Trial::new();
            trial.until(3308);
            let close = trial.request_close();
            trial.click(panel);
            for _ in 0..26 {
                trial.pending_close_frame(&close);
            }
            trial.close_panel(panel);
            assert!(trial.panel_visible(panel));
            // Keep Close pending while the panel still owns the exit fade.
            while trial.panel_visible(panel) {
                trial.key(key);
                trial.pending_close_frame(&close);
            }
            trial
                .game
                .acknowledge(close.generation, close.id, true)
                .unwrap();
            while trial.now < 5000 {
                trial.service_frame();
            }
            assert_eq!(
                trial.plays(),
                0,
                "{panel:?} {key:?} exit cannot queue replay"
            );
        }
    }
}

#[test]
fn buffered_close_replay_waits_for_return_panels_and_their_exit_fades() {
    for panel in [E::Settings, E::Server] {
        let mut trial = Trial::new();
        trial.until(3308);
        let close = trial.request_close();
        trial.key(egui::Key::Enter); // A deliberate request on the closing login.
        trial.click(panel);
        assert!(trial.panel_visible(panel));
        for _ in 0..26 {
            trial.pending_close_frame(&close);
        }
        trial
            .game
            .acknowledge(close.generation, close.id, true)
            .unwrap();
        while trial.now < 5000 {
            trial.service_frame();
            assert_eq!(trial.plays(), 0, "replay cannot start behind {panel:?}");
            assert!(trial.panel_visible(panel));
        }
        assert_eq!(trial.login.state.view(true).phase, MenuPhase::Login);
        trial.close_panel(panel);
        while trial.panel_visible(panel) {
            trial.service_frame();
            assert_eq!(trial.plays(), 0, "replay must wait for the exit fade");
        }
        // The close completes during step; the next service seam admits replay.
        trial.service_frame();
        assert_eq!(trial.plays(), 1);
        trial.service_frame();
        assert_eq!(trial.plays(), 1, "the buffered request is consumed once");
    }
}

#[test]
fn return_panels_keep_the_session_fade_stats_and_modal_focus_then_close_once() {
    for button in [E::Server, E::Settings] {
        for close_during_return in [false, true] {
            let mut trial = Trial::new();
            let mut control = Trial::new();
            let stats = trial.login.state.view(false).last_life;
            trial.until(2300);
            assert_eq!(trial.login.state.view(false).phase, MenuPhase::DeathHold);
            for blocked in [E::Server, E::Settings, E::Skin] {
                trial.click(blocked);
                assert!(!trial.login.picker.policy.visible());
                assert!(!trial.login.state.settings.visible());
                assert!(!trial.login.studio.state.visible());
            }
            trial.until(3308);
            control.until(3308);
            assert_eq!(trial.login.state.view(false).phase, MenuPhase::Returning);
            trial.click(E::Skin);
            assert!(!trial.login.studio.state.visible());
            trial.click(button);
            let picker = button == E::Server;
            assert_eq!(trial.login.picker.policy.visible(), picker);
            assert_eq!(trial.login.state.settings.visible(), !picker);
            let modal_focus = trial.focused();
            assert!(modal_focus.is_some());
            let close_time = if close_during_return { 3500 } else { 4508 };
            let mut closed = false;
            let target =
                slither_wgpu_slice::login_egui::widget_id(if picker || close_during_return {
                    NICKNAME
                } else {
                    SETTINGS
                });
            let mut restorations = 0;
            let mut previous = modal_focus;
            let mut returned = 0;
            let mut phase = MenuPhase::Returning;
            while trial.now < 5020 {
                if !closed && trial.now >= close_time {
                    if picker {
                        trial.login.picker.policy.close();
                    } else {
                        trial.login.close_settings();
                    }
                    closed = true;
                }
                trial.frame();
                control.frame();
                let view = trial.login.state.view(false);
                assert_eq!(view.transition, control.login.state.view(false).transition);
                let actual = trial.game.login_session();
                let expected = control.game.login_session();
                assert_eq!(
                    (actual.phase, actual.death_pending, actual.return_fraction),
                    (
                        expected.phase,
                        expected.death_pending,
                        expected.return_fraction
                    )
                );
                assert_eq!(view.last_life, stats);
                if view.phase == MenuPhase::Login && phase != MenuPhase::Login {
                    returned += 1;
                }
                phase = view.phase;
                let focus = trial.focused();
                if !closed {
                    assert_eq!(
                        focus, modal_focus,
                        "return completion cannot steal modal focus"
                    );
                }
                if focus == Some(target) && previous != focus {
                    restorations += 1;
                }
                if restorations > 0 {
                    assert_eq!(
                        focus,
                        Some(target),
                        "{button:?}, early close={close_during_return}, at {}: restored focus cannot flicker",
                        trial.now
                    );
                }
                previous = focus;
            }
            assert_eq!(returned, 1);
            assert_eq!(restorations, 1);
            assert_eq!(trial.plays(), 0);
            assert!(!trial.login.picker.policy.visible());
            assert!(!trial.login.state.settings.visible());
            let text = format!("{} {}{}", stats.label, stats.number, stats.suffix);
            assert!(
                trial
                    .login
                    .egui
                    .prepared_shapes()
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == text))
            );
        }
    }
}

#[test]
fn return_panels_keep_modal_play_routing_and_queued_play_remains_once_only() {
    for button in [E::Server, E::Settings] {
        let mut trial = Trial::new();
        trial.until(3308);
        trial.click(button);
        trial.login.activate(false); // Enter belongs to the panel, as on settled Login.
        let quality = trial.login.state.settings.preferences().quality;
        if button == E::Settings {
            assert_eq!(quality, slither_client::menu_preferences::MenuQuality::Low);
        }
        assert_eq!(trial.login.state.view(false).phase, MenuPhase::Returning);
        trial.click(E::Play); // A covered login control cannot queue Play.
        trial.until(4316);
        assert_eq!(trial.plays(), 0);
        trial.login.picker.policy.close();
        trial.login.close_settings();
        trial.until(4700);
        trial.click(E::Play);
        trial.frame();
        assert_eq!(trial.plays(), 1);
        assert!(trial.intents.iter().any(
            |i| matches!(i, StartMenuIntent::Play { quality: frozen, .. } if *frozen == quality)
        ));
        trial.frame();
        assert_eq!(trial.plays(), 1);

        let mut queued = Trial::new();
        queued.until(3308);
        queued.click(E::Play);
        assert_eq!(queued.login.state.view(false).phase, MenuPhase::Connecting);
        queued.click(button);
        assert!(!queued.login.picker.policy.visible());
        assert!(!queued.login.state.settings.visible());
        queued.until(4316);
        assert_eq!(queued.plays(), 1);
        queued.frame();
        assert_eq!(queued.plays(), 1);
    }
}
