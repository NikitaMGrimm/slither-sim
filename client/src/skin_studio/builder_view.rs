//! Concrete section and windowed builder controls; no platform input policy.
use super::*;
use crate::skin_builder::{MAX_COLOURS, PALETTES};

/// Grid/tab/strip navigation shared by native input and authored tests.
#[derive(Clone, Copy)]
pub enum Navigation {
    /// Previous column or section.
    Left,
    /// Next column or section.
    Right,
    /// Previous grid row.
    Up,
    /// Next grid row.
    Down,
    /// First item.
    Home,
    /// Last item.
    End,
}
impl StudioState {
    /// Clipped editor area leaves the preview, status and footer stationary.
    pub fn editor_bounds(viewport: [f64; 2]) -> MenuRect {
        let b = Self::bounds(viewport);
        let wide = b.width >= 640.;
        let top = if wide {
            b.y + 112.
        } else {
            (b.y + b.height - 224.)
                .max(Self::preview_bounds(viewport).y + Self::preview_bounds(viewport).height + 12.)
        };
        MenuRect {
            x: b.x + if wide { b.width - 280. } else { 24. },
            y: top,
            width: if wide { 256. } else { b.width - 48. },
            height: (b.y + b.height - 112. - top).max(28.),
        }
    }
    fn editor_window(&self, viewport: [f64; 2]) -> ScrollList {
        let mut window =
            ScrollList::new((Self::editor_bounds(viewport).height / 28.).floor() as usize);
        let first = self.editor_scroll.clone().window(9).start;
        window.scroll(first as isize, 9);
        window
    }
    /// Scroll only active builder content; capture is cancelled by the adapter.
    pub fn scroll_editor(&mut self, rows: isize, viewport: [f64; 2]) {
        if self.interactive() && self.section == StudioSection::Builder {
            self.editor_scroll = self.editor_window(viewport);
            self.editor_scroll.scroll(rows, 9);
        }
    }
    /// Keep the roving item or a Tab destination visible without moving the preview/footer.
    pub fn reveal_control(&mut self, id: ControlId, viewport: [f64; 2]) {
        if self.section != StudioSection::Builder {
            return;
        }
        let row = if id.0 >= RUN_BASE {
            let index = usize::from(id.0 - RUN_BASE);
            let total = self
                .draft
                .as_ref()
                .map_or(0, |d| d.builder.pattern().runs().len());
            self.run_window.reveal(index, total);
            7
        } else if id.0 >= SWATCH_BASE {
            PALETTES
                .iter()
                .position(|p| SWATCH_BASE + u16::from(*p) == id.0)
                .unwrap_or(0)
                / 6
        } else if [ADD, REMOVE, RECOLOUR, RESET].contains(&id) {
            8
        } else {
            return;
        };
        self.editor_scroll = self.editor_window(viewport);
        self.editor_scroll.reveal(row, 9);
    }
    /// Roving focus never appends, saves or samples entropy.
    pub fn navigate(&mut self, focus: ControlId, direction: Navigation, now: f64) -> ControlId {
        use Navigation::*;
        if !self.interactive() {
            return focus;
        }
        if [BUILT_IN_TAB, BUILDER_TAB].contains(&focus) {
            let section = match direction {
                Left | Home => StudioSection::BuiltIn,
                Right | End => StudioSection::Builder,
                _ => return focus,
            };
            self.set_section(section, now);
            return if section == StudioSection::Builder {
                BUILDER_TAB
            } else {
                BUILT_IN_TAB
            };
        }
        if self.section != StudioSection::Builder {
            return focus;
        }
        let builder = &mut self.draft.as_mut().expect("mounted draft").builder;
        let step = |index: usize, total: usize, columns: usize| match direction {
            Left => index.saturating_sub(1),
            Right => (index + 1).min(total.saturating_sub(1)),
            Up => {
                if index >= columns {
                    index - columns
                } else {
                    index
                }
            }
            Down => (index + columns).min(total.saturating_sub(1)),
            Home => 0,
            End => total.saturating_sub(1),
        };
        if focus.0 >= RUN_BASE && !builder.pattern().runs().is_empty() {
            builder.selected = step(builder.selected, builder.pattern().runs().len(), 1);
            ControlId(RUN_BASE + builder.selected as u16)
        } else if let Some(index) = PALETTES
            .iter()
            .position(|p| SWATCH_BASE + u16::from(*p) == focus.0)
        {
            builder.palette = step(index, PALETTES.len(), 6);
            ControlId(SWATCH_BASE + u16::from(PALETTES[builder.palette]))
        } else {
            focus
        }
    }
    pub(super) fn mount_sections(&self, panel: &mut PanelView, viewport: [f64; 2]) {
        let b = Self::bounds(viewport);
        panel.controls.retain(|c| c.id != ControlId(2));
        for (id, label, selected, x) in [
            (
                BUILT_IN_TAB,
                "Built-in".to_owned(),
                self.section == StudioSection::BuiltIn,
                24.,
            ),
            (
                BUILDER_TAB,
                "Builder".to_owned(),
                self.section == StudioSection::Builder,
                b.width / 2. + 4.,
            ),
        ] {
            panel.controls.push(Control {
                id,
                placement: Placement::rect(b.x + x, b.y + 68., (b.width - 56.) / 2., 32.),
                clip: None,
                visible: true,
                enabled: self.interactive(),
                alpha: 1.,
                name: label.clone(),
                tab_order: selected.then_some(0),
                style: ControlStyle::StudioAction,
                kind: ControlKind::SectionTab { label, selected },
            });
        }
        panel.initial_focus = Some(if self.section == StudioSection::Builder {
            BUILDER_TAB
        } else {
            BUILT_IN_TAB
        });
        for c in &mut panel.controls {
            match c.id {
                PREVIOUS => c.tab_order = Some(1),
                NEXT => c.tab_order = Some(2),
                CANCEL => c.tab_order = Some(10),
                SAVE => c.tab_order = Some(11),
                _ => {}
            }
        }
        if self.section != StudioSection::Builder {
            for control in &mut panel.controls {
                if [ControlId(3), PREVIOUS, NEXT].contains(&control.id) {
                    control.alpha = self.section_alpha;
                }
            }
            return;
        }
        panel
            .controls
            .retain(|c| ![ControlId(3), PREVIOUS, NEXT].contains(&c.id));
        let Some(draft) = &self.draft else {
            return;
        };
        let builder = &draft.builder;
        if let Some(control) = panel.controls.iter_mut().find(|c| c.id == ControlId(4))
            && let ControlKind::Label { text } = &mut control.kind
            && text.is_empty()
        {
            let count = builder.pattern().colours();
            *text = if count > MAX_COLOURS {
                format!("{count} colours")
            } else {
                format!("{count} / 47 colours")
            };
            control.name = text.clone();
        }
        let area = Self::editor_bounds(viewport);
        let offset = self.editor_window(viewport).window(9).start as f64 * 28.;
        let cell = |id, row: usize, column: usize, columns: usize, kind, name, order, enabled| {
            let inset = if matches!(kind, ControlKind::Button { .. }) {
                2.
            } else {
                0.
            };
            Control {
                id,
                placement: Placement::rect(
                    area.x + column as f64 * area.width / columns as f64 + inset,
                    area.y + row as f64 * 28. - offset,
                    area.width / columns as f64 - 2. * inset,
                    28.,
                ),
                clip: Some(area),
                visible: true,
                enabled: self.interactive() && enabled,
                alpha: self.section_alpha,
                name,
                tab_order: order,
                style: ControlStyle::StudioAction,
                kind,
            }
        };
        for (index, palette) in PALETTES.into_iter().enumerate() {
            panel.controls.push(cell(
                ControlId(SWATCH_BASE + u16::from(palette)),
                index / 6,
                index % 6,
                6,
                ControlKind::Swatch {
                    palette,
                    count: None,
                    selected: index == builder.palette,
                },
                format!("Palette {palette}"),
                (index == builder.palette).then_some(1),
                true,
            ));
        }
        for index in self
            .run_window
            .clone()
            .window(builder.pattern().runs().len())
        {
            let run = builder.pattern().runs()[index];
            let column = index
                - self
                    .run_window
                    .clone()
                    .window(builder.pattern().runs().len())
                    .start;
            panel.controls.push(cell(
                ControlId(RUN_BASE + index as u16),
                7,
                column + 1,
                6,
                ControlKind::Swatch {
                    palette: run.palette,
                    count: Some(run.count),
                    selected: index == builder.selected,
                },
                format!(
                    "Run {}, palette {}, {} colours",
                    index + 1,
                    run.palette,
                    run.count
                ),
                (index == builder.selected).then_some(2),
                true,
            ));
        }
        for (column, id, label, enabled) in [
            (0, RUN_PREVIOUS, "‹", builder.selected > 0),
            (
                5,
                RUN_NEXT,
                "›",
                builder.selected + 1 < builder.pattern().runs().len(),
            ),
        ] {
            panel.controls.push(cell(
                id,
                7,
                column,
                6,
                ControlKind::Button {
                    label: label.into(),
                },
                if column == 0 {
                    "Previous run".into()
                } else {
                    "Next run".into()
                },
                None,
                enabled,
            ));
        }
        for (column, (id, label, enabled)) in [
            (ADD, "Add", builder.pattern().colours() < MAX_COLOURS),
            (REMOVE, "Remove", !builder.pattern().runs().is_empty()),
            (RECOLOUR, "Recolour", !builder.pattern().runs().is_empty()),
            (RESET, "Reset", !builder.pattern().runs().is_empty()),
        ]
        .into_iter()
        .enumerate()
        {
            panel.controls.push(cell(
                id,
                8,
                column,
                4,
                ControlKind::Button {
                    label: label.into(),
                },
                label.into(),
                Some(3 + column as u16),
                enabled,
            ));
        }
    }
}
