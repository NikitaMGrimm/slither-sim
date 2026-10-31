//! Fixed-clock Settings entry/exit; procedural geometry, no art or network.
use super::*;
use slither_client::{
    login_controls::SETTINGS,
    menu_preferences::MenuQuality,
    settings::{SettingsPreferences, SettingsState, WorldResolution},
};
use slither_wgpu_slice::{login_native::NativeLogin, menu_input_native::KeyInput};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

fn key(login: &mut NativeLogin, named: NamedKey) {
    login.key(
        KeyInput {
            logical: &Key::Named(named),
            physical: PhysicalKey::Code(if named == NamedKey::Escape {
                KeyCode::Escape
            } else {
                KeyCode::Enter
            }),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        true,
    );
}

pub async fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let mut ledger = Vec::new();
    for dpr in [1_f64, 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
        for logical in [[960., 540.], [360., 640.], [960., 360.]] {
            for keyboard in [false, true] {
                for worst in [false, true] {
                    renderer.sync_resources(&[])?;
                    let extent = logical.map(|v| (v * dpr).round() as u32);
                    let mut login = NativeLogin::new("Fade")?;
                    if worst {
                        login.state.settings = SettingsState::new(SettingsPreferences {
                            quality: MenuQuality::Low,
                            resolution: WorldResolution::Native,
                        });
                        login.state.settings.storage_result(false);
                        login.state.settings.native_available(false);
                    }
                    for now in (0..=9600).step_by(16) {
                        login.step(now as f64, true, false)?;
                    }
                    login.prepare_host(extent, dpr, true, None)?;
                    if keyboard {
                        login.focus(SETTINGS);
                        key(&mut login, NamedKey::Enter);
                    } else {
                        let controls = login.panel.resolve(logical).map_err(str::to_owned)?;
                        let r = controls
                            .iter()
                            .find(|r| r.control.id == SETTINGS)
                            .ok_or("Settings entry missing")?;
                        let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
                        login.pointer(point, Some(true), true);
                        login.pointer(point, Some(false), true);
                    }
                    if !login.state.settings.visible() {
                        return Err("Settings entry did not open".into());
                    }
                    for (phase, start) in [("entry", 9600.), ("exit", 10240.)] {
                        if phase == "exit" {
                            key(&mut login, NamedKey::Escape);
                        }
                        for elapsed in (0..=640).step_by(16) {
                            let (intents, _) = login.step(start + elapsed as f64, true, false)?;
                            if intents.iter().any(|i| {
                                matches!(
                                    i,
                                    slither_client::start_page::StartMenuIntent::Play { .. }
                                )
                            }) {
                                return Err("Settings fixture requested Play".into());
                            }
                            if elapsed % 64 != 0 && ![16, 80, 240, 400].contains(&elapsed) {
                                continue;
                            }
                            if worst {
                                for control in &login.panel.controls {
                                    if control.enabled {
                                        login.controls.set_progress(control.id, [1., 1.]);
                                    }
                                }
                            }
                            let picture = login.prepare_host(extent, dpr, true, None)?;
                            let counts = picture.draw_counts(0);
                            let bytes: usize =
                                picture.resources.iter().map(|r| r.image.pixels.len()).sum();
                            if counts.admitted_menu() > 512
                                || picture.resources.len() > 64
                                || bytes > 13 * 1024 * 1024
                            {
                                return Err(format!(
                                    "Settings allowance {logical:?} DPR{dpr} {phase} {elapsed}: {} records, {} slots, {bytes} bytes",
                                    counts.admitted_menu(),
                                    picture.resources.len()
                                ));
                            }
                            if counts.components.total() != picture.frame.draws.len() {
                                return Err("Settings attribution mismatch".into());
                            }
                            let visible = login.state.settings.visible();
                            let alpha = if visible { login.panel.alpha } else { 0. };
                            ledger.push(serde_json::json!({"dpr":dpr,"logical":logical,"keyboard":keyboard,"worst":worst,"phase":phase,"elapsed":elapsed,"alpha":alpha,"visible":visible,"records":counts.admitted_menu(),"slots":picture.resources.len(),"bytes":bytes}));
                            if !worst
                                && !keyboard
                                && logical == [960., 540.]
                                && [1., 1.5, 2.].contains(&dpr)
                                && [80, 240, 400].contains(&elapsed)
                            {
                                capture(
                                    renderer,
                                    &picture,
                                    &output.join(format!("settings-{phase}-{elapsed}-{dpr}x.png")),
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    std::fs::write(output.join("settings-fade.json"), serde_json::to_vec_pretty(&serde_json::json!({"revision":String::from_utf8_lossy(&revision.stdout).trim(),"fixture":"fixed-clock native Settings; authored geometry only; no art folders, windows or servers","cases":ledger})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
