//! Finite panel-only specimens. Synthetic input/addresses, no art, preferences or network host.
use super::*;
use slither_client::{
    menu_controls::*, menu_input::*, menu_theme::MenuTheme, server_directory::Server,
    server_picker::*,
};
use slither_wgpu_slice::{menu_input_native::KeyInput, server_picker_native::NativeServerPicker};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

fn shot(
    renderer: &mut Renderer,
    menu: &mut Menu,
    text: &mut TextCache,
    picker: &mut NativeServerPicker,
    extent: [u32; 2],
    dpr: f64,
    path: &Path,
) -> Result<serde_json::Value> {
    picker.viewport = extent.map(|n| f64::from(n) / dpr);
    picker.sync();
    let picture = menu.prepare_panel(
        text,
        &picker.panel,
        &picker.controls,
        &picker.input.state,
        &mut picker.scroll,
        MenuTheme::default(),
        extent,
        dpr,
    )?;
    capture(renderer, &picture, path)?;
    Ok(
        serde_json::json!({"file":path.file_name().unwrap().to_string_lossy(),"extent":extent,"dpr":dpr,
        "draws":picture.frame.draws.len(),"resources":picture.resources.len(),"bytes":picture.resources.iter().map(|r|r.image.pixels.len()).sum::<usize>()}),
    )
}
fn click(picker: &mut NativeServerPicker, text: &mut TextCache, id: ControlId) -> Result<()> {
    picker.sync();
    let resolved = picker
        .panel
        .resolve(picker.viewport)
        .map_err(str::to_owned)?;
    let r = resolved
        .iter()
        .find(|r| r.control.id == id)
        .ok_or("Missing specimen control")?;
    let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
    for down in [true, false] {
        let effects = picker.pointer(text, point, Some(down))?;
        picker.commit_effects(effects);
    }
    Ok(())
}
fn enter(picker: &mut NativeServerPicker, value: &str) {
    picker.controls.focus_control(&picker.panel, ADDRESS);
    picker
        .controls
        .sync_focus(&picker.panel, &mut picker.input.state);
    picker.input.state.reset_editor(ADDRESS_EDITOR, "");
    let effects = picker.input.state.apply(EditIntent::KeyText(value.into()));
    picker.commit_effects(effects);
}
pub fn specimens(renderer: &mut Renderer, output: &Path) -> Result<()> {
    let mut picker = NativeServerPicker::default();
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    picker.open();
    picker.policy.alpha = 1.;
    let mut measurements = Vec::new();
    for (extent, dpr) in [([960, 720], 1.), ([1440, 1080], 1.5), ([720, 1280], 2.)] {
        measurements.push(shot(
            renderer,
            &mut menu,
            &mut text,
            &mut picker,
            extent,
            dpr,
            &output.join(format!("picker-{}x{}-{dpr}x.png", extent[0], extent[1])),
        )?);
    }
    picker.viewport = [960., 720.];
    picker.sync();
    for address in ["192.0.2.10:445", "192.0.2.20:80"] {
        enter(&mut picker, address);
        click(&mut picker, &mut text, ADD_CUSTOM)?;
    }
    measurements.push(shot(
        renderer,
        &mut menu,
        &mut text,
        &mut picker,
        [960, 720],
        1.,
        &output.join("picker-custom.png"),
    )?);
    // Same Add target, pointer modality versus a real mapped Tab key in this fixture.
    enter(&mut picker, "192.0.2.10:445");
    click(&mut picker, &mut text, ADD_CUSTOM)?;
    measurements.push(shot(
        renderer,
        &mut menu,
        &mut text,
        &mut picker,
        [960, 720],
        1.,
        &output.join("picker-mouse.png"),
    )?);
    click(&mut picker, &mut text, ADDRESS)?;
    let logical = Key::Named(NamedKey::Tab);
    let result = picker.key(KeyInput {
        logical: &logical,
        physical: PhysicalKey::Code(KeyCode::Tab),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    });
    picker.commit_effects(result.effects);
    measurements.push(shot(
        renderer,
        &mut menu,
        &mut text,
        &mut picker,
        [960, 720],
        1.,
        &output.join("picker-keyboard.png"),
    )?);
    enter(&mut picker, "invalid host");
    click(&mut picker, &mut text, ADD_CUSTOM)?;
    measurements.push(shot(
        renderer,
        &mut menu,
        &mut text,
        &mut picker,
        [960, 720],
        1.,
        &output.join("picker-invalid.png"),
    )?);
    enter(&mut picker, "");
    picker.policy.custom_error.clear();
    click(&mut picker, &mut text, OFFICIAL)?;
    let servers = (1..=4)
        .map(|id| Server {
            id,
            cluster: id,
            address: ([192, 0, 2, 100], 400 + id).into(),
            population: 40,
            active: true,
        })
        .collect::<Vec<_>>();
    picker
        .policy
        .replace(
            &servers,
            &std::collections::BTreeMap::from([(1, 18), (2, 67), (3, 130)]),
        )
        .map_err(str::to_owned)?;
    for (extent, dpr) in [([960, 720], 1.), ([1440, 1080], 1.5), ([720, 1280], 2.)] {
        measurements.push(shot(
            renderer,
            &mut menu,
            &mut text,
            &mut picker,
            extent,
            dpr,
            &output.join(format!(
                "picker-mixed-{}x{}-{dpr}x.png",
                extent[0], extent[1]
            )),
        )?);
    }
    std::fs::write(output.join("picker-panel.json"),serde_json::to_vec_pretty(&serde_json::json!({"synthetic":true,"network":false,"art":false,"physical_input":false,"states":measurements})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
