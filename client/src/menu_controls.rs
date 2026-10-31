//! Copied, bounded menu descriptions and interaction. No host handles or policy callbacks.
use crate::{
    menu_input::{EditIntent, EditorId, HostEditEffect, MenuInputState, TextRule},
    menu_layout::MenuRect,
    menu_theme::{ControlStyle, MenuTheme},
};

/// Stable control identity within a panel generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ControlId(pub u16);
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
/// The three kinds with real adopters in this slice.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
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
        /// Logical content insets.
        inset: [f64; 2],
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
                    ControlKind::Label { text } => (AccessibleRole::Status, text, vec![]),
                    ControlKind::Button { .. } => (
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
                        || control.placement.size[0] <= 2. * inset[0]
                        || control.placement.size[1] <= 2. * inset[1])
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
}
impl PanelInputState {
    /// Current logical focus.
    pub fn focus(&self) -> Option<ControlId> {
        self.focus
    }
    /// Current pointer capture, also used by host drag-selection.
    pub fn capture(&self) -> Option<ControlId> {
        self.capture
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
        self.modal = panel.modal && panel.visible;
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
        self.focus = self.focus.filter(|id| valid(*id));
        self.capture = self.capture.filter(|id| valid(*id));
        self.hovered = self.hovered.filter(|id| valid(*id));
        self.progress
            .retain(|id, _| panel.controls.iter().any(|c| c.id == *id));
        effects.extend(edit.apply(EditIntent::Modal(panel.visible && panel.modal)));
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
        let mut effects = edit.apply(EditIntent::Focus(editor));
        let focus = self.focus.map(|id| (panel.id, panel.generation, id));
        if focus != self.synced_focus && focus.is_some() {
            MenuInputState::releases(&mut effects);
        }
        self.synced_focus = focus;
        effects
    }
    /// Blur cancels capture/hover; logical focus survives a window blur.
    pub fn blur(&mut self) {
        self.capture = None;
        self.hovered = None;
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
            self.capture = hit;
            if hit.is_some() {
                self.focus = hit;
            }
        }
        if down == Some(false) {
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
    /// Damping shares the menu's elapsed/8, clamped core-step seam.
    pub fn step(
        &mut self,
        panel: &PanelView,
        now: f64,
        theme: MenuTheme,
    ) -> Result<(), &'static str> {
        if !now.is_finite() || self.previous.is_some_and(|p| now < p) {
            return Err("Invalid panel clock");
        }
        let delta = self.previous.map_or(0., |p| ((now - p) / 8.).clamp(0., 5.));
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
                let target = f64::from(target && Self::available(panel, c));
                p[i] += (target - p[i]) * (rate * delta).min(1.);
            }
        }
        Ok(())
    }
}
