//! Offline native settings/atomic storage regressions; no windows or sockets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    login_controls::{self, CHANGE_SKIN, NICKNAME, PLAY, SERVER, SETTINGS},
    menu_controls::ControlKind,
    menu_input::EditorId,
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
    let resolved = login.panel.resolve(area).unwrap();
    let r = resolved.iter().find(|r| r.control.id == id).unwrap().rect;
    [r.x + r.width / 2., r.y + r.height / 2.]
}

#[test]
fn entry_order_modality_escape_done_and_play_snapshot() {
    let mut login = settled();
    login.prepare([960, 540], true, None).unwrap();
    let appearance = login.studio.state.committed().clone();
    login.picker.policy.selection = slither_client::server_picker::ServerPreference(Some(7));
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
    assert_eq!(login.controls.focus(), Some(QUALITY));
    assert_eq!(login.input.state.focus(), None);
    assert!(login.blocks_gameplay());
    login.open_studio();
    assert!(!login.studio.state.visible());
    assert!(!login.picker.policy.visible());
    login.prepare([960, 540], true, None).unwrap();
    login.pointer([0., 0.], Some(true), true);
    login.pointer([0., 0.], Some(false), true);
    assert!(login.state.settings.visible()); // no outside-click dismissal.
    login.focus(QUALITY);
    assert!(key(&mut login, NamedKey::Space));
    assert_eq!(login.state.view(true).quality, Q::Low);
    assert!(login.step(9616., true, false).unwrap().0.is_empty());
    assert!(!login.state.view(true).spinner_visible);
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.controls.focus(), Some(RESOLUTION));
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.controls.focus(), Some(DONE));
    assert!(key(&mut login, NamedKey::Tab));
    assert_eq!(login.controls.focus(), Some(QUALITY));
    assert!(key(&mut login, NamedKey::Escape));
    assert!(!login.state.settings.visible());
    assert_eq!(login.controls.focus(), Some(SETTINGS));
    assert!(key(&mut login, NamedKey::Enter));
    login.focus(DONE);
    assert!(key(&mut login, NamedKey::Space));
    assert!(!login.state.settings.visible());
    assert_eq!(login.controls.focus(), Some(SETTINGS));
    assert_eq!(login.state.view(true).nickname, "Player");
    assert_eq!(login.studio.state.committed().nickname, "Player");
    assert_eq!(login.studio.state.committed(), &appearance);
    assert_eq!(login.picker.policy.selection.0, Some(7));
    login.focus(PLAY);
    key(&mut login, NamedKey::Enter);
    login.open_settings(true);
    assert!(!login.state.settings.visible());
    assert_eq!(
        login.step(9632., true, false).unwrap().0,
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
    assert!(login.state.settings.panel([960., 540.]).controls.iter().any(|c| matches!(&c.kind, ControlKind::Label { text } if text == "Not saved; applies this session")));
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
fn all_settings_states_resize_and_retained_pictures_fit_compact_budgets() {
    let mut login = settled();
    let old = login.prepare([960, 540], true, None).unwrap();
    let frozen = old
        .resources
        .iter()
        .map(|r| (r.asset, r.image.pixels.clone()))
        .collect::<Vec<_>>();
    login.open_settings(true);
    for dpr in [1., 1.25, 1.5, 2., 3., 4., 5., 6., 7., 8.] {
        for area in [[960., 540.], [360., 640.]] {
            for state in 0..9 {
                login.focus(RESOLUTION);
                if (login.state.settings.preferences().resolution
                    == slither_client::settings::WorldResolution::Native)
                    != (state >= 7)
                {
                    key(&mut login, NamedKey::Space);
                }
                login.state.settings.native_available(state != 8);
                if state == 1 {
                    login.focus(QUALITY);
                    key(&mut login, NamedKey::Space);
                }
                login
                    .state
                    .settings
                    .storage_result(!matches!(state, 4 | 6 | 8));
                login
                    .prepare_host(area.map(|v| (v * dpr) as u32), dpr, true, None)
                    .unwrap();
                login.focus(if matches!(state, 2 | 6) {
                    DONE
                } else {
                    QUALITY
                });
                login.controls.set_progress(
                    QUALITY,
                    if matches!(state, 3 | 6) {
                        [1., 1.]
                    } else {
                        [0., 0.]
                    },
                );
                login
                    .controls
                    .set_progress(RESOLUTION, if state == 8 { [1., 1.] } else { [0., 0.] });
                let mut picture = login
                    .prepare_host(area.map(|v| (v * dpr) as u32), dpr, true, None)
                    .unwrap();
                if state == 5 {
                    let mut panel = login.panel.clone();
                    panel
                        .controls
                        .iter_mut()
                        .find(|c| c.id == QUALITY)
                        .unwrap()
                        .enabled = false;
                    picture = slither_wgpu_slice::menu::Menu::default()
                        .prepare_panel(
                            &mut slither_wgpu_slice::text::TextCache::menu(),
                            &panel,
                            &login.controls,
                            &login.input.state,
                            &mut Default::default(),
                            Default::default(),
                            area.map(|v| (v * dpr) as u32),
                            dpr,
                        )
                        .unwrap();
                    let page = slither_wgpu_slice::menu::MenuFrame::world_crossfade(
                        area.map(|v| (v * dpr) as u32),
                        dpr,
                        0.,
                    )
                    .unwrap();
                    picture.backdrop = page.backdrop;
                    picture.resources.extend(page.resources);
                }
                let bytes = picture
                    .resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>();
                assert!(
                    picture.resources.len() <= 32,
                    "{area:?} {dpr} {state}: {} slots {:?}",
                    picture.resources.len(),
                    picture
                        .resources
                        .iter()
                        .map(|r| [r.asset.width, r.asset.height])
                        .collect::<Vec<_>>()
                );
                assert!(
                    bytes <= 4 * 1024 * 1024,
                    "{dpr} {state}: {bytes} bytes, {} quads, {:?}",
                    picture.frame.draws.len(),
                    picture
                        .resources
                        .iter()
                        .map(|r| [r.asset.width, r.asset.height])
                        .collect::<Vec<_>>()
                );
                let ceiling = if dpr <= 2. { 64 } else { 160 };
                assert!(
                    picture.frame.draws.len() <= ceiling,
                    "{dpr} {state}: {} quads",
                    picture.frame.draws.len()
                );
                assert!(
                    picture
                        .resources
                        .iter()
                        .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
                );
                let resident = picture
                    .resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect();
                picture.frame.validate(&resident).unwrap();
            }
        }
    }
    assert_eq!(
        old.resources
            .iter()
            .map(|r| (r.asset, r.image.pixels.clone()))
            .collect::<Vec<_>>(),
        frozen
    );
    old.frame
        .validate(
            &old.resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )
        .unwrap();
    assert_eq!(login.input.state.focus(), None);
    assert_ne!(login.input.state.focus(), Some(EditorId::Nickname));
    assert!(
        login
            .panel
            .controls
            .iter()
            .all(|c| c.id != login_controls::CHANGE_SKIN || c.name != "Change Skin")
    );
}
