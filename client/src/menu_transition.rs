//! Shared native-modal timing; owners retain their own open and activation gates.

pub(crate) fn step(
    alpha: &mut f64,
    previous: &mut Option<f64>,
    open: bool,
    now: f64,
) -> Result<(), &'static str> {
    if !now.is_finite() || previous.is_some_and(|p| now < p) {
        return Err("Invalid modal clock");
    }
    let delta = previous.map_or(0., |p| ((now - p) / 8.).min(5.));
    *previous = Some(now);
    *alpha = (*alpha + if open { 0.02 * delta } else { -0.025 * delta }).clamp(0., 1.);
    Ok(())
}

pub(crate) fn scale(alpha: f64) -> f64 {
    0.9 + 0.1 * alpha.powf(0.4)
}
