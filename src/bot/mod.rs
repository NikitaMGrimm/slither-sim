//! Frozen received-observation Forager policy, available with the `bot` feature.
//!
//! A4 (the adopted A4-mean parameters) is the default; A1 remains selectable.
//! Both releases have immutable, private parameters.
//! The policy uses no hidden world state and retains only reusable scratch space.
//! See the [adoption evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-a4-adoption/REPORT.md)
//! and [frozen port provenance](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-forager-port/REPORT.md).
// Preserve the frozen be3a035 control flow; see the port evidence linked above.
#[allow(clippy::collapsible_if)]
mod kernel;
mod params;

pub use kernel::{Intent, MAX_SENT, Sent, Timing};

use crate::{Action, Observation};

/// Immutable named policy release.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyId {
    /// The earlier A1 parameters, resolved against the frozen be3a035 kernel.
    A1,
    /// The adopted A4-mean parameters, resolved against the frozen be3a035 kernel.
    #[default]
    A4,
}

/// SHA256 of the complete frozen `bot/data/resolved.json` parameter manifest.
/// Identifies the resolved A1 and A4 data, not a performance result.
pub const PARAMETER_MANIFEST_SHA256: &str =
    "ff7591bea57be7925878d1e7407a7a8ddaf7d37e6c2045180d1e666f3c57a482";

/// Reusable policy instance. Decisions carry no learned or per-seat state.
#[derive(Clone, Debug)]
pub struct Forager {
    params: kernel::Params,
    scratch: kernel::Scratch,
}

impl Default for Forager {
    /// Construct the default policy with reusable decision buffers.
    fn default() -> Self {
        Self::new(PolicyId::default())
    }
}

impl Forager {
    /// Construct the named policy with reusable decision buffers.
    pub fn new(policy: PolicyId) -> Self {
        let params = match policy {
            PolicyId::A1 => params::A1,
            PolicyId::A4 => params::A4_MEAN,
        };
        Self {
            params,
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
