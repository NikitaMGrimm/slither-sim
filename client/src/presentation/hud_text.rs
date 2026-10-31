//! Platform-independent HUD typography and layout. Row/score policy is supplied
//! by HudHistory and LeaderboardHistory, as in the Canvas reference views.
use super::{BodyRect, HudHistory, HudOpacity, LeaderboardHistory, LeaderboardRow};
use crate::{
    gameplay_policy::GameplayPolicy,
    session_policy::{Phase, SessionPolicy},
};

/// Canvas font stack; native Windows uses the installed Arial faces.
pub const PLATFORM_FONT_FAMILY: &str = "Arial, Helvetica Neue, Helvetica, sans-serif";
/// Explicit official panel stacks. Native adapters document face substitution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextFamily {
    /// Arial, Helvetica Neue, Helvetica, sans-serif.
    #[default]
    Arial,
    /// Lucida Sans Unicode, Lucida Grande, sans-serif.
    Lucida,
}
impl TextFamily {
    /// CSS font-family value, shared with Canvas adapters.
    pub fn css(self) -> &'static str {
        match self {
            Self::Arial => PLATFORM_FONT_FAMILY,
            Self::Lucida => "Lucida Sans Unicode, Lucida Grande, sans-serif",
        }
    }
}
/// A real face selection; never an artificial shear.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextSlant {
    /// Upright face.
    #[default]
    Normal,
    /// Installed italic face.
    Italic,
}
/// Maximum active readout, message, heading and top-ten column spans.
pub const MAX_HUD_TEXT_SPANS: usize = 40;

/// Platform font face weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextWeight {
    /// Regular face.
    Normal,
    /// Bold face.
    Bold,
}
/// Raster style, independent of placement and lifecycle opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Explicit platform stack.
    pub family: TextFamily,
    /// Requested face slant.
    pub slant: TextSlant,
    /// Logical viewport pixels.
    pub size: f64,
    /// Requested face weight.
    pub weight: TextWeight,
    /// Straight sRGB bytes.
    pub color: [u8; 3],
}
impl TextStyle {
    /// Explicit logical line cell for automatically sized panel text.
    /// Perceptual spacing policy; independent of platform font tables and DPR.
    pub fn line_height(self) -> f64 {
        (1.15 * self.size).round()
    }
}
/// Horizontal text anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    /// Start of the advance.
    Left,
    /// Midpoint of the advance.
    Center,
    /// End of the advance.
    Right,
}
/// Vertical text anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextBaseline {
    /// Font's alphabetic baseline.
    Alphabetic,
    /// Canvas em-square middle.
    Middle,
}

/// Copied shared history for both graphical and compact host readouts.
#[derive(Clone, Debug, PartialEq)]
pub struct HudTextView {
    /// Session playing observation.
    pub playing: bool,
    /// Independent role fades from HUD history.
    pub opacity: HudOpacity,
    /// Non-playing session status.
    pub status: String,
    /// Current decimal length, or the absent marker.
    pub score: String,
    /// Retained final length, or the absent marker.
    pub final_score: String,
    /// Display-time rank.
    pub rank: String,
    /// Display-time population.
    pub population: String,
    /// Existing top-ten projection in wire order.
    pub rows: Vec<LeaderboardRow>,
    /// Optional host failure text.
    pub message: Option<String>,
}
impl HudTextView {
    /// Reuse retained formatting, visibility and row-selection policy.
    pub fn project(
        hud: &HudHistory,
        rows: &LeaderboardHistory,
        session: &SessionPolicy,
        failure: Option<&str>,
        reset_rows: bool,
    ) -> Self {
        let phase = session.view().phase;
        let status = if phase == Phase::Failed {
            "FAILED"
        } else {
            match GameplayPolicy::status(phase.label(), session.view().connected) {
                1 => "READY",
                2 => "CONNECTED",
                4 => "CONNECTING",
                _ => "NOT CONNECTED",
            }
        };
        Self {
            playing: session.view().playing,
            opacity: hud.requested_opacity,
            status: status.into(),
            score: hud
                .current
                .and_then(|s| s.score_text().ok())
                .unwrap_or_else(|| "--".into()),
            final_score: hud
                .final_capture
                .and_then(|s| s.score_text().ok())
                .unwrap_or_else(|| "--".into()),
            rank: hud.current.map_or_else(|| "--".into(), |s| s.rank_text()),
            population: hud
                .current
                .map_or_else(|| "--".into(), |s| s.population_text()),
            rows: if reset_rows {
                Vec::new()
            } else {
                rows.visible_rows().to_vec()
            },
            message: failure.map(str::to_owned),
        }
    }
    /// Plain host status without font or graphics dependencies.
    pub fn compact_text(&self) -> String {
        let top = self
            .rows
            .first()
            .map(|r| format!("{} {}", r.name_text, r.score_text))
            .unwrap_or_else(|| "--".into());
        format!(
            "{} | ENTER PLAY / ESC STOP\nLENGTH {}  RANK {} / {}\nFINAL {}  TOP {}\n{}",
            self.status,
            self.score,
            self.rank,
            self.population,
            self.final_score,
            top,
            self.message
                .as_deref()
                .unwrap_or("LOCAL AUTHORED BODY/PAIRED EYES; ARROWS/POINTER/SPACE")
        )
    }
}

/// One independently styled inline fragment.
#[derive(Clone, Debug, PartialEq)]
pub struct HudTextSpan {
    /// Literal projected text, without renderer normalization.
    pub text: String,
    /// Font and opaque colour.
    pub style: TextStyle,
    /// Field opacity multiplied by the role's lifecycle fade.
    pub alpha: f64,
}
/// Spans flow by their measured platform advance, preserving proportional text.
/// The first span starts at position; single-span columns also use align.
#[derive(Clone, Debug, PartialEq)]
pub struct HudTextLine {
    /// Ordered proportional fragments.
    pub spans: Vec<HudTextSpan>,
    /// First fragment's logical alphabetic anchor.
    pub position: [f64; 2],
    /// Horizontal anchor for the line.
    pub align: TextAlign,
    /// Logical column/panel clipping rectangle.
    pub clip: BodyRect,
}
/// Ordered text submission policy, without platform metrics or assets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudTextPlan {
    /// Readout followed by heading and row columns.
    pub lines: Vec<HudTextLine>,
}

fn span(text: &str, size: f64, weight: TextWeight, color: [u8; 3], alpha: f64) -> HudTextSpan {
    HudTextSpan {
        text: text.into(),
        style: TextStyle {
            family: TextFamily::Arial,
            slant: TextSlant::Normal,
            size,
            weight,
            color,
        },
        alpha: alpha.clamp(0., 1.),
    }
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> BodyRect {
    BodyRect {
        x,
        y,
        width,
        height,
    }
}
fn viewport(extent: [u32; 2], scale: f64) -> Result<[f64; 2], &'static str> {
    if extent.iter().any(|v| *v > 8192) || !scale.is_finite() || scale <= 0. {
        return Err("Invalid HUD viewport");
    }
    let size = extent.map(|v| f64::from(v) / scale);
    if size.iter().any(|v| !v.is_finite() || *v > 8192.) {
        return Err("HUD logical extent budget");
    }
    Ok(size)
}

/// English desktop readout and top-ten columns. The Canvas HUD uses 14px length,
/// 12px rank, and per-field opacity (web/stock-hud-dom.mjs); its leaderboard
/// columns use 18px lines and emphasize only the own nickname
/// (web/game-owned-leaderboard-ui.mjs). Fixed desktop anchors use CSS pixels.
pub fn plan_hud_text(
    view: &HudTextView,
    extent: [u32; 2],
    scale: f64,
) -> Result<HudTextPlan, &'static str> {
    let [w, h] = viewport(extent, scale)?;
    let mut plan = HudTextPlan::default();
    if w < 48. || h < 40. {
        return Ok(plan);
    }
    let white = [255; 3];
    let normal = TextWeight::Normal;
    let bold = TextWeight::Bold;
    // The 12px desktop panel has an inherited 18px line box, including
    // its 14px first line. Arial alphabetic baselines are 14px and 13px
    // below the two line tops (37px panel, 4px bottom inset).
    let clip = rect(8., (h - 41.).max(0.), (w - 16.).min(280.), 37.);
    if view.playing && view.score != "--" {
        let a = view.opacity.readout.clamp(0., 1.);
        plan.lines.push(HudTextLine {
            spans: vec![
                span("Your length: ", 14., normal, white, a * 0.4),
                span(&view.score, 14., bold, white, a * 0.8),
            ],
            position: [8., h - 27.],
            align: TextAlign::Left,
            clip,
        });
        plan.lines.push(HudTextLine {
            spans: vec![
                // Rank values differ in opacity, not weight. Preserve semantic
                // styles rather than compensating here for raster coverage.
                span("Your rank: ", 12., normal, white, a * 0.3),
                span(&view.rank, 12., normal, white, a * 0.35),
                span(" of ", 12., normal, white, a * 0.3),
                span(&view.population, 12., normal, white, a * 0.35),
            ],
            position: [8., h - 10.],
            align: TextAlign::Left,
            clip,
        });
    }
    // Returning results belong to the login. Diagnostic entries call
    // plan_hud_diagnostic explicitly rather than covering those controls.

    if let Some(message) = &view.message {
        plan.lines.push(HudTextLine {
            spans: vec![span(
                &message.chars().take(256).collect::<String>(),
                12.,
                normal,
                white,
                1.,
            )],
            position: [8., (h - 48.).max(12.)],
            align: TextAlign::Left,
            clip: rect(8., 0., w - 16., h),
        });
    }
    if view.rows.is_empty() || w < 300. || h < 68. {
        return Ok(plan);
    }
    // On narrow/short windows, retain the readout and an ordered prefix of rows.
    let bottom = if !plan.lines.is_empty() && w - 290. < 288. {
        clip.y
    } else {
        h - 4.
    };
    let count = (((bottom - 32.) / 18.).floor().max(0.) as usize)
        .min(10)
        .min(view.rows.len());
    if count == 0 {
        return Ok(plan);
    }
    plan.lines.push(HudTextLine {
        spans: vec![span("Leaderboard", 21., bold, white, view.opacity.header)],
        position: [w - 146.5, 23.],
        align: TextAlign::Center,
        clip: rect(w - 289., 4., 285., 28.),
    });
    for (i, row) in view.rows.iter().take(count).enumerate() {
        let rgb = u32::from_str_radix(
            row.css_color.strip_prefix('#').ok_or("Invalid HUD color")?,
            16,
        )
        .map_err(|_| "Invalid HUD color")?;
        let color = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
        let alpha = view.opacity.rows.clamp(0., 1.) * row.opacity_request.clamp(0., 1.);
        // The desktop own-row emphasis bolds the nickname only. Scores and
        // ordinal labels retain regular weight even at the own row's full alpha.
        for (text, x, left, width, align, weight) in [
            (
                &row.score_text,
                w - 29.,
                w - 54.,
                50.,
                TextAlign::Center,
                normal,
            ),
            (
                &row.name_text,
                w - 244.,
                w - 244.,
                180.,
                TextAlign::Left,
                if row.own { bold } else { normal },
            ),
            (
                &row.number_text,
                w - 260.,
                w - 290.,
                30.,
                TextAlign::Right,
                normal,
            ),
        ] {
            plan.lines.push(HudTextLine {
                spans: vec![span(text, 12., weight, color, alpha)],
                position: [x, 45. + i as f64 * 18.],
                align,
                clip: rect(left, 32., width, count as f64 * 18.),
            });
        }
    }
    Ok(plan)
}

/// Diagnostic strings use the same text path; they carry no gameplay policy.
pub fn plan_hud_diagnostic(
    text: &str,
    extent: [u32; 2],
    scale: f64,
) -> Result<HudTextPlan, &'static str> {
    let [w, h] = viewport(extent, scale)?;
    let mut plan = HudTextPlan::default();
    if w < 48. || h < 40. {
        return Ok(plan);
    }
    let count = text
        .lines()
        .take(4)
        .count()
        .min(((h - 16.) / 18.).floor() as usize);
    for (i, text) in text.lines().take(count).enumerate() {
        plan.lines.push(HudTextLine {
            spans: vec![span(
                &text.chars().take(256).collect::<String>(),
                12.,
                TextWeight::Normal,
                [255; 3],
                1.,
            )],
            position: [8., h - 8. - (count - 1 - i) as f64 * 18.],
            align: TextAlign::Left,
            clip: rect(8., 0., w - 16., h),
        });
    }
    Ok(plan)
}
