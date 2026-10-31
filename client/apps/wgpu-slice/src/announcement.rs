//! Thin submission of the shared record projection; no receipt parser or clock.
use crate::{Result, frame::Frame, scene::Resource, text::*};
use slither_client::presentation::*;

#[derive(Default)]
pub struct Announcement {
    retained: AnnouncementTextView,
    blocks: Vec<PanelText>,
}
struct Metrics<'a>(&'a mut TextCache);
impl TextMetrics for Metrics<'_> {
    fn measure(
        &mut self,
        text: &str,
        style: TextStyle,
    ) -> std::result::Result<TextMeasurement, &'static str> {
        self.0
            .measure_panel(text, style)
            .map_err(|_| "Announcement font metrics")
    }
}
impl Announcement {
    /// Admission is retained across fades/resizes. Source indices preserve equal
    /// style spans (quote/message/quote) and their distinct draw-time opacity.
    pub fn plan(
        &mut self,
        view: &AnnouncementView,
        text: &mut TextCache,
        scale: f64,
    ) -> Result<(PanelTextPlan, Vec<f64>)> {
        if self.retained != view.text {
            let blocks = view
                .text
                .blocks
                .iter()
                .map(|b| {
                    PanelText::preserving_fragments(
                        &b.fragments
                            .iter()
                            .map(|f| match f {
                                AnnouncementTextFragment::Text { text, style, .. } => {
                                    PanelTextFragment::Text(text, *style)
                                }
                                AnnouncementTextFragment::Break => PanelTextFragment::Break,
                            })
                            .collect::<Vec<_>>(),
                    )
                    .map_err(str::to_owned)
                })
                .collect::<Result<Vec<_>>>()?;
            self.retained = view.text.clone();
            self.blocks = blocks;
        }
        let mut output = PanelTextPlan::default();
        let mut alpha = Vec::new();
        let mut top = f64::from(view.top);
        let bottom = top + f64::from(view.height);
        for (block, admitted) in self.retained.blocks.iter().zip(&self.blocks) {
            if top >= bottom || output.runs.len() == 48 {
                break;
            }
            let mut line_height = 14_f64;
            for f in &block.fragments {
                if let AnnouncementTextFragment::Text { style, .. } = f {
                    line_height = line_height.max(style.line_height());
                }
            }
            let (plan, sources) = layout_panel_text_with_sources(
                &PanelTextBlock {
                    text: admitted,
                    bounds: BodyRect {
                        x: f64::from(view.left),
                        y: top,
                        width: f64::from(view.width),
                        height: bottom - top,
                    },
                    line_height,
                    device_scale: scale,
                    max_runs: 48 - output.runs.len(),
                },
                &mut Metrics(text),
            )?;
            top = plan
                .runs
                .iter()
                .map(|r| r.clip.y + r.clip.height)
                .fold(top, f64::max)
                + f64::from(block.space_after);
            for source in sources {
                let AnnouncementTextFragment::Text { alpha: a, .. } = block.fragments[source]
                else {
                    return Err("Announcement source fragment".into());
                };
                alpha.push(a);
            }
            output.scanned_bytes += plan.scanned_bytes;
            output.runs.extend(plan.runs);
        }
        Ok((output, alpha))
    }
    pub fn append(
        &mut self,
        view: &AnnouncementView,
        text: &mut TextCache,
        frame: &mut Frame,
        resources: &mut Vec<Resource>,
    ) -> Result<()> {
        // Hide/return sentinel retires this producer without clearing its content.
        if !view.present || view.opacity <= 0. {
            text.draw_panel_composition(&[], frame.viewport_scale)?;
            return Ok(());
        }
        let (plan, alpha) = self.plan(view, text, frame.viewport_scale)?;
        let draws = text
            .draw_overlay_panels_with_run_alphas(
                &[PanelSubmission {
                    group: PanelGroup::Announcement,
                    plan: &plan,
                    alpha: view.opacity.clamp(0., 1.),
                }],
                frame.viewport_scale,
                &[&alpha],
            )?
            .remove(0);
        if draws.len() > crate::render::PANEL_PRODUCER_DRAWS {
            return Err("Announcement frame draw budget".into());
        }
        let viewport = BodyRect {
            x: 0.,
            y: 0.,
            width: f64::from(frame.extent[0]) / frame.viewport_scale,
            height: f64::from(frame.extent[1]) / frame.viewport_scale,
        };
        for (mut draw, resource) in draws {
            if crate::render::clip_overlay_text(&mut draw, viewport, frame.viewport_scale) {
                crate::frame::push_draw(&mut frame.draws, draw)?;
                resources.push(resource);
            }
        }
        Ok(())
    }
}
