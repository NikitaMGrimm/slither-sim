//! Frozen received-observation Forager policy, available with the `bot` feature.
//!
//! A1 is the sole selectable release. Its parameters are immutable and private.
//! The policy uses no hidden world state and retains only reusable scratch space.
//! Historical defaults, A4-mean test data and port provenance are described in
//! [the evidence note](https://github.com/NikitaMGrimm/slither-sim/blob/50578c9ce90c36af19b3612e0657b1ad24bc03ea/research/bots/2026-10-26-forager-port/REPORT.md).
// Preserve the frozen be3a035 control flow; see the port evidence linked above.
#[allow(clippy::collapsible_if)]
mod kernel;
mod params;

pub use kernel::{Intent, MAX_SENT, Sent, Timing};

use crate::{Action, Observation};

/// Immutable named policy release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PolicyId {
    /// The adopted A1 parameters, resolved against the frozen be3a035 kernel.
    A1,
}

/// Reusable policy instance. Decisions carry no learned or per-seat state.
#[derive(Clone, Debug)]
pub struct Forager {
    params: kernel::Params,
    scratch: kernel::Scratch,
}

impl Forager {
    /// Construct the named policy with reusable decision buffers.
    pub fn new(policy: PolicyId) -> Self {
        let params = match policy {
            PolicyId::A1 => params::A1,
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

    #[cfg(all(test, feature = "networking"))]
    pub(crate) fn a4_mean_for_test() -> Self {
        Self {
            params: params::A4_MEAN,
            scratch: Default::default(),
        }
    }

    #[cfg(feature = "networking")]
    pub(crate) fn uplink_share(&self) -> f64 {
        self.params.uplink_share
    }
}

#[cfg(test)]
mod tests;
