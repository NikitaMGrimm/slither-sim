//! Native login controls copied from the existing layout and return projection.
use crate::{
    menu_controls::*,
    menu_input::{EditorId, TextRule},
    menu_layout::{MenuElement as E, MenuLayout},
    menu_theme::ControlStyle,
    start_page::{MenuPhase, StartMenuView},
};
/// Nickname field identity.
pub const NICKNAME: ControlId = ControlId(0);
/// Deferred Play action identity.
pub const PLAY: ControlId = ControlId(1);
/// Quality toggle identity.
pub const QUALITY: ControlId = ControlId(2);
/// Server directory link identity.
pub const SERVER: ControlId = ControlId(3);
/// Build controls without adding admission or return-phase policy.
pub fn panel(view: &StartMenuView, layout: &MenuLayout, viewport: [f64; 2]) -> PanelView {
    let holder = layout.logical_rect(E::NicknameHolder);
    let input = layout.logical_rect(E::Nickname);
    // The native field retains #358's extra 2px inline text inset.
    let nickname_inset = [
        input.x - holder.x + 2.,
        input.y - holder.y,
        holder.x + holder.width - input.x - input.width + 2.,
        holder.y + holder.height - input.y - input.height,
    ];
    let control = |id, element, scale, style, alpha, visible, kind, name: String| {
        let r = layout.viewport_rect(element);
        let mut placement = Placement::rect(r.x, r.y, r.width / scale, r.height / scale);
        placement.scale = scale;
        Control {
            id,
            placement,
            clip: None,
            visible,
            enabled: if id == PLAY {
                !view.play_disabled
            } else if id == SERVER {
                crate::server_picker::available(view)
            } else {
                !view.disabled
            },
            alpha,
            name,
            tab_order: Some(id.0),
            style,
            kind,
        }
    };
    PanelView {
        id: 0,
        generation: 0,
        placement: Placement::rect(0., 0., viewport[0], viewport[1]),
        visible: view.visible,
        enabled: true,
        alpha: view.login_alpha,
        modal: false,
        initial_focus: Some(NICKNAME),
        restore_focus: Some(NICKNAME),
        controls: vec![
            control(
                NICKNAME,
                E::NicknameHolder,
                layout.scale,
                ControlStyle::Nickname,
                view.nickname_alpha,
                view.ordinary_controls_visible,
                ControlKind::TextField {
                    editor: EditorId::Nickname,
                    placeholder: view.wording.nickname.into(),
                    rule: TextRule::Nickname,
                    inset: nickname_inset,
                    fit: true,
                },
                view.wording.nickname.into(),
            ),
            control(
                PLAY,
                E::Play,
                layout.scale,
                ControlStyle::LoginPlay,
                view.play_alpha
                    * view.play_return_alpha
                    * if view.phase == MenuPhase::Connecting {
                        0.38
                    } else {
                        1.
                    },
                view.ordinary_controls_visible,
                ControlKind::Button {
                    label: format!("\u{a0}{}\u{a0}", view.play_label),
                },
                view.play_label.into(),
            ),
            control(
                QUALITY,
                E::Quality,
                1.,
                ControlStyle::QualityButton,
                1.,
                true,
                ControlKind::Button {
                    label: if view.quality == crate::menu_preferences::MenuQuality::High {
                        "High quality"
                    } else {
                        "Low quality"
                    }
                    .into(),
                },
                view.wording.graphics_quality.into(),
            ),
            control(
                SERVER,
                E::Server,
                1.,
                ControlStyle::MenuLink,
                view.skin_alpha,
                view.ordinary_controls_visible,
                ControlKind::IconLink {
                    label: "Choose Server".into(),
                },
                "Choose server".into(),
            ),
        ],
    }
}
