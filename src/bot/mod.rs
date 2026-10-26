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

/// SHA256 of the shipped `bot/data/resolved.json` parameter manifest, not a
/// performance result. It keeps only the A4-mean entry, unchanged from the
/// two-policy manifest `ff7591be…a57a482` that the adoption evidence pins.
pub const PARAMETER_MANIFEST_SHA256: &str =
    "94669e26cb399b3f7facb9de14da01df01823cecc90fd3a1e6f5970f2fa0490e";

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
