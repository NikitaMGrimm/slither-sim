//! Login layout from the measured text the core layout depends on.
use crate::{
    Result,
    text::{Family, Slant, TextCache, TextStyle, Weight},
};
use slither_client::{
    menu_layout::{MenuLayout, MenuMetrics},
    start_page::StartMenuView,
};

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
