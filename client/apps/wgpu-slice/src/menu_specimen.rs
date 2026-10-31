//! Authored offline visual specimens, like `scene`; never enabled by a game host.
use crate::{
    Result,
    menu::{Menu, MenuFrame},
    text::{Family, Slant, TextCache, TextStyle, Weight},
};
use slither_client::{
    menu_layout::{MenuLayout, MenuMetrics},
    menu_preferences::MenuQuality,
    start_page::{MenuLocale, StartMenuState, StartMenuView},
};

pub fn view(state: u8) -> Result<StartMenuView> {
    if state > 4 {
        return Err("Menu specimen state 0..4".into());
    }
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.nickname = "Player".into();
    view.tip = view.wording.tips[2];
    view.tip_alpha = 1.;
    view.hover_alpha = f64::from(state == 1);
    view.press_alpha = f64::from(state == 2);
    view.disabled = state >= 3;
    view.play_disabled = view.disabled;
    if state == 4 {
        view.spinner_visible = true;
        view.spinner_alpha = 1.;
        view.spinner_scale = 0.9;
        view.spinner_time = 30.;
    }
    Ok(view)
}
pub fn layout(
    text: &mut TextCache,
    view: &StartMenuView,
    extent: [u32; 2],
    dpr: f64,
) -> Result<MenuLayout> {
    crate::render::dimensions(extent, dpr)?;
    MenuLayout::new(
        f64::from(extent[0]) / dpr,
        f64::from(extent[1]) / dpr,
        view.transition,
        view.tips_visible,
        metrics(text, view)?,
    )
    .map_err(str::to_owned)
}
pub fn metrics(text: &mut TextCache, view: &StartMenuView) -> Result<MenuMetrics> {
    let mut style = TextStyle {
        size: 20.,
        weight: Weight::Bold,
        family: Family::Arial,
        slant: Slant::Normal,
        color: [255; 3],
    };
    let padded_play_label_width = text
        .measure_panel(&format!("\u{a0}{}\u{a0}", view.play_label), style)?
        .advance;
    style.size = 15.;
    style.weight = Weight::Normal;
    style.family = Family::Lucida;
    let line = text.measure_panel("\u{a0}", style)?;
    Ok(MenuMetrics {
        last_length_line_height: (line.font_ascent + line.font_descent).round(),
        // The nickname row already includes its border box; Play's 20px
        // margin follows it directly, without an extra synthetic descender.
        nickname_baseline_space: 0.,
        padded_play_label_width,
    })
}
pub fn prepare(extent: [u32; 2], dpr: f64, state: u8) -> Result<MenuFrame> {
    let mut text = TextCache::menu();
    let view = view(state)?;
    let layout = layout(&mut text, &view, extent, dpr)?;
    Menu::default().prepare(&mut text, &view, &layout, extent, dpr, None)
}
