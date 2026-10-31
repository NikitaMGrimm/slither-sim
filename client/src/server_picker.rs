//! Server picker policy. Opening requests directory work only, never Play.
#![allow(missing_docs)]
use crate::{
    menu_controls::*,
    menu_input::{EditorId, TextRule},
    menu_theme::ControlStyle as S,
    server_directory::Server,
};
use std::collections::BTreeMap;

pub const SEARCH: ControlId = ControlId(2);
pub const AUTOMATIC: ControlId = ControlId(3);
pub const CLOSE: ControlId = ControlId(4);
pub const BACKDROP: ControlId = ControlId(0);
pub const SEARCH_EDITOR: EditorId = EditorId(2);
/// The directory UI is available on ordinary login even without Play admission.
pub fn available(view: &crate::start_page::StartMenuView) -> bool {
    view.visible
        && view.ordinary_controls_visible
        && view.phase == crate::start_page::MenuPhase::Login
}
const ROW_BASE: u16 = 16;
const VISIBLE_ROWS: usize = 6;

/// Separate bounded menu preference, like the independent quality preference.
/// Store an ID, never a stale endpoint or measured latency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerPreference(pub Option<u16>);
impl ServerPreference {
    pub fn restore(value: Option<&str>) -> Self {
        Self(
            value
                .and_then(|s| s.strip_prefix("1:"))
                .filter(|s| !s.is_empty() && s.len() <= 5 && s.bytes().all(|c| c.is_ascii_digit()))
                .and_then(|s| s.parse().ok()),
        )
    }
    pub fn serialize(self) -> String {
        self.0
            .map_or_else(|| "1:auto".into(), |id| format!("1:{id}"))
    }
}
#[derive(Clone, Debug)]
pub struct DirectoryRow {
    pub server: Server,
    pub ping: Option<u32>,
}
/// Dedupe IDs by fastest address; unknown pings sort last. Ties are deterministic.
pub fn ranked(servers: &[Server], pings: &BTreeMap<u16, u32>) -> Vec<DirectoryRow> {
    let mut by_id = BTreeMap::<u16, DirectoryRow>::new();
    for server in servers {
        let row = DirectoryRow {
            server: server.clone(),
            ping: pings.get(&server.cluster).copied(),
        };
        let key = |r: &DirectoryRow| {
            (
                r.ping.is_none(),
                r.ping.unwrap_or(0),
                std::cmp::Reverse(r.server.cluster),
                r.server.address,
            )
        };
        if by_id.get(&server.id).is_none_or(|old| key(&row) < key(old)) {
            by_id.insert(server.id, row);
        }
    }
    let mut rows: Vec<_> = by_id.into_values().collect();
    rows.sort_by_key(|r| (r.ping.is_none(), r.ping.unwrap_or(0), r.server.id));
    rows
}
#[derive(Clone, Debug)]
pub struct ServerPicker {
    pub selection: ServerPreference,
    pub rows: Vec<DirectoryRow>,
    pub list: ScrollList,
    pub search: String,
    pub status: String,
    pub alpha: f64,
    pub generation: u64,
    open: bool,
    previous: Option<f64>,
    request: bool,
    directory_requested: bool,
    changed: bool,
    dirty: bool,
}
impl Default for ServerPicker {
    fn default() -> Self {
        Self {
            selection: ServerPreference::default(),
            rows: Vec::new(),
            list: ScrollList::new(VISIBLE_ROWS),
            search: String::new(),
            status: "No directory loaded".into(),
            alpha: 0.,
            generation: 0,
            open: false,
            previous: None,
            request: false,
            directory_requested: false,
            changed: false,
            dirty: false,
        }
    }
}
impl ServerPicker {
    pub fn visible(&self) -> bool {
        self.open || self.alpha > 0.
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn open(&mut self) {
        if self.open {
            return;
        }
        self.open = true;
        self.generation = self.generation.wrapping_add(1);
        self.search.clear();
        self.list = ScrollList::new(VISIBLE_ROWS);
        // Opens re-rank retained samples; they never refresh acquisition or pings.
        if !self.directory_requested {
            self.request = true;
            self.directory_requested = true;
        }
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn take_directory_request(&mut self) -> bool {
        std::mem::take(&mut self.request)
    }
    pub fn take_preference(&mut self) -> Option<String> {
        std::mem::take(&mut self.dirty).then(|| self.selection.serialize())
    }
    pub fn step(&mut self, now: f64) -> Result<(), &'static str> {
        if !now.is_finite() || self.previous.is_some_and(|p| now < p) {
            return Err("Invalid picker clock");
        }
        let delta = self.previous.map_or(0., |p| ((now - p) / 8.).min(5.));
        self.previous = Some(now);
        self.alpha = (self.alpha
            + if self.open {
                0.02 * delta
            } else {
                -0.025 * delta
            })
        .clamp(0., 1.);
        Ok(())
    }
    pub fn replace(
        &mut self,
        servers: &[Server],
        pings: &BTreeMap<u16, u32>,
    ) -> Result<(), &'static str> {
        if servers.len() > 4096 {
            return Err("Directory server limit");
        }
        self.rows = ranked(servers, pings);
        self.directory_requested = true;
        self.request = false;
        self.generation = self.generation.wrapping_add(1); // Cancel captures across reordered rows.
        self.list.window(self.entries().len());
        self.status = if self.rows.is_empty() {
            "No servers available".into()
        } else {
            format!("{} servers / measured latency", self.rows.len())
        };
        Ok(())
    }
    pub fn entries(&self) -> Vec<&DirectoryRow> {
        self.entry_iter().collect()
    }
    fn entry_iter(&self) -> impl Iterator<Item = &DirectoryRow> {
        self.rows
            .iter()
            .filter(|r| r.server.active || self.selection.0 == Some(r.server.id))
    }
    pub fn search(&mut self, value: &str) -> bool {
        let value =
            TextRule::NumericServer.normalize_units(&value.encode_utf16().collect::<Vec<_>>());
        if value == self.search || !self.open {
            return false;
        }
        self.search = value;
        // Numeric conversion treats the empty input as zero, matching the source rule.
        let id = if self.search.is_empty() {
            Some(0)
        } else {
            self.search.parse().ok()
        };
        if let Some(id) = id.filter(|id| self.rows.iter().any(|r| r.server.id == *id)) {
            self.choose(Some(id));
            return true;
        }
        false
    }
    fn choose(&mut self, id: Option<u16>) {
        self.selection = ServerPreference(id);
        self.changed = true;
        self.dirty = true;
        self.close();
    }
    pub fn activate(&mut self, intent: ControlIntent) -> bool {
        if !self.open || intent.panel != 2 || intent.generation != self.generation {
            return false;
        }
        match intent.control {
            BACKDROP | CLOSE => self.close(),
            AUTOMATIC => self.choose(None),
            ControlId(n) if n >= ROW_BASE => {
                let total = self.entries().len();
                if !self.list.window(total).contains(&usize::from(n - ROW_BASE)) {
                    return false;
                }
                let id = self
                    .entries()
                    .get(usize::from(n - ROW_BASE))
                    .map(|r| r.server.id);
                if let Some(id) = id {
                    self.choose(Some(id));
                }
            }
            _ => return false,
        }
        true
    }
    pub fn scroll(&mut self, rows: isize) {
        self.list.scroll(rows, self.entries().len());
    }
    pub fn navigate(&mut self, current: Option<ControlId>, delta: isize) -> ControlId {
        let total = self.entries().len();
        if total == 0 {
            return AUTOMATIC;
        }
        let index = current.and_then(|c| c.0.checked_sub(ROW_BASE)).map_or_else(
            || {
                if delta == isize::MIN {
                    0
                } else if delta < 0 || delta == isize::MAX {
                    total - 1
                } else {
                    0
                }
            },
            |i| usize::from(i).saturating_add_signed(delta).min(total - 1),
        );
        self.list.reveal(index, total);
        ControlId(ROW_BASE + index as u16)
    }
    pub fn panel(&mut self, viewport: [f64; 2]) -> PanelView {
        let scale = (viewport[0] / 620.).min(viewport[1] / 580.).min(1.)
            * (0.9 + 0.1 * self.alpha.powf(0.4));
        let origin = [
            (viewport[0] - 560. * scale) / 2.,
            (viewport[1] - 520. * scale) / 2.,
        ];
        let mut controls = Vec::new();
        let mut add = |id, x, y, w, h, style, name: &str, tab, kind| {
            let mut placement = Placement::rect(origin[0] + x * scale, origin[1] + y * scale, w, h);
            placement.scale = scale;
            controls.push(Control {
                id,
                placement,
                clip: None,
                visible: true,
                enabled: true,
                alpha: 1.,
                name: name.into(),
                tab_order: tab,
                style,
                kind,
            });
        };
        add(
            BACKDROP,
            0.,
            0.,
            0.,
            0.,
            S::Surface,
            "Close server picker",
            None,
            ControlKind::Backdrop,
        );
        add(
            ControlId(1),
            0.,
            0.,
            560.,
            520.,
            S::Surface,
            "Server picker",
            None,
            ControlKind::Surface,
        );
        add(
            ControlId(5),
            24.,
            16.,
            512.,
            36.,
            S::Heading,
            "Choose server",
            None,
            ControlKind::Label {
                text: "Choose server".into(),
            },
        );
        add(
            SEARCH,
            24.,
            66.,
            152.,
            42.,
            S::Field,
            "Server ID",
            Some(0),
            ControlKind::TextField {
                editor: SEARCH_EDITOR,
                placeholder: "Server ID".into(),
                rule: TextRule::NumericServer,
                inset: [12., 0., 12., 0.],
                fit: false,
            },
        );
        add(
            AUTOMATIC,
            192.,
            66.,
            344.,
            42.,
            if self.selection.0.is_none() {
                S::PrimaryButton
            } else {
                S::SecondaryButton
            },
            "Automatic, best ping",
            Some(1),
            ControlKind::Button {
                label: "Automatic · best ping".into(),
            },
        );
        add(
            ControlId(6),
            24.,
            116.,
            512.,
            26.,
            S::HelpText,
            "Directory status",
            None,
            ControlKind::Label {
                text: self.status.clone(),
            },
        );
        let total = self.entry_iter().count();
        let window = self.list.window(total);
        let rows = self
            .entry_iter()
            .skip(window.start)
            .take(window.len())
            .cloned()
            .collect::<Vec<_>>();
        for (slot, row) in rows.iter().enumerate() {
            let index = window.start + slot;
            add(
                ControlId(ROW_BASE + index as u16),
                24.,
                150. + slot as f64 * 46.,
                512.,
                40.,
                S::ListRow,
                "Server",
                Some(2 + slot as u16),
                ControlKind::ListRow {
                    label: format!("ID {}", row.server.id),
                    detail: row
                        .ping
                        .map_or_else(|| "— ms".into(), |p| format!("{p} ms")),
                    selected: self.selection.0 == Some(row.server.id),
                    ping: row.ping,
                },
            );
        }
        add(
            ControlId(7),
            24.,
            436.,
            370.,
            28.,
            S::HelpText,
            "List position",
            None,
            ControlKind::Label {
                text: if total == 0 {
                    "Enter a numeric ID to select".into()
                } else {
                    format!(
                        "{}–{} of {} · scroll or use arrows",
                        window.start + 1,
                        window.end,
                        total
                    )
                },
            },
        );
        add(
            CLOSE,
            400.,
            466.,
            136.,
            38.,
            S::SecondaryButton,
            "Close",
            Some(40),
            ControlKind::Button {
                label: "Close".into(),
            },
        );
        add(
            ControlId(8),
            24.,
            470.,
            350.,
            28.,
            S::HelpText,
            "Current selection",
            None,
            ControlKind::Label {
                text: self.selection.0.map_or_else(
                    || "Automatic selection".into(),
                    |id| format!("Selected server ID: {id}"),
                ),
            },
        );
        controls[0].placement = Placement::rect(0., 0., viewport[0], viewport[1]);
        controls[0].alpha = 0.88;
        PanelView {
            id: 2,
            generation: self.generation,
            placement: Placement::rect(0., 0., viewport[0], viewport[1]),
            visible: self.visible(),
            // Keep visual states through exit; activation still rechecks `open`.
            enabled: self.visible(),
            alpha: self.alpha,
            modal: self.visible(),
            initial_focus: Some(SEARCH),
            restore_focus: None,
            controls,
        }
    }
}
