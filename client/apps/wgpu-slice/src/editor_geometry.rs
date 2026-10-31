//! Shared measurement for painted text, drag-selection and IME candidate placement.
use crate::{
    Result,
    text::{TextCache, TextStyle},
};
use slither_client::{
    menu_controls::TextHit,
    menu_input::TextEditState,
    menu_layout::{FieldTextPlacement, MenuRect, field_content_rect, single_line_field_placement},
};

pub struct EditorGeometry {
    pub display: String,
    pub caret: f64,
    pub selection: [f64; 2],
    pub preedit: Option<[f64; 2]>,
    pub scroll: f64,
    pub rect: MenuRect,
    pub placement: FieldTextPlacement,
    pub style: TextStyle,
}
impl EditorGeometry {
    pub fn new(
        text: &mut TextCache,
        editor: &TextEditState,
        field: MenuRect,
        inset: [f64; 4],
        mut style: TextStyle,
        scroll: f64,
        fit: bool,
    ) -> Result<Self> {
        let (anchor, caret) = editor.selection();
        let range = anchor.min(caret)..anchor.max(caret);
        let mut display = editor.text().to_owned();
        let mut cursor = caret;
        let mut preedit = None;
        if let Some(p) = editor.preedit() {
            display.replace_range(range.clone(), &p.text);
            cursor = range.start + p.selection.map_or(p.text.len(), |(_, end)| end);
            preedit = Some([range.start, range.start + p.text.len()]);
        }
        let rect = field_content_rect(field, inset);
        if fit {
            let width = text.measure_panel(&display, style)?.advance;
            let available = rect.width;
            if width > available {
                style.size *= available / width;
            }
        }
        let preedit = preedit
            .map(|[start, end]| -> Result<[f64; 2]> {
                Ok([
                    text.measure_panel(&display[..start], style)?.advance,
                    text.measure_panel(&display[..end], style)?.advance,
                ])
            })
            .transpose()?;
        let caret = text.measure_panel(&display[..cursor], style)?.advance;
        let metrics = text.measure_panel(&display, style)?;
        let width = metrics.advance;
        let placement = single_line_field_placement(
            rect,
            [0.; 2],
            style.size,
            metrics.font_ascent,
            metrics.font_descent,
        );
        let scroll = if fit {
            0.
        } else {
            scroll
                .max(caret - rect.width + 1.)
                .min(caret)
                .clamp(0., (width - rect.width + 1.).max(0.))
        };
        let selection = if preedit.is_some() {
            [caret; 2]
        } else {
            [
                text.measure_panel(&display[..range.start], style)?.advance,
                text.measure_panel(&display[..range.end], style)?.advance,
            ]
        };
        Ok(Self {
            display,
            caret,
            selection,
            preedit,
            scroll,
            rect,
            placement,
            style,
        })
    }
    pub fn hit(&self, text: &mut TextCache, committed: &str, x: f64) -> Result<TextHit> {
        if x <= self.rect.x {
            return Ok(TextHit {
                caret: 0,
                character: 0,
            });
        }
        if x >= self.rect.x + self.rect.width {
            return Ok(TextHit {
                caret: committed.len(),
                character: committed.len(),
            });
        }
        let x = x - self.rect.x + self.scroll;
        let mut previous = 0.;
        for (byte, ch) in committed.char_indices() {
            let end = byte + ch.len_utf8();
            let next = text.measure_panel(&committed[..end], self.style)?.advance;
            if x < next {
                return Ok(TextHit {
                    caret: if x < (previous + next) / 2. {
                        byte
                    } else {
                        end
                    },
                    character: byte,
                });
            }
            previous = next;
        }
        Ok(TextHit {
            caret: committed.len(),
            character: committed.len(),
        })
    }
    pub fn text_origin(&self) -> [f64; 2] {
        [
            self.placement.origin[0] - self.scroll,
            self.placement.origin[1],
        ]
    }
    fn range_rect(&self, [left, right]: [f64; 2]) -> MenuRect {
        MenuRect {
            x: self.placement.origin[0] + left - self.scroll,
            y: self.placement.caret_top,
            width: right - left,
            height: self.placement.caret_height,
        }
    }
    pub fn selection_rect(&self) -> MenuRect {
        self.range_rect(self.selection)
    }
    pub fn preedit_rect(&self) -> Option<MenuRect> {
        self.preedit.map(|range| {
            let r = self.range_rect(range);
            MenuRect {
                y: r.y + r.height - 1.,
                height: 1.,
                ..r
            }
        })
    }
    /// Physical-pixel caret, using the same content clip as the painter.
    pub fn snapped_cursor_rect(&self, dpr: f64) -> MenuRect {
        let r = self.cursor_rect();
        let x = ((r.x * dpr).round()).max((self.rect.x * dpr).ceil());
        let y = ((r.y * dpr).round()).max((self.rect.y * dpr).ceil());
        let right = (x + (r.width * dpr).round().max(1.))
            .min(((self.rect.x + self.rect.width) * dpr).floor());
        let bottom = (y + (r.height * dpr).round().max(1.))
            .min(((self.rect.y + self.rect.height) * dpr).floor());
        MenuRect {
            x: x / dpr,
            y: y / dpr,
            width: (right - x).max(0.) / dpr,
            height: (bottom - y).max(0.) / dpr,
        }
    }
    pub fn cursor_rect(&self) -> MenuRect {
        MenuRect {
            x: self.placement.origin[0]
                + (self.caret - self.scroll).clamp(0., self.rect.width - 1.),
            y: self.placement.caret_top,
            width: 1.,
            height: self.placement.caret_height,
        }
    }
}

/// Resolve the same field description used by the painter for pointer and IME input.
pub fn field_geometry(
    text: &mut TextCache,
    resolved: &slither_client::menu_controls::ResolvedControl,
    edit: &slither_client::menu_input::MenuInputState,
    scroll: &mut std::collections::BTreeMap<slither_client::menu_input::EditorId, f64>,
) -> Result<Option<(slither_client::menu_input::EditorId, EditorGeometry)>> {
    use slither_client::{menu_controls::ControlKind, menu_theme::MenuTheme};
    let ControlKind::TextField {
        editor, inset, fit, ..
    } = resolved.control.kind
    else {
        return Ok(None);
    };
    let mut style = MenuTheme::default().text_style(resolved.control.style);
    style.size *= resolved.scale;
    let geometry = EditorGeometry::new(
        text,
        edit.editor(editor),
        resolved.rect,
        inset.map(|v| v * resolved.scale),
        style,
        scroll.get(&editor).copied().unwrap_or(0.),
        fit,
    )?;
    scroll.insert(editor, geometry.scroll);
    Ok(Some((editor, geometry)))
}
