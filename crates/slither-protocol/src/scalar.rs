//! Client expressions are intentionally not algebraically normalized. Raw wire
//! integers remain available on the views; these helpers only convert units.
use core::f64::consts::{PI, TAU};
pub fn angle24(raw: u32) -> f64 {
    f64::from(raw) * TAU / 16_777_215.
}
pub fn prey_angle24(raw: u32) -> f64 {
    f64::from(raw) * 2. * PI / 16_777_215.
}
pub fn angle8(raw: u8) -> f64 {
    f64::from(raw) * TAU / 256.
}
/// Appearance's effective final-body direction uses this parenthesization.
pub fn body_angle16(raw: u16) -> f64 {
    f64::from(raw) * (TAU / 65_536.)
}
/// Anchor reconstruction in state.rs uses this distinct operation order.
pub fn anchor_angle16(raw: u16) -> f64 {
    f64::from(raw) * TAU / 65_536.
}
pub fn fraction24(raw: u32) -> f64 {
    f64::from(raw) / 16_777_215.
}
pub fn position5(raw: u32) -> f64 {
    f64::from(raw) / 5.
}
pub fn delta_half(raw: u8) -> f64 {
    (f64::from(raw) - 127.) / 2.
}
pub fn compact_speed_per_32ms(raw: u8) -> f64 {
    f64::from(raw) / 18.
}
pub fn compact_speed(raw: u8) -> f64 {
    compact_speed_per_32ms(raw) / 0.032
}
pub fn full_speed_per_32ms(raw: u16) -> f64 {
    f64::from(raw) / 1000.
}
pub fn full_speed(raw: u16) -> f64 {
    full_speed_per_32ms(raw) / 0.032
}
pub fn prey_position(raw: u16) -> f64 {
    1. + f64::from(raw) * 3.
}
pub fn team_score(bytes: [u8; 4]) -> i32 {
    i32::from_be_bytes(bytes)
}
