//! World attachment policy, independent of projection, input and GPU allocation.
use crate::settings::WorldResolution;

/// Maximum supported world or overlay attachment axis.
pub const MAX_EXTENT: u32 = 8192;
/// Existing hard attachment guard, independent of sampled image residency.
pub const ATTACHMENT_BYTES: u64 = 128 * 1024 * 1024;
/// Opt-in Native keeps MSAA4 only within this persistent-target allowance.
/// At larger outputs single-sample native coverage preserves full physical resolution.
/// This is a memory policy, not a frame-time promise; see docs/world-resolution.md.
pub const NATIVE_MSAA_BYTES: u64 = 64 * 1024 * 1024;

/// Validate actual attachment pixels, including downlevel negotiated limits.
pub fn validate_target(
    extent: [u32; 2],
    samples: u32,
    max_dimension: u32,
) -> Result<(), &'static str> {
    if !matches!(samples, 1 | 4)
        || extent.contains(&0)
        || extent.iter().any(|n| *n > MAX_EXTENT || *n > max_dimension)
    {
        return Err("World attachment extent/sample limit");
    }
    if u64::from(extent[0]) * u64::from(extent[1]) * 4 * u64::from(samples) > ATTACHMENT_BYTES {
        return Err("Render attachment budget");
    }
    Ok(())
}

/// One immutable presentation decision, independent of model/frame preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldTargetPlan {
    /// Effective mode after capability fallback.
    pub effective: WorldResolution,
    /// Raster pixels; never changes the frame projection.
    pub attachment: [u32; 2],
    /// Whether Original needs its sampled backing and linear blit.
    pub blit: bool,
    /// World attachment sample count (overlay remains single-sample).
    pub samples: u32,
    /// Native cannot fit even at sample count one.
    pub native_unavailable: bool,
}
impl WorldTargetPlan {
    /// Select attachments without mutating the requested preference.
    pub fn new(
        requested: WorldResolution,
        projection: [u32; 2],
        physical: [u32; 2],
        samples: u32,
        max_dimension: u32,
    ) -> Result<Self, &'static str> {
        if projection.contains(&0) || physical.contains(&0) {
            return Err("Zero world/presentation extent");
        }
        if !matches!(samples, 1 | 4) {
            return Err("World attachment extent/sample limit");
        }
        // Native's physical pixels already improve edge coverage. Avoid a large
        // persistent MSAA target instead of silently returning to a soft backing.
        // Original retains its exact sample count and allocation behavior.
        let native_samples = if u64::from(physical[0]) * u64::from(physical[1])
            > NATIVE_MSAA_BYTES / (4 * u64::from(samples))
        {
            1
        } else {
            samples
        };
        let native_unavailable = requested == WorldResolution::Native
            && validate_target(physical, native_samples, max_dimension).is_err();
        let effective = if requested == WorldResolution::Native && !native_unavailable {
            WorldResolution::Native
        } else {
            WorldResolution::Original
        };
        let attachment = if effective == WorldResolution::Native {
            physical
        } else {
            projection
        };
        let samples = if effective == WorldResolution::Native {
            native_samples
        } else {
            samples
        };
        let blit = effective == WorldResolution::Original && projection != physical;
        // Preserve Original's existing scaled-backing guard.
        if blit && projection.iter().any(|n| *n > 1500) {
            return Err("Desktop backing limit");
        }
        validate_target(attachment, samples, max_dimension)?;
        Ok(Self {
            effective,
            attachment,
            blit,
            samples,
            native_unavailable,
        })
    }
    /// Persistent multisample target bytes, excluding driver padding.
    pub fn msaa_bytes(self) -> u64 {
        if self.samples == 1 {
            0
        } else {
            u64::from(self.attachment[0])
                * u64::from(self.attachment[1])
                * 4
                * u64::from(self.samples)
        }
    }
    /// Persistent sampled Original backing bytes.
    pub fn backing_bytes(self) -> u64 {
        if self.blit {
            u64::from(self.attachment[0]) * u64::from(self.attachment[1]) * 4
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use WorldResolution::{Native, Original};
    #[test]
    fn native_msaa_budget_is_inclusive_and_fallback_keeps_original_samples() {
        let plan = |mode, extent, samples, limit| {
            WorldTargetPlan::new(mode, [1500, 845], extent, samples, limit).unwrap()
        };
        assert_eq!(plan(Native, [2048, 2048], 4, 8192).samples, 4);
        assert_eq!(plan(Native, [2048, 2049], 4, 8192).samples, 1);
        let unsupported = plan(Native, [3840, 2160], 4, 2048);
        assert!(unsupported.native_unavailable);
        assert_eq!(unsupported.effective, Original);
        assert_eq!(unsupported.samples, 4);
        assert_eq!(unsupported.msaa_bytes(), 1500 * 845 * 16);
        assert!(plan(Native, [u32::MAX, u32::MAX], 4, 8192).native_unavailable);
        assert!(plan(Native, [8192, 4097], 4, 8192).native_unavailable);
        assert!(WorldTargetPlan::new(Native, [1500, 845], [3840, 2160], 2, 8192).is_err());
    }
    #[test]
    fn original_guard_and_native_direct_path_use_distinct_extents() {
        for physical in [[1920, 1080], [2560, 1440], [3840, 2160]] {
            for samples in [1, 4] {
                let original =
                    WorldTargetPlan::new(Original, [1500, 845], physical, samples, 8192).unwrap();
                assert_eq!(original.attachment, [1500, 845]);
                assert!(original.blit);
                assert_eq!(original.backing_bytes(), 5_070_000);
                let native =
                    WorldTargetPlan::new(Native, [1500, 845], physical, samples, 8192).unwrap();
                assert_eq!(native.attachment, physical);
                assert_eq!(native.effective, Native);
                assert!(!native.blit);
                assert_eq!(native.backing_bytes(), 0);
                assert_eq!(
                    native.msaa_bytes(),
                    if samples == 1 || physical == [3840, 2160] {
                        0
                    } else {
                        u64::from(physical[0]) * u64::from(physical[1]) * 16
                    }
                );
            }
        }
        assert!(WorldTargetPlan::new(Original, [1501, 845], [1920, 1080], 4, 8192).is_err());
        assert!(WorldTargetPlan::new(Native, [1501, 845], [1920, 1080], 4, 8192).is_ok());
        assert!(
            !WorldTargetPlan::new(Original, [64, 64], [64, 64], 1, 8192)
                .unwrap()
                .blit
        );
    }
    #[test]
    fn unsupported_native_falls_back_and_recovers_without_changing_preference() {
        // WebGL2's baseline is not the negotiated resolution on every adapter.
        let p = WorldTargetPlan::new(Native, [1500, 845], [2560, 1440], 4, 2048).unwrap();
        assert!(p.native_unavailable);
        assert_eq!(p.effective, Original);
        assert!(p.blit);
        assert_eq!(
            WorldTargetPlan::new(Native, [1500, 845], [1920, 1080], 4, 2048)
                .unwrap()
                .effective,
            Native
        );
        assert_eq!(
            WorldTargetPlan::new(Native, [1500, 845], [2560, 1440], 4, 4096)
                .unwrap()
                .effective,
            Native
        );
        let large = WorldTargetPlan::new(Native, [1500, 845], [4096, 2160], 4, 8192).unwrap();
        assert!(!large.native_unavailable);
        assert_eq!(large.effective, Native);
        assert_eq!(large.samples, 1);
        for (extent, samples, limit) in [
            ([0, 64], 1, 8192),
            ([64, 0], 4, 8192),
            ([8193, 1], 1, 16384),
            ([4096, 1], 1, 2048),
            ([64, 64], 2, 8192),
        ] {
            assert!(validate_target(extent, samples, limit).is_err());
        }
        assert!(validate_target([4096, 2048], 4, 8192).is_ok()); // inclusive 128 MiB
        assert!(validate_target([4096, 2049], 4, 8192).is_err());
        assert!(validate_target([8192, 4096], 1, 8192).is_ok());
        assert!(validate_target([8192, 4097], 1, 8192).is_err());
    }
}
