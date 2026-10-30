//! Retained public announcements and their shared HUD-clock projection.
//!
//! There is no independent timeout. Setup and world reset preserve content;
//! accepted leaderboard and existing session lifecycle observations drive fade.

use std::{error::Error, fmt, sync::Arc};

use super::{
    PanelText, PanelTextFragment, PresentationError, ScoreError, ScoreTables, TextFamily,
    TextSlant, TextStyle, TextWeight,
};
use crate::{
    protocol::hud::{HudPacketError, parse_announcement},
    state::{HudCommandBoundary, HudCommandMarker, PresentationEvent},
};

/// Invalid announcement bytes, unsupported score or recoverable allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnouncementError {
    /// The complete bare command failed validation.
    Packet(HudPacketError),
    /// The command requires a score outside the selected table domain.
    Score(ScoreError),
    /// Announcement scoring requires an earlier Setup.
    MissingTable,
    /// The bounded owned packet could not be reserved.
    AllocationFailed,
}

impl fmt::Display for AnnouncementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packet(error) => error.fmt(f),
            Self::Score(error) => error.fmt(f),
            Self::MissingTable => f.write_str("announcement requires a score table"),
            Self::AllocationFailed => f.write_str("announcement allocation failed"),
        }
    }
}
impl Error for AnnouncementError {}

/// Validated bounded wire bytes; private fields prevent unvalidated construction.
#[derive(Debug)]
pub(crate) struct AnnouncementPacket(Vec<u8>);

impl AnnouncementPacket {
    pub(crate) fn copy(command: &[u8]) -> Result<Self, AnnouncementError> {
        parse_announcement(command).map_err(AnnouncementError::Packet)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(command.len())
            .map_err(|_| AnnouncementError::AllocationFailed)?;
        bytes.extend_from_slice(command);
        Ok(Self(bytes))
    }

    fn validate_table(&self, table_max: Option<u16>) -> Result<(), AnnouncementError> {
        let table_max = table_max.ok_or(AnnouncementError::MissingTable)?;
        if table_max == 0 {
            return Err(AnnouncementError::Score(ScoreError::ZeroTableMax));
        }
        let packet = parse_announcement(&self.0).map_err(AnnouncementError::Packet)?;
        if packet.count() > u32::from(table_max) + 2048 {
            return Err(AnnouncementError::Score(ScoreError::IndexOutOfRange));
        }
        Ok(())
    }
}

/// Admit every announcement's lookup before any resolver or Setup callback.
/// Only table availability/bounds are needed here; scoring still happens at the
/// original command position with the unchanged ordered f64 arithmetic.
pub(crate) fn prevalidate_announcement_scores(
    mut table_max: Option<u16>,
    events: &[PresentationEvent],
    boundaries: &[HudCommandBoundary],
) -> Result<(), AnnouncementError> {
    let mut event_start = 0;
    for boundary in boundaries {
        for event in &events[event_start..boundary.event_end] {
            if let PresentationEvent::Setup(setup) = event {
                table_max = Some(setup.max_segments);
            }
        }
        if let HudCommandMarker::Announcement(packet) = &boundary.marker {
            packet.validate_table(table_max)?;
        }
        event_start = boundary.event_end;
    }
    Ok(())
}

/// Frozen receipt-time content, independent of own identity and later tables.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementContent {
    /// Positive score computed at the command's position, with no own R addition.
    pub score: f64,
    /// Independently admitted Latin-1 name, with ordinary spaces preserved.
    pub name: String,
    /// Independently admitted Latin-1 message, with no outgoing editor cap.
    pub message: String,
}

/// Existing session lifecycle at the pre-motion control boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnouncementLifecycle {
    /// A death timestamp exists, including the ordinary death hold.
    pub death_pending: bool,
    /// The existing return animation is active on this step.
    pub return_active: bool,
    /// This step completes the existing return animation.
    pub return_completed: bool,
    /// Existing return fraction, without a separate announcement timer.
    pub return_fraction: f64,
}

/// A text line and its complete local presentation policy.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementLine {
    /// Plain text; never interpret as markup.
    pub text: String,
    /// Font size in CSS pixels.
    pub font_size: u8,
    /// Bold weight selection.
    pub bold: bool,
    /// Italic style selection.
    pub italic: bool,
    /// Space following this line in CSS pixels.
    pub space_after: u8,
}

/// Semantic inline content. Alpha is independent of the retained panel fade.
#[derive(Clone, Debug, PartialEq)]
pub enum AnnouncementTextFragment {
    /// Literal text, never markup, using the shared panel font selection.
    Text {
        /// Visible text after HTML input-stream control normalization.
        text: String,
        /// Family, weight, slant, size and white fill.
        style: TextStyle,
        /// Multiply by the panel fade only at submission time.
        alpha: f64,
    },
    /// An explicit line break, unlike collapsible incoming whitespace.
    Break,
}

/// One CSS-normal block, with a separate gap following it.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementTextBlock {
    /// Ordered literal spans and explicit breaks; at most six fragments.
    pub fragments: Vec<AnnouncementTextFragment>,
    /// Logical pixels following this block (the quote uses five).
    pub space_after: u8,
}

impl AnnouncementTextBlock {
    /// Admit the complete block into bounded panel layout. Fragment alpha stays
    /// outside raster styles; future GPU submission must retain that metadata.
    pub fn panel_text(&self) -> Result<PanelText, &'static str> {
        PanelText::new(
            &self
                .fragments
                .iter()
                .map(|fragment| match fragment {
                    AnnouncementTextFragment::Text { text, style, .. } => {
                        PanelTextFragment::Text(text, *style)
                    }
                    AnnouncementTextFragment::Break => PanelTextFragment::Break,
                })
                .collect::<Vec<_>>(),
        )
    }
}

/// Official record-message wording and styles, with no lifecycle opacity baked in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnouncementTextView {
    /// Quote and attribution/length, or a single length block; at most two.
    pub blocks: Vec<AnnouncementTextBlock>,
}

/// Project the four record-message branches. Source-derived wording/typography
/// and control handling are documented in docs/announcements.md; no HTML is used.
pub fn project_announcement_text(content: &AnnouncementContent) -> AnnouncementTextView {
    let span = |text: &str, size, weight, alpha| AnnouncementTextFragment::Text {
        // The reference's escaped HTML text drops NUL and normalizes CR/CRLF.
        // Keep the received owner untouched and do only that work in projection.
        text: text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\0', ""),
        style: TextStyle {
            family: TextFamily::Arial,
            slant: TextSlant::Italic,
            size,
            weight,
            color: [255; 3],
        },
        alpha,
    };
    let normal = TextWeight::Normal;
    let bold = TextWeight::Bold;
    let mut blocks = Vec::new();
    if !content.message.is_empty() {
        blocks.push(AnnouncementTextBlock {
            fragments: vec![
                span("\"", 17., bold, 0.5),
                span(&content.message, 17., bold, 1.),
                span("\"", 17., bold, 0.5),
            ],
            space_after: 5,
        });
    }
    let number = content.score.to_string();
    let mut fragments = if !content.name.is_empty() {
        if !content.message.is_empty() {
            vec![
                span("- ", 13., normal, 0.5),
                span(&content.name, 13., bold, 0.75),
                span(", today's longest", 13., normal, 0.5),
            ]
        } else {
            vec![
                span("Today's longest was ", 13., normal, 0.5),
                span(&content.name, 13., bold, 0.75),
            ]
        }
    } else if !content.message.is_empty() {
        vec![span("- today's longest", 13., normal, 0.5)]
    } else {
        vec![
            span("Today's longest: ", 13., normal, 0.5),
            span(&number, 13., bold, 0.75),
        ]
    };
    if !content.name.is_empty() || !content.message.is_empty() {
        fragments.extend([
            AnnouncementTextFragment::Break,
            span("with a length of ", 13., normal, 0.5),
            span(&number, 13., bold, 0.65),
        ]);
    }
    blocks.push(AnnouncementTextBlock {
        fragments,
        space_after: 0,
    });
    AnnouncementTextView { blocks }
}

/// Backend-neutral authored text and bounded desktop placement.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementView {
    /// Whether an admitted positive-score announcement has been retained.
    pub present: bool,
    /// Raw shared-HUD opacity request, including the completion sentinel -1.
    pub opacity: f64,
    /// Left inset in CSS pixels.
    pub left: u16,
    /// Top inset in CSS pixels.
    pub top: u16,
    /// Container width in CSS pixels.
    pub width: u16,
    /// Container height in CSS pixels.
    pub height: u16,
    /// Message, attribution and length in presentation order, at most three.
    pub lines: Vec<AnnouncementLine>,
    /// Styled projection for graphical adapters. `lines` remains a plain view.
    pub text: AnnouncementTextView,
}

/// Constant-size history aside from one bounded retained announcement.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnouncementState {
    content: Option<Arc<AnnouncementContent>>,
    fade: f64,
    opacity: f64,
}

impl Default for AnnouncementState {
    fn default() -> Self {
        Self {
            content: None,
            fade: -1.,
            opacity: 0.,
        }
    }
}

impl AnnouncementState {
    /// Borrow frozen content without consuming a draw or update.
    pub fn content(&self) -> Option<&AnnouncementContent> {
        self.content.as_deref()
    }

    /// Setup hides but retains the previous announcement.
    pub fn setup(&mut self) {
        self.fade = -1.;
        self.opacity = 0.;
    }

    /// An accepted playing leaderboard starts an alive fade only outside death.
    pub fn leaderboard(&mut self, death_pending: bool) {
        if self.fade == -1. && !death_pending {
            self.fade = 0.;
        }
    }

    pub(crate) fn receive(
        &mut self,
        packet: &AnnouncementPacket,
        tables: Option<&ScoreTables>,
    ) -> Result<(), AnnouncementError> {
        let packet = parse_announcement(&packet.0).map_err(AnnouncementError::Packet)?;
        let tables = tables.ok_or(AnnouncementError::MissingTable)?;
        let score = tables
            .score(packet.count() as usize, packet.fractional_mass())
            .map_err(AnnouncementError::Score)?;
        if score > 0. {
            self.content = Some(Arc::new(AnnouncementContent {
                score,
                name: admitted_text(packet.name()),
                message: admitted_text(packet.message()),
            }));
        }
        Ok(())
    }

    /// Advance once with the existing lag-adjusted frame delta. Invalid lifecycle
    /// observations preserve the entire previous state, including opacity.
    pub fn advance(
        &mut self,
        delta: f64,
        life: AnnouncementLifecycle,
    ) -> Result<(), PresentationError> {
        if !delta.is_finite()
            || !(0. ..=5.).contains(&delta)
            || !life.return_fraction.is_finite()
            || !(0. ..=1.).contains(&life.return_fraction)
            || if life.return_completed {
                !life.return_active || life.death_pending || life.return_fraction != 0.
            } else {
                life.return_active && (!life.death_pending || life.return_fraction <= 0.)
            }
        {
            return Err(PresentationError);
        }
        if life.return_completed {
            self.fade = -1.;
            self.opacity = self.fade;
        } else if life.return_active {
            self.fade = life.return_fraction;
            self.opacity = self.fade;
        } else if !life.death_pending && self.fade != -1. && self.fade != 1. {
            self.fade = (self.fade + 0.01 * delta).min(1.);
            self.opacity = self.fade;
        }
        Ok(())
    }

    /// Project styled text and the retained plain-line view without advancing time.
    pub fn view(&self) -> AnnouncementView {
        let text = self
            .content()
            .map(project_announcement_text)
            .unwrap_or_default();
        let mut lines = Vec::new();
        for block in &text.blocks {
            let mut line = AnnouncementLine {
                text: String::new(),
                font_size: 13,
                bold: false,
                italic: true,
                space_after: 0,
            };
            for fragment in &block.fragments {
                match fragment {
                    AnnouncementTextFragment::Text { text, style, .. } => {
                        if line.text.is_empty() {
                            line.font_size = style.size as u8;
                            line.bold = style.weight == TextWeight::Bold;
                        }
                        line.text.push_str(text);
                    }
                    AnnouncementTextFragment::Break => {
                        lines.push(line.clone());
                        line.text.clear();
                    }
                }
            }
            line.space_after = block.space_after;
            lines.push(line);
        }
        AnnouncementView {
            present: self.content.is_some(),
            opacity: self.opacity,
            left: 8,
            top: 4,
            width: 300,
            height: 228,
            lines,
            text,
        }
    }
}

fn admitted_text(bytes: &[u8]) -> String {
    let mut digits = 0;
    for &byte in bytes {
        if byte.is_ascii_alphabetic() {
            digits = 0;
        } else if byte.is_ascii_digit() {
            digits += 1;
            if digits == 7 {
                return String::new();
            }
        }
    }
    bytes.iter().map(|&byte| char::from(byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(count: u32, mass: u32, name: &[u8], message: &[u8]) -> AnnouncementPacket {
        // Authored synthetic wire data; no reference fixture.
        let mut bytes = vec![b'm'];
        bytes.extend_from_slice(&count.to_be_bytes()[1..]);
        bytes.extend_from_slice(&mass.to_be_bytes()[1..]);
        bytes.push(name.len() as u8);
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(message);
        AnnouncementPacket::copy(&bytes).unwrap()
    }

    fn alive() -> AnnouncementLifecycle {
        AnnouncementLifecycle {
            death_pending: false,
            return_active: false,
            return_completed: false,
            return_fraction: 1.,
        }
    }

    #[test]
    fn replacement_is_positive_only_and_freezes_receipt_score_without_own_identity() {
        let mut state = AnnouncementState::default();
        let tables = ScoreTables::new(1).unwrap();
        state
            .receive(
                &packet(1, 16777215, b"  Name  ", b"<tag> &  text"),
                Some(&tables),
            )
            .unwrap();
        assert_eq!(state.content().unwrap().score, 10.);
        let retained = state.clone();
        for input in [packet(0, 0, b"other", b""), packet(1, 0, b"", b"")] {
            state.receive(&input, Some(&tables)).unwrap();
            assert_eq!(state, retained);
        }
        assert!(
            state
                .receive(&packet(0xffffff, 0, b"", b""), Some(&tables))
                .is_err()
        );
        assert_eq!(state, retained);
        state.setup();
        assert_eq!(state.content(), retained.content());
        assert_eq!(state.view().opacity, 0.);
        state
            .receive(
                &packet(2, 0xffffff, b"new", b"new"),
                Some(&ScoreTables::new(2).unwrap()),
            )
            .unwrap();
        assert_ne!(state.content(), retained.content());
    }

    #[test]
    fn text_admission_is_independent_latin1_uncapped_and_never_markup() {
        let tables = ScoreTables::new(1).unwrap();
        let mut state = AnnouncementState::default();
        let long = [b'x'; 600];
        state
            .receive(&packet(1, 0xffffff, b"123456-7", &long), Some(&tables))
            .unwrap();
        assert_eq!(state.content().unwrap().name, "");
        assert_eq!(state.content().unwrap().message.len(), 600);
        assert_eq!(state.view().lines[1].text, "- today's longest");
        state
            .receive(
                &packet(1, 0xffffff, b"123456a7\xff ", b"123456 7"),
                Some(&tables),
            )
            .unwrap();
        assert_eq!(state.content().unwrap().name, "123456a7ÿ ");
        assert_eq!(state.content().unwrap().message, "");
        assert_eq!(state.view().lines.len(), 2);
        assert_eq!(admitted_text(b"<>&\0\r\n\x7f"), "<>&\0\r\n\u{7f}");
        state
            .receive(&packet(1, 0xffffff, b"", b""), Some(&tables))
            .unwrap();
        assert_eq!(state.view().lines.len(), 1);
        state
            .receive(&packet(1, 0xffffff, b"N", b"A  B"), Some(&tables))
            .unwrap();
        let lines = state.view().lines;
        assert_eq!(
            lines
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>(),
            ["\"A  B\"", "- N, today's longest", "with a length of 10"]
        );
        assert_eq!(
            (
                lines[0].font_size,
                lines[0].bold,
                lines[0].italic,
                lines[0].space_after
            ),
            (17, true, true, 5)
        );
    }

    #[test]
    fn four_branches_have_explicit_breaks_shared_faces_and_independent_alpha() {
        for (name, message, expected, alphas) in [
            (
                "N",
                "M",
                vec!["\"M\"", "- N, today's longest", "with a length of 10"],
                vec![0.5, 1., 0.5, 0.5, 0.75, 0.5, 0.5, 0.65],
            ),
            (
                "",
                "M",
                vec!["\"M\"", "- today's longest", "with a length of 10"],
                vec![0.5, 1., 0.5, 0.5, 0.5, 0.65],
            ),
            (
                "N",
                "",
                vec!["Today's longest was N", "with a length of 10"],
                vec![0.5, 0.75, 0.5, 0.65],
            ),
            ("", "", vec!["Today's longest: 10"], vec![0.5, 0.75]),
        ] {
            let mut state = AnnouncementState::default();
            state
                .receive(
                    &packet(1, 0xffffff, name.as_bytes(), message.as_bytes()),
                    Some(&ScoreTables::new(1).unwrap()),
                )
                .unwrap();
            let view = state.view();
            assert_eq!(
                view.lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                (view.left, view.top, view.width, view.height),
                (8, 4, 300, 228)
            );
            let mut actual = Vec::new();
            let mut breaks = 0;
            for (index, block) in view.text.blocks.iter().enumerate() {
                block.panel_text().unwrap();
                assert_eq!(
                    block.space_after,
                    if !message.is_empty() && index == 0 {
                        5
                    } else {
                        0
                    }
                );
                for fragment in &block.fragments {
                    match fragment {
                        AnnouncementTextFragment::Break => breaks += 1,
                        AnnouncementTextFragment::Text { text, style, alpha } => {
                            actual.push(*alpha);
                            assert_eq!(style.family, TextFamily::Arial);
                            assert_eq!(style.slant, TextSlant::Italic);
                            assert_eq!(style.color, [255; 3]);
                            let quote = !message.is_empty() && index == 0;
                            assert_eq!(style.size, if quote { 17. } else { 13. });
                            assert_eq!(
                                style.weight,
                                if quote || text == "N" || text == "10" {
                                    TextWeight::Bold
                                } else {
                                    TextWeight::Normal
                                }
                            );
                        }
                    }
                }
            }
            assert_eq!(actual, alphas);
            assert_eq!(breaks, usize::from(!name.is_empty() || !message.is_empty()));
            state.leaderboard(false);
            state.advance(5., alive()).unwrap();
            assert_eq!(
                state.view().text,
                view.text,
                "fade never changes fragment styles/alpha"
            );
        }
    }

    #[test]
    fn controls_are_retained_but_project_like_escaped_html_text_and_digits_are_independent() {
        let mut state = AnnouncementState::default();
        let tables = ScoreTables::new(1).unwrap();
        state
            .receive(
                &packet(1, 0xffffff, b"\xff\0\t \r\nN\r", b"<>&\0\r\nA\rB\t\x0c\x7f"),
                Some(&tables),
            )
            .unwrap();
        assert_eq!(state.content().unwrap().name, "ÿ\0\t \r\nN\r");
        assert_eq!(
            state.content().unwrap().message,
            "<>&\0\r\nA\rB\t\u{c}\u{7f}"
        );
        assert_eq!(state.view().lines[0].text, "\"<>&\nA\nB\t\u{c}\u{7f}\"");
        assert_eq!(state.view().lines[1].text, "- ÿ\t \nN\n, today's longest");
        for separator in [b'-', b' ', b'\t', 0, 255] {
            let text = [b'1', b'2', b'3', separator, b'4', b'5', b'6', b'7'];
            state
                .receive(&packet(1, 0xffffff, &text, b"valid"), Some(&tables))
                .unwrap();
            assert_eq!(state.content().unwrap().name, "");
            assert_eq!(state.content().unwrap().message, "valid");
            state
                .receive(&packet(1, 0xffffff, b"valid", &text), Some(&tables))
                .unwrap();
            assert_eq!(state.content().unwrap().name, "valid");
            assert_eq!(state.content().unwrap().message, "");
        }
        assert_eq!(admitted_text(b"123456A789012"), "123456A789012");
        assert_eq!(admitted_text(b"123456z7"), "123456z7");
    }

    #[test]
    fn maximum_latin1_message_is_retained_and_panel_admission_does_not_apply_editor_cap() {
        use super::super::{
            BodyRect, PanelTextBlock, TextMeasurement, TextMetrics, layout_panel_text,
        };
        use crate::protocol::MAX_FRAME_BYTES;
        struct Metrics;
        impl TextMetrics for Metrics {
            fn measure(
                &mut self,
                text: &str,
                _: TextStyle,
            ) -> Result<TextMeasurement, &'static str> {
                Ok(TextMeasurement {
                    advance: text.chars().count() as f64 * 10.,
                    ascent: 13.,
                    descent: 4.,
                    font_ascent: 13.,
                    font_descent: 4.,
                })
            }
        }
        for name in [vec![], vec![255; 255]] {
            let message = vec![255; MAX_FRAME_BYTES - 8 - name.len()];
            let mut state = AnnouncementState::default();
            state
                .receive(
                    &packet(1, 0xffffff, &name, &message),
                    Some(&ScoreTables::new(1).unwrap()),
                )
                .unwrap();
            assert_eq!(
                state.content().unwrap().message.chars().count(),
                message.len()
            );
            assert_eq!(state.content().unwrap().name.chars().count(), name.len());
            for block in &state.view().text.blocks {
                let text = block.panel_text().unwrap();
                let plan = layout_panel_text(
                    &PanelTextBlock {
                        text: &text,
                        bounds: BodyRect {
                            x: 8.,
                            y: 4.,
                            width: 300.,
                            height: 228.,
                        },
                        line_height: 20.,
                        device_scale: 1.,
                        max_runs: 48,
                    },
                    &mut Metrics,
                )
                .unwrap();
                assert!(plan.scanned_bytes <= super::super::PANEL_SCAN_BYTES);
                assert!(plan.runs.len() <= 48);
            }
            let mut oversized = packet(1, 0xffffff, &name, &message).0;
            oversized.push(255);
            assert!(AnnouncementPacket::copy(&oversized).is_err());
        }
    }

    #[test]
    fn fixed_step_fade_has_no_expiry_and_preserves_death_hold_and_completion_sentinel() {
        let mut state = AnnouncementState::default();
        state
            .receive(
                &packet(1, 0xffffff, b"N", b"M"),
                Some(&ScoreTables::new(1).unwrap()),
            )
            .unwrap();
        state.advance(5., alive()).unwrap();
        assert_eq!(state.view().opacity, 0.);
        state.leaderboard(true);
        state.advance(5., alive()).unwrap();
        assert_eq!(state.view().opacity, 0.);
        state.leaderboard(false);
        state.advance(2.5, alive()).unwrap();
        assert_eq!(state.view().opacity, 0.025);
        let hold = AnnouncementLifecycle {
            death_pending: true,
            ..alive()
        };
        state.advance(5., hold).unwrap();
        assert_eq!(state.view().opacity, 0.025);
        state
            .advance(
                0.,
                AnnouncementLifecycle {
                    return_active: true,
                    return_fraction: 0.6,
                    ..hold
                },
            )
            .unwrap();
        assert_eq!(state.view().opacity, 0.6);
        state
            .advance(
                0.,
                AnnouncementLifecycle {
                    return_active: true,
                    return_completed: true,
                    return_fraction: 0.,
                    ..alive()
                },
            )
            .unwrap();
        assert_eq!(state.view().opacity, -1.);
        for _ in 0..200 {
            state.advance(5., alive()).unwrap();
        }
        assert_eq!(state.view().opacity, -1.);
        assert!(state.content().is_some());
        state.setup();
        state.leaderboard(false);
        for _ in 0..100 {
            state.advance(5., alive()).unwrap();
        }
        assert_eq!(state.view().opacity, 1.);
        assert!(state.content().is_some());
    }

    #[test]
    fn malformed_packet_and_invalid_lifecycle_are_atomic() {
        assert!(AnnouncementPacket::copy(b"m\0\0\0\0\0\0\x02x").is_err());
        let mut state = AnnouncementState::default();
        let before = state.clone();
        assert_eq!(
            state.receive(&packet(1, 0xffffff, b"", b""), None),
            Err(AnnouncementError::MissingTable)
        );
        for delta in [-1., 5.1, f64::NAN, f64::INFINITY] {
            assert!(state.advance(delta, alive()).is_err());
        }
        assert!(
            state
                .advance(
                    1.,
                    AnnouncementLifecycle {
                        return_active: true,
                        ..alive()
                    }
                )
                .is_err()
        );
        assert_eq!(state, before);
    }
}
