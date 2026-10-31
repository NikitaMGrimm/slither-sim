//! Shared measurement for painted text, drag-selection and IME candidate placement.
use crate::{
    Result,
    text::{TextCache, TextStyle},
};
use slither_client::{menu_input::TextEditState, menu_layout::MenuRect};

pub struct EditorGeometry {
    pub display: String,
    pub caret: f64,
    pub selection: [f64; 2],
    pub preedit: Option<[f64; 2]>,
    pub scroll: f64,
    pub rect: MenuRect,
    pub style: TextStyle,
}
impl EditorGeometry {
    pub fn new(
        text: &mut TextCache,
        editor: &TextEditState,
        rect: MenuRect,
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
        let width = text.measure_panel(&display, style)?.advance;
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
    pub fn cursor_rect(&self) -> MenuRect {
        MenuRect {
            x: self.rect.x + (self.caret - self.scroll).clamp(0., self.rect.width - 1.),
            width: 1.,
            ..self.rect
        }
    }
}
