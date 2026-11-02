# Changelog

## Unreleased

- Reduce community join/refresh hitches by staging only new food in sector
  listings and applying sector clears/notices without whole-world copies.

- Reduce crowded GPU work by skipping exactly invisible additive pairs and
  initial black shadows, grouping paired glow sources, and keeping world
  drawing and glow composition in one MSAA pass without changing source rasters.

- Reduce CPU work for paired pellet draws by reusing identical validated quad
  projections within a picture, preserving vertex bits, alpha and draw order.

- Let settled native menus sleep between input, result updates and egui repaint
  deadlines, while preserving menu animations, active gameplay and surface retries.

- Reduce crowded-frame GPU bandwidth with single-sample padded food outlines,
  smaller glow targets, preserved mip chains and combined additive draw pairs;
  keep sparse food on the ordinary path when extra passes cannot pay for themselves.

- Smooth pellet rims and zoom-out glows with denser food body rasters and
  filtered food mip levels, preserving logical sizes and draw planning.

- Reduce cold sprite preparation by reusing exact geometric coverage across
  neighbouring shadow blur taps, preserving raster pixels and accumulation order.

- Reduce crowded native frame preparation by reusing exact minimap blend
  arithmetic during crossfades and avoiding read-only control-probe snapshots.

- Discover official servers and measure latency at native startup by default;
  Automatic Play uses the best-ping selection without a directory-loading button.
  Remove the redundant official opt-in flag. Direct and headless server launches
  continue to bypass official discovery. Closing the app cancels acquisition
  without waiting for a stalled network request.
  Returning from Custom to Automatic restarts cancelled startup acquisition.
- Fix sliding body shades and stripes on rivals under presentation pressure by
  preserving head-relative pattern indices across compact motion and detail quotas.
- Return quietly to native login when the server disconnects after own death or
  game-over, retaining deferred terminal packets; show complete connection errors
  between the logo and nickname field without hover or covering login controls.
