//! 1080p and 4K offscreen specimens of every menu screen, for visual review.
//! Authored names, synthetic addresses and injected pings; no window, socket,
//! preference file or art directory.
use super::*;
use slither_client::{
    server_directory::Server,
    server_picker::ServerPreference,
    settings::{SettingsPreferences, SettingsState},
};
use slither_wgpu_slice::login_native::NativeLogin;
use std::{collections::BTreeMap, net::SocketAddr};

const SIZES: [(&str, [u32; 2], f64); 2] = [("1080p", [1920, 1080], 1.), ("4k", [3840, 2160], 2.)];

fn key(login: &mut NativeLogin, key: egui::Key) {
    key_with(login, key, Default::default());
}
fn key_with(login: &mut NativeLogin, key: egui::Key, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        login.settings_input(egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        });
    }
}
fn settled(record: &str, extent: [u32; 2], dpr: f64) -> Result<NativeLogin> {
    let mut login = NativeLogin::new("Player")?;
    login.state.settings =
        SettingsState::new(SettingsPreferences::parse(record).ok_or("Invalid authored record")?);
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), true, false)?;
    }
    login.prepare_host(extent, dpr, true, None)?;
    Ok(login)
}
const SETTLED: f64 = 9600.;
/// Let a panel's entry fade finish on the shared clock, then capture.
#[allow(clippy::too_many_arguments)]
fn shot(
    renderer: &mut Renderer,
    login: &mut NativeLogin,
    clock: &mut f64,
    output: &Path,
    name: &str,
    extent: [u32; 2],
    dpr: f64,
) -> Result<String> {
    for _ in 0..50 {
        *clock += 16.;
        login.step(*clock, true, false)?;
        login.studio.presented();
    }
    renderer.sync_resources(&[])?;
    let (picture, top, _) = studio_picture(login, extent, dpr, true, None)?;
    // Painting consumes the prepared shapes.
    let shapes = login.egui.prepared_shapes().len();
    capture_layered(
        renderer,
        &picture,
        &top,
        login,
        &output.join(format!("{name}.png")),
    )?;
    Ok(format!(
        "{name}.png: {extent:?} DPR {dpr}; egui shapes {shapes}"
    ))
}

pub fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let servers = (1..=40)
        .map(|id| Server {
            id,
            cluster: id % 5,
            address: SocketAddr::from(([192, 0, 2, 1], id)),
            population: 40,
            active: true,
        })
        .collect::<Vec<_>>();
    let pings = BTreeMap::from([(0, 18), (1, 67), (2, 130), (3, 285)]);
    let mut ledger =
        vec!["Authored offscreen specimens; no window, socket, preference file or art.".to_owned()];
    for (size, extent, dpr) in SIZES {
        for mode in ["original", "native"] {
            let record = if mode == "native" {
                "3:auto:native:auto"
            } else {
                "3:auto:original:auto"
            };
            // The login keeps keyboard focus on Play, as after Tab.
            let mut login = settled(record, extent, dpr)?;
            let mut clock = SETTLED;
            key(&mut login, egui::Key::Tab);
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("login-{size}-{mode}"),
                extent,
                dpr,
            )?);
            let mut login = settled(record, extent, dpr)?;
            let mut clock = SETTLED;
            login.open_settings(true);
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("settings-{size}-{mode}"),
                extent,
                dpr,
            )?);
            // The pointer over the World resolution toggle; Quality keeps focus.
            let toggle = login
                .settings_ui
                .controls
                .iter()
                .find(|c| c.0 == slither_client::settings::RESOLUTION)
                .ok_or("Resolution toggle not laid out")?
                .1
                .center();
            login.settings_input(egui::RawInput {
                events: vec![egui::Event::PointerMoved(toggle)],
                ..Default::default()
            });
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("settings-hover-{size}-{mode}"),
                extent,
                dpr,
            )?);
            login.settings_input(egui::RawInput {
                events: vec![egui::Event::PointerGone],
                ..Default::default()
            });
            // Keyboard focus on Done, the panel's primary action, last in Tab order.
            key_with(&mut login, egui::Key::Tab, egui::Modifiers::SHIFT);
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("settings-done-{size}-{mode}"),
                extent,
                dpr,
            )?);
            let mut login = settled(record, extent, dpr)?;
            let mut clock = SETTLED;
            login.picker.policy.start_directory();
            login
                .picker
                .policy
                .replace(&servers, &pings)
                .map_err(str::to_owned)?;
            login.picker.policy.selection = ServerPreference::Official(12);
            login.login_ui.keyboard_focus = true;
            login.open_picker();
            for _ in 0..4 {
                key(&mut login, egui::Key::Tab);
            }
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("picker-{size}-{mode}"),
                extent,
                dpr,
            )?);
            let mut login = settled(record, extent, dpr)?;
            let mut clock = SETTLED;
            login.open_studio();
            ledger.push(shot(
                renderer,
                &mut login,
                &mut clock,
                output,
                &format!("studio-{size}-{mode}"),
                extent,
                dpr,
            )?);
        }
    }
    std::fs::write(output.join("state.txt"), ledger.join("\n") + "\n").map_err(|e| e.to_string())
}
