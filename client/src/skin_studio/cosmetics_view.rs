//! Fixed catalogue, windowed image cells and roving focus. No art or wire authority.
use super::*;

impl StudioState {
    /// Current local selection, separate from the wire appearance snapshot.
    pub fn preview_cosmetic(&self) -> Option<u8> {
        self.draft
            .as_ref()
            .map_or(self.committed.cosmetic, |d| d.record.cosmetic)
    }
    /// All 33 choices are selectable, including missing art.
    pub fn select_cosmetic(&mut self, cosmetic: Option<u8>) {
        if self.interactive() && cosmetic.is_none_or(|id| id < 32) {
            self.draft.as_mut().expect("mounted draft").record.cosmetic = cosmetic;
            self.cosmetic_focus = cosmetic.map_or(0, |id| usize::from(id) + 1);
            self.cosmetic_window.reveal(self.cosmetic_focus / 4, 9);
        }
    }
    fn cosmetic_layout(viewport: [f64; 2]) -> (MenuRect, usize) {
        let b = Self::bounds(viewport);
        let wide = b.width >= 640.;
        let preview = Self::preview_bounds(viewport);
        let top = if wide {
            b.y + 112.
        } else {
            (b.y + b.height - 224.).max(preview.y + preview.height + 12.)
        };
        let area = MenuRect {
            x: b.x + if wide { b.width - 280. } else { 24. },
            y: top,
            width: if wide { 256. } else { b.width - 48. },
            height: (b.y + b.height - if wide { 84. } else { 112. } - top).max(56.),
        };
        let rows = (((area.height - 28.) / 28.).floor() as usize).clamp(1, 3);
        (area, rows)
    }
    fn cosmetic_list(&self, viewport: [f64; 2]) -> ScrollList {
        let (_, rows) = Self::cosmetic_layout(viewport);
        let mut window = ScrollList::new(rows);
        window.scroll(self.cosmetic_window.clone().window(9).start as isize, 9);
        window.reveal(self.cosmetic_focus / 4, 9);
        window
    }
    /// At most twelve cells; very short windows retain 28px targets and scroll fewer rows.
    pub fn cosmetic_range(&self, viewport: [f64; 2]) -> std::ops::Range<usize> {
        let range = self.cosmetic_list(viewport).window(9);
        range.start * 4..(range.end * 4).min(33)
    }
    /// Only the visible window and one metadata row are projected.
    pub fn cosmetic_metadata_range(&self, viewport: [f64; 2]) -> std::ops::Range<usize> {
        let range = self.cosmetic_range(viewport);
        range.start..(range.end + 4).min(33)
    }
    /// Replacing a window invalidates a pressed cell without retargeting its release.
    pub fn scroll_cosmetics(&mut self, rows: isize, viewport: [f64; 2]) {
        if self.interactive() && self.section == StudioSection::Cosmetics {
            let before = self.cosmetic_range(viewport);
            self.cosmetic_window = self.cosmetic_list(viewport);
            self.cosmetic_window.scroll(rows, 9);
            let rows = self.cosmetic_window.clone().window(9);
            let range = rows.start * 4..(rows.end * 4).min(33);
            self.cosmetic_focus = self.cosmetic_focus.clamp(range.start, range.end - 1);
            if range != before {
                self.generation += 1;
            }
        }
    }
    pub(super) fn navigate_cosmetic(
        &mut self,
        focus: ControlId,
        direction: Navigation,
    ) -> ControlId {
        if !(COSMETIC_BASE..COSMETIC_BASE + 33).contains(&focus.0) {
            return focus;
        }
        use Navigation::*;
        let at = usize::from(focus.0 - COSMETIC_BASE);
        self.cosmetic_focus = match direction {
            Left => at.saturating_sub(1),
            Right => (at + 1).min(32),
            Up => at.saturating_sub(4),
            Down => (at + 4).min(32),
            Home => 0,
            End => 32,
        };
        self.cosmetic_window.reveal(self.cosmetic_focus / 4, 9);
        ControlId(COSMETIC_BASE + self.cosmetic_focus as u16)
    }
    pub(super) fn mount_cosmetics(&self, panel: &mut PanelView, viewport: [f64; 2]) {
        panel
            .controls
            .retain(|c| ![ControlId(3), PREVIOUS, NEXT].contains(&c.id));
        let b = Self::bounds(viewport);
        let wide = b.width >= 640.;
        let (area, rows) = Self::cosmetic_layout(viewport);
        let cell_h = ((area.height - 28.) / rows as f64).clamp(28., 64.);
        let range = self.cosmetic_range(viewport);
        for index in range.clone() {
            let ordinal = index - range.start;
            let cosmetic = (index > 0).then(|| (index - 1) as u8);
            panel.controls.push(Control {
                id: ControlId(COSMETIC_BASE + index as u16),
                placement: Placement::rect(
                    area.x + (ordinal % 4) as f64 * area.width / 4. + 2.,
                    area.y + (ordinal / 4) as f64 * cell_h,
                    area.width / 4. - 4.,
                    cell_h,
                ),
                clip: Some(area),
                visible: true,
                enabled: self.interactive(),
                alpha: self.section_alpha,
                name: cosmetic.map_or_else(|| "None".into(), |id| format!("Cosmetic {id}")),
                tab_order: (index == self.cosmetic_focus).then_some(1),
                style: ControlStyle::SelectedCell,
                kind: ControlKind::CosmeticCell {
                    cosmetic,
                    selected: cosmetic == self.preview_cosmetic(),
                },
            });
        }
        for (id, label, offset, enabled) in [
            (COSMETIC_PREVIOUS, "<", 0., range.start > 0),
            (COSMETIC_NEXT, ">", area.width - 40., range.end < 33),
        ] {
            panel.controls.push(Control {
                id,
                placement: Placement::rect(
                    area.x + offset,
                    area.y + rows as f64 * cell_h,
                    40.,
                    28.,
                ),
                clip: None,
                visible: true,
                enabled: self.interactive() && enabled,
                alpha: self.section_alpha,
                name: if id == COSMETIC_PREVIOUS {
                    "Previous cosmetics"
                } else {
                    "Next cosmetics"
                }
                .into(),
                tab_order: Some(if id == COSMETIC_PREVIOUS { 2 } else { 3 }),
                style: ControlStyle::StudioAction,
                kind: ControlKind::Button {
                    label: label.into(),
                },
            });
        }
        panel.controls.push(Control {
            id: ControlId(22),
            placement: Placement::rect(
                area.x + 42.,
                area.y + rows as f64 * cell_h,
                area.width - 84.,
                28.,
            ),
            clip: None,
            visible: true,
            enabled: false,
            alpha: self.section_alpha,
            name: "Cosmetics window".into(),
            tab_order: None,
            style: ControlStyle::SettingsHelp,
            kind: ControlKind::Label {
                text: format!("{}–{} / 33", range.start + 1, range.end),
            },
        });
        if let Some(c) = panel.controls.iter_mut().find(|c| c.id == ControlId(4)) {
            if wide {
                c.placement.size[0] = b.width - 328.;
            }
            c.style = ControlStyle::SettingsHelp;
            if let ControlKind::Label { text } = &mut c.kind
                && text.is_empty()
            {
                *text = self
                    .preview_cosmetic()
                    .map_or_else(|| "No cosmetic".into(), |id| format!("Cosmetic {id}"));
                c.name = text.clone();
            }
        }
    }
}
