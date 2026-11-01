# Changelog

## Unreleased

- Smooth pellet rims and zoom-out glows with denser food body rasters and
  filtered food mip levels, preserving logical sizes and draw planning.

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
