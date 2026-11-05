//! Page and world composition below the egui menus. No input, session or clock.
use crate::{
    Result,
    frame::{self, Draw},
    materials,
    render::PreparedFrame,
    scene::Resource,
    text::*,
};
use slither_client::presentation::*;
use std::collections::BTreeMap;

// Windows scaling tops out at 500%. Above that, capping texel density is
// visually negligible and keeps the wordmark and spinner rasters bounded.
pub const MAX_MENU_RASTER_DENSITY: f64 = 5.;
pub(crate) const MENU_PICTURE_QUOTA: &str = "Menu whole-picture pixel budget";

const PAGE_HANDLE: u64 = crate::bank::MENU_MATERIAL_HANDLE_BASE + 1;
const PAGE_COLOR: [u8; 3] = [22, 28, 34];

/// Immutable prepared resources survive subsequent resize, retirement and art replacement.
pub struct MenuFrame {
    pub extent: [u32; 2],
    pub viewport_scale: f64,
    /// Page colour below the world, in the frame's coordinate space; absent when login is hidden.
    pub backdrop: Option<Draw>,
    /// Crossfade cover after the world blit, before the independently fading HUD.
    pub world_cover: Option<Draw>,
    pub resources: Vec<Resource>,
}
impl MenuFrame {
    /// A quota failure skips this overlay for the frame, preserving the world
    /// picture and editor/session state. Invalid geometry still surfaces.
    pub fn append_optional(menu: Result<Self>, picture: &mut PreparedFrame) -> Result<bool> {
        Self::admit_quota(menu.and_then(|menu| menu.append_to(picture))).map(|v| v.is_some())
    }
    /// Preserve the retry picture and #359's single-allocation composition.
    pub fn compose_optional(
        menu: Result<Self>,
        picture: &PreparedFrame,
    ) -> Result<Option<PreparedFrame>> {
        Self::admit_quota(menu.and_then(|menu| menu.compose_over(picture)))
    }
    fn admit_quota<T>(result: Result<T>) -> Result<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(error)
                if matches!(
                    error.as_str(),
                    MENU_PICTURE_QUOTA
                        | PANEL_TOTALS_QUOTA
                        | PANEL_SLOT_QUOTA
                        | PANEL_DRAW_QUOTA
                        | HUD_SLOT_QUOTA
                        | TEXT_CACHE_RUN_QUOTA
                        | TEXT_CACHE_PIXEL_QUOTA
                        | TEXT_CACHE_QUOTA
                        | TEXT_RASTER_QUOTA
                        | frame::DRAW_MEMORY_LIMIT
                        | frame::DRAW_ALLOCATION_FAILED
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Nothing below egui: the login is hidden during the death hold and over play.
    pub fn hidden(extent: [u32; 2], scale: f64) -> Result<Self> {
        crate::render::dimensions(extent, scale)?;
        Ok(Self {
            extent,
            viewport_scale: scale,
            backdrop: None,
            world_cover: None,
            resources: Vec::new(),
        })
    }

    /// Menu hosts share one page/world composition; their controls paint above it.
    pub fn world_crossfade(extent: [u32; 2], scale: f64, world_alpha: f64) -> Result<Self> {
        crate::render::dimensions(extent, scale)?;
        if !world_alpha.is_finite() || !(0. ..=1.).contains(&world_alpha) {
            return Err("Invalid world opacity".into());
        }
        let resource = Resource {
            food_glow: false,
            food_outline: false,
            asset: BodyAsset {
                handle: PAGE_HANDLE,
                revision: 1,
                width: 1,
                height: 1,
                status: BodyAssetStatus::Ready,
            },
            image: materials::RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![PAGE_COLOR[0], PAGE_COLOR[1], PAGE_COLOR[2], 255],
            }
            .into(),
        };
        let backdrop = Draw {
            corners: None,
            repeat_uv: None,
            black_effect: false,
            asset: resource.asset,
            source: None,
            destination: BodyRect {
                x: 0.,
                y: 0.,
                width: f64::from(extent[0]) / scale,
                height: f64::from(extent[1]) / scale,
            },
            translation: [0., 0.],
            rotation: 0.,
            effective_alpha: 1.,
            blend: BodyBlend::SourceOver,
        };
        let mut cover = backdrop.clone();
        cover.effective_alpha = 1. - world_alpha;
        Ok(Self {
            extent,
            viewport_scale: scale,
            backdrop: Some(backdrop),
            world_cover: Some(cover),
            resources: vec![resource],
        })
    }

    /// Keep the retry picture intact and allocate its composed draw storage once.
    pub fn compose_over(self, picture: &PreparedFrame) -> Result<PreparedFrame> {
        let world_extra = usize::from(self.backdrop.is_some());
        let ui_extra = usize::from(self.world_cover.is_some());
        frame::check_draw_count(picture.draw_count() + world_extra + ui_extra)?;
        let copy = |frame: &frame::Frame, extra: usize| -> Result<frame::Frame> {
            let mut canonical;
            if frame.food_batch.is_some() {
                canonical = frame.canonical()?;
                frame::reserve_draws(&mut canonical.draws, extra)?;
                return Ok(canonical);
            }
            let mut draws = Vec::new();
            draws
                .try_reserve_exact(frame.draws.len() + extra)
                .map_err(|_| frame::DRAW_ALLOCATION_FAILED)?;
            draws.extend_from_slice(&frame.draws);
            Ok(frame::Frame {
                identity: frame.identity,
                extent: frame.extent,
                viewport_scale: frame.viewport_scale,
                tick: frame.tick,
                draws,
                food_batch: None,
                snakes: frame.snakes.clone(),
            })
        };
        let mut composed = PreparedFrame {
            background_draws: picture.background_draws,
            world_draws: picture.world_draws,
            #[cfg(feature = "frame-bench")]
            food_submission: picture.food_submission,
            frame: copy(
                &picture.frame,
                world_extra
                    + if picture.overlay.is_none() {
                        ui_extra
                    } else {
                        0
                    },
            )?,
            overlay: picture
                .overlay
                .as_ref()
                .map(|f| copy(f, ui_extra))
                .transpose()?,
            resources: picture.resources.clone(),
        };
        self.append_to(&mut composed)?;
        Ok(composed)
    }
    /// Put the page below every world draw and the cover above them.
    /// A failed whole-picture admission publishes nothing.
    pub fn append_to(self, picture: &mut PreparedFrame) -> Result<()> {
        let count = usize::from(self.backdrop.is_some()) + usize::from(self.world_cover.is_some());
        if self.extent != picture.ui_frame().extent
            || self.viewport_scale != picture.ui_frame().viewport_scale
        {
            return Err("Menu picture extent/scale mismatch".into());
        }
        // Keyed by handle alone: the renderer keeps one revision per handle.
        let mut resources = BTreeMap::new();
        for r in picture.resources.iter().chain(&self.resources) {
            if let Some(old) = resources.insert(r.asset.handle, r.clone())
                && old.asset != r.asset
            {
                return Err("Menu resource handle collides with the world picture".into());
            }
        }
        if resources.len() > 2048
            || resources
                .values()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                > crate::bank::PIXEL_BUDGET
        {
            return Err(MENU_PICTURE_QUOTA.into());
        }
        frame::check_draw_count(picture.draw_count() + count)?;
        // A page prefix invalidates the retained path's black-outline omission.
        // Stage expansion and all admission before changing the issued picture.
        let mut canonical = if self.backdrop.is_some() && picture.frame.food_batch.is_some() {
            Some(picture.frame.canonical()?)
        } else {
            None
        };
        let world = canonical.as_mut().unwrap_or(&mut picture.frame);
        if let Some(overlay) = &mut picture.overlay {
            frame::reserve_draws(&mut overlay.draws, usize::from(self.world_cover.is_some()))?;
            frame::reserve_draws(&mut world.draws, usize::from(self.backdrop.is_some()))?;
        } else {
            frame::reserve_draws(&mut world.draws, count)?;
        }
        let mut admitted_resources = Vec::new();
        admitted_resources
            .try_reserve_exact(resources.len())
            .map_err(|_| frame::DRAW_ALLOCATION_FAILED)?;
        admitted_resources.extend(resources.into_values());
        if let Some(canonical) = canonical {
            picture.frame = canonical;
        }
        if let Some(mut backdrop) = self.backdrop {
            // The page fills the world backing, whose scale can differ from the UI's.
            backdrop.destination = BodyRect {
                x: 0.,
                y: 0.,
                width: f64::from(picture.frame.extent[0]) / picture.frame.viewport_scale,
                height: f64::from(picture.frame.extent[1]) / picture.frame.viewport_scale,
            };
            picture.frame.draws.insert(0, backdrop);
            picture.world_draws += 1;
            picture.background_draws += 1;
        }
        if let Some(cover) = self.world_cover {
            if let Some(overlay) = &mut picture.overlay {
                overlay.draws.insert(0, cover);
            } else {
                picture.frame.draws.push(cover);
            }
        }
        picture.resources = admitted_resources;
        Ok(())
    }
}
