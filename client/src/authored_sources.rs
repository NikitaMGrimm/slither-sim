//! Numeric plans for the existing authored local source artwork.
//!
//! No Canvas, image, source identity, randomness, clocks or loading live here.

mod standard;

/// One ordered paint operation. The browser only binds these copied operands.
#[derive(Clone, Debug, PartialEq)]
pub enum PaintCommand {
    /// Owned RGBA samples, copied before native ImageData binding.
    Pixels {
        /// Sample width.
        width: u32,
        /// Sample height.
        height: u32,
        /// Row-major unpremultiplied RGBA bytes.
        rgba: Vec<u8>,
    },
    /// Closed native coverage operation.
    Composite(&'static str),
    /// Native shadow parameters for one source.
    Shadow {
        /// Native blur in source pixels.
        blur: f64,
        /// Vertical source offset.
        offset_y: f64,
        /// Shadow CSS color.
        color: String,
    },
    /// Select a literal authored CSS color.
    FillStyle(String),
    /// Select the authored source opacity.
    GlobalAlpha(f64),
    /// Begin an independent path.
    BeginPath,
    /// Circle center, radius and start/end angles.
    Arc([f64; 5]),
    /// Move to a source-pixel point.
    MoveTo([f64; 2]),
    /// Append a source-pixel point.
    LineTo([f64; 2]),
    /// Fill the current path.
    Fill,
    /// Fill a source-pixel rectangle.
    FillRect([f64; 4]),
    /// Create a radial gradient, add its ordered stops, then select it for filling.
    RadialGradient {
        /// Inner circle followed by outer circle.
        circles: [f64; 6],
        /// Exact offset and authored CSS color in submission order.
        stops: Vec<(f64, String)>,
    },
}

/// One source's native dimensions and complete ordered painting plan.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasRecipe {
    /// Native backing width.
    pub width: u32,
    /// Native backing height.
    pub height: u32,
    /// Source-center metadata used by food/prey factories.
    pub half_width: f64,
    /// Source-center metadata used by food/prey factories.
    pub half_height: f64,
    /// Browser operations, without source allocation or readback.
    pub commands: Vec<PaintCommand>,
}

impl CanvasRecipe {
    fn new(width: u32, height: u32, commands: Vec<PaintCommand>) -> Self {
        Self {
            width,
            height,
            half_width: f64::from(width) / 2.,
            half_height: f64::from(height) / 2.,
            commands,
        }
    }
}

/// Standard shaded body variants and their boost source.
#[derive(Clone, Debug, PartialEq)]
pub struct BodySourcePalette {
    /// Ordered source variants (seven ordinary, sixty for the cycle family).
    pub variants: Vec<CanvasRecipe>,
    /// One existing boost-glow source.
    pub boost_glow: CanvasRecipe,
}

/// Complete authored body source families; geometry selection remains `resolve_local_skin`.
#[derive(Clone, Debug, PartialEq)]
pub struct BodySourceRecipes {
    /// Standard palette families in numeric order.
    pub palettes: Vec<BodySourcePalette>,
    /// Early body outline.
    pub outline: CanvasRecipe,
    /// Body shadow.
    pub shadow: CanvasRecipe,
    /// Death highlight.
    pub death_highlight: CanvasRecipe,
}

/// One food or prey source family, before browser object binding.
#[derive(Clone, Debug, PartialEq)]
pub struct EntitySourcePalette {
    /// Creation-selected authored family ID.
    pub color: u32,
    /// Existing body-family count; food exposes this separately.
    pub count: usize,
    /// Ordered body sources.
    pub bodies: Vec<CanvasRecipe>,
    /// Ordered outline sources (empty for prey).
    pub outlines: Vec<CanvasRecipe>,
    /// Ordered glow sources.
    pub glows: Vec<CanvasRecipe>,
}

/// Fresh numeric source data for one authored local source pool.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoredSourceRecipes {
    /// Existing authored body colors, also used as nickname tint bases.
    pub palette_rgb: Vec<[u8; 3]>,
    /// Three legacy flat source canvases.
    pub palettes: Vec<CanvasRecipe>,
    /// Existing shaded body/effect sources.
    pub body_sources: BodySourceRecipes,
    /// Ten authored food families.
    pub food: Vec<EntitySourcePalette>,
    /// Nine authored prey families.
    pub prey: Vec<EntitySourcePalette>,
}

fn css_hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn circle(width: u32, height: u32, color: &str, alpha: Option<f64>, radius: f64) -> CanvasRecipe {
    let mut commands = vec![PaintCommand::FillStyle(color.into())];
    if let Some(alpha) = alpha {
        commands.push(PaintCommand::GlobalAlpha(alpha));
    }
    commands.extend([
        PaintCommand::BeginPath,
        PaintCommand::Arc([
            f64::from(width) / 2.,
            f64::from(height) / 2.,
            radius,
            0.,
            std::f64::consts::PI * 2.,
        ]),
        PaintCommand::Fill,
    ]);
    CanvasRecipe::new(width, height, commands)
}

/// Authored standard palette ink, shared by body sources and studio swatches.
pub fn standard_palette_rgb(palette: u8) -> Option<[u8; 3]> {
    standard::COLORS.get(usize::from(palette)).copied()
}

/// Plan standard body/entity source materials.
pub fn authored_source_recipes() -> AuthoredSourceRecipes {
    let palette_rgb = standard::COLORS.to_vec();
    // Keep the three legacy context canvases separate from standard body sources.
    let palettes = [[0x83, 0xd5, 0xb0], [0xef, 0xbb, 0x62], [0x76, 0xb9, 0xed]]
        .map(|rgb| circle(32, 32, &css_hex(rgb), None, 16.))
        .into();
    AuthoredSourceRecipes {
        palette_rgb,
        palettes,
        body_sources: standard::bodies(),
        food: standard::food(),
        prey: standard::prey(),
    }
}
