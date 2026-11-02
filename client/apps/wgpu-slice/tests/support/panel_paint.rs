//! Paints one panel alone through the production composition: a hidden login
//! adds no wordmark, labels, controls, page or cover.
use slither_client::{
    menu_controls::{PanelInputState, PanelView},
    menu_input::{EditorId, MenuInputState},
    menu_preferences::MenuQuality,
    menu_theme::MenuTheme,
    start_page::{MenuLocale, StartMenuState},
};
use slither_wgpu_slice::{
    Result,
    menu::{Menu, MenuFrame, PanelPaint},
    menu_specimen,
    text::TextCache,
};
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub fn prepare_panel(
    menu: &mut Menu,
    text: &mut TextCache,
    panel: &PanelView,
    input: &PanelInputState,
    edit: &MenuInputState,
    scroll: &mut BTreeMap<EditorId, f64>,
    theme: MenuTheme,
    extent: [u32; 2],
    dpr: f64,
) -> Result<MenuFrame> {
    let mut view = StartMenuState::new(MenuLocale::English, true, MenuQuality::High).view(false);
    view.visible = false;
    let layout = menu_specimen::layout(text, &view, extent, dpr)?;
    menu.prepare_with_panel(
        text,
        &view,
        &layout,
        extent,
        dpr,
        None,
        None,
        Some(PanelPaint {
            panel,
            input,
            edit,
            scroll,
            theme,
        }),
    )
}
