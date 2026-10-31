//! Procedural native menu focus and fractional clipping regressions; no external art.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{menu_controls::ControlId, skin_studio::*};
use slither_wgpu_slice::{login_native::NativeLogin, menu_storage::MenuStorage};

#[test]
fn fractional_narrow_builder_reveal_keeps_mirrored_capsule_sources_in_bounds() {
    let mut login =
        NativeLogin::with_preferences(AppearanceRecord::default(), MenuStorage::at(None)).unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    login.open_studio();
    login.step(9830., true, false).unwrap();
    login.studio.activate(BUILDER_TAB, "Focus");
    login.studio.state.edit_builder(|b| {
        b.append(1);
        b.append(2);
    });
    login.step(10010., true, false).unwrap();
    for dpr in [1.25, 1.5, 1.75, 2.5] {
        let logical = [360., 640.];
        let extent = logical.map(|v| (v * dpr) as u32);
        for id in [ControlId(SWATCH_BASE + 2), ControlId(RUN_BASE), ADD] {
            login.studio.state.reveal_control(id, logical);
            login.prepare_host(extent, dpr, true, None).unwrap();
            login.controls.keyboard_navigation();
            login.focus(id);
            let frame = login.prepare_host(extent, dpr, true, None).unwrap();
            for draw in &frame.frame.draws {
                if let Some(source) = draw.source {
                    assert!(source.x >= 0. && source.x + source.width <= draw.asset.width as f64);
                    assert!(source.y >= 0. && source.y + source.height <= draw.asset.height as f64);
                }
            }
        }
    }
}
