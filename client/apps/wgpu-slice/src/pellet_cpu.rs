//! Exact numeric preparation shared by serial and native chunked submission.
use super::{soft_layers::covered_area, zero_pair};
use crate::{
    Result,
    frame::Frame,
    pellet_batch::{Sources, exact_instance, glow_pair},
};
use slither_client::presentation::{
    BodyPaintView, FoodPaintSnapshot, FoodPaintState, food_body_alpha, food_pulse,
};

#[derive(Clone, Copy)]
pub(super) struct Input {
    pub slot: usize,
    pub animation: FoodPaintState,
}

pub(super) struct Projected {
    slot: usize,
    bytes: [u8; 96],
    bits: u32,
    areas: [f64; 2],
    bins: [u8; 2],
}

impl Projected {
    pub fn apply(&self, mirror: &mut [[u8; 96]], flags: &mut [u32], coverage: &mut [f64; 4]) {
        mirror[self.slot] = self.bytes;
        flags[self.slot] = self.bits;
        // Merge in reverse world order, inner then outer. Chunk totals would
        // reassociate f64 addition and could alter glow target admission.
        for i in 0..2 {
            if self.bins[i] > 0 {
                coverage[self.bins[i] as usize] += self.areas[i];
            }
        }
    }
}

#[inline(always)]
pub(super) fn project(
    frame: &Frame,
    input: Input,
    sources: &Sources,
    view: BodyPaintView,
    user_zoom: f64,
    attachment: [u32; 2],
) -> Result<Projected> {
    let f = FoodPaintSnapshot {
        animation: input.animation,
        sources: sources.groups[sources.slot_groups[input.slot]].sources,
    };
    let pulse = food_pulse(f);
    let glows = [
        glow_pair(frame, f, view, false, pulse, user_zoom)?,
        glow_pair(frame, f, view, true, pulse, user_zoom)?,
    ];
    let mut bits = 1;
    let alpha = food_body_alpha(f, 1., pulse);
    if zero_pair(alpha[0] as f32, alpha[1] as f32) {
        bits |= 32;
    }
    let mut areas = [0.; 2];
    let mut bins = [0; 2];
    for (outer, (q, second)) in glows.iter().enumerate() {
        let bin = super::glow_bin(q, *second, attachment);
        bins[outer] = bin;
        if bin > 0 {
            areas[outer] = covered_area(&q.vertices);
        }
        bits |= u32::from(bin) << if outer == 1 { 3 } else { 1 };
        if zero_pair(q.vertices[0][4], *second) {
            bits |= if outer == 1 { 128 } else { 64 };
        }
    }
    Ok(Projected {
        slot: input.slot,
        bytes: exact_instance(frame, f, view, &glows, pulse, user_zoom)?.bytes(),
        bits,
        areas,
        bins,
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "pellet_workers.rs"]
pub(super) mod workers;
