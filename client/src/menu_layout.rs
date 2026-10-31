//! Logical login geometry. Source rationale and authored fixtures: docs/start-menu.md.
//! All dimensions are viewport/CSS pixels; world backing dimensions never enter here.

/// A logical axis-aligned rectangle, with half-open hit boundaries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl MenuRect {
    /// Test a logical point.
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }
}

/// Logical single-line text origin and a text-height caret before pixel snapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldTextPlacement {
    /// Content left edge and alphabetic baseline.
    pub origin: [f64; 2],
    /// Top of the caret in viewport/CSS pixels.
    pub caret_top: f64,
    /// Caret height, limited to the available content height.
    pub caret_height: f64,
}

/// Content rectangle after left/top/right/bottom field padding.
/// The caller validates finite dimensions and padding before resolving controls.
pub fn field_content_rect(field: MenuRect, inset: [f64; 4]) -> MenuRect {
    MenuRect {
        x: field.x + inset[0],
        y: field.y + inset[1],
        width: (field.width - inset[0] - inset[2]).max(0.),
        height: (field.height - inset[1] - inset[3]).max(0.),
    }
}

/// Place any single-line field's value or placeholder using the selected font.
/// `inset` is symmetric horizontal/vertical content padding in the same logical
/// units as `field`; include borders here if the rect still includes them.
/// Callers supply validated finite geometry and nonnegative font metrics.
/// Font ascent/descent are positive distances from the alphabetic baseline.
/// Center the font line box in the content box and a font-size caret on that
/// same centre. Painters and IME hosts snap these outputs once at their DPR.
pub fn single_line_field_placement(
    field: MenuRect,
    inset: [f64; 2],
    font_size: f64,
    ascent: f64,
    descent: f64,
) -> FieldTextPlacement {
    let height = (field.height - 2. * inset[1]).max(0.);
    let center = field.y + inset[1] + height / 2.;
    let caret_height = font_size.max(0.).min(height);
    FieldTextPlacement {
        origin: [field.x + inset[0], center + (ascent - descent) / 2.],
        caret_top: center - caret_height / 2.,
        caret_height,
    }
}

/// Platform text/inline-flow measurements, independent of world rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuMetrics {
    /// Used normal line-box height of the 15px last-length slot (also when blank).
    pub last_length_line_height: f64,
    /// Inline nickname holder's trailing baseline space in its containing row.
    pub nickname_baseline_space: f64,
    /// Untransformed, zoom-adjusted CSS layout width of the padded 20px bold
    /// label, before offsetWidth's nearest-integer conversion. Not ink bounds.
    pub padded_play_label_width: f64,
}

/// Elements in the login projection, including passive slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuElement {
    /// Logo holder, not its backing canvas.
    Logo,
    /// Persistent last-life line.
    LastLength,
    /// Cycling hint.
    Tips,
    /// Nickname outer border box.
    NicknameHolder,
    /// Nickname input.
    Nickname,
    /// Play button.
    Play,
    /// Bottom-left link.
    Skin,
    /// Bottom-centre server-directory link.
    Server,
    /// Bottom-right product Settings icon and caption.
    Settings,
    /// Top-right link.
    Quality,
    /// Centered spinner, before its independent animated center-origin scale.
    Spinner,
}

/// Complete login geometry with CSS center-origin scaling and inverse mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuLayout {
    /// Unrounded responsive ratio, before transition scaling.
    pub base_scale: f64,
    /// Rounded combined login scale.
    pub scale: f64,
    /// Login's top adjustment, excluding its 64px margin.
    pub top: f64,
    /// Whether the tips row participates in layout and hit testing.
    pub tips_visible: bool,
    origin: (f64, f64),
    rects: [(MenuElement, MenuRect); 11],
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> MenuRect {
    MenuRect {
        x,
        y,
        width,
        height,
    }
}

pub(crate) fn round5(value: f64) -> f64 {
    (value * 100_000.).round() / 100_000.
}

impl MenuLayout {
    /// Initialize at a resize from logical viewport pixels and measured text.
    /// Corner links are viewport-fixed siblings, so they fade but never scale.
    /// Use `update` between resizes to retain the resize-time top adjustment.
    pub fn new(
        width: f64,
        height: f64,
        progress: f64,
        tips_visible: bool,
        metrics: MenuMetrics,
    ) -> Result<Self, &'static str> {
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || !progress.is_finite()
            || !(0. ..=1.).contains(&progress)
            || [
                metrics.last_length_line_height,
                metrics.nickname_baseline_space,
                metrics.padded_play_label_width,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Invalid menu layout input");
        }
        let base_scale = if height < 560. {
            height.max(50.) / 560.
        } else {
            1.
        };
        let scale = round5(base_scale * (1. + 0.1 * progress.powi(2)));
        let top = if scale == 1. {
            0.
        } else {
            -round5(height * (1. - base_scale))
        };
        // The fixed login's height is the viewport height, with a separate margin.
        // Its default transform origin is its border-box center, not its content.
        let origin = (width / 2., height / 2.);
        let last_y = 50. + 115. + 44.;
        let tips_y = last_y + metrics.last_length_line_height;
        let tips_height = if tips_visible { 30. } else { 0. };
        let nick_y = tips_y + tips_height + 34.;
        let play_y = nick_y + 47. + metrics.nickname_baseline_space + 20.;
        let play_width = metrics.padded_play_label_width + 35.;
        Ok(Self {
            base_scale,
            scale,
            top,
            tips_visible,
            origin,
            rects: [
                (
                    MenuElement::Logo,
                    rect((width - 450.) / 2., 50., 450., 115.),
                ),
                (
                    MenuElement::LastLength,
                    rect(0., last_y, width, metrics.last_length_line_height),
                ),
                (
                    MenuElement::Tips,
                    rect((width - 360.) / 2., tips_y, 360., tips_height),
                ),
                // CSS width/height exclude the two-pixel border on each side.
                (
                    MenuElement::NicknameHolder,
                    rect((width - 248.) / 2., nick_y, 248., 47.),
                ),
                (
                    MenuElement::Nickname,
                    rect((width - 248.) / 2. + 2. + 12., nick_y + 2. + 10., 220., 24.),
                ),
                (
                    MenuElement::Play,
                    rect((width - play_width) / 2., play_y, play_width, 47.),
                ),
                (MenuElement::Skin, rect(20., height - 16. - 89., 95., 89.)),
                (
                    MenuElement::Server,
                    rect((width - 107.) / 2., height - 16. - 89., 107., 89.),
                ),
                (
                    MenuElement::Settings,
                    rect(width - 20. - 95., height - 16. - 89., 95., 89.),
                ),
                (MenuElement::Quality, rect(width - 20. - 90., 16., 90., 87.)),
                (
                    MenuElement::Spinner,
                    rect(width / 2. - 64., height / 2. - 64., 128., 128.),
                ),
            ],
        })
    }

    /// Update animation, text metrics and row visibility without a resize.
    /// Source fade callbacks change only the transform, leaving top untouched.
    /// Invalid inputs leave this layout unchanged.
    pub fn update(
        &mut self,
        progress: f64,
        tips_visible: bool,
        metrics: MenuMetrics,
    ) -> Result<(), &'static str> {
        let mut next = Self::new(
            self.origin.0 * 2.,
            self.origin.1 * 2.,
            progress,
            tips_visible,
            metrics,
        )?;
        next.top = self.top;
        *self = next;
        Ok(())
    }

    /// Map login-local coordinates to viewport coordinates, including margins.
    pub fn to_viewport(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.origin.0 + (x - self.origin.0) * self.scale,
            64. + self.top + self.origin.1 + (y - self.origin.1) * self.scale,
        )
    }

    /// Invert the complete login transform for a pointer in viewport pixels.
    pub fn to_login(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.origin.0 + (x - self.origin.0) / self.scale,
            self.origin.1 + (y - 64. - self.top - self.origin.1) / self.scale,
        )
    }

    /// Untransformed element rectangle (login-local or viewport-fixed).
    pub fn logical_rect(&self, element: MenuElement) -> MenuRect {
        self.rects
            .iter()
            .find(|(key, _)| *key == element)
            .unwrap()
            .1
    }

    /// Complete viewport rectangle, ready for a later DOM or GPU adapter.
    pub fn viewport_rect(&self, element: MenuElement) -> MenuRect {
        let r = self.logical_rect(element);
        if fixed(element) {
            return r;
        }
        let (x, y) = self.to_viewport(r.x, r.y);
        rect(x, y, r.width * self.scale, r.height * self.scale)
    }

    /// Inverse hit-test a specified element. Visibility/admission is separate.
    pub fn hit_test(&self, element: MenuElement, x: f64, y: f64) -> bool {
        let (x, y) = if fixed(element) {
            (x, y)
        } else {
            self.to_login(x, y)
        };
        self.logical_rect(element).contains(x, y)
    }
}

fn fixed(element: MenuElement) -> bool {
    matches!(
        element,
        MenuElement::Skin
            | MenuElement::Server
            | MenuElement::Settings
            | MenuElement::Quality
            | MenuElement::Spinner
    )
}
