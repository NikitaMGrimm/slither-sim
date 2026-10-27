//! Effective geometric rules, separate from limited observations and entity data.
use crate::{BoundaryMode, ErrorKind, SimError, Simulator};
use serde::{Deserialize, Serialize};

// Fields stay in alphabetical wire order: the canonical JSON writer otherwise
// sorts them through scratch space on every encoded frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldRules {
    pub boundary_mode: BoundaryMode,
    pub free_boost_at_floor: bool,
    pub kill_score_seconds: Option<f64>,
    pub min_score: Option<u32>,
    pub pit_radius: Option<f64>,
}

/// Rules and the requesting slot's rolling score for a raw geometric bot driver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometricContext {
    pub rules: WorldRules,
    pub kill_score: usize,
}

impl Simulator {
    #[inline]
    pub fn rules(&self) -> WorldRules {
        let config = self.config();
        WorldRules {
            boundary_mode: config.boundary_mode,
            free_boost_at_floor: config.min_score.is_some(),
            kill_score_seconds: config.kill_score_seconds,
            min_score: config.min_score,
            pit_radius: self.pit_radius(),
        }
    }

    pub fn geometric_context(&self, slot: usize) -> Result<GeometricContext, SimError> {
        let kill_score = self.kill_score(slot).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownSlot, "Unknown agent slot").at_slot(slot)
        })?;
        Ok(GeometricContext {
            rules: self.rules(),
            kill_score,
        })
    }
}
