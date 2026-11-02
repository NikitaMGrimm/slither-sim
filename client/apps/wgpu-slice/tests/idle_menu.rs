//! Synthetic menu/scheduler presents at a CPU sink: no window, GPU or sockets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "../src/scheduling.rs"]
mod scheduling;
#[path = "support/announcement_tape.rs"]
mod tape;

use scheduling::Schedule;
use slither_client::{session_policy::Phase, start_page::MenuAction};
use slither_wgpu_slice::{egui_native::NativeEguiHost, login_native::NativeLogin};
use std::time::{Duration, Instant};

fn returned() -> NativeLogin {
    let mut login = NativeLogin::new("Player").unwrap();
    login.state.action(MenuAction::Play, true);
    login.step(8., true, false).unwrap();
    assert!(login.state.begin_outgoing());
    for now in (24..=10008).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    login.state.disconnected();
    assert!(!login.state.view(true).tips_visible);
    login
}

/// Mirrors the old visibility demand against the new menu decision, servicing
/// 600 synthetic 60-Hz opportunities over [0, 10s). A successful scheduled draw
/// is acknowledged as a present; egui, GPU work and actual vsync are excluded.
fn idle_presents(mut login: NativeLogin, old: bool) -> u32 {
    let mut schedule = Schedule::default();
    let start = Instant::now();
    // Initial picture is outside the idle interval.
    schedule.activity(if old {
        login.state.view(true).visible
    } else {
        login.needs_redraw()
    });
    assert!(schedule.request_redraw(start));
    assert!(schedule.begin_redraw(start));
    schedule.presented();
    let mut count = 0;
    for tick in 0..600 {
        let now = start + Duration::from_secs_f64(f64::from(tick) / 60.);
        schedule.activity(if old {
            login.state.view(true).visible
        } else {
            login.needs_redraw()
        });
        if schedule.request_redraw(now) {
            assert!(schedule.begin_redraw(now));
            login
                .step(11000. + f64::from(tick) * 1000. / 60., true, false)
                .unwrap();
            schedule.presented();
            count += 1;
        }
    }
    count
}

#[test]
fn fixed_idle_interval_counts_static_and_animated_menus() {
    for (name, expected, settings, initial) in [
        ("returned login", 0, false, false),
        ("Settings", 0, true, true),
        ("initial login tips", 600, false, true),
    ] {
        let build = || {
            let mut login = if initial {
                NativeLogin::new("Player").unwrap()
            } else {
                returned()
            };
            if settings {
                assert!(login.state.settings.open(&login.state.view(true)));
            }
            for now in (10016..=10992).step_by(16) {
                login.step(f64::from(now), true, false).unwrap();
            }
            login
        };
        let before = idle_presents(build(), true);
        let after = idle_presents(build(), false);
        println!("{name}: 10s synthetic idle presents {before} -> {after}");
        assert_eq!(before, 600);
        assert_eq!(after, expected);
    }
}

#[test]
fn fades_spinner_picker_and_studio_keep_demand_until_settled() {
    let mut login = returned();
    assert!(!login.needs_redraw());
    assert!(login.state.settings.open(&login.state.view(true)));
    assert!(login.needs_redraw()); // alpha is still zero at entry
    for now in (10016..=11008).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    assert!(!login.needs_redraw());
    login.state.settings.close();
    assert!(login.needs_redraw()); // alpha is still one at exit
    for now in (11024..=12000).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    assert!(!login.needs_redraw());
    login.picker.policy.open();
    assert!(login.needs_redraw());
    for now in (12016..=13008).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    assert!(!login.needs_redraw());
    login.picker.policy.close();
    assert!(login.needs_redraw());
    for now in (13024..=14000).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    login.open_studio();
    for now in (14016..=15008).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    assert!(login.needs_redraw()); // moving snake, even after the panel fade
    login.studio.state.cancel(15008.);
    for now in (15024..=16000).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    login.state.action(MenuAction::Play, true);
    assert!(login.needs_redraw()); // spinner before transport construction
    login.state.cancel_pending_play();
    assert!(!login.needs_redraw());
}

#[test]
fn input_after_a_long_idle_starts_studio_fades_on_the_event_clock() {
    let mut login = returned();
    assert!(!login.needs_redraw());
    login.input_time(60000.);
    login.open_studio();
    login.step(60016., true, false).unwrap();
    assert!(login.studio.state.alpha() > 0. && login.studio.state.alpha() < 1.);
    login.input_time(90000.);
    login.studio.state.cancel(login.studio.now);
    login.step(90016., true, false).unwrap();
    assert!(login.studio.state.visible());
    assert!(login.needs_redraw());
}

#[test]
fn terminal_socket_during_hidden_death_hold_keeps_host_running() {
    let mut game = tape::playing();
    let mut login = returned();
    login.state.action(MenuAction::Play, true);
    login.step(11000., true, false).unwrap();
    login.observe_game(&game).unwrap();
    tape::pump(&mut game, 700.);
    login.observe_game(&game).unwrap();
    game.receive(game.generation(), &[b'v', 0], 800.).unwrap();
    tape::pump(&mut game, 808.);
    game.transport_failed(game.generation(), "Authored terminal socket")
        .unwrap();
    login.observe_game(&game).unwrap();
    assert_eq!(game.phase(), Phase::Disconnected);
    assert!(game.champion_session().death_pending);
    assert!(!login.state.view(true).visible);
    assert!(login.needs_redraw());
}

#[test]
fn champion_exit_control_reveal_and_save_overlays_settle() {
    use slither_client::{champion::ChampionState, session_policy::SessionPolicy};
    let mut session = SessionPolicy::new().view();
    session.phase = Phase::Victory;
    session.outcome = Some(Phase::Victory);
    let mut champion = ChampionState::default();
    assert!(champion.observe(1, &session));
    champion.step(0., 1.).unwrap();
    assert!(!champion.needs_animation());
    session.phase = Phase::Disconnected;
    champion.observe(1, &session);
    assert!(champion.needs_animation());
    for now in (16..=1600).step_by(16) {
        if !champion.finished() || champion.ordinary_alpha() < 1. {
            assert!(champion.needs_animation());
        }
        champion.step(f64::from(now), 1.).unwrap();
    }
    assert!(champion.finished());
    assert_eq!(champion.ordinary_alpha(), 1.);
    assert!(!champion.needs_animation());
    let mut ui = slither_wgpu_slice::champion_egui::ChampionUi::default();
    assert!(!ui.needs_animation());
    ui.fade = [0.5, 0.25]; // released overlays must finish fading out
    assert!(ui.needs_animation());
}

#[test]
fn egui_repaint_output_callback_and_caret_have_bounded_deadlines() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mut host = NativeEguiHost::default();
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    host.set_repaint_wake(move || {
        count.fetch_add(1, Ordering::Relaxed);
    });
    host.set_active(true);
    // Drain egui's startup settling requests.
    for tick in 0..4 {
        host.step(f64::from(tick) * 16.);
        host.frame([960, 540], 1., &[], |_, _| {});
    }
    assert_eq!(host.repaint_deadline(), None);
    let before = Instant::now();
    host.frame([960, 540], 1., &[], |ctx, _| {
        ctx.request_repaint_after(Duration::from_millis(500));
    });
    let at = host.repaint_deadline().unwrap();
    assert!(at >= before + Duration::from_millis(400));
    assert!(at <= Instant::now() + Duration::from_millis(500));
    // A context clone can request repaint while the event loop is asleep.
    let count = wakes.load(Ordering::Relaxed);
    let context = host.context().unwrap().clone();
    std::thread::spawn(move || context.request_repaint())
        .join()
        .unwrap();
    assert!(wakes.load(Ordering::Relaxed) > count);
    assert!(host.repaint_deadline().unwrap() <= Instant::now());
    host.set_active(false);
    assert_eq!(host.repaint_deadline(), None);
    host.set_active(true);
    let mut text = "Authored field".to_owned();
    for tick in 0..8 {
        host.step(1000. + f64::from(tick) * 16.);
        host.frame([960, 540], 1., &[], |ctx, _| {
            egui::Area::new(egui::Id::unique("authored-caret")).show(ctx, |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
            });
        });
    }
    let at = host
        .repaint_deadline()
        .expect("focused caret schedules a blink");
    assert!(at > Instant::now());
    assert!(at <= Instant::now() + Duration::from_secs(1));
    // Admission can move focus after presentation, outside an egui pass.
    host.focus(egui::Id::unique("authored-post-present-focus"));
    assert!(host.repaint_deadline().unwrap() <= Instant::now());
}
