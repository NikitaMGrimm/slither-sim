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
