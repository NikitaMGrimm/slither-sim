//! Thin adapter names for the shared HUD projection and text layout.
pub use slither_client::presentation::{
    HudTextPlan as Plan, HudTextView as View, plan_hud_diagnostic as plan_text,
    plan_hud_text as plan,
};

// Includes tiled, clipped runs at every supported raster size. The actual
// submission also checks the remaining frame budget before adding each tile.
pub const MAX_DRAWS: usize = 128;
pub const MAX_PIXEL_BYTES: usize = crate::text::HUD_CACHE_BYTES;
