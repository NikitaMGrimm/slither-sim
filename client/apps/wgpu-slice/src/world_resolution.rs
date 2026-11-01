//! Attachment policy only. Never changes Frame extent, projection or input.
use crate::{Result, frame::MAX_EXTENT};
use slither_client::settings::WorldResolution;

/// Existing renderer attachment guard, independent of uploaded-image limits.
pub const ATTACHMENT_BYTES: u64 = 128 * 1024 * 1024;

/// Native host window/input boundary, carrying the requested presentation mode.
/// Projection and CSS pointer dimensions remain independent of that request.
#[derive(Clone, Copy)]
pub struct WorldWindow {
    pub physical: [u32; 2],
    pub dpr: f64,
    pub requested: WorldResolution,
}
impl WorldWindow {
    /// Refresh zoom's host observation using current DPR and Blink's mouse CSS
    /// conversion, without admitting a stock pointer event or a boost press.
    pub fn observe_cursor(
        &self,
        game: &mut crate::game::LocalGame,
        physical: [f64; 2],
    ) -> Result<()> {
        if self.physical.contains(&0) {
            return Ok(());
        }
        let [x, y] = slither_client::desktop_input::mouse_client_position(physical, self.dpr);
        let [width, height] = self.css_size();
        game.observe_cursor(x, y, width, height)
    }
    pub fn css_size(self) -> [f64; 2] {
        self.physical.map(|n| f64::from(n) / self.dpr)
    }
    pub fn viewport(self, game: &mut crate::game::LocalGame) -> Result<[u32; 2]> {
        game.window_viewport(self.physical, self.dpr)
    }
    pub fn pointer(
        self,
        game: &mut crate::game::LocalGame,
        cursor: [f64; 2],
        down: bool,
    ) -> Result<()> {
        if self.physical.contains(&0) {
            return Ok(());
        }
        let [width, height] = self.css_size();
        game.pointer(cursor[0], cursor[1], width, height, down)
    }
}

/// Validate actual attachment pixels, including downlevel negotiated limits.
pub fn validate_target(extent: [u32; 2], samples: u32, max_dimension: u32) -> Result<()> {
    if !matches!(samples, 1 | 4)
        || extent.contains(&0)
        || extent.iter().any(|n| *n > MAX_EXTENT || *n > max_dimension)
    {
        return Err("World attachment extent/sample limit".into());
    }
    if u64::from(extent[0]) * u64::from(extent[1]) * 4 * u64::from(samples) > ATTACHMENT_BYTES {
        return Err("Render attachment budget".into());
    }
    Ok(())
}

/// One immutable presentation decision, independent of model/frame preparation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldTargetPlan {
    pub effective: WorldResolution,
    pub attachment: [u32; 2],
    pub blit: bool,
    pub samples: u32,
    pub native_unavailable: bool,
}
impl WorldTargetPlan {
    pub fn new(
        requested: WorldResolution,
        projection: [u32; 2],
        physical: [u32; 2],
        samples: u32,
        max_dimension: u32,
    ) -> Result<Self> {
        if projection.contains(&0) || physical.contains(&0) {
            return Err("Zero world/presentation extent".into());
        }
        let native_unavailable = requested == WorldResolution::Native
            && validate_target(physical, samples, max_dimension).is_err();
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
        let blit = effective == WorldResolution::Original && projection != physical;
        // Preserve Original's existing scaled-backing guard.
        if blit && projection.iter().any(|n| *n > 1500) {
            return Err("Desktop backing limit".into());
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
                    if samples == 1 {
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
        assert!(
            WorldTargetPlan::new(Native, [1500, 845], [4096, 2160], 4, 8192)
                .unwrap()
                .native_unavailable
        );
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
