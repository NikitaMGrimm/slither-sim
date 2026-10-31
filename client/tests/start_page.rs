//! Authored synthetic fixtures; no official program, assets or curve data.
//! Constant-to-test/source table: ../docs/start-menu.md (private reference citations).

use slither_client::menu_layout::{MenuElement as E, MenuLayout, MenuMetrics};
use slither_client::menu_preferences::MenuQuality;
use slither_client::presentation::{
    HudFinal, HudHistory, HudHistoryEvent, HudSample, SnakeObjectToken,
};
use slither_client::session_policy::Phase;
use slither_client::start_page::{
    MenuAction as A, MenuLocale as L, MenuPhase as P, StartMenuIntent as I, StartMenuState,
    normalize_nickname, normalize_nickname_units,
};

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
fn metrics() -> MenuMetrics {
    // Authored platform metrics, not measured official font claims.
    MenuMetrics {
        last_length_line_height: 18.,
        nickname_baseline_space: 4.,
        padded_play_label_width: 52.2,
    }
}
fn menu() -> StartMenuState {
    StartMenuState::new(L::English, false, MenuQuality::High)
}
fn capture(score: f64) -> HudFinal {
    HudHistory::default()
        .packet(&[HudHistoryEvent::FinalScore(HudSample {
            score,
            token: SnakeObjectToken::new(7).unwrap(),
        })])
        .unwrap()
        .final_capture
        .unwrap()
}
fn world(state: &mut StartMenuState, at: f64) {
    state.action(A::Play, true);
    assert!(matches!(
        state.step(at, false).unwrap().as_slice(),
        [I::Play { .. }]
    ));
    assert!(state.begin_outgoing());
    state.step(at + 500., false).unwrap();
    assert_eq!(state.view(true).phase, P::World);
}

struct ReturnClock {
    session: slither_client::session_policy::SessionPolicy,
    clock: slither_client::presentation::FrameClock,
}
impl ReturnClock {
    fn new(death: f64, last: f64) -> Self {
        use slither_client::session_policy::{Command, SessionPolicy};
        let mut session = SessionPolicy::new();
        session
            .prepare_controls(
                &[Command {
                    opcode: b'a',
                    argument: 0,
                }],
                0.,
            )
            .unwrap();
        session.commit_controls().unwrap();
        session.ui_tick(last).unwrap();
        session
            .prepare_controls(
                &[Command {
                    opcode: b'v',
                    argument: 0,
                }],
                death,
            )
            .unwrap();
        session.commit_controls().unwrap();
        Self {
            session,
            clock: slither_client::presentation::FrameClock::new(last).unwrap(),
        }
    }
    fn step(
        &mut self,
        state: &mut StartMenuState,
        now: f64,
        lagging: bool,
    ) -> Result<Vec<I>, &'static str> {
        self.session.ui_tick(now).unwrap();
        let step = self.clock.advance(now, lagging, false).unwrap();
        self.session
            .advance_step(now, step.delta, f64::from(step.whole_steps))
            .unwrap();
        state.observe_session((&self.session.view()).into(), None)?;
        state.step(now, lagging)
    }
}

#[test]
fn layout_border_boxes_and_measured_flow() {
    let layout = MenuLayout::new(1280., 720., 0., true, metrics()).unwrap();
    assert_eq!((layout.base_scale, layout.scale, layout.top), (1., 1., 0.));
    let fixtures = [
        (E::Logo, [415., 114., 450., 115.]),
        (E::LastLength, [0., 273., 1280., 18.]),
        (E::Tips, [460., 291., 360., 30.]),
        (E::NicknameHolder, [516., 355., 248., 47.]),
        (E::Nickname, [530., 367., 220., 24.]),
        (E::Play, [596.5, 426., 87., 47.]),
        (E::Skin, [20., 615., 95., 89.]),
        (E::Server, [586.5, 615., 107., 89.]),
        (E::Settings, [1165., 615., 95., 89.]),
        (E::Quality, [1170., 16., 90., 87.]),
        (E::Spinner, [576., 296., 128., 128.]),
    ];
    for (element, values) in fixtures {
        let r = layout.viewport_rect(element);
        for (actual, expected) in [r.x, r.y, r.width, r.height].into_iter().zip(values) {
            assert!((actual - expected).abs() <= 0.5, "{element:?}");
        }
    }
    let mut changed = metrics();
    changed.last_length_line_height = 21.;
    changed.nickname_baseline_space = 6.;
    changed.padded_play_label_width = 90.01;
    let changed = MenuLayout::new(1280., 720., 0., true, changed).unwrap();
    assert_eq!(changed.viewport_rect(E::Play).y, 431.);
    assert!((changed.viewport_rect(E::Play).width - 125.).abs() <= 0.5);
}

#[test]
fn resize_small_height_rounding_and_transform_origin() {
    // Base ratio remains unrounded; only combined scale and top round to 1e-5.
    for (height, p, scale, top) in [
        (560., 0., 1., 0.),
        (559., 0., 0.99821, -0.99821),
        (280., 0., 0.5, -140.),
        (40., 0., 0.08929, -36.42857),
        (720., 0.5, 1.025, 0.),
        (280., 0.5, 0.5125, -140.),
        (720., 1., 1.1, 0.),
    ] {
        let layout = MenuLayout::new(960., height, p, true, metrics()).unwrap();
        assert_eq!(layout.scale, scale);
        assert_eq!(layout.top, top);
        let center = layout.to_viewport(480., height / 2.);
        near(center.0, 480.);
        near(center.1, 64. + top + height / 2.);
        assert_eq!(layout.viewport_rect(E::Quality).y, 16.);
    }
    let small = MenuLayout::new(960., 280., 0., true, metrics()).unwrap();
    assert_eq!(small.viewport_rect(E::Logo).y, 19.);
    assert_eq!(small.viewport_rect(E::Play).y, 175.);
    let wide = MenuLayout::new(1920., 280., 0., true, metrics()).unwrap();
    near(
        wide.viewport_rect(E::Play).x - small.viewport_rect(E::Play).x,
        480.,
    );
    for invalid in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(MenuLayout::new(invalid, 720., 0., true, metrics()).is_err());
        assert!(MenuLayout::new(1280., invalid, 0., true, metrics()).is_err());
    }
}

#[test]
fn inverse_hit_test_round_trips() {
    for (w, h) in [(1280., 720.), (960., 540.), (320., 180.), (200., 40.)] {
        for p in [0., 0.5, 1.] {
            let layout = MenuLayout::new(w, h, p, true, metrics()).unwrap();
            for e in [
                E::Logo,
                E::LastLength,
                E::Tips,
                E::NicknameHolder,
                E::Nickname,
                E::Play,
                E::Skin,
                E::Server,
                E::Quality,
            ] {
                let r = layout.viewport_rect(e);
                assert!(layout.hit_test(e, r.x + r.width / 2., r.y + r.height / 2.));
                assert!(!layout.hit_test(e, r.x - 1., r.y - 1.));
            }
            for (x, y) in [(0., 0.), (50., 355.), (500., 220.)] {
                let v = layout.to_viewport(x, y);
                let l = layout.to_login(v.0, v.1);
                near(l.0, x);
                near(l.1, y);
            }
        }
    }
    let layout = MenuLayout::new(1280., 720., 0., true, metrics()).unwrap();
    let r = layout.viewport_rect(E::Play);
    assert!(!layout.hit_test(E::Play, r.x + r.width, r.y));
}

#[test]
fn translations_and_source_order() {
    let fixtures = [
        (
            "en",
            L::English,
            "Play",
            "Play Again",
            "Nickname",
            "Your final length was",
            [
                "Eat to grow longer!",
                "Don't run into other players!",
                "When longer, hold the mouse for a speed boost!",
            ],
        ),
        (
            "de",
            L::German,
            "Spielen",
            "Play Again",
            "Nickname",
            "Deine endgültige Länge war",
            [
                "Esse um zu wachsen!",
                "Klicke für mehr Geschwindigkeit!",
                "Bewege dich nicht in andere Schlangen!",
            ],
        ),
        (
            "fr",
            L::French,
            "Jouer",
            "Jouer",
            "Surnom",
            "Votre longueur finale était de",
            [
                "Mange pour croître !",
                "Clique pour courir !",
                "Ne laissez pas votre tête toucher d'autres serpents !",
            ],
        ),
        (
            "pt",
            L::Portuguese,
            "Joga",
            "Joga",
            "Apelido",
            "Seu comprimento final foi de",
            [
                "Coma para crescer!",
                "Clique para correr!",
                "Não deixe que sua cabeça para tocar outras cobras!",
            ],
        ),
    ];
    for (code, locale, play, again, nickname, final_length, tips) in fixtures {
        assert_eq!(L::from_code(code), locale);
        let text = locale.wording();
        assert_eq!(
            (
                text.play,
                text.play_again,
                text.nickname,
                text.final_length,
                text.tips
            ),
            (play, again, nickname, final_length, tips)
        );
    }
    assert_eq!(L::from_code("unknown"), L::English);
}

#[test]
fn nickname_utf16_del_surrogates_and_limit() {
    assert_eq!(
        normalize_nickname("A\u{1f}\u{20}\u{7f}\u{80}é😀Z"),
        "A  \u{7f}    Z"
    );
    assert_eq!(
        normalize_nickname_units(&[31, 32, 126, 127, 128, 0xd800, 0xdc00]),
        "  ~\u{7f}   "
    );
    assert_eq!(
        normalize_nickname(&format!("{}😀Z", "a".repeat(23))),
        format!("{} ", "a".repeat(23))
    );
    assert_eq!(normalize_nickname(&"😀".repeat(20)), " ".repeat(24));
    let mut state = menu();
    state.edit_nickname(&"a😀é".encode_utf16().collect::<Vec<_>>());
    assert_eq!(state.view(true).nickname, "a   ");
    state.action(A::Play, true);
    state.edit_nickname(&[b'x' as u16]);
    assert_eq!(state.view(true).nickname, "a   ");
}

#[test]
fn logo_clock_checkpoints_are_not_clamped_or_lagged() {
    let mut state = StartMenuState::new(L::English, true, MenuQuality::High);
    let first = state.view(true);
    assert_eq!(first.tip_alpha, 0.);
    assert_eq!(
        (
            first.logo_alpha,
            first.logo_reveal,
            first.nickname_alpha,
            first.skin_alpha,
            first.play_alpha
        ),
        (0., 0., 0., 0., 0.)
    );
    state.step(25., true).unwrap();
    let v = state.view(true);
    near(v.logo_alpha, 0.05);
    near(v.logo_reveal, 0.00375);
    near(v.nickname_alpha, 0.036);
    near(v.skin_alpha, 0.);
    near(v.play_alpha, 0.);
    state.step(250., true).unwrap();
    let v = state.view(true);
    near(v.logo_alpha, 0.5);
    near(v.logo_reveal, 0.0375);
    near(v.nickname_alpha, 0.36);
    near(v.skin_alpha, 0.05);
    near(v.play_alpha, 0.25);
    state.step(10_000., true).unwrap();
    let v = state.view(true);
    assert_eq!(
        (
            v.logo_alpha,
            v.logo_reveal,
            v.nickname_alpha,
            v.skin_alpha,
            v.play_alpha
        ),
        (1., 1., 1., 1., 1.)
    );
    assert_eq!(menu().view(true).logo_reveal, 1.);
}

#[test]
fn tip_cycle_clamp_lag_and_rounding() {
    let mut state = menu();
    for step in 1..=51 {
        state.step(f64::from(step) * 40., false).unwrap();
    }
    assert_eq!(state.view(true).tip, "\u{a0}");
    state.step(2080., false).unwrap();
    assert_eq!(state.view(true).tip, "Eat to grow longer!");
    near(state.view(true).tip_alpha, 0.01841);
    for step in 53..=126 {
        state.step(f64::from(step) * 40., false).unwrap();
    }
    assert_eq!(state.view(true).tip, "Don't run into other players!");
    for step in 127..=200 {
        state.step(f64::from(step) * 40., false).unwrap();
    }
    assert_eq!(
        state.view(true).tip,
        "When longer, hold the mouse for a speed boost!"
    );
    for step in 201..=274 {
        state.step(f64::from(step) * 40., false).unwrap();
    }
    assert_eq!(state.view(true).tip, "Eat to grow longer!");
    let mut slow = menu();
    slow.step(8000., true).unwrap();
    // Clamp 1000 units to 5, then damp to 4.25; phase 1.97225.
    near(slow.view(true).tip_alpha, 0.83389);
    let before = slow.view(true).tip_alpha;
    slow.step(7999., false).unwrap();
    assert_eq!(slow.view(true).tip_alpha, before); // negative elapsed clamps to zero
    let before = slow.clone();
    assert!(slow.step(f64::NAN, false).is_err());
    assert_eq!(slow, before);
}

#[test]
fn overlays_are_per_callback_and_disabled_layers_are_zero() {
    let mut state = menu();
    state.play_pointer(true, true, true);
    for (time, expected) in [(1., 0.33), (2., 0.66), (5000., 0.99), (5001., 1.)] {
        state.step(time, true).unwrap();
        near(state.view(true).hover_alpha, expected);
        near(state.view(true).press_alpha, expected);
    }
    assert_eq!(state.view(false).hover_alpha, 0.);
    state.play_pointer(false, false, true);
    state.step(5002., false).unwrap();
    near(state.view(true).hover_alpha, 0.8);
    state.action(A::Play, true);
    assert_eq!(state.view(true).press_alpha, 0.);
}

#[test]
fn click_once_deferred_admission_focus_and_links() {
    let mut state = menu();
    assert_eq!(state.step(0., false).unwrap(), vec![I::FocusNickname]);
    assert!(state.step(0., false).unwrap().is_empty());
    assert_eq!(
        state.action(A::FocusNickname, false),
        Some(I::FocusNickname)
    );
    assert_eq!(state.action(A::Skin, false), Some(I::OpenSkinChooser));
    assert_eq!(state.action(A::Server, false), Some(I::OpenServerPicker));
    assert_eq!(state.action(A::Enter { composing: true }, true), None);
    state.action(A::Play, false);
    assert_eq!(state.view(true).phase, P::Login);
    state.action(A::Enter { composing: false }, true);
    assert!(!state.begin_outgoing()); // request must reach its later update first
    assert!(state.view(true).disabled && state.view(true).spinner_visible);
    for action in [
        A::Play,
        A::Enter { composing: false },
        A::Skin,
        A::Server,
        A::FocusNickname,
        A::Quality,
    ] {
        assert_eq!(state.action(action, true), None);
    }
    assert_eq!(
        state.step(8., false).unwrap(),
        vec![I::Play {
            nickname: String::new(),
            quality: MenuQuality::High
        }]
    );
    assert!(state.step(16., false).unwrap().is_empty());
}

#[test]
fn early_death_does_not_stop_the_independent_outgoing_clock() {
    let mut state = menu();
    state.action(A::Play, true);
    state.step(0., false).unwrap();
    state.begin_outgoing();
    state.step(100., false).unwrap();
    state
        .accepted_game_over(Phase::Dead, Some(capture(5.)))
        .unwrap();
    let mut clock = ReturnClock::new(100., 100.);
    assert!(state.view(true).visible);
    clock.step(&mut state, 500., false).unwrap();
    assert_eq!(state.view(true).phase, P::DeathHold);
    assert_eq!(state.view(true).transition, 1.);
    assert!(!state.view(true).visible);
    clock.step(&mut state, 1700., false).unwrap();
    assert_eq!(state.view(true).phase, P::DeathHold);
    clock.step(&mut state, 1701., false).unwrap();
    assert_eq!(state.view(true).phase, P::Returning);
}

#[test]
fn spinner_uses_unlagged_clamped_clock_and_fades_out() {
    let mut state = menu();
    state.action(A::Play, true);
    state.step(8000., true).unwrap();
    let v = state.view(true);
    assert_eq!(v.spinner_time, 5.);
    near(v.spinner_alpha, 5. / 86.);
    near(v.spinner_scale, 0.13424);
    for step in 1..=18 {
        state.step(8000. + f64::from(step) * 40., true).unwrap();
    }
    let v = state.view(true);
    assert_eq!(v.spinner_alpha, 1.);
    near(v.spinner_scale, 0.9);
    assert!(state.begin_outgoing());
    state.step(8760., false).unwrap();
    near(state.view(true).spinner_alpha, 81. / 86.);
    for step in 1..=18 {
        state.step(8760. + f64::from(step) * 40., false).unwrap();
    }
    assert!(!state.view(true).spinner_visible);
}

#[test]
fn outgoing_elapsed_clock_and_rounding() {
    let mut state = menu();
    state.action(A::Play, true);
    state.step(0., false).unwrap();
    assert!(state.begin_outgoing());
    state.step(250., true).unwrap();
    let v = state.view(true);
    assert_eq!(
        (v.transition, v.login_alpha, v.world_alpha),
        (0.5, 0.5, 0.5)
    );
    let layout = MenuLayout::new(960., 540., v.transition, v.tips_visible, metrics()).unwrap();
    near(layout.scale, 0.98839);
    state.step(500., true).unwrap();
    assert_eq!(state.view(true).phase, P::World);
    assert!(!state.view(true).visible);
    assert_eq!(state.view(true).tip_alpha, 0.);
}

#[test]
fn lengths_1000_and_1001_and_translated_replay() {
    for locale in [L::English, L::German, L::French, L::Portuguese] {
        for (score, number, suffix) in [(1000., "1000", ""), (1001., "1001", "!")] {
            let mut state = StartMenuState::new(locale, false, MenuQuality::High);
            world(&mut state, 0.);
            assert!(
                state
                    .accepted_game_over(Phase::Dead, Some(capture(score)))
                    .unwrap()
            );
            let v = state.view(true);
            assert_eq!(v.last_life.label, locale.wording().final_length);
            assert_eq!(v.last_life.number, number);
            assert_eq!(v.last_life.suffix, suffix);
            assert_eq!(v.last_life.label_alpha, 0.45);
            assert_eq!(v.play_label, locale.wording().play_again);
            assert!(!v.visible);
        }
    }
}

#[test]
fn strict_return_boundary_lag_floor_recovery_and_focus() {
    let mut state = menu();
    world(&mut state, 0.);
    state
        .accepted_game_over(Phase::Dead, Some(capture(1001.)))
        .unwrap();
    let mut clock = ReturnClock::new(500., 500.);
    clock.step(&mut state, 2100., false).unwrap();
    assert_eq!(state.view(true).phase, P::DeathHold);
    state.action(A::Play, true);
    clock.step(&mut state, 2101., false).unwrap();
    let v = state.view(true);
    assert_eq!(v.phase, P::Returning);
    assert!(v.visible && v.disabled);
    near(v.transition, 0.9995);
    near(v.play_return_alpha, 0.5);
    clock.step(&mut state, 2141., true).unwrap();
    near(state.view(true).transition, 0.9825); // .004 * 5 * .85
    let mut expected = 0.9825;
    let mut lag: f64 = 0.85;
    for step in 1..=12 {
        lag = (lag * 0.85).max(0.2);
        expected -= 0.004 * 5. * lag;
        clock
            .step(&mut state, 2141. + f64::from(step) * 40., true)
            .unwrap();
        near(state.view(true).transition, expected);
    }
    clock.step(&mut state, 2661., false).unwrap();
    expected -= 0.004 * 5. * 0.25;
    near(state.view(true).transition, expected);
    let mut focus_count = 0;
    for step in 1..=100 {
        focus_count += clock
            .step(&mut state, 2661. + f64::from(step) * 40., false)
            .unwrap()
            .iter()
            .filter(|i| **i == I::FocusNickname)
            .count();
    }
    assert_eq!(focus_count, 1);
    assert_eq!(state.view(true).phase, P::Login);
    assert!(!state.view(true).disabled);
    assert!(state.view(false).disabled);
}

#[test]
fn early_return_play_locks_but_waits_for_return_and_admission() {
    let mut state = menu();
    state.edit_nickname(&"Stored".encode_utf16().collect::<Vec<_>>());
    world(&mut state, 0.);
    state
        .accepted_game_over(Phase::Dead, Some(capture(1001.)))
        .unwrap();
    state.action(A::Play, false); // Hidden death hold cannot queue a request.
    assert!(
        state
            .step_with_admission(2100., false, false)
            .unwrap()
            .is_empty()
    );
    let mut clock = ReturnClock::new(500., 500.);
    clock.step(&mut state, 2101., false).unwrap();
    let returning = state.view(false);
    assert!(returning.visible && returning.disabled && !returning.play_disabled);
    state.action(A::Enter { composing: false }, false); // Old connection is live.
    assert_eq!(state.view(false).phase, P::Returning);
    state.action(A::Play, false);
    state.action(A::Play, false);
    state.edit_nickname(&"Changed".encode_utf16().collect::<Vec<_>>());
    let queued = state.view(false);
    assert_eq!(queued.phase, P::Connecting);
    assert!(queued.disabled && queued.play_disabled && queued.spinner_visible);
    assert_eq!(queued.nickname, "Stored");
    near(queued.transition, returning.transition);
    near(queued.play_return_alpha, returning.play_return_alpha);
    assert!(
        state
            .step_with_admission(2117., false, false)
            .unwrap()
            .is_empty()
    );
    let session = |fraction, pending| slither_client::start_page::MenuSession {
        phase: Phase::Disconnected,
        outcome: Some(Phase::Dead),
        playing: false,
        death_pending: pending,
        return_active: pending,
        return_fraction: fraction,
    };
    state.observe_session(session(0.4, true), None).unwrap();
    near(state.view(false).transition, 0.4);
    assert!(
        state
            .step_with_admission(2200., false, true)
            .unwrap()
            .is_empty()
    );
    assert!(state.observe_session(session(0., false), None).unwrap());
    assert!(!state.observe_session(session(0., false), None).unwrap());
    assert!(
        state
            .step_with_admission(2300., false, false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state.step_with_admission(2316., false, true).unwrap(),
        vec![I::Play {
            nickname: "Stored".into(),
            quality: MenuQuality::High
        }]
    );
    assert!(
        state
            .step_with_admission(2332., false, true)
            .unwrap()
            .is_empty()
    );
    assert!(state.begin_outgoing());
    near(state.view(true).transition, 0.);
    state.step(2348., false).unwrap();
    near(state.view(true).transition, 0.032);
}

#[test]
fn terminal_return_enter_can_queue_and_cancel_preserves_the_fade() {
    let mut state = menu();
    world(&mut state, 0.);
    state.accepted_game_over(Phase::Dead, None).unwrap();
    let session = slither_client::start_page::MenuSession {
        phase: Phase::Disconnected,
        outcome: Some(Phase::Dead),
        playing: false,
        death_pending: true,
        return_active: true,
        return_fraction: 0.6,
    };
    state.observe_session(session, None).unwrap();
    state.action(A::Enter { composing: false }, false);
    assert_eq!(state.view(false).phase, P::Connecting);
    state.cancel_pending_play();
    let view = state.view(false);
    assert_eq!(view.phase, P::Returning);
    near(view.transition, 0.6);
    assert!(view.disabled && !view.play_disabled && !view.spinner_visible);
    assert!(
        state
            .step_with_admission(2200., false, true)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn persistent_last_life_second_life_and_disconnect_has_no_result_authority() {
    let mut state = menu();
    assert_eq!(state.view(true).last_life.number, "\u{a0}");
    assert!(
        !state
            .accepted_game_over(Phase::Disconnected, Some(capture(99.)))
            .unwrap()
    );
    state.disconnected();
    assert_eq!(state.view(true).last_life.number, "\u{a0}");
    world(&mut state, 0.);
    state
        .accepted_game_over(Phase::Dead, Some(capture(1234567.)))
        .unwrap();
    let mut clock = ReturnClock::new(500., 500.);
    let retained = state.view(true).last_life;
    assert!(
        !state
            .accepted_game_over(Phase::Dead, Some(capture(2.)))
            .unwrap()
    );
    state.disconnected(); // accepted outcome and hold survive close
    assert_eq!(state.view(true).phase, P::DeathHold);
    for step in 0..=51 {
        clock
            .step(&mut state, 2101. + f64::from(step) * 40., false)
            .unwrap();
    }
    assert_eq!(state.view(true).phase, P::Login);
    assert_eq!(state.view(true).last_life, retained);
    world(&mut state, 4200.);
    assert_eq!(state.view(true).last_life, retained);
    state.disconnected();
    assert_eq!(state.view(true).last_life, retained);
    assert_eq!(state.view(true).play_label, "Play Again");
    world(&mut state, 4800.);
    state
        .accepted_game_over(Phase::Dead, Some(capture(88.)))
        .unwrap();
    assert_eq!(state.view(true).last_life.number, "88");
}

#[test]
fn champion_suppresses_ordinary_controls_without_fake_editor() {
    let mut state = menu();
    world(&mut state, 0.);
    state
        .accepted_game_over(Phase::Victory, Some(capture(1001.)))
        .unwrap();
    let mut clock = ReturnClock::new(500., 500.);
    for step in 0..=51 {
        clock
            .step(&mut state, 2101. + f64::from(step) * 40., false)
            .unwrap();
        assert!(state.view(false).play_disabled);
        state.action(A::Play, false);
        state.action(A::Enter { composing: false }, false);
        assert_ne!(state.view(false).phase, P::Connecting);
    }
    let v = state.view(true);
    assert!(v.champion_pending && v.disabled);
    assert!(!v.ordinary_controls_visible);
    assert_eq!(state.action(A::Enter { composing: false }, true), None);
}

#[test]
fn quality_memory_survives_missing_storage_and_snapshots_on_play() {
    for (stored, quality) in [
        (None, MenuQuality::High),
        (Some("1"), MenuQuality::High),
        (Some("bad"), MenuQuality::High),
        (Some("0"), MenuQuality::Low),
    ] {
        assert_eq!(MenuQuality::restore(stored), quality);
    }
    let mut state = menu();
    assert_eq!(state.action(A::Quality, true), Some(I::PersistQuality("0")));
    // Intentionally do not execute the storage intent: memory remains low.
    assert_eq!(state.view(true).quality, MenuQuality::Low);
    state.action(A::Play, true);
    assert_eq!(
        state.step(8., false).unwrap(),
        vec![I::Play {
            nickname: String::new(),
            quality: MenuQuality::Low
        }]
    );
    state.disconnected();
    assert_eq!(state.action(A::Quality, true), Some(I::PersistQuality("1")));
}

#[test]
fn round1_return_removes_tips_from_flow_and_hits() {
    let mut state = menu();
    let initial = state.view(true);
    assert!(initial.tips_visible);
    let mut layout = MenuLayout::new(
        1280.,
        720.,
        initial.transition,
        initial.tips_visible,
        metrics(),
    )
    .unwrap();
    assert_eq!(layout.viewport_rect(E::Play).y, 426.);
    world(&mut state, 0.);
    state
        .accepted_game_over(Phase::Dead, Some(capture(1001.)))
        .unwrap();
    let mut clock = ReturnClock::new(500., 500.);
    for frame in 0..=51 {
        clock
            .step(&mut state, 2101. + f64::from(frame) * 40., false)
            .unwrap();
        let view = state.view(true);
        assert!(!view.tips_visible);
        layout
            .update(view.transition, view.tips_visible, metrics())
            .unwrap();
        let tips = layout.viewport_rect(E::Tips);
        assert_eq!(tips.height, 0.);
        assert!(!layout.hit_test(E::Tips, tips.x + tips.width / 2., tips.y));
        let play = layout.viewport_rect(E::Play);
        assert!(layout.hit_test(E::Play, play.x + play.width / 2., play.y + play.height / 2.));
    }
    assert_eq!(state.view(true).phase, P::Login);
    assert_eq!(layout.viewport_rect(E::Play).y, 396.);
    assert_eq!(layout.viewport_rect(E::NicknameHolder).y, 325.);
    assert!(layout.hit_test(E::Play, 640., 400.));
    assert!(!layout.hit_test(E::Play, 640., 460.)); // old row's lower hit area
    state.disconnected();
    assert!(!state.view(true).tips_visible);
    state.action(A::Play, true);
    assert!(!state.view(true).tips_visible); // never restored on a second life
}

#[test]
fn round1_animation_retains_resize_top_when_scale_crosses_one() {
    let mut layout = MenuLayout::new(1280., 540., 0., true, metrics()).unwrap();
    assert_eq!(layout.top, -19.28571);
    for progress in [0.6085, 0.60858, 0.6087, 1., 0.60858, 0.] {
        layout.update(progress, true, metrics()).unwrap();
        assert_eq!(layout.top, -19.28571);
        let r = layout.viewport_rect(E::Nickname);
        assert!(layout.hit_test(E::Nickname, r.x + r.width / 2., r.y + r.height / 2.));
        if progress == 0.60858 {
            assert_eq!(layout.scale, 1.);
            near(layout.viewport_rect(E::Logo).y, 94.71429);
        }
    }
    // An actual resize at scale 1 *does* clear top, and later animation retains it.
    let mut resized = MenuLayout::new(1280., 540., 0.60858, false, metrics()).unwrap();
    assert_eq!(resized.top, 0.);
    resized.update(0.5, false, metrics()).unwrap();
    assert_eq!(resized.top, 0.);
    let before = resized.clone();
    assert!(resized.update(f64::NAN, false, metrics()).is_err());
    assert_eq!(resized, before);
}

#[test]
fn round1_disabled_pointer_rejects_activation_but_accepts_releases() {
    let mut state = menu();
    state.action(A::Play, true);
    state.play_pointer(true, true, true); // disabled by the connection attempt
    state.step(8., false).unwrap();
    state.disconnected();
    assert_eq!(state.view(true).hover_alpha, 0.);
    assert_eq!(state.view(true).press_alpha, 0.);

    state.play_pointer(true, true, false); // disabled only by admission availability
    state.step(16., false).unwrap();
    assert_eq!(state.view(true).hover_alpha, 0.);
    assert_eq!(state.view(true).press_alpha, 0.);

    state.play_pointer(true, true, true);
    state.step(24., false).unwrap();
    near(state.view(true).hover_alpha, 0.33);
    near(state.view(true).press_alpha, 0.33);
    state.action(A::Play, true);
    state.play_pointer(false, false, false); // releases must clear retained input
    for now in [32., 40., 48.] {
        state.step(now, false).unwrap();
    }
    state.disconnected();
    assert_eq!(state.view(true).hover_alpha, 0.);
    assert_eq!(state.view(true).press_alpha, 0.);
}

#[test]
fn round1_label_width_rounds_offset_width_before_adding_padding() {
    // Old integer measurements remain the baseline; transforms and hits use
    // the fractional rectangle under resize and animation.
    for (measured, expected) in [
        (52., 87.),
        (52.2, 87.),
        (52.49, 87.),
        (52.5, 88.),
        (52.51, 88.),
        (90.01, 125.),
    ] {
        let mut m = metrics();
        m.padded_play_label_width = measured;
        for (w, h, progress) in [(1280., 720., 0.), (960., 280., 0.5), (320., 180., 1.)] {
            let mut layout = MenuLayout::new(w, h, progress, true, m).unwrap();
            assert!((layout.logical_rect(E::Play).width - expected).abs() <= 0.5);
            let mut old_metrics = m;
            old_metrics.padded_play_label_width = measured.round();
            let old = MenuLayout::new(w, h, progress, true, old_metrics)
                .unwrap()
                .viewport_rect(E::Play);
            let current = layout.viewport_rect(E::Play);
            for (actual, baseline) in [
                (current.x, old.x),
                (current.x + current.width, old.x + old.width),
                (current.y, old.y),
                (current.y + current.height, old.y + old.height),
            ] {
                assert!((actual - baseline).abs() <= 2.);
            }
            assert!(layout.hit_test(E::Play, current.x + 0.01, current.y + 0.01));
            assert!(!layout.hit_test(E::Play, current.x + current.width + 0.01, current.y));
            let mut replay = m;
            replay.padded_play_label_width = 110.5;
            layout.update(progress, false, replay).unwrap();
            assert!((layout.logical_rect(E::Play).width - 146.).abs() <= 0.5);
        }
    }
}
