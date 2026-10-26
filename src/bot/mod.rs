//! Frozen received-observation Forager policy, available with the `bot` feature.
//!
//! A4 (the adopted A4-mean parameters) is the only policy.
//! Its parameters are immutable and private.
//! The policy uses no hidden world state and retains only reusable scratch space.
//! See the [adoption evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-a4-adoption/REPORT.md)
//! and [frozen port provenance](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-forager-port/REPORT.md).
// Preserve the frozen policy's decision bits; see the port evidence above.
#[allow(clippy::collapsible_if)]
mod kernel;
mod params;

pub use kernel::{Intent, MAX_SENT, Sent, Timing};

use crate::{Action, Observation};

/// SHA256 of the original frozen parameter manifest pinned by the adoption evidence.
/// The shipped `bot/data/resolved.json` retains only its unchanged A4-mean entry;
/// this identifies the source manifest, not the reduced file or a performance result.
pub const PARAMETER_MANIFEST_SHA256: &str =
    "ff7591bea57be7925878d1e7407a7a8ddaf7d37e6c2045180d1e666f3c57a482";

/// Reusable policy instance. Decisions carry no learned or per-seat state.
#[derive(Clone, Debug)]
pub struct Forager {
    params: kernel::Params,
    scratch: kernel::Scratch,
}

impl Default for Forager {
    /// Construct the A4 policy with reusable decision buffers.
    fn default() -> Self {
        Self::new()
    }
}

impl Forager {
    /// Construct the A4 policy with reusable decision buffers.
    pub fn new() -> Self {
        Self {
            params: params::A4_MEAN,
            scratch: Default::default(),
        }
    }

    /// Choose an action from received observations, timing and opponent intentions.
    pub fn decide(
        &mut self,
        observation: &Observation,
        timing: &Timing,
        intents: &[Intent],
    ) -> Action {
        kernel::decide(
            observation,
            timing,
            intents,
            &self.params,
            &mut self.scratch,
        )
    }

    #[cfg(feature = "networking")]
    pub(crate) fn uplink_share(&self) -> f64 {
        self.params.uplink_share
    }
}

#[cfg(test)]
mod tests;
