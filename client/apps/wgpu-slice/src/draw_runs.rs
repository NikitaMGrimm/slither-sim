//! Consecutive resolved paint states only; never sort or copy geometry.
use crate::frame::{Quad, ResourceKey};
use slither_client::presentation::BodyBlend;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pipeline {
    Over,
    Lighter,
    BlackOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DrawState {
    pub pipeline: Pipeline,
    pub key: ResourceKey,
    pub repeat: bool,
}
impl DrawState {
    pub fn new(quad: &Quad, black: bool) -> Self {
        Self {
            pipeline: if quad.black_effect && black && quad.blend == BodyBlend::SourceOver {
                Pipeline::BlackOver
            } else if quad.blend == BodyBlend::Lighter {
                Pipeline::Lighter
            } else {
                Pipeline::Over
            },
            key: quad.key,
            repeat: quad.repeat,
        }
    }
}

/// Vertex ranges for maximal adjacent runs. The caller has preflighted MAX_DRAWS.
pub(super) fn draw_runs(
    states: impl Iterator<Item = DrawState>,
) -> impl Iterator<Item = (DrawState, Range<u32>)> {
    let mut states = states.enumerate().peekable();
    std::iter::from_fn(move || {
        let (start, state) = states.next()?;
        let mut end = start + 1;
        while states.next_if(|(_, next)| *next == state).is_some() {
            end += 1;
        }
        // Preserve triangle-list primitive order, including overlapping translucent quads.
        // WebGPU CRD 2026-09-15, rasterization order and output merging:
        // https://www.w3.org/TR/2026/CRD-webgpu-20260915/#rasterization
        // https://www.w3.org/TR/2026/CRD-webgpu-20260915/#output-merging
        Some((state, start as u32 * 6..end as u32 * 6))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> DrawState {
        DrawState {
            pipeline: Pipeline::Over,
            key: ResourceKey {
                handle: 1,
                revision: 1,
            },
            repeat: false,
        }
    }

    #[test]
    fn empty_single_and_full_budget_run() {
        assert_eq!(draw_runs(std::iter::empty()).count(), 0);
        let a = state();
        assert_eq!(draw_runs([a].into_iter()).collect::<Vec<_>>(), [(a, 0..6)]);
        assert_eq!(
            draw_runs(std::iter::repeat_n(a, crate::frame::MAX_DRAWS)).collect::<Vec<_>>(),
            [(a, 0..crate::frame::MAX_DRAWS as u32 * 6)]
        );
    }

    #[test]
    fn adjacent_runs_are_maximal_and_never_reordered() {
        let a = state();
        let b = DrawState {
            pipeline: Pipeline::Lighter,
            ..a
        };
        assert_eq!(
            draw_runs([a, a, b, b, a].into_iter()).collect::<Vec<_>>(),
            [(a, 0..12), (b, 12..24), (a, 24..30)]
        );
        assert_eq!(
            draw_runs([a, b, a, b].into_iter()).collect::<Vec<_>>(),
            [(a, 0..6), (b, 6..12), (a, 12..18), (b, 18..24)]
        );
    }

    #[test]
    fn pipeline_texture_revision_and_repeat_each_split_a_run() {
        let a = state();
        for b in [
            DrawState {
                pipeline: Pipeline::BlackOver,
                ..a
            },
            DrawState {
                key: ResourceKey { handle: 2, ..a.key },
                ..a
            },
            DrawState {
                key: ResourceKey {
                    revision: 2,
                    ..a.key
                },
                ..a
            },
            DrawState { repeat: true, ..a },
        ] {
            assert_eq!(
                draw_runs([a, b].into_iter()).collect::<Vec<_>>(),
                [(a, 0..6), (b, 6..12)]
            );
        }
    }

    #[test]
    fn runs_use_resolved_pipeline_rather_than_black_effect_flag() {
        let mut quad = Quad {
            key: state().key,
            repeat: false,
            black_effect: false,
            blend: BodyBlend::SourceOver,
            vertices: [[0.; 5]; 6],
        };
        let ordinary = DrawState::new(&quad, true);
        quad.black_effect = true;
        assert_eq!(DrawState::new(&quad, false), ordinary);
        assert_eq!(DrawState::new(&quad, true).pipeline, Pipeline::BlackOver);
        quad.blend = BodyBlend::Lighter;
        assert_eq!(DrawState::new(&quad, true).pipeline, Pipeline::Lighter);
        assert_eq!(DrawState::new(&quad, false).pipeline, Pipeline::Lighter);
    }
}
