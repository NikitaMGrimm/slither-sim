//! Bounded CSS-normal inline layout. Owners retain the complete received text;
//! this projection visits only a bounded visible prefix and owns no raster data.
use super::{BodyRect, TextStyle};
use unicode_segmentation::UnicodeSegmentation;

/// Artificial cache chunks never exceed this UTF-8 byte length.
pub const PANEL_RUN_BYTES: usize = 1024;
/// Visible non-whitespace byte quota, after one-time admission normalization.
pub const PANEL_SCAN_BYTES: usize = 4 * PANEL_RUN_BYTES;
/// Raw admission bounds; normalization is linear and retained across layouts.
pub const PANEL_INPUT_BYTES: usize = 2 * 1024 * 1024;
/// Bounds metadata work independently of raw text bytes.
pub const PANEL_INPUT_FRAGMENTS: usize = 4096;
/// Shared slot allowance, divided into announcement (48) and result/editor (16).
pub const PANEL_RUNS: usize = 64;
/// These bounds include the 24px result heading and 18px announcement text.
/// Allocation rationale: https://github.com/NikitaMGrimm/slither-sim/blob/main/client/docs/panel-text.md#aggregate-resource-proof;
/// regression: https://github.com/NikitaMGrimm/slither-sim/blob/main/client/apps/wgpu-slice/tests/panel_text.rs#L20
pub const PANEL_MAX_FONT_SIZE: f64 = 24.;
/// Fixed line cells make both clipping and the allocation proof independent of ink.
pub const PANEL_MIN_LINE_HEIGHT: f64 = 16.;
/// Largest supported panel line cell.
pub const PANEL_MAX_LINE_HEIGHT: f64 = 32.;

/// Copied backend metrics in requested font pixels, for a complete style run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMeasurement {
    /// Proportional, kerned advance.
    pub advance: f64,
    /// Ink ascent above the alphabetic baseline.
    pub ascent: f64,
    /// Ink descent below the alphabetic baseline.
    pub descent: f64,
    /// Style-level font box ascent, independent of the measured glyphs.
    pub font_ascent: f64,
    /// Style-level font box descent, independent of the measured glyphs.
    pub font_descent: f64,
}
/// A backend supplies font metrics; line breaking remains in the shared core.
pub trait TextMetrics {
    /// Measure the complete run at the requested font size. Panel layout supplies
    /// device-pixel sizes and converts advances back to logical pixels.
    fn measure(&mut self, text: &str, style: TextStyle) -> Result<TextMeasurement, &'static str>;
}
/// Borrowed semantic input: ordinary newlines collapse; explicit breaks do not.
#[derive(Clone, Copy, Debug)]
pub enum PanelTextFragment<'a> {
    /// Literal text with its face and fill.
    Text(&'a str, TextStyle),
    /// Explicit block/line break.
    Break,
}
#[derive(Clone, Debug)]
enum AdmittedFragment {
    Text(String, TextStyle, usize),
    Break,
}
/// Retained admission projection. Build once when content changes, then reuse
/// for layout/resize. Owners retain their original, unnormalized content.
#[derive(Clone, Debug)]
pub struct PanelText {
    fragments: Vec<AdmittedFragment>,
}
impl PanelText {
    /// Collapse whitespace once, preserving its first style across inline
    /// boundaries. UAX #29 graphemes crossing a style boundary take the first
    /// scalar's style. A cluster over 1024 bytes projects to one U+FFFD, never a
    /// partial cluster. Raw admission limits fail explicitly, without truncation.
    pub fn new(fragments: &[PanelTextFragment<'_>]) -> Result<Self, &'static str> {
        Self::admit(fragments, false)
    }
    /// Preserve source fragment boundaries for independent draw-time opacity.
    /// Normalization and grapheme ownership still cross inline boundaries.
    pub fn preserving_fragments(fragments: &[PanelTextFragment<'_>]) -> Result<Self, &'static str> {
        Self::admit(fragments, true)
    }
    fn admit(fragments: &[PanelTextFragment<'_>], preserve: bool) -> Result<Self, &'static str> {
        if fragments.len() > PANEL_INPUT_FRAGMENTS
            || fragments
                .iter()
                .try_fold(0_usize, |sum, f| {
                    sum.checked_add(match f {
                        PanelTextFragment::Text(t, _) => t.len(),
                        PanelTextFragment::Break => 1,
                    })
                })
                .is_none_or(|n| n > PANEL_INPUT_BYTES)
        {
            return Err("Panel input admission budget");
        }
        let mut result = Self {
            fragments: Vec::new(),
        };
        let mut paragraph = String::new();
        let mut styles: Vec<(usize, TextStyle, usize)> = Vec::new();
        let mut whitespace = false;
        for (index, fragment) in fragments.iter().enumerate() {
            match *fragment {
                PanelTextFragment::Break => {
                    result.paragraph(&paragraph, &styles);
                    result.fragments.push(AdmittedFragment::Break);
                    paragraph.clear();
                    styles.clear();
                    whitespace = false;
                }
                PanelTextFragment::Text(text, style) => {
                    for c in text.chars() {
                        let space = matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}');
                        if space && whitespace {
                            continue;
                        }
                        whitespace = space;
                        let source = if preserve { index } else { 0 };
                        if styles
                            .last()
                            .is_none_or(|(_, s, id)| *s != style || *id != source)
                        {
                            styles.push((paragraph.len(), style, source));
                        }
                        paragraph.push(if space { ' ' } else { c });
                    }
                }
            }
        }
        result.paragraph(&paragraph, &styles);
        Ok(result)
    }
    fn paragraph(&mut self, text: &str, styles: &[(usize, TextStyle, usize)]) {
        let mut style = 0;
        for (start, cluster) in text.grapheme_indices(true) {
            while style + 1 < styles.len() && styles[style + 1].0 <= start {
                style += 1;
            }
            let requested = styles[style].1;
            let source = styles[style].2;
            let cluster = if cluster.len() > PANEL_RUN_BYTES {
                "\u{fffd}"
            } else {
                cluster
            };
            if let Some(AdmittedFragment::Text(last, _, _)) = self.fragments.last_mut().filter(
                |f| matches!(f, AdmittedFragment::Text(_,s,id) if *s == requested && *id == source),
            ) {
                last.push_str(cluster);
            } else {
                self.fragments
                    .push(AdmittedFragment::Text(cluster.into(), requested, source));
            }
        }
    }
}
/// A producer supplies one bounded logical box and fixed line-cell policy.
pub struct PanelTextBlock<'a> {
    /// Retained normalized content, scanned lazily by whole graphemes.
    pub text: &'a PanelText,
    /// Visible logical rectangle (at most 600 by 228).
    pub bounds: BodyRect,
    /// Baselines share a fixed line cell, between 16 and 32px.
    pub line_height: f64,
    /// Device pixels per logical pixel, independent of the capped raster scale.
    pub device_scale: f64,
    /// Producer's replaceable slot allowance, at most 64.
    pub max_runs: usize,
}
/// One bounded cache piece with complete-run positioning.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelTextRun {
    /// Grapheme-safe piece, at most 1024 bytes.
    pub text: String,
    /// Raster style, independent of placement/fade/pulse.
    pub style: TextStyle,
    /// Logical left alphabetic anchor, including cross-chunk kerning.
    pub position: [f64; 2],
    /// Clip includes up to one em of italic overhang, inside the box/line cell.
    pub clip: BodyRect,
}
/// A fresh value is returned only after every visited metric validates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelTextPlan {
    /// Ordered visible prefix only.
    pub runs: Vec<PanelTextRun>,
    /// Normalized bytes visited, including single collapsed spaces and breaks.
    pub scanned_bytes: usize,
}
#[derive(Clone)]
struct Segment(String, TextStyle, usize);
fn append(to: &mut Vec<Segment>, text: &str, style: TextStyle, source: usize) {
    if let Some(last) = to
        .last_mut()
        .filter(|last| last.1 == style && last.2 == source)
    {
        last.0.push_str(text);
    } else {
        to.push(Segment(text.into(), style, source));
    }
}
fn pieces(runs: &[Segment]) -> usize {
    runs.iter()
        .map(|r| {
            let mut start = 0;
            let mut count = 0;
            while start < r.0.len() {
                let end = chunk_end(&r.0, start);
                count += 1;
                start = end;
            }
            count
        })
        .sum()
}
fn chunk_end(text: &str, start: usize) -> usize {
    let mut end = start;
    for cluster in text[start..].graphemes(true) {
        if end + cluster.len() - start > PANEL_RUN_BYTES {
            break;
        }
        end += cluster.len();
    }
    end
}
// Convert complete-run metrics to logical pixels once, before line layout.
struct Measured {
    advance: f64,
    font_ascent: f64,
    font_descent: f64,
}
fn measured(
    m: &mut impl TextMetrics,
    text: &str,
    style: TextStyle,
    scale: f64,
) -> Result<Measured, &'static str> {
    if !style.size.is_finite() || style.size <= 0. || style.size > PANEL_MAX_FONT_SIZE {
        return Err("Invalid panel style");
    }
    let v = m.measure(
        text,
        TextStyle {
            size: style.size * scale,
            ..style
        },
    )?;
    if ![
        v.advance,
        v.ascent,
        v.descent,
        v.font_ascent,
        v.font_descent,
    ]
    .into_iter()
    .all(|v| v.is_finite() && v >= 0.)
        || v.advance / scale > 1e7
        || (v.ascent + v.descent) / scale > 128.
        || v.font_ascent + v.font_descent <= 0.
        || (v.font_ascent + v.font_descent) / scale > 128.
    {
        return Err("Invalid panel metrics");
    }
    Ok(Measured {
        advance: v.advance / scale,
        font_ascent: v.font_ascent / scale,
        font_descent: v.font_descent / scale,
    })
}
fn width(m: &mut impl TextMetrics, runs: &[Segment], scale: f64) -> Result<f64, &'static str> {
    runs.iter().try_fold(
        0.,
        |sum, r| Ok(sum + measured(m, &r.0, r.1, scale)?.advance),
    )
}
struct Layout<'a, M> {
    block: &'a PanelTextBlock<'a>,
    metrics: &'a mut M,
    plan: PanelTextPlan,
    line: Vec<Segment>,
    row: usize,
    sources: Vec<usize>,
}
impl<M: TextMetrics> Layout<'_, M> {
    fn visible(&self) -> bool {
        (self.row as f64) * self.block.line_height < self.block.bounds.height
            && self.plan.runs.len() < self.block.max_runs
    }
    fn flush(&mut self) -> Result<(), &'static str> {
        let b = self.block.bounds;
        let top = b.y + self.row as f64 * self.block.line_height;
        let height = self.block.line_height.min(b.y + b.height - top).max(0.);
        let mut strut = [0_f64; 2];
        let scale = self.block.device_scale;
        for r in &self.line {
            let m = measured(self.metrics, &r.0, r.1, scale)?;
            strut[0] = strut[0].max(m.font_ascent);
            strut[1] = strut[1].max(m.font_descent);
        }
        // Center one font strut in the explicit cell, even for compact cells.
        // Ink never changes the baseline or the following line's position.
        let baseline = top + (self.block.line_height - strut[0] - strut[1]) / 2. + strut[0];
        let mut x = b.x;
        for r in &self.line {
            let full = measured(self.metrics, &r.0, r.1, scale)?.advance;
            let mut start = 0;
            while start < r.0.len() && self.plan.runs.len() < self.block.max_runs {
                let end = chunk_end(&r.0, start);
                let text = &r.0[start..end];
                // Prefix-end minus piece advance retains pair kerning at the
                // artificial boundary. Never sum isolated character widths.
                let end_x = measured(self.metrics, &r.0[..end], r.1, scale)?.advance;
                let piece = measured(self.metrics, text, r.1, scale)?.advance;
                let start_x = measured(self.metrics, &r.0[..start], r.1, scale)?.advance;
                let left = (x + start_x - r.1.size).max(b.x);
                let right = (x + end_x + r.1.size).min(b.x + b.width);
                self.sources.push(r.2);
                self.plan.runs.push(PanelTextRun {
                    text: text.into(),
                    style: r.1,
                    position: [x + end_x - piece, baseline],
                    clip: BodyRect {
                        x: left,
                        y: top,
                        width: (right - left).max(0.),
                        height,
                    },
                });
                start = end;
            }
            x += full;
        }
        self.line.clear();
        self.row += 1;
        Ok(())
    }
    fn word(
        &mut self,
        word: &[Segment],
        space: Option<(TextStyle, usize)>,
    ) -> Result<(), &'static str> {
        if word.is_empty() {
            return Ok(());
        }
        let mut next = self.line.clone();
        if !next.is_empty()
            && let Some((style, source)) = space
        {
            append(&mut next, " ", style, source);
        }
        for r in word {
            append(&mut next, &r.0, r.1, r.2);
        }
        if !self.line.is_empty()
            && width(self.metrics, &next, self.block.device_scale)? > self.block.bounds.width
        {
            self.flush()?;
            next.clear();
            for r in word {
                append(&mut next, &r.0, r.1, r.2);
            }
        }
        if self.visible() {
            self.line = next;
        }
        Ok(())
    }
}
/// Collapse CSS-normal ASCII whitespace, honour explicit breaks, wrap words,
/// then split overlong tokens at grapheme boundaries. NBSP is a token.
/// Exhausting a visible-prefix/work/slot limit clips; it never rejects content.
pub fn layout_panel_text(
    block: &PanelTextBlock<'_>,
    metrics: &mut impl TextMetrics,
) -> Result<PanelTextPlan, &'static str> {
    if block.line_height < PANEL_MIN_LINE_HEIGHT {
        return Err("Invalid panel bounds");
    }
    layout_panel_text_with_sources(block, metrics).map(|(plan, _)| plan)
}
/// Layout plus the input-fragment index for each run. Alpha is never raster style.
/// The official 13px announcement font also permits its rounded normal line box (at least 14 CSS pixels);
/// callers must establish their producer allocation bound for that smaller cell.
pub fn layout_panel_text_with_sources(
    block: &PanelTextBlock<'_>,
    metrics: &mut impl TextMetrics,
) -> Result<(PanelTextPlan, Vec<usize>), &'static str> {
    let b = block.bounds;
    if ![b.x, b.y, b.width, b.height, block.line_height]
        .into_iter()
        .all(f64::is_finite)
        || b.width < 0.
        || b.width > 600.
        || b.height < 0.
        || b.height > 228.
        || !(14. ..=PANEL_MAX_LINE_HEIGHT).contains(&block.line_height)
        || block.max_runs > PANEL_RUNS
        || !block.device_scale.is_finite()
        || block.device_scale <= 0.
        || block.device_scale > 8192.
    {
        return Err("Invalid panel bounds");
    }
    let mut l = Layout {
        block,
        metrics,
        plan: PanelTextPlan::default(),
        line: Vec::new(),
        row: 0,
        sources: Vec::new(),
    };
    let mut word: Vec<Segment> = Vec::new();
    let mut space = None;
    let mut visible_bytes = 0;
    'fragments: for fragment in &block.text.fragments {
        if !l.visible() || b.width == 0. {
            break;
        }
        match fragment {
            AdmittedFragment::Break => {
                l.plan.scanned_bytes += 1;
                l.word(&word, space.take())?;
                word.clear();
                if l.visible() {
                    l.flush()?;
                }
            }
            AdmittedFragment::Text(text, style, source) => {
                let style = *style;
                let source = *source;
                for cluster in text.graphemes(true) {
                    let whitespace = cluster == " ";
                    if !l.visible()
                        || (!whitespace && visible_bytes + cluster.len() > PANEL_SCAN_BYTES)
                    {
                        break 'fragments;
                    }
                    l.plan.scanned_bytes += cluster.len();
                    if whitespace {
                        if !word.is_empty() {
                            l.word(&word, space.take())?;
                            word.clear();
                        }
                        space.get_or_insert((style, source));
                    } else {
                        visible_bytes += cluster.len();
                        let mut next = word.clone();
                        append(&mut next, cluster, style, source);
                        let mut visible = l.line.clone();
                        if !visible.is_empty()
                            && let Some((style, source)) = space
                        {
                            append(&mut visible, " ", style, source);
                        }
                        for r in &next {
                            append(&mut visible, &r.0, r.1, r.2);
                        }
                        if pieces(&visible) > block.max_runs - l.plan.runs.len() {
                            l.word(&word, space.take())?;
                            if l.visible() && !l.line.is_empty() {
                                l.flush()?;
                            }
                            break 'fragments;
                        }
                        if !word.is_empty()
                            && width(l.metrics, &next, block.device_scale)? > b.width
                        {
                            l.word(&word, space.take())?;
                            word.clear();
                            if !l.visible() {
                                break 'fragments;
                            }
                            if !l.line.is_empty() {
                                l.flush()?;
                            }
                            if !l.visible() {
                                break 'fragments;
                            }
                        }
                        append(&mut word, cluster, style, source);
                        // Even the first character's metrics must validate.
                        measured(
                            l.metrics,
                            &word.last().unwrap().0,
                            style,
                            block.device_scale,
                        )?;
                    }
                }
            }
        }
    }
    if l.visible() && !word.is_empty() {
        l.word(&word, space)?;
    }
    if l.visible() && !l.line.is_empty() {
        l.flush()?;
    }
    Ok((l.plan, l.sources))
}
