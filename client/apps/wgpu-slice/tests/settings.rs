//! Offline native settings/atomic storage regressions; no windows or sockets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    login_controls::{CHANGE_SKIN, NICKNAME, PLAY, SERVER, SETTINGS},
    menu_preferences::MenuQuality as Q,
    settings::{DONE, QUALITY, RESOLUTION},
    start_page::StartMenuIntent,
};
use slither_wgpu_slice::{
    login_native::NativeLogin, menu_input_native::KeyInput, menu_storage::SettingsStorage,
};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

fn key(login: &mut NativeLogin, named: NamedKey) -> bool {
    login
        .key(
            KeyInput {
                logical: &Key::Named(named),
                physical: PhysicalKey::Code(match named {
                    NamedKey::Enter => KeyCode::Enter,
                    NamedKey::Space => KeyCode::Space,
                    NamedKey::Escape => KeyCode::Escape,
                    NamedKey::Tab => KeyCode::Tab,
                    _ => KeyCode::F1,
                }),
                text: None,
                state: ElementState::Pressed,
                repeat: false,
            },
            true,
        )
        .consumed
}
fn settled() -> NativeLogin {
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    login
}
fn point(
    login: &NativeLogin,
    id: slither_client::menu_controls::ControlId,
    area: [f64; 2],
) -> [f64; 2] {
    let _ = area;
    let rect = login
        .settings_ui
        .controls
        .iter()
        .find(|(control, _, _)| *control == id)
        .unwrap()
        .1;
    [f64::from(rect.center().x), f64::from(rect.center().y)]
}

#[test]
fn entry_order_modality_escape_done_and_play_snapshot() {
    let mut login = settled();
    login.prepare([960, 540], true, None).unwrap();
    let appearance = login.studio.state.committed().clone();
    login.picker.policy.selection = slither_client::server_picker::ServerPreference::Official(7);
    login.focus(NICKNAME);
    for id in [PLAY, CHANGE_SKIN, SERVER, SETTINGS, NICKNAME] {
        assert!(key(&mut login, NamedKey::Tab));
        assert_eq!(login.controls.focus(), Some(id));
    }
    login.focus(SETTINGS);
    let opened = login.key(
        KeyInput {
            logical: &Key::Named(NamedKey::Enter),
            physical: PhysicalKey::Code(KeyCode::Enter),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        true,
    );
    assert!(opened.consumed);
    assert!(
        opened
            .effects
            .contains(&slither_client::menu_input::HostEditEffect::ReleasePointerBoost)
    );
    assert!(login.state.settings.visible());
    assert_eq!(login.settings_ui.focused(), Some(QUALITY));
    assert_eq!(login.input.state.focus(), None);
    assert!(login.blocks_gameplay());
    login.open_studio();
    assert!(!login.studio.state.visible());
    assert!(!login.picker.policy.visible());
    login.prepare([960, 540], true, None).unwrap();
    login.focus(QUALITY);
    assert!(key(&mut login, NamedKey::Space));
    assert_eq!(login.state.view(true).quality, Q::Low);
    assert!(login.step(9616., true, false).unwrap().0.is_empty());
    assert!(!login.state.view(true).spinner_visible);
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.settings_ui.focused(), Some(RESOLUTION));
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.settings_ui.focused(), Some(DONE));
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.settings_ui.focused(), Some(QUALITY));
    assert!(key(&mut login, NamedKey::Escape));
    assert!(!login.state.settings.is_open());
    login.step(9846., true, false).unwrap();
    assert_eq!(login.controls.focus(), Some(SETTINGS));
    login.prepare([960, 540], true, None).unwrap();
    assert!(key(&mut login, NamedKey::Enter));
    login.focus(DONE);
    assert!(key(&mut login, NamedKey::Space));
    assert!(!login.state.settings.visible());
    assert_eq!(login.controls.focus(), Some(SETTINGS));
    assert_eq!(login.state.view(true).nickname, "Player");
    assert_eq!(login.studio.state.committed().nickname, "Player");
    assert_eq!(login.studio.state.committed(), &appearance);
    assert_eq!(login.picker.policy.selection.official_id(), Some(7));
    login.focus(PLAY);
    key(&mut login, NamedKey::Enter);
    login.open_settings(true);
    assert!(!login.state.settings.visible());
    assert_eq!(
        login.step(9862., true, false).unwrap().0,
        vec![StartMenuIntent::Play {
            nickname: "Player".into(),
            quality: Q::Low
        }]
    );
    assert!(!key(&mut login, NamedKey::Escape)); // host retains Stop/cancel.
}

#[test]
fn pointer_blur_resize_and_mutually_exclusive_entry() {
    let mut login = settled();
    login.open_settings(true);
    login.prepare([960, 540], true, None).unwrap();
    let p = point(&login, QUALITY, [960., 540.]);
    login.pointer(p, Some(true), true);
    login.event(&winit::event::WindowEvent::Focused(false), true);
    login.pointer(p, Some(false), true);
    assert_eq!(login.state.view(true).quality, Q::High);
    login.pointer(p, Some(true), true);
    login.prepare_host([1920, 1080], 2., true, None).unwrap();
    login.pointer(p, Some(false), true);
    assert_eq!(login.state.view(true).quality, Q::High);
    login.pointer(p, Some(true), true);
    login.pointer(p, Some(false), true);
    assert_eq!(login.state.view(true).quality, Q::Low);
    let p = point(&login, RESOLUTION, [960., 540.]);
    login.pointer(p, Some(true), true);
    login.event(&winit::event::WindowEvent::Focused(false), true);
    login.pointer(p, Some(false), true);
    assert_eq!(
        login.state.settings.preferences().resolution,
        slither_client::settings::WorldResolution::Original
    );
    login.pointer(p, Some(true), true);
    login.pointer(p, Some(false), true);
    assert_eq!(
        login.state.settings.preferences().resolution,
        slither_client::settings::WorldResolution::Native
    );
    assert_eq!(login.state.view(true).quality, Q::Low);
    login.close_settings(true);
    login.open_studio();
    login.open_settings(true);
    assert!(!login.state.settings.visible());
    let mut login = settled();
    login.picker.open();
    login.open_settings(true);
    assert!(!login.state.settings.visible());
}

struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "client-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_storage_bounded_read_no_startup_write_atomic_replace_and_later_retry() {
    let temp = Temp::new();
    let path = temp.0.join("settings.txt");
    let storage = SettingsStorage::at(Some(temp.0.clone()));
    assert_eq!(storage.restore().quality, Q::High);
    assert!(!path.exists());
    for (bytes, quality) in [
        (b"1:0".as_slice(), Q::Low),
        (b"1:1", Q::High),
        (b"2:0:native", Q::Low),
        (b"1:0\n", Q::High),
        (&[255; 65], Q::High),
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(storage.restore().quality, quality);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    std::fs::write(&path, "1:1").unwrap();
    storage.save("1:0").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:0:original");
    assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 1);
    storage.save("2:0:native").unwrap();
    assert_eq!(
        storage.restore().resolution,
        slither_client::settings::WorldResolution::Native
    );
    assert_eq!(storage.restore().quality, Q::Low);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:0:native");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(storage.restore().quality, Q::High);
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, "1:0").unwrap();
    storage.save("1:1").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:1:original");
    storage.save("1:0").unwrap();
    let mut login = settled();
    login.restore_settings(SettingsStorage::at(Some(temp.0.clone())));
    assert_eq!(login.state.view(true).quality, Q::Low);
    login.focus(PLAY);
    login.activate(true);
    assert!(
        login
            .step(9616., true, false)
            .unwrap()
            .0
            .iter()
            .any(|i| matches!(
                i,
                StartMenuIntent::Play {
                    quality: Q::Low,
                    ..
                }
            ))
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:0:original");
}

#[test]
fn failed_replace_keeps_old_data_and_memory_and_does_not_retry_each_frame() {
    let temp = Temp::new();
    let path = temp.0.join("settings.txt");
    let mut login = settled();
    login.restore_settings(SettingsStorage::at(Some(temp.0.clone())));
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("old"), "keep").unwrap();
    login.open_settings(true);
    key(&mut login, NamedKey::Space);
    login.step(9616., true, false).unwrap();
    assert_eq!(login.state.view(true).quality, Q::Low);
    assert_eq!(std::fs::read_to_string(path.join("old")).unwrap(), "keep");
    assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 1);
    assert!(login.state.settings.not_saved());
    std::fs::remove_file(path.join("old")).unwrap();
    std::fs::remove_dir(&path).unwrap();
    login.step(9632., true, false).unwrap();
    assert!(!path.exists());
    key(&mut login, NamedKey::Space);
    login.step(9648., true, false).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:1:original");
}

#[cfg(windows)]
#[test]
fn startup_read_failure_does_not_block_later_explicit_save() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = Temp::new();
    let path = temp.0.join("settings.txt");
    std::fs::write(&path, "1:1").unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let mut login = settled();
    login.restore_settings(SettingsStorage::at(Some(temp.0.clone())));
    assert_eq!(login.state.view(true).quality, Q::High);
    drop(lock);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "1:1");
    login.open_settings(true);
    key(&mut login, NamedKey::Space);
    login.step(9616., true, false).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "2:0:original");
}

#[test]
fn egui_layout_focus_keyboard_reachability_and_retained_login() {
    let mut login = settled();
    let old = login.prepare([960, 540], true, None).unwrap();
    let frozen = old
        .resources
        .iter()
        .map(|r| r.image.pixels.clone())
        .collect::<Vec<_>>();
    login.open_settings(true);
    login.step(9900., true, false).unwrap();
    login.state.settings.activate(login.state.settings.intent(
        RESOLUTION,
        Some(true),
        slither_client::menu_controls::Activation::Enter,
    ));
    for dpr in [1., 1.25, 1.5, 2., 3., 5., 8.] {
        for area in [[960., 540.], [360., 640.], [640., 360.]] {
            login.state.settings.storage_result(false);
            login.state.settings.native_available(false);
            for _ in 0..3 {
                login
                    .prepare_host(area.map(|v| (v * dpr) as u32), dpr, true, None)
                    .unwrap();
            }
            let window = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(area[0] as f32, area[1] as f32),
            );
            assert!(
                window.contains_rect(login.settings_ui.panel_rect),
                "{area:?} {dpr}: {:?}",
                login.settings_ui.panel_rect
            );
            let controls = &login.settings_ui.controls;
            assert_eq!(controls.len(), 3);
            assert!(controls.iter().filter(|(_, _, focused)| *focused).count() <= 1);
            for (i, (_, rect, _)) in controls.iter().enumerate() {
                assert!(login.settings_ui.panel_rect.contains_rect(*rect));
                assert!(
                    controls[i + 1..]
                        .iter()
                        .all(|(_, other, _)| !rect.intersects(*other))
                );
            }
            login.focus(QUALITY);
            for id in [RESOLUTION, DONE, QUALITY] {
                key(&mut login, NamedKey::Tab);
                for _ in 0..2 {
                    login
                        .prepare_host(area.map(|v| (v * dpr) as u32), dpr, true, None)
                        .unwrap();
                }
                assert_eq!(login.settings_ui.focused(), Some(id));
                assert!(
                    login
                        .settings_ui
                        .controls
                        .iter()
                        .find(|(control, _, _)| *control == id)
                        .unwrap()
                        .1
                        .is_positive(),
                    "{area:?} {dpr} {id:?}"
                );
                assert_eq!(
                    login
                        .settings_ui
                        .controls
                        .iter()
                        .filter(|(_, _, focus)| *focus)
                        .count(),
                    1
                );
            }
        }
    }
    assert_eq!(
        old.resources
            .iter()
            .map(|r| r.image.pixels.clone())
            .collect::<Vec<_>>(),
        frozen
    );
    assert_eq!(login.input.state.focus(), None);
}

#[test]
fn fade_reverses_continuously_and_return_keeps_modal_input_until_finished() {
    let mut login = settled();
    login.open_settings(true);
    assert_eq!(login.settings_ui.alpha(), 0.);
    login.step(9715., true, false).unwrap();
    let alpha = login.settings_ui.alpha();
    assert!(alpha > 0. && alpha < 1.);
    login.close_settings(true);
    assert_eq!(login.settings_ui.alpha(), alpha);
    assert!(login.settings_ui.active());
    login.step(9945., true, false).unwrap();
    assert!(!login.settings_ui.active());
    assert_eq!(login.controls.focus(), Some(SETTINGS));
}

#[test]
fn native_settings_uses_the_shared_picker_fade_durations() {
    let mut login = settled();
    login.open_settings(true);
    for now in (9616..=10_000).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    assert_eq!(login.settings_ui.alpha(), 1.);
    login.close_settings(true);
    assert!(login.settings_ui.active());
    for now in (10_016..=10_320).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    assert!(!login.settings_ui.active());
    assert_eq!(login.controls.focus(), Some(SETTINGS));
}

#[test]
fn egui_shift_tab_repeats_and_previous_mount_pointer_release() {
    let mut login = settled();
    login.open_settings(true);
    login.step(9900., true, false).unwrap();
    login.prepare([960, 540], true, None).unwrap();
    let key = |key, repeat, modifiers| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    };
    login.settings_input(egui::RawInput {
        events: vec![key(egui::Key::Tab, false, egui::Modifiers::SHIFT)],
        ..Default::default()
    });
    assert_eq!(login.settings_ui.focused(), Some(DONE));
    login.focus(QUALITY);
    login.settings_input(egui::RawInput {
        events: vec![key(egui::Key::Space, false, egui::Modifiers::NONE)],
        ..Default::default()
    });
    assert_eq!(login.state.settings.preferences().quality, Q::Low);
    login.settings_input(egui::RawInput {
        events: vec![key(egui::Key::Space, true, egui::Modifiers::NONE)],
        ..Default::default()
    });
    assert_eq!(login.state.settings.preferences().quality, Q::Low);
    let pos = point(&login, RESOLUTION, [960., 540.]);
    login.pointer(pos, Some(true), true);
    login.close_settings(true);
    login.step(9916., true, false).unwrap();
    login.step(9932., true, false).unwrap();
    login.open_settings(true);
    login.pointer(pos, Some(false), true);
    assert_eq!(
        login.state.settings.preferences().resolution,
        slither_client::settings::WorldResolution::Original
    );
}

#[test]
fn backdrop_closes_on_matching_release_and_retains_the_return_fade() {
    let mut login = settled();
    login.open_settings(true);
    for now in (9616..=10_000).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    login.prepare([960, 540], true, None).unwrap();
    login.pointer([0., 0.], Some(true), true);
    assert!(login.state.settings.is_open());
    login.pointer([0., 0.], Some(false), true);
    assert!(!login.state.settings.is_open());
    assert!(login.state.settings.visible());
    assert!(login.blocks_gameplay());
}

#[test]
fn nickname_composition_blocks_settings_entry() {
    use slither_client::menu_input::{EditIntent, EditorId};
    let mut login = settled();
    login
        .input
        .state
        .apply(EditIntent::Focus(Some(EditorId::Nickname)));
    login
        .input
        .state
        .apply(EditIntent::Preedit("authored preedit".into(), None));
    login.open_settings(true);
    assert!(!login.state.settings.visible());
    login.input.state.apply(EditIntent::CancelComposition);
    login.open_settings(true);
    assert!(login.state.settings.visible());
    assert_eq!(login.input.state.focus(), None);
}

#[test]
fn reopening_a_short_panel_reveals_its_initial_keyboard_control() {
    let mut login = settled();
    login.prepare([640, 360], true, None).unwrap();
    login.open_settings(true);
    key(&mut login, NamedKey::Tab);
    for _ in 0..2 {
        login.prepare([640, 360], true, None).unwrap();
    }
    assert_eq!(login.settings_ui.focused(), Some(RESOLUTION));
    login.close_settings(true);
    login.open_settings(true);
    assert_eq!(login.settings_ui.focused(), Some(QUALITY));
    assert!(
        login
            .settings_ui
            .controls
            .iter()
            .find(|(id, _, _)| *id == QUALITY)
            .unwrap()
            .1
            .is_positive()
    );
}

#[test]
fn fractional_dpi_preserves_the_physical_viewport() {
    let mut login = settled();
    login.open_settings(true);
    for (extent, dpr) in [([453, 945], 1.75), ([951, 1200], 1.1)] {
        login.prepare_host(extent, dpr, true, None).unwrap();
        let logical = login
            .settings_ui
            .context
            .input(|input| input.viewport_rect().size());
        let physical = [logical.x, logical.y].map(|n| (n * dpr as f32).round() as u32);
        assert_eq!(physical, extent);
        assert!(logical.is_finite());
    }
}
