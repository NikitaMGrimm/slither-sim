//! Shared measurement for painted text, drag-selection and IME candidate placement.
use crate::{
    Result,
    text::{TextCache, TextStyle},
};
use slither_client::{
    menu_input::TextEditState,
    menu_layout::{FieldTextPlacement, MenuRect, single_line_field_placement},
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
        inset: [f64; 2],
        style: TextStyle,
        scroll: f64,
    ) -> Result<Self> {
        let (anchor, caret) = editor.selection();
        let range = anchor.min(caret)..anchor.max(caret);
        let mut display = editor.text().to_owned();
        let mut cursor = caret;
        let mut preedit = None;
        if let Some(p) = editor.preedit() {
            display.replace_range(range.clone(), &p.text);
            cursor = range.start + p.selection.map_or(p.text.len(), |(_, end)| end);
            let start = text.measure_panel(&display[..range.start], style)?.advance;
            let end = text
                .measure_panel(&display[..range.start + p.text.len()], style)?
                .advance;
            preedit = Some([start, end]);
        }
        let caret = text.measure_panel(&display[..cursor], style)?.advance;
        let metrics = text.measure_panel(&display, style)?;
        let width = metrics.advance;
        let placement = single_line_field_placement(
            field,
            inset,
            style.size,
            metrics.font_ascent,
            metrics.font_descent,
        );
        let height = (field.height - 2. * inset[1]).max(0.);
        let rect = MenuRect {
            x: placement.origin[0],
            y: placement.caret_top + (placement.caret_height - height) / 2.,
            width: (field.x + field.width - inset[0] - placement.origin[0]).max(0.),
            height,
        };
        let scroll = scroll
            .max(caret - rect.width + 1.)
            .min(caret)
            .clamp(0., (width - rect.width + 1.).max(0.));
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
    pub fn hit(&self, text: &mut TextCache, committed: &str, x: f64) -> Result<usize> {
        let x = x - self.rect.x + self.scroll;
        let mut previous = 0.;
        for (byte, ch) in committed.char_indices() {
            let end = byte + ch.len_utf8();
            let next = text.measure_panel(&committed[..end], self.style)?.advance;
            if x < (previous + next) / 2. {
                return Ok(byte);
            }
            previous = next;
        }
        Ok(committed.len())
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
