//! Copied, bounded menu descriptions and interaction. No host handles or policy callbacks.
use crate::{
    menu_input::{EditIntent, EditorId, HostEditEffect, MenuInputState, TextRule},
    menu_layout::MenuRect,
    menu_theme::{ControlStyle, MenuTheme},
};

/// Stable control identity within a panel generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ControlId(pub u16);
/// Fixed-height list window. Only this range is copied, rasterised and drawn.
#[derive(Clone, Debug)]
pub struct ScrollList {
    first: usize,
    visible: usize,
}
impl ScrollList {
    /// Bound a window to at most 32 rows, independent of directory length.
    pub fn new(visible: usize) -> Self {
        Self {
            first: 0,
            visible: visible.clamp(1, 32),
        }
    }
    /// Clamp after refresh/resize and return only the visible row indices.
    pub fn window(&mut self, total: usize) -> std::ops::Range<usize> {
        self.first = self.first.min(total.saturating_sub(self.visible));
        self.first..(self.first + self.visible).min(total)
    }
    /// Move a whole number of rows, saturating at both ends.
    pub fn scroll(&mut self, rows: isize, total: usize) {
        self.first = self.first.saturating_add_signed(rows);
        self.window(total);
    }
    /// Keep a keyboard target visible.
    pub fn reveal(&mut self, index: usize, total: usize) {
        if index < self.first {
            self.first = index;
        }
        if index >= self.first + self.visible {
            self.first = index + 1 - self.visible;
        }
        self.window(total);
    }
}
/// Logical placement relative to the containing panel (or viewport).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Fractional anchor in the parent rectangle.
    pub anchor: [f64; 2],
    /// Logical offset from the anchor.
    pub offset: [f64; 2],
    /// Unscaled logical size.
    pub size: [f64; 2],
    /// Fractional transform origin in this rectangle.
    pub origin: [f64; 2],
    /// Sampled local scale, multiplied by the parent's scale.
    pub scale: f64,
}
impl Placement {
    /// Fixed logical rectangle relative to its parent.
    pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            anchor: [0.; 2],
            offset: [x, y],
            size: [width, height],
            origin: [0.; 2],
            scale: 1.,
        }
    }
    fn resolve(self, parent: MenuRect, scale: f64) -> Option<(MenuRect, f64)> {
        if self
            .anchor
            .into_iter()
            .chain(self.offset)
            .chain(self.size)
            .chain(self.origin)
            .chain([self.scale, scale])
            .any(|n| !n.is_finite())
            || self.size.iter().any(|n| *n < 0.)
            || self.scale <= 0.
            || scale <= 0.
        {
            return None;
        }
        let s = scale * self.scale;
        if !s.is_finite() || self.size.iter().any(|n| !(n * s).is_finite()) {
            return None;
        }
        let rect = MenuRect {
            x: parent.x
                + parent.width * self.anchor[0]
                + self.offset[0] * scale
                + self.origin[0] * self.size[0] * (scale - s),
            y: parent.y
                + parent.height * self.anchor[1]
                + self.offset[1] * scale
                + self.origin[1] * self.size[1] * (scale - s),
            width: self.size[0] * s,
            height: self.size[1] * s,
        };
        [rect.x, rect.y, rect.width, rect.height]
            .into_iter()
            .all(f64::is_finite)
            .then_some((rect, s))
    }
}
/// Concrete controls used by the mounted menu screens.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    /// One login link: server icon above an unboxed caption.
    IconLink {
        /// Visible caption and accessible name.
        label: String,
    },
    /// Full viewport modal hit shield.
    Backdrop,
    /// Passive themed panel surface.
    Surface,
    /// One visible row of a windowed list. Offscreen rows never enter the view.
    ListRow {
        /// Primary row text.
        label: String,
        /// Secondary status text.
        detail: String,
        /// Current committed selection.
        selected: bool,
        /// Measured latency, absent until supplied by the adapter.
        ping: Option<u32>,
    },
    /// Single-line label; long instructions are not result-panel text.
    Label {
        /// Label content.
        text: String,
    },
    /// Activatable action.
    Button {
        /// Button caption.
        label: String,
    },
    /// Single-line editor. Insets describe its content box within the holder.
    TextField {
        /// Stable registered editor.
        editor: EditorId,
        /// Empty-value hint.
        placeholder: String,
        /// Committed normalization.
        rule: TextRule,
        /// Logical left/top/right/bottom content padding.
        inset: [f64; 4],
        /// Fit the complete value into the content width; otherwise scroll.
        fit: bool,
    },
}
/// One copied control in paint/reading order.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    /// Stable ID.
    pub id: ControlId,
    /// Parent-relative geometry.
    pub placement: Placement,
    /// Optional clip in panel-local coordinates.
    pub clip: Option<MenuRect>,
    /// Explicit drawing/input visibility.
    pub visible: bool,
    /// Explicit input gate, independent of alpha.
    pub enabled: bool,
    /// Sampled opacity.
    pub alpha: f64,
    /// Accessible name, independent of artwork.
    pub name: String,
    /// Explicit Tab order; absent for passive labels.
    pub tab_order: Option<u16>,
    /// Theme role.
    pub style: ControlStyle,
    /// Copied content.
    pub kind: ControlKind,
}
/// A panel and its current policy generation. At most 64 controls.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelView {
    /// Stable panel ID.
    pub id: u16,
    /// Changing generation cancels capture and editor focus.
    pub generation: u64,
    /// Viewport-relative geometry.
    pub placement: Placement,
    /// Explicit mount gate.
    pub visible: bool,
    /// Explicit admission gate.
    pub enabled: bool,
    /// Sampled panel opacity.
    pub alpha: f64,
    /// Consume gameplay input while mounted.
    pub modal: bool,
    /// Initial keyboard target.
    pub initial_focus: Option<ControlId>,
    /// Preferred surviving parent control after closing a modal.
    pub restore_focus: Option<ControlId>,
    /// Bounded controls in paint order.
    pub controls: Vec<Control>,
}
/// Resolved control, retaining the shared rectangle/hit convention.
#[derive(Clone, Debug)]
pub struct ResolvedControl {
    /// Copied description.
    pub control: Control,
    /// Viewport rectangle.
    pub rect: MenuRect,
    /// Effective viewport clip.
    pub clip: MenuRect,
    /// Complete local scale.
    pub scale: f64,
}
impl ResolvedControl {
    /// Inverse map viewport input into the control's logical content.
    pub fn local(&self, point: [f64; 2]) -> [f64; 2] {
        [
            (point[0] - self.rect.x) / self.scale,
            (point[1] - self.rect.y) / self.scale,
        ]
    }
    /// Half-open hit after clipping; opacity does not gate input.
    pub fn hit(&self, point: [f64; 2]) -> bool {
        let p = self.local(point);
        self.clip.contains(point[0], point[1])
            && MenuRect {
                x: 0.,
                y: 0.,
                width: self.rect.width / self.scale,
                height: self.rect.height / self.scale,
            }
            .contains(p[0], p[1])
    }
}
/// Intersect axis-aligned clips using the same logical rectangle type as login.
pub fn intersection(a: MenuRect, b: MenuRect) -> MenuRect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    MenuRect {
        x,
        y,
        width: (a.x + a.width).min(b.x + b.width).max(x) - x,
        height: (a.y + a.height).min(b.y + b.height).max(y) - y,
    }
}
/// Actions exposed by the copied semantic view; bridges use the same input path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PanelAction {
    /// Focus a control.
    Focus,
    /// Activate a button.
    Activate,
    /// Edit a field through its registered editor.
    Edit,
}
/// Copied semantic node. Publishing it does not install a native OS bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelNode {
    /// Stable ID within the panel generation.
    pub id: ControlId,
    /// Shared existing role vocabulary.
    pub role: crate::menu_input::AccessibleRole,
    /// Policy-supplied name.
    pub name: String,
    /// Committed value, excluding raw preedit.
    pub value: String,
    /// Logical clipped bounds.
    pub bounds: MenuRect,
    /// Current logical focus.
    pub focused: bool,
    /// Current admission gate.
    pub enabled: bool,
    /// Supported actions.
    pub actions: Vec<PanelAction>,
}
impl PanelView {
    /// Project visible controls for a future native or browser semantic bridge.
    pub fn semantics(
        &self,
        viewport: [f64; 2],
        input: &PanelInputState,
        edit: &MenuInputState,
    ) -> Result<Vec<PanelNode>, &'static str> {
        use crate::menu_input::AccessibleRole;
        if !self.visible {
            return Ok(Vec::new());
        }
        Ok(self
            .resolve(viewport)?
            .into_iter()
            .filter(|r| r.control.visible)
            .map(|r| {
                let enabled = self.enabled && r.control.enabled;
                let (role, value, actions) = match r.control.kind {
                    ControlKind::Backdrop => (
                        AccessibleRole::Button,
                        String::new(),
                        vec![PanelAction::Activate],
                    ),
                    ControlKind::Surface => (AccessibleRole::Status, String::new(), vec![]),
                    ControlKind::ListRow { label, detail, .. } => (
                        AccessibleRole::Button,
                        format!("{label}, {detail}"),
                        vec![PanelAction::Focus, PanelAction::Activate],
                    ),
                    ControlKind::Label { text } => (AccessibleRole::Status, text, vec![]),
                    ControlKind::Button { .. } | ControlKind::IconLink { .. } => (
                        AccessibleRole::Button,
                        String::new(),
                        vec![PanelAction::Focus, PanelAction::Activate],
                    ),
                    ControlKind::TextField { editor, .. } => (
                        AccessibleRole::TextInput,
                        edit.registered_editor(editor)
                            .map_or_else(String::new, |e| e.text().into()),
                        vec![PanelAction::Focus, PanelAction::Edit],
                    ),
                };
                PanelNode {
                    id: r.control.id,
                    role,
                    name: r.control.name,
                    value,
                    bounds: intersection(r.rect, r.clip),
                    focused: enabled && input.focus() == Some(r.control.id),
                    enabled,
                    actions: if enabled { actions } else { vec![] },
                }
            })
            .collect())
    }
    /// Resolve bounded placements; reject duplicate IDs and malformed geometry.
    pub fn resolve(&self, viewport: [f64; 2]) -> Result<Vec<ResolvedControl>, &'static str> {
        if self.controls.len() > 64
            || viewport.iter().any(|n| !n.is_finite() || *n <= 0.)
            || !self.alpha.is_finite()
            || !(0. ..=1.).contains(&self.alpha)
        {
            return Err("Invalid panel");
        }
        let viewport = MenuRect {
            x: 0.,
            y: 0.,
            width: viewport[0],
            height: viewport[1],
        };
        let (panel, scale) = self
            .placement
            .resolve(viewport, 1.)
            .ok_or("Invalid panel placement")?;
        let mut ids = std::collections::BTreeSet::new();
        self.controls
            .iter()
            .map(|control| {
                if !ids.insert(control.id)
                    || !control.alpha.is_finite()
                    || !(0. ..=1.).contains(&control.alpha)
                {
                    return Err("Invalid control");
                }
                if let ControlKind::TextField { inset, .. } = control.kind
                    && (inset.iter().any(|n| !n.is_finite() || *n < 0.)
                        || control.placement.size[0] <= inset[0] + inset[2]
                        || control.placement.size[1] <= inset[1] + inset[3])
                {
                    return Err("Invalid field content box");
                }
                let (rect, s) = control
                    .placement
                    .resolve(panel, scale)
                    .ok_or("Invalid control placement")?;
                let clip = if let Some(r) = control.clip {
                    Placement::rect(r.x, r.y, r.width, r.height)
                        .resolve(panel, scale)
                        .ok_or("Invalid control clip")?
                        .0
                } else {
                    panel
                };
                Ok(ResolvedControl {
                    control: control.clone(),
                    rect,
                    clip: intersection(viewport, clip),
                    scale: s,
                })
            })
            .collect()
    }
}
/// Activation origin preserves the champion's empty-Enter distinction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Activation {
    /// Matching pointer release.
    Pointer,
    /// Enter, including an editor submission.
    Enter,
    /// Space activation on a focused button.
    Space,
}
/// Intent envelope; owners revalidate current generation and authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlIntent {
    /// Panel ID.
    pub panel: u16,
    /// Description generation.
    pub generation: u64,
    /// Stable control.
    pub control: ControlId,
    /// Activation origin.
    pub activation: Activation,
}
/// Copied text hit in UTF-8 byte offsets, measured with the painted font.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextHit {
    /// Nearest insertion boundary, used for caret placement and dragging.
    pub caret: usize,
    /// Character under the pointer; text length when past the final character.
    pub character: usize,
}
/// Generic focus/capture/damping, sampled by the existing core clock.
#[derive(Clone, Debug, Default)]
pub struct PanelInputState {
    identity: Option<(u16, u64)>,
    focus: Option<ControlId>,
    capture: Option<ControlId>,
    hovered: Option<ControlId>,
    progress: std::collections::BTreeMap<ControlId, [f64; 2]>,
    previous: Option<f64>,
    parent_focus: Option<(u16, u64, ControlId)>,
    modal: bool,
    synced_focus: Option<(u16, u64, ControlId)>,
    selection: MouseSelection,
}
impl PanelInputState {
    /// Set an available keyboard target through the same admission gate as Tab.
    pub fn focus_control(&mut self, panel: &PanelView, id: ControlId) {
        if panel
            .controls
            .iter()
            .any(|c| c.id == id && Self::available(panel, c))
        {
            self.focus = Some(id);
        }
    }
    /// Current logical focus.
    pub fn focus(&self) -> Option<ControlId> {
        self.focus
    }
    /// Current pointer capture, also used by host drag-selection.
    pub fn capture(&self) -> Option<ControlId> {
        self.capture
    }
    /// Restore an already sampled overlay (used by copied visual specimens).
    pub fn set_progress(&mut self, id: ControlId, progress: [f64; 2]) {
        self.progress.insert(id, progress.map(|v| v.clamp(0., 1.)));
    }
    /// Sampled hover/press opacity.
    pub fn progress(&self, id: ControlId) -> [f64; 2] {
        self.progress.get(&id).copied().unwrap_or([0.; 2])
    }
    fn available(panel: &PanelView, c: &Control) -> bool {
        panel.visible
            && panel.enabled
            && c.visible
            && c.enabled
            && !matches!(c.kind, ControlKind::Label { .. })
    }
    /// Reconcile a copied generation without resetting equal editors on animation.
    pub fn sync(&mut self, panel: &PanelView, edit: &mut MenuInputState) -> Vec<HostEditEffect> {
        let mut effects = Vec::new();
        if self.identity != Some((panel.id, panel.generation)) {
            if panel.modal
                && !self.modal
                && let (Some((id, generation)), Some(focus)) = (self.identity, self.focus)
            {
                self.parent_focus = Some((id, generation, focus));
            }
            self.blur();
            self.focus = None;
            self.progress.clear();
            effects.extend(edit.apply(EditIntent::Focus(None)));
            self.identity = Some((panel.id, panel.generation));
            self.focus = panel.initial_focus;
            if !panel.modal
                && let Some((id, generation, focus)) = self.parent_focus.take()
                && (id, generation) == (panel.id, panel.generation)
            {
                self.focus = panel.restore_focus.or(Some(focus));
            }
        }
        for c in &panel.controls {
            if let ControlKind::TextField { editor, rule, .. } = c.kind {
                edit.register(editor, rule, "");
            }
        }
        let valid = |id| {
            panel
                .controls
                .iter()
                .any(|c| c.id == id && Self::available(panel, c))
        };
        self.focus = self.focus.or(panel.initial_focus).filter(|id| valid(*id));
        self.capture = self.capture.filter(|id| valid(*id));
        self.hovered = self.hovered.filter(|id| valid(*id));
        self.progress
            .retain(|id, _| panel.controls.iter().any(|c| c.id == *id));
        let modal = panel.visible && panel.modal;
        if modal != self.modal {
            effects.extend(edit.apply(EditIntent::Modal(modal)));
            self.modal = modal;
        }
        effects.extend(self.sync_focus(panel, edit));
        effects
    }
    /// Synchronize the generic control focus into the editor owner.
    pub fn sync_focus(
        &mut self,
        panel: &PanelView,
        edit: &mut MenuInputState,
    ) -> Vec<HostEditEffect> {
        let editor = panel
            .controls
            .iter()
            .find(|c| Some(c.id) == self.focus)
            .and_then(|c| {
                if let ControlKind::TextField { editor, .. } = c.kind {
                    Some(editor)
                } else {
                    None
                }
            });
        let editor_changed = edit.focus() != editor;
        let mut effects = if editor_changed {
            edit.apply(EditIntent::Focus(editor))
        } else {
            Vec::new()
        };
        let focus = self.focus.map(|id| (panel.id, panel.generation, id));
        if focus != self.synced_focus && focus.is_some() && !(editor_changed && editor.is_some()) {
            MenuInputState::releases(&mut effects);
        }
        self.synced_focus = focus;
        effects
    }
    /// Blur cancels capture/hover; logical focus survives a window blur.
    pub fn blur(&mut self) {
        self.capture = None;
        self.hovered = None;
        self.selection = MouseSelection::default();
    }
    /// Set a semantic focus target, revalidated by `sync`.
    pub fn set_focus(&mut self, id: ControlId) {
        self.focus = Some(id);
    }
    /// Apply a measured field hit through the shared mouse-selection policy.
    #[allow(clippy::too_many_arguments)]
    pub fn select_text(
        &mut self,
        edit: &mut MenuInputState,
        editor: EditorId,
        hit: TextHit,
        point: [f64; 2],
        pressed: bool,
        shift: bool,
        now: f64,
        settings: ClickSettings,
    ) -> Vec<HostEditEffect> {
        let Some(control) = self.capture else {
            return Vec::new();
        };
        let Some(state) = edit.registered_editor(editor) else {
            return Vec::new();
        };
        let intent = self
            .selection
            .select(control, state, hit, point, pressed, shift, now, settings);
        intent.map_or_else(Vec::new, |intent| edit.apply(intent))
    }
    /// Explicit Tab order wraps within the mounted panel and skips unavailable controls.
    pub fn tab(&mut self, panel: &PanelView, reverse: bool) {
        let mut order: Vec<_> = panel
            .controls
            .iter()
            .filter(|c| Self::available(panel, c))
            .filter_map(|c| c.tab_order.map(|n| (n, c.id)))
            .collect();
        order.sort();
        if order.is_empty() {
            self.focus = None;
            return;
        }
        let index = order.iter().position(|(_, id)| Some(*id) == self.focus);
        let next = index.map_or(if reverse { order.len() - 1 } else { 0 }, |i| {
            if reverse {
                (i + order.len() - 1) % order.len()
            } else {
                (i + 1) % order.len()
            }
        });
        self.focus = Some(order[next].1);
    }
    /// Reverse-paint-order hit and capture; activate only on matching release.
    pub fn pointer(
        &mut self,
        panel: &PanelView,
        resolved: &[ResolvedControl],
        point: [f64; 2],
        down: Option<bool>,
    ) -> Option<ControlIntent> {
        let hit = resolved
            .iter()
            .rev()
            .find(|r| Self::available(panel, &r.control) && r.hit(point))
            .map(|r| r.control.id);
        self.hovered = hit;
        if down == Some(true) {
            if self.selection.last.is_some_and(|(id, ..)| Some(id) != hit) {
                self.selection = MouseSelection::default();
            }
            self.capture = hit;
            if hit.is_some() {
                self.focus = hit;
            }
        }
        if down == Some(false) {
            self.selection.release();
            let captured = self.capture.take();
            if let Some(control) = captured.filter(|id| Some(*id) == hit) {
                return Some(ControlIntent {
                    panel: panel.id,
                    generation: panel.generation,
                    control,
                    activation: Activation::Pointer,
                });
            }
        }
        None
    }
    /// Produce a generation-bound activation only for an available target.
    pub fn activate(&self, panel: &PanelView, activation: Activation) -> Option<ControlIntent> {
        let id = self.focus?;
        panel
            .controls
            .iter()
            .any(|c| c.id == id && Self::available(panel, c))
            .then_some(ControlIntent {
                panel: panel.id,
                generation: panel.generation,
                control: id,
                activation,
            })
    }
    /// Sample the existing login's per-callback linear overlay damping.
    pub fn step(
        &mut self,
        panel: &PanelView,
        now: f64,
        theme: MenuTheme,
    ) -> Result<(), &'static str> {
        if !now.is_finite() || self.previous.is_some_and(|p| now < p) {
            return Err("Invalid panel clock");
        }
        self.previous = Some(now);
        for c in &panel.controls {
            let p = self.progress.entry(c.id).or_insert([0.; 2]);
            for (i, target, rate) in [
                (0, self.hovered == Some(c.id), theme.hover_rate),
                (
                    1,
                    self.capture == Some(c.id) && self.hovered == Some(c.id),
                    theme.press_rate,
                ),
            ] {
                p[i] = if target && Self::available(panel, c) {
                    (p[i] + rate).min(1.)
                } else {
                    (p[i] - theme.release_rate).max(0.)
                };
            }
        }
        Ok(())
    }
}

/// Copied platform multi-click settings. Distances are half the double-click
/// rectangle, expressed in the same logical coordinates as the pointer.
#[derive(Clone, Copy, Debug)]
pub struct ClickSettings {
    /// Maximum milliseconds between presses.
    pub time_ms: f64,
    /// Maximum displacement on each axis.
    pub distance: [f64; 2],
}
impl Default for ClickSettings {
    fn default() -> Self {
        Self {
            time_ms: 500.,
            distance: [2.; 2],
        }
    }
}
#[derive(Clone, Debug, Default)]
struct MouseSelection {
    last: Option<(ControlId, f64, [f64; 2], u8)>,
    drag: Option<(usize, usize)>,
}
impl MouseSelection {
    fn release(&mut self) {
        self.drag = None;
    }
    #[allow(clippy::too_many_arguments)]
    fn select(
        &mut self,
        control: ControlId,
        editor: &crate::menu_input::TextEditState,
        hit: TextHit,
        point: [f64; 2],
        pressed: bool,
        shift: bool,
        now: f64,
        settings: ClickSettings,
    ) -> Option<EditIntent> {
        if editor.composing() {
            self.drag = None;
            return None;
        }
        let text = editor.text();
        let mut caret = hit.caret.min(text.len());
        while !text.is_char_boundary(caret) {
            caret -= 1;
        }
        if pressed {
            let count = self
                .last
                .filter(|(id, time, p, _)| {
                    *id == control
                        && now >= *time
                        && now - time <= settings.time_ms
                        && (0..2).all(|i| (point[i] - p[i]).abs() <= settings.distance[i])
                })
                .map_or(1, |(_, _, _, count)| count % 3 + 1);
            self.last = if shift {
                None
            } else {
                Some((control, now, point, count))
            };
            let range = if shift {
                (editor.selection().0, caret)
            } else if count == 3 {
                (0, text.len())
            } else if count == 2 {
                word_at(text, hit.character)
            } else {
                (caret, caret)
            };
            self.drag = Some(if shift { (range.0, range.0) } else { range });
            Some(EditIntent::Select {
                anchor: range.0,
                caret: range.1,
            })
        } else {
            let (left, right) = self.drag?;
            if self.last.is_some_and(|(_, _, p, _)| {
                (0..2).any(|i| (point[i] - p[i]).abs() > settings.distance[i])
            }) {
                self.last = None;
            }
            let (anchor, caret) = if left == right {
                (left, caret)
            } else if caret < left {
                (right, word_at(text, caret).0)
            } else if caret > right {
                (left, word_at(text, caret).1)
            } else {
                (left, right)
            };
            Some(EditIntent::Select { anchor, caret })
        }
    }
}
fn word_at(text: &str, caret: usize) -> (usize, usize) {
    let mut caret = caret.min(text.len());
    while !text.is_char_boundary(caret) {
        caret -= 1;
    }
    let start = if caret == text.len() {
        text.char_indices().next_back().map_or(0, |(i, _)| i)
    } else {
        caret
    };
    let class = |c: char| {
        if c.is_alphanumeric() || c == '_' {
            0
        } else if c.is_whitespace() {
            1
        } else {
            2
        }
    };
    let Some(ch) = text[start..].chars().next() else {
        return (0, 0);
    };
    let kind = class(ch);
    let left = text[..start]
        .char_indices()
        .rev()
        .find(|(_, c)| class(*c) != kind)
        .map_or(0, |(i, c)| i + c.len_utf8());
    let right = text[start..]
        .char_indices()
        .find(|(_, c)| class(*c) != kind)
        .map_or(text.len(), |(i, _)| start + i);
    (left, right)
}
