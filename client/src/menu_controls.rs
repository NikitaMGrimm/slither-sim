//! Copied, bounded menu descriptions. No host handles or policy callbacks.
use crate::{menu_layout::MenuRect, menu_theme::ControlStyle};

/// Stable control identity within a panel generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ControlId(pub u16);
/// Fixed-height list window. Only this range is copied, rasterised and drawn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    /// Settings icon above the same unboxed caption as the server link.
    SettingsLink {
        /// Visible caption.
        label: String,
    },
    /// One section in a roving tab set.
    SectionTab {
        /// Visible section title.
        label: String,
        /// Currently mounted section.
        selected: bool,
    },
    /// One palette or windowed pattern-run cell; its colour comes from authored sources.
    Swatch {
        /// Normal palette ID.
        palette: u8,
        /// Expanded run count; absent for palette cells.
        count: Option<usize>,
        /// Roving selected item.
        selected: bool,
    },
    /// One item in the local-art window; None is a real selectable item.
    CosmeticCell {
        /// Local art ID, never a wire field.
        cosmetic: Option<u8>,
        /// Committed draft selection, independent of roving focus.
        selected: bool,
    },
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
        /// Empty-value hint.
        placeholder: String,
        /// Logical left/top/right/bottom content padding.
        inset: [f64; 4],
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
    /// Initial keyboard target.
    pub initial_focus: Option<ControlId>,
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
impl PanelView {
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
    /// Explicit next checked value from the activated description, for toggles only.
    pub checked: Option<bool>,
    /// Panel ID.
    pub panel: u16,
    /// Description generation.
    pub generation: u64,
    /// Stable control.
    pub control: ControlId,
    /// Activation origin.
    pub activation: Activation,
}
