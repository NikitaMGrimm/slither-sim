# Changelog

## Unreleased

- Smooth pellet rims and zoom-out glows with denser food body rasters and
  filtered food mip levels, preserving logical sizes and draw planning.

- Reduce crowded native frame preparation by reusing exact minimap blend
  arithmetic during crossfades and avoiding read-only control-probe snapshots.

- Fix sliding body shades and stripes on rivals under presentation pressure by
  preserving head-relative pattern indices across compact motion and detail quotas.
- Return quietly to native login when the server disconnects after own death or
  game-over, retaining deferred terminal packets; show complete connection errors
  between the logo and nickname field without hover or covering login controls.
