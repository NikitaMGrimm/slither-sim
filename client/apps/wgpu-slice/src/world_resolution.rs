//! Attachment policy only. Never changes Frame extent, projection or input.
use crate::Result;
use slither_client::settings::WorldResolution;
pub use slither_client::world_resolution::{ATTACHMENT_BYTES, WorldTargetPlan, validate_target};

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
