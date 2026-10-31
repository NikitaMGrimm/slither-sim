#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
//! Authored glyph-half regressions through the production native pointer seam.
use slither_client::{
    champion::{ChampionState, MESSAGE},
    login_controls::{self, NICKNAME},
    menu_controls::{PanelInputState, PanelView},
    menu_input::{EditIntent, EditorId},
    menu_preferences::MenuQuality,
    session_policy::{Command, SessionPolicy},
    start_page::{MenuLocale, StartMenuState},
};
use slither_wgpu_slice::{
    editor_geometry::field_geometry, menu_input_native::NativeMenuInput, menu_specimen,
    text::TextCache,
};
use std::collections::BTreeMap;

struct Field {
    panel: PanelView,
    controls: PanelInputState,
    input: NativeMenuInput,
    text: TextCache,
    scroll: BTreeMap<EditorId, f64>,
    editor: EditorId,
}
impl Field {
    fn new(editor: EditorId, value: &str, dpr: f64) -> Self {
        let mut text = TextCache::menu();
        let panel = if editor == EditorId::Nickname {
            let view = StartMenuState::new(MenuLocale::English, true, MenuQuality::High).view(true);
            let layout = menu_specimen::layout(
                &mut text,
                &view,
                [(960. * dpr) as u32, (540. * dpr) as u32],
                dpr,
            )
            .unwrap();
            login_controls::panel(&view, &layout, [960., 540.])
        } else {
            let mut session = SessionPolicy::new();
            session
                .prepare_controls(
                    &[
                        Command {
                            opcode: b'a',
                            argument: 0,
                        },
                        Command {
                            opcode: b'v',
                            argument: 1,
                        },
                    ],
                    0.,
                )
                .unwrap();
            session.commit_controls().unwrap();
            let mut champion = ChampionState::default();
            champion.observe(1, &session.view());
            champion.step(0., 1.).unwrap();
            champion.panel(&session.view(), [960., 540.])
        };
        let mut controls = PanelInputState::default();
        let mut input = NativeMenuInput::default();
        controls.sync(&panel, &mut input.state);
        input.state.reset_editor(editor, value);
        Self {
            panel,
            controls,
            input,
            text,
            scroll: BTreeMap::new(),
            editor,
        }
    }
    fn double_click(&mut self, byte: usize, fraction: f64, dpr: f64) {
        let id = if self.editor == EditorId::Nickname {
            NICKNAME
        } else {
            MESSAGE
        };
        let resolved = self.panel.resolve([960., 540.]).unwrap();
        let field = resolved.iter().find(|r| r.control.id == id).unwrap();
        let (_, geometry) =
            field_geometry(&mut self.text, field, &self.input.state, &mut self.scroll)
                .unwrap()
                .unwrap();
        let value = self.input.state.editor(self.editor).text();
        let left = self
            .text
            .measure_panel(&value[..byte], geometry.style)
            .unwrap()
            .advance;
        let right = if byte < value.len() {
            self.text
                .measure_panel(&value[..byte + 1], geometry.style)
                .unwrap()
                .advance
        } else {
            left + 10.
        };
        let point = [
            geometry.text_origin()[0] + left + (right - left) * fraction,
            geometry.rect.y + geometry.rect.height / 2.,
        ]
        .map(|v| (v * dpr).round() / dpr);
        for down in [true, false, true, false] {
            self.input
                .panel_pointer(
                    &mut self.controls,
                    &self.panel,
                    &mut self.scroll,
                    &mut self.text,
                    [960., 540.],
                    point,
                    Some(down),
                    dpr,
                )
                .unwrap();
        }
    }
}
fn word_halves(editor: EditorId) {
    let mut failures = Vec::new();
    for dpr in [1., 1.5, 2.] {
        for (value, byte, fractions, expected, replacement) in [
            ("one_12 two", 5, &[0.2, 0.8][..], (0, 6), "X two"),
            ("one_12   two", 7, &[0.2, 0.8][..], (6, 9), "one_12Xtwo"),
            ("one_12 two", 10, &[0.8][..], (7, 10), "one_12 X"),
            ("", 0, &[0.8][..], (0, 0), "X"),
        ] {
            for fraction in fractions {
                let mut field = Field::new(editor, value, dpr);
                field.double_click(byte, *fraction, dpr);
                let selected = field.input.state.editor(editor).selection();
                field.input.state.apply(EditIntent::KeyText("X".into()));
                let replaced = field.input.state.editor(editor).text();
                if selected != expected || replaced != replacement {
                    failures.push(format!("DPR {dpr}, {value:?}, byte {byte}, fraction {fraction}: selection {selected:?}, replacement {replaced:?}"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{editor:?}: {failures:#?}");
}
#[test]
fn nickname_word_right_half() {
    word_halves(EditorId::Nickname);
}
#[test]
fn champion_word_right_half() {
    word_halves(EditorId::Champion);
}
