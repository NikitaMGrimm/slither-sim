/// Observed own-spawn radii lie on `0.009 + 0.011*n` of the arena radius.
/// The probability of each band is not identified: keep the area-weighted
/// draw and quantize its radius to the observed grid.
/// Callers supply a positive finite arena radius and a unit draw in [0, 1).
pub fn sample_spawn_distance(arena_radius: f64, unit: f64) -> f64 {
    let edge = arena_radius - 100.0;
    let continuous = unit.sqrt() * edge;
    const OFFSET: f64 = 0.009;
    const STEP: f64 = 0.011;
    // Tiny custom arenas may have no band inside the 100-unit edge margin.
    if edge < OFFSET * arena_radius {
        return 0.0;
    }
    let max_index = ((edge / arena_radius - OFFSET) / STEP).floor().max(0.0);
    let index = ((continuous / arena_radius - OFFSET) / STEP)
        .round()
        .clamp(0.0, max_index);
    (OFFSET + STEP * index) * arena_radius
}
