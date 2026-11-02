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
pub const ADDRESS: ControlId = ControlId(9);
pub const ADD_CUSTOM: ControlId = ControlId(10);
pub const ADDRESS_EDITOR: EditorId = EditorId(3);
pub const MAX_CUSTOM_SERVERS: usize = 32;
pub const MAX_CUSTOM_ADDRESS: usize = 512;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CustomServerOperation {
    Add(String),
    Remove(String),
}
const REMOVE_BASE: u16 = 8192;
/// The directory UI is available on ordinary login even without Play admission.
pub fn available(view: &crate::start_page::StartMenuView) -> bool {
    view.visible
        && view.ordinary_controls_visible
        && view.phase == crate::start_page::MenuPhase::Login
}
const ROW_BASE: u16 = 16;
const VISIBLE_ROWS: usize = 6;

/// Separate bounded menu preference, like the independent quality preference.
/// Official choices store IDs only; custom authorities are explicitly entered user data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ServerPreference {
    #[default]
    Automatic,
    Official(u16),
    Custom(String),
}
impl ServerPreference {
    pub fn official_id(&self) -> Option<u16> {
        if let Self::Official(id) = self {
            Some(*id)
        } else {
            None
        }
    }
    pub fn custom_address(&self) -> Option<&str> {
        if let Self::Custom(address) = self {
            Some(address)
        } else {
            None
        }
    }
    pub fn restore(value: Option<&str>) -> Self {
        if let Some(address) = value
            .and_then(|v| v.strip_prefix("2:custom:"))
            .filter(|v| valid_custom_record(v))
        {
            return Self::Custom(address.to_owned());
        }
        value
            .and_then(|s| s.strip_prefix("1:"))
            .filter(|s| !s.is_empty() && s.len() <= 5 && s.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|s| s.parse().ok())
            .map_or(Self::Automatic, Self::Official)
    }
    pub fn serialize(&self) -> String {
        match self {
            Self::Automatic => "1:auto".into(),
            Self::Official(id) => format!("1:{id}"),
            Self::Custom(address) => format!("2:custom:{address}"),
        }
    }
}
fn valid_custom_record(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CUSTOM_ADDRESS
        && !value.chars().any(char::is_whitespace)
        && !value.chars().any(char::is_control)
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
    directory_ready: bool,
    dirty: bool,
    pending_rows: Option<Vec<DirectoryRow>>,
    pub custom: Vec<String>,
    pub custom_error: String,
    official_context: bool,
    add_request: bool,
    custom_operations: Vec<CustomServerOperation>,
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
            directory_ready: false,
            dirty: false,
            pending_rows: None,
            custom: Vec::new(),
            custom_error: String::new(),
            official_context: false,
            add_request: false,
            custom_operations: Vec::new(),
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
    pub fn official_context(&self) -> bool {
        self.official_context
    }
    pub fn can_play(&self) -> bool {
        self.selection.custom_address().is_some() || self.official_context
    }
    /// Enter official context and request acquisition once. Direct startup skips this.
    pub fn start_directory(&mut self) {
        self.official_context = true;
        // Default launch uses official Automatic; saved custom entries remain
        // available for direct selection. Startup never writes the preference.
        if self.selection.custom_address().is_some() {
            self.selection = ServerPreference::Automatic;
        }
        if !self.directory_requested {
            self.request = true;
            self.directory_requested = true;
        }
    }
    pub fn open(&mut self) {
        if self.open {
            return;
        }
        self.open = true;
        self.generation = self.generation.wrapping_add(1);
        self.search.clear();
        self.list = ScrollList::new(VISIBLE_ROWS);
        if let Some(rows) = self.pending_rows.take() {
            self.rows = rows;
        }
        self.rows
            .sort_by_key(|r| (r.ping.is_none(), r.ping.unwrap_or(0), r.server.id));
        // Completed samples are reused; only unfinished failures re-arm acquisition.
        if self.official_context && !self.directory_requested {
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
    /// Retry incomplete acquisition on the next open, never in a failure loop.
    /// A successful empty directory is still a completed, reusable result.
    pub fn directory_failed(&mut self, status: &str) {
        if !self.directory_ready {
            // Provisional samples belong to the failed attempt, not the next retry.
            if !self.rows.is_empty() {
                self.generation = self.generation.wrapping_add(1);
            }
            self.rows.clear();
            self.pending_rows = None;
            self.list = ScrollList::new(VISIBLE_ROWS);
            self.directory_requested = false;
            self.request = false;
            self.status = status.to_owned();
        }
    }
    pub fn take_preference(&mut self) -> Option<String> {
        std::mem::take(&mut self.dirty).then(|| self.selection.serialize())
    }
    pub fn take_add_request(&mut self) -> bool {
        std::mem::take(&mut self.add_request)
    }
    pub fn take_custom_operations(&mut self) -> Vec<CustomServerOperation> {
        std::mem::take(&mut self.custom_operations)
    }
    /// Storage may merge edits from other instances; invalidate row captures if indices change.
    pub fn refresh_custom(&mut self, servers: Vec<String>) {
        if self.custom != servers {
            self.custom = servers;
            self.generation = self.generation.wrapping_add(1);
            self.list.window(self.total());
        }
    }
    /// The adapter supplies an authority validated by the same parser as --server.
    pub fn add_custom(&mut self, address: String) -> Result<(), &'static str> {
        if !valid_custom_record(&address) {
            return Err("Enter a valid host:port");
        }
        if self.custom.contains(&address) {
            self.custom_operations
                .push(CustomServerOperation::Add(address));
            return Ok(());
        }
        if self.custom.len() >= MAX_CUSTOM_SERVERS {
            return Err("Custom server limit: 32");
        }
        self.custom.push(address.clone());
        self.custom_operations
            .push(CustomServerOperation::Add(address));
        self.generation = self.generation.wrapping_add(1);
        self.list.reveal(self.custom.len() - 1, self.total());
        Ok(())
    }
    fn remove_custom(&mut self, index: usize) {
        let address = self.custom.remove(index);
        if self.selection.custom_address() == Some(address.as_str()) {
            self.selection = ServerPreference::Automatic;
            self.dirty = true;
        }
        self.custom_operations
            .push(CustomServerOperation::Remove(address));
        self.generation = self.generation.wrapping_add(1);
        self.list.window(self.total());
    }
    fn total(&self) -> usize {
        self.custom.len() + self.entry_iter().count()
    }
    pub fn step(&mut self, now: f64) -> Result<(), &'static str> {
        crate::menu_transition::step(&mut self.alpha, &mut self.previous, self.open, now)
            .map_err(|_| "Invalid picker clock")
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
        self.official_context = true;
        self.pending_rows = None;
        self.directory_ready = true;
        self.directory_requested = true;
        self.request = false;
        self.generation = self.generation.wrapping_add(1); // Cancel captures across reordered rows.
        self.list.window(self.total());
        self.status = if self.rows.is_empty() {
            "No servers available".into()
        } else {
            format!("{} servers / measured latency", self.rows.len())
        };
        Ok(())
    }
    /// Progress is provisional until the sweep completes. Keep row identity/order
    /// while visible (including exit fade); re-sort on the next open instead.
    /// See docs/server-picker.md for the focus/capture/scroll rule.
    pub fn update_directory(
        &mut self,
        servers: &[Server],
        pings: &BTreeMap<u16, u32>,
        complete: bool,
    ) -> Result<(), &'static str> {
        if servers.len() > 4096 {
            return Err("Directory server limit");
        }
        let mut rows = ranked(servers, pings);
        let old_by_id: BTreeMap<_, _> = self
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| (row.server.id, (index, row.server.active)))
            .collect();
        let same_ids = rows.len() == self.rows.len()
            && rows
                .iter()
                .all(|row| old_by_id.contains_key(&row.server.id));
        if self.visible() && !self.rows.is_empty() && same_ids {
            self.pending_rows = Some(rows.clone());
            // Progress belongs to the same decoded directory. Retain the visible
            // representative's active flag too, so deduplication cannot move rows.
            rows.sort_by_key(|row| old_by_id[&row.server.id].0);
            for row in &mut rows {
                row.server.active = old_by_id[&row.server.id].1;
            }
        } else {
            self.pending_rows = None;
            if !self.rows.is_empty() && !same_ids {
                self.generation = self.generation.wrapping_add(1);
            }
        }
        self.rows = rows;
        self.directory_ready = complete;
        self.directory_requested = true;
        self.request = false;
        self.list.window(self.total());
        self.status = if !complete {
            format!("{} servers / measuring latency...", self.rows.len())
        } else if self.rows.is_empty() {
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
            .filter(|r| r.server.active || self.selection.official_id() == Some(r.server.id))
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
        self.start_directory();
        self.selection = id.map_or(ServerPreference::Automatic, ServerPreference::Official);
        self.dirty = true;
        self.close();
    }
    pub fn activate(&mut self, intent: ControlIntent) -> bool {
        if !self.open || intent.panel != 2 || intent.generation != self.generation {
            return false;
        }
        match intent.control {
            BACKDROP | CLOSE => self.close(),
            ADD_CUSTOM => self.add_request = true,
            ADDRESS if intent.activation != Activation::Pointer => self.add_request = true,
            AUTOMATIC => self.choose(None),
            ControlId(n) if n >= REMOVE_BASE => {
                let index = usize::from(n - REMOVE_BASE);
                let total = self.total();
                if index >= self.custom.len() || !self.list.window(total).contains(&index) {
                    return false;
                }
                self.remove_custom(index);
            }
            ControlId(n) if n >= ROW_BASE => {
                let total = self.total();
                if !self.list.window(total).contains(&usize::from(n - ROW_BASE)) {
                    return false;
                }
                let index = usize::from(n - ROW_BASE);
                if index < self.custom.len() {
                    self.selection = ServerPreference::Custom(self.custom[index].clone());
                    self.official_context = false;
                    self.request = false;
                    self.dirty = true;
                    self.close();
                    return true;
                }
                let id = self
                    .entries()
                    .get(index - self.custom.len())
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
        self.list.scroll(rows, self.total());
    }
    pub fn navigate(&mut self, current: Option<ControlId>, delta: isize) -> ControlId {
        let total = self.total();
        if total == 0 {
            return if self.official_context {
                AUTOMATIC
            } else {
                ADDRESS
            };
        }
        let index = current
            .and_then(|c| {
                if c.0 >= REMOVE_BASE {
                    c.0.checked_sub(REMOVE_BASE)
                        .filter(|i| usize::from(*i) < self.custom.len())
                } else {
                    c.0.checked_sub(ROW_BASE)
                        .filter(|i| usize::from(*i) < total)
                }
            })
            .map_or_else(
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
        let scale = (viewport[0] / 620.).min(viewport[1] / 680.).min(1.)
            * crate::menu_transition::scale(self.alpha);
        let origin = [
            (viewport[0] - 560. * scale) / 2.,
            (viewport[1] - 620. * scale) / 2.,
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
            620.,
            S::SettingsSurface,
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
            "Servers",
            None,
            ControlKind::Label {
                text: "Servers".into(),
            },
        );
        add(
            ADDRESS,
            24.,
            66.,
            376.,
            42.,
            S::Field,
            "Server address",
            Some(0),
            ControlKind::TextField {
                editor: ADDRESS_EDITOR,
                placeholder: "host:port".into(),
                rule: TextRule::ServerAddress,
                inset: [12., 0., 12., 0.],
                fit: false,
            },
        );
        add(
            ADD_CUSTOM,
            416.,
            66.,
            120.,
            42.,
            S::StudioAction,
            "Add custom server",
            Some(1),
            ControlKind::Button {
                label: "Add".into(),
            },
        );
        add(
            ControlId(12),
            24.,
            112.,
            512.,
            26.,
            S::HelpText,
            "Custom server error",
            None,
            ControlKind::Label {
                text: self.custom_error.clone(),
            },
        );
        add(
            SEARCH,
            24.,
            150.,
            140.,
            42.,
            S::Field,
            "Server ID",
            Some(2),
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
            176.,
            150.,
            360.,
            42.,
            if self.selection == ServerPreference::Automatic && self.official_context {
                S::StudioSave
            } else {
                S::StudioAction
            },
            "Automatic, best ping",
            Some(3),
            ControlKind::Button {
                label: "Automatic (best ping)".into(),
            },
        );
        add(
            ControlId(6),
            24.,
            196.,
            512.,
            26.,
            S::HelpText,
            "Directory status",
            None,
            ControlKind::Label {
                text: if self.official_context && !self.directory_ready {
                    self.status.clone()
                } else {
                    String::new()
                },
            },
        );
        let total = self.total();
        let window = self.list.window(total);
        let official = self.entries();
        let mut y = 250.;
        let mut previous_custom = None;
        for (slot, index) in window.clone().enumerate() {
            let custom = index < self.custom.len();
            if previous_custom != Some(custom) {
                if previous_custom.is_some() {
                    y += 24.;
                }
                add(
                    ControlId(13 + u16::from(!custom)),
                    24.,
                    y - 26.,
                    512.,
                    24.,
                    S::HelpText,
                    "Server group",
                    None,
                    ControlKind::Label {
                        text: if custom { "Custom" } else { "Official" }.into(),
                    },
                );
            }
            previous_custom = Some(custom);
            let (label, detail, selected, ping) = if custom {
                let address = &self.custom[index];
                let label = if address.chars().count() > 40 {
                    format!("{}…", address.chars().take(40).collect::<String>())
                } else {
                    address.clone()
                };
                (
                    label,
                    String::new(),
                    self.selection.custom_address() == Some(address),
                    None,
                )
            } else {
                let row = official[index - self.custom.len()];
                (
                    format!("ID {}", row.server.id),
                    row.ping
                        .map_or_else(|| "— ms".into(), |p| format!("{p} ms")),
                    self.selection.official_id() == Some(row.server.id),
                    row.ping,
                )
            };
            add(
                ControlId(ROW_BASE + index as u16),
                24.,
                y,
                if custom { 398. } else { 512. },
                40.,
                S::ListRow,
                if custom { "Custom server" } else { "Server" },
                Some(5 + slot as u16 * 2),
                ControlKind::ListRow {
                    label,
                    detail,
                    selected,
                    ping,
                },
            );
            if custom {
                add(
                    ControlId(REMOVE_BASE + index as u16),
                    434.,
                    y,
                    102.,
                    40.,
                    S::StudioAction,
                    "Remove custom server",
                    Some(6 + slot as u16 * 2),
                    ControlKind::Button {
                        label: "Remove".into(),
                    },
                );
            }
            y += 46.;
        }
        add(
            ControlId(7),
            24.,
            550.,
            370.,
            28.,
            S::HelpText,
            "List position",
            None,
            ControlKind::Label {
                text: if total == 0 {
                    "No servers available".into()
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
            578.,
            136.,
            38.,
            S::StudioCancel,
            "Close",
            Some(40),
            ControlKind::Button {
                label: "Close".into(),
            },
        );
        add(
            ControlId(8),
            24.,
            582.,
            350.,
            28.,
            S::HelpText,
            "Current selection",
            None,
            ControlKind::Label {
                text: match &self.selection {
                    ServerPreference::Automatic => if self.official_context {
                        ""
                    } else {
                        "No server selected"
                    }
                    .into(),
                    ServerPreference::Official(id) => format!("Selected ID: {id}"),
                    ServerPreference::Custom(address) => {
                        format!("Direct: {}", address.chars().take(32).collect::<String>())
                    }
                },
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
            initial_focus: Some(ADDRESS),
            restore_focus: None,
            controls,
        }
    }
}
