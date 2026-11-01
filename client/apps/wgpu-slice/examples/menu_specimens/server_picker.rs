//! Finite server picker before/after specimens. Synthetic addresses and injected
//! pings; no window, socket, preference file or art.
use super::*;
use slither_client::{
    menu_controls::{ControlId, PanelInputState},
    menu_input::MenuInputState,
    menu_theme::MenuTheme,
    server_directory::Server,
    server_picker::*,
};
use slither_wgpu_slice::{login_native::NativeLogin, menu::PanelPaint};
use std::{collections::BTreeMap, net::SocketAddr};

/// The retained toolkit painter fed the same core panel, field values and focus.
fn before(login: &NativeLogin, extent: [u32; 2], dpr: f64) -> Result<MenuFrame> {
    let picker = &login.picker;
    let mut edit = MenuInputState::default();
    let mut input = PanelInputState::default();
    input.sync(&picker.panel, &mut edit);
    edit.reset_editor(ADDRESS_EDITOR, &picker.ui.address);
    edit.reset_editor(SEARCH_EDITOR, &picker.ui.search);
    if let Some(focus) = picker.ui.focused() {
        input.set_focus(focus);
    }
    if picker.ui.keyboard_focus {
        input.keyboard_navigation();
    }
    input.sync_focus(&picker.panel, &mut edit);
    Menu::default().prepare_with_panel(
        &mut TextCache::menu(),
        &login.state.view(true),
        &login.layout,
        extent,
        dpr,
        None,
        Some(&edit),
        Some(PanelPaint {
            thumbnails: None,
            panel: &picker.panel,
            input: &input,
            edit: &edit,
            scroll: &mut BTreeMap::new(),
            theme: MenuTheme::default(),
        }),
    )
}
fn shot(
    renderer: &mut Renderer,
    login: &mut NativeLogin,
    output: &Path,
    name: &str,
    extent: [u32; 2],
    dpr: f64,
) -> Result<serde_json::Value> {
    login.prepare_host(extent, dpr, true, None)?;
    let after = login.prepare_host(extent, dpr, true, None)?;
    let shapes = login.egui.prepared_shapes().len();
    capture(
        renderer,
        &before(login, extent, dpr)?,
        &output.join(format!("{name}-before.png")),
    )?;
    capture_settings(
        renderer,
        &after,
        login,
        &output.join(format!("{name}-after.png")),
    )?;
    Ok(serde_json::json!({
        "state": name, "extent": extent, "dpr": dpr, "egui_shapes": shapes,
        "rows": login.picker.panel.controls.iter()
            .filter(|c| matches!(c.kind, slither_client::menu_controls::ControlKind::ListRow { .. }))
            .count(),
    }))
}
fn input(login: &mut NativeLogin, event: egui::Event) {
    login.settings_input(egui::RawInput {
        events: vec![event],
        ..Default::default()
    });
}
fn press(login: &mut NativeLogin, key: egui::Key) {
    for pressed in [true, false] {
        input(
            login,
            egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: Default::default(),
            },
        );
    }
}
fn click(login: &mut NativeLogin, id: ControlId) -> Result<()> {
    let r = login
        .picker
        .ui
        .controls
        .iter()
        .find(|c| c.0 == id)
        .ok_or("Missing specimen control")?
        .1;
    let p = [f64::from(r.center().x), f64::from(r.center().y)];
    login.pointer(p, Some(true), true);
    login.pointer(p, Some(false), true);
    Ok(())
}
/// Replace the address through ordinary field editing, then press Add.
fn add(login: &mut NativeLogin, value: &str) -> Result<()> {
    click(login, ADDRESS)?;
    login.picker.set_address("");
    input(login, egui::Event::Text(value.into()));
    click(login, ADD_CUSTOM)
}
fn settled() -> Result<NativeLogin> {
    let mut login = NativeLogin::new("")?;
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false)?;
    }
    Ok(login)
}
fn open(login: &mut NativeLogin) {
    login.open_picker();
    login.picker.policy.alpha = 1.;
}

pub async fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let validation = renderer
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let servers = (1..=2000)
        .map(|id| Server {
            id,
            cluster: id % 5,
            address: SocketAddr::from(([192, 0, 2, 1], id)),
            population: 40,
            active: true,
        })
        .collect::<Vec<_>>();
    let pings = BTreeMap::from([(0, 18), (1, 67), (2, 130), (3, 285)]);
    let mut states = Vec::new();
    // One resource owner across DPR changes, like the actual native host.
    let mut login = settled()?;
    login.picker.policy.start_directory();
    login
        .picker
        .policy
        .replace(&servers, &pings)
        .map_err(str::to_owned)?;
    login.picker.policy.selection = ServerPreference::Official(25);
    open(&mut login);
    for dpr in [1., 1.5, 2.] {
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        for (name, offset) in [("official", isize::MIN), ("scrolled", 1994)] {
            login.picker.policy.scroll(offset);
            states.push(shot(
                renderer,
                &mut login,
                output,
                &format!("picker-{name}-{dpr}x"),
                extent,
                dpr,
            )?);
        }
    }
    // Retain the same native menu across the reported DPR-3 resize trigger.
    for logical in [[960., 540.], [960., 720.]] {
        let extent = logical.map(|v| (v * 3.) as u32);
        states.push(shot(
            renderer,
            &mut login,
            output,
            &format!("picker-resize-3x-{}x{}", logical[0], logical[1]),
            extent,
            3.,
        )?);
    }
    // Custom entries, pointer versus keyboard focus on the same Add target,
    // validation, then mixed groups with official rows.
    let mut login = settled()?;
    open(&mut login);
    login.prepare_host([960, 720], 1., true, None)?;
    for address in ["192.0.2.10:445", "192.0.2.20:80"] {
        add(&mut login, address)?;
    }
    states.push(shot(
        renderer,
        &mut login,
        output,
        "picker-custom",
        [960, 720],
        1.,
    )?);
    add(&mut login, "192.0.2.10:445")?;
    states.push(shot(
        renderer,
        &mut login,
        output,
        "picker-mouse",
        [960, 720],
        1.,
    )?);
    click(&mut login, ADDRESS)?;
    press(&mut login, egui::Key::Tab);
    states.push(shot(
        renderer,
        &mut login,
        output,
        "picker-keyboard",
        [960, 720],
        1.,
    )?);
    add(&mut login, "invalid host")?;
    states.push(shot(
        renderer,
        &mut login,
        output,
        "picker-invalid",
        [960, 720],
        1.,
    )?);
    login.picker.set_address("");
    login.picker.policy.custom_error.clear();
    login.picker.policy.start_directory();
    login
        .picker
        .policy
        .replace(&servers[..4], &BTreeMap::from([(1, 18), (2, 67), (3, 130)]))
        .map_err(str::to_owned)?;
    for (extent, dpr) in [([960, 720], 1.), ([1440, 1080], 1.5), ([720, 1280], 2.)] {
        states.push(shot(
            renderer,
            &mut login,
            output,
            &format!("picker-mixed-{}x{}-{dpr}x", extent[0], extent[1]),
            extent,
            dpr,
        )?);
    }
    std::fs::write(
        output.join("server-picker.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "synthetic": true, "network": false, "art": false, "physical_input": false,
            "before": "retained toolkit fed the same panel", "after": "egui host",
            "states": states,
        }))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Some(error) = validation.pop().await {
        return Err(error.to_string());
    }
    Ok(())
}
