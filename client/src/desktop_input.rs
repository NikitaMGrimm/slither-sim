//! Caller-clocked desktop input policy. No transport, scheduler, DOM or scene.

use std::fmt;

/// JavaScript safe-integer ceiling for retained turn debts.
pub const MAX_DEBT: u64 = 9_007_199_254_740_991;

/// Boundary error category, independent of a browser error object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Invalid scalar or missing appearance data.
    Type,
    /// Invalid range or overflow.
    Range,
    /// Reentrant phase admission.
    State,
}

/// Stable input-policy error and its boundary category.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputError {
    /// Error category.
    pub kind: ErrorKind,
    /// Human-readable boundary message.
    pub message: &'static str,
}
impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for InputError {}
type Result<T> = std::result::Result<T, InputError>;
fn error(kind: ErrorKind, message: &'static str) -> InputError {
    InputError { kind, message }
}
fn finite(value: f64, message: &'static str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(error(ErrorKind::Type, message))
    }
}
fn nonnegative(value: f64, message: &'static str) -> Result<f64> {
    finite(value, message)?;
    if value < 0.0 {
        Err(error(ErrorKind::Range, message))
    } else {
        Ok(value)
    }
}
fn steps(value: f64) -> Result<u64> {
    if value.is_finite() && value.fract() == 0.0 && (0.0..=5.0).contains(&value) {
        Ok(value as u64)
    } else {
        Err(error(ErrorKind::Range, "Invalid whole steps"))
    }
}
fn protocol(value: f64) -> Result<u8> {
    if value.is_finite() && value.fract() == 0.0 && (2.0..=255.0).contains(&value) {
        Ok(value as u8)
    } else {
        Err(error(ErrorKind::Range, "Invalid protocol version"))
    }
}

/// A copied CSS displacement, preserving floating-point signs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal CSS displacement.
    pub x: f64,
    /// Vertical CSS displacement.
    pub y: f64,
}

/// Admit a native window-relative physical position as integer mouse CSS pixels.
/// The host supplies finite coordinates and its positive device scale factor.
/// Scale in Blink's f32 precision before flooring, including outside the window.
/// See the pinned sources and synthetic regressions in tools/feel-replay/browser-peers.md.
/// Browser hosts read DOM getters directly instead of converting twice.
pub fn mouse_client_position(physical: [f64; 2], device_scale: f64) -> [f64; 2] {
    let inverse_scale = 1.0_f32 / device_scale as f32;
    physical.map(|value| f64::from(value as f32 * inverse_scale).floor())
}

/// The pointer displacement and immediate gaze angle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerView {
    /// CSS displacement.
    pub point: Point,
    /// Immediate pointer angle, independent of outgoing heading dead zone.
    pub gaze_heading: f64,
}

/// Project copied pointer scalars in the established arithmetic order.
/// Layout: client x/y, viewport width/height, follow (0/1), head present (0/1),
/// camera x/y, raw head x/y, zoom. Unused correction fields are ignored.
pub fn project_pointer(values: &[f64]) -> Result<PointerView> {
    if values.len() != 11 || ![0.0, 1.0].contains(&values[4]) || ![0.0, 1.0].contains(&values[5]) {
        return Err(error(ErrorKind::Type, "Invalid pointer projection"));
    }
    let cx = finite(values[0], "Invalid pointer x")?;
    let cy = finite(values[1], "Invalid pointer y")?;
    let width = finite(values[2], "Invalid viewport width")?;
    let height = finite(values[3], "Invalid viewport height")?;
    if width <= 0.0 || height <= 0.0 {
        return Err(error(ErrorKind::Range, "Invalid viewport size"));
    }
    let mut x = cx - width / 2.0;
    let mut y = cy - height / 2.0;
    if values[4] == 0.0 && values[5] == 1.0 {
        let camera_x = finite(values[6], "Invalid camera x")?;
        let camera_y = finite(values[7], "Invalid camera y")?;
        let head_x = finite(values[8], "Invalid head x")?;
        let head_y = finite(values[9], "Invalid head y")?;
        let zoom = finite(values[10], "Invalid zoom")?;
        if zoom <= 0.0 {
            return Err(error(ErrorKind::Range, "Invalid zoom"));
        }
        x += (camera_x - head_x) * zoom;
        y += (camera_y - head_y) * zoom;
    }
    finite(x, "Invalid pointer displacement x")?;
    finite(y, "Invalid pointer displacement y")?;
    finite(x * x + y * y, "Invalid pointer squared distance")?;
    Ok(PointerView {
        point: Point { x, y },
        gaze_heading: crate::math::atan2(y, x),
    })
}

/// Appearance-owned numeric state. Identity comparison is supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnInput {
    /// Server wanted heading.
    pub wanted_heading: f64,
    /// Retained local gaze heading.
    pub gaze_heading: f64,
    /// Retained dead appearances remain input-eligible.
    pub dead: bool,
    /// Keyboard turn rate.
    pub turn_rate: f64,
    /// Keyboard size multiplier.
    pub size_turn_factor: f64,
    /// Keyboard speed multiplier.
    pub speed_turn_factor: f64,
    /// Current requested boost.
    pub requested_boost: bool,
    /// Last attempted boost value.
    pub sent_boost: bool,
}
/// Detached numeric observation of all semantic input state.
#[derive(Clone, Debug, PartialEq)]
pub struct InputView {
    /// Selected wire version.
    pub protocol: u8,
    /// Initial tick admission gate.
    pub connected: bool,
    /// Current CSS pointer displacement.
    pub pointer: Point,
    /// Pointer consumed at the last eligible heading attempt.
    pub compared_pointer: Point,
    /// Pending pointer change.
    pub want_heading: bool,
    /// Last heading gate consumption.
    pub last_heading_ms: f64,
    /// Last boost send attempt.
    pub last_boost_ms: f64,
    /// Last turn send attempt.
    pub last_turn_ms: f64,
    /// Last attempted quantized heading; includes signed zero.
    pub last_quantized_heading: f64,
    /// Held left arrow.
    pub left_held: bool,
    /// Held right arrow.
    pub right_held: bool,
    /// Held Space or up arrow.
    pub boost_key_held: bool,
    /// Retained left debt.
    pub left_steps: u64,
    /// Retained right debt.
    pub right_steps: u64,
    /// Current appearance-owned numeric fields.
    pub own: Option<OwnInput>,
}

/// Input owner with explicit phases so no host callback runs under a Rust borrow.
pub struct DesktopInput {
    view: InputView,
    ticking: bool,
}
impl DesktopInput {
    /// Create disconnected input with no own appearance.
    pub fn new(version: f64) -> Result<Self> {
        Ok(Self {
            view: InputView {
                protocol: protocol(version)?,
                connected: false,
                pointer: Point::default(),
                compared_pointer: Point::default(),
                want_heading: false,
                last_heading_ms: 0.0,
                last_boost_ms: 0.0,
                last_turn_ms: 0.0,
                last_quantized_heading: 0.0,
                left_held: false,
                right_held: false,
                boost_key_held: false,
                left_steps: 0,
                right_steps: 0,
                own: None,
            },
            ticking: false,
        })
    }
    /// Observe the current numeric state without mutating it.
    pub fn view(&self) -> &InputView {
        &self.view
    }
    /// Update the wire version, including during a callback.
    pub fn set_protocol(&mut self, value: f64) -> Result<()> {
        self.view.protocol = protocol(value)?;
        Ok(())
    }
    /// Gate future tick admission; an already entered tick continues its phases.
    pub fn set_connected(&mut self, value: bool) {
        self.view.connected = value;
    }
    /// Replace metadata, retaining gaze/boost only for an identical current token.
    pub fn set_own(&mut self, mut own: OwnInput, same_identity: bool) -> Result<()> {
        finite(own.wanted_heading, "Invalid wanted heading")?;
        finite(own.gaze_heading, "Invalid gaze heading")?;
        nonnegative(own.turn_rate, "Invalid turn rate")?;
        nonnegative(own.size_turn_factor, "Invalid size turn factor")?;
        nonnegative(own.speed_turn_factor, "Invalid speed turn factor")?;
        if let Some(previous) = self.view.own.filter(|_| same_identity) {
            own.gaze_heading = previous.gaze_heading;
            own.requested_boost = previous.requested_boost;
            own.sent_boost = previous.sent_boost;
        } else {
            own.requested_boost = false;
            own.sent_boost = false;
        }
        self.view.own = Some(own);
        Ok(())
    }
    /// Detach the current own appearance only.
    pub fn clear_own(&mut self) {
        self.view.own = None;
    }
    /// Update the CSS pointer; no-own movement ignores head correction.
    pub fn pointer(&mut self, values: &[f64]) -> Result<PointerView> {
        if values.len() != 11 {
            return Err(error(ErrorKind::Type, "Invalid pointer projection"));
        }
        if self.view.own.is_some() && values[4] == 0.0 && values[5] == 0.0 {
            return Err(error(ErrorKind::Type, "Expected raw own head"));
        }
        let mut copied = [0.0; 11];
        copied.copy_from_slice(values);
        if self.view.own.is_none() {
            copied[5] = 0.0;
        }
        let point = project_pointer(&copied)?;
        self.view.pointer = point.point;
        Ok(point)
    }
    /// Mouse press only affects a retained own appearance.
    pub fn mouse_down(&mut self, values: &[f64]) -> Result<()> {
        if self.view.own.is_some() {
            self.pointer(values)?;
            self.set_boost(true);
        }
        Ok(())
    }
    fn set_boost(&mut self, value: bool) {
        if let Some(own) = &mut self.view.own {
            own.requested_boost = value;
        }
    }
    /// Mouse release clears requested boost without changing held-key state.
    pub fn mouse_up(&mut self) {
        self.set_boost(false);
    }
    /// Update one recognized held key; unrecognized codes have no effect.
    pub fn key(&mut self, code: &str, down: bool) -> bool {
        match code {
            "ArrowLeft" => self.view.left_held = down,
            "ArrowRight" => self.view.right_held = down,
            "ArrowUp" | "Space" => {
                self.view.boost_key_held = down;
                self.set_boost(down);
            }
            _ => return false,
        }
        true
    }
    /// Focus loss deliberately invents neither release nor send.
    pub fn blur(&mut self) {}
    /// Page teardown only gates future sends.
    pub fn pagehide(&mut self) {
        self.view.connected = false;
    }
    /// Reset only connection/own, preserving global debts, keys and cadence.
    pub fn reset(&mut self) {
        self.view.connected = false;
        self.view.own = None;
    }
    /// Reject reentrant accumulation or tick entry.
    pub fn ensure_idle(&self) -> Result<()> {
        if self.ticking {
            Err(error(ErrorKind::State, "Input tick is already active"))
        } else {
            Ok(())
        }
    }
    /// Enter the tick guard before validation and callbacks.
    pub fn enter_tick(&mut self) -> Result<()> {
        self.ensure_idle()?;
        self.ticking = true;
        Ok(())
    }
    /// Release the guard in the host's finally path, retaining committed effects.
    pub fn leave_tick(&mut self) {
        self.ticking = false;
    }
    /// Validate copied tick inputs without committing a phase.
    pub fn validate_tick(&self, now_ms: f64, whole_steps: f64) -> Result<()> {
        finite(now_ms, "Invalid timestamp")?;
        steps(whole_steps)?;
        Ok(())
    }
    fn pending(&self, count: u64) -> Result<(u64, u64)> {
        let eligible = self.view.connected && self.view.own.is_some();
        let left = self.view.left_steps
            + if eligible && self.view.left_held {
                count
            } else {
                0
            };
        let right = self.view.right_steps
            + if eligible && self.view.right_held {
                count
            } else {
                0
            };
        if left > MAX_DEBT || right > MAX_DEBT {
            return Err(error(
                ErrorKind::Range,
                "Keyboard step accumulation overflow",
            ));
        }
        Ok((left, right))
    }
    /// Accumulate before session close/reset without running any send phase.
    pub fn accumulate(&mut self, whole_steps: f64) -> Result<()> {
        self.ensure_idle()?;
        let (left, right) = self.pending(steps(whole_steps)?)?;
        self.view.left_steps = left;
        self.view.right_steps = right;
        Ok(())
    }
    /// Commit the keyboard attempt and return copied bytes before host send.
    pub fn keyboard_phase(&mut self, now_ms: f64, whole_steps: f64) -> Result<Vec<u8>> {
        finite(now_ms, "Invalid timestamp")?;
        let count = steps(whole_steps)?;
        let Some(own) = self.view.own else {
            return Ok(Vec::new());
        };
        let (mut left, mut right) = self.pending(count)?;
        if (left > 0 || right > 0) && now_ms - self.view.last_turn_ms > 50.0 {
            if right > 0 && left > right {
                left -= right;
                right = 0;
            }
            if left > 0 && right > left {
                right -= left;
                left = 0;
            }
            let count = (if left > 0 { left } else { right }).min(127);
            let increasing = left == 0;
            let displacement =
                own.turn_rate * count as f64 * own.size_turn_factor * own.speed_turn_factor;
            let gaze = if increasing {
                own.gaze_heading + displacement
            } else {
                own.gaze_heading - displacement
            };
            finite(displacement, "Invalid keyboard gaze displacement")?;
            finite(gaze, "Invalid keyboard gaze")?;
            self.view.last_turn_ms = now_ms;
            self.view.left_steps = left - if increasing { 0 } else { count };
            self.view.right_steps = right - if increasing { count } else { 0 };
            self.view.own.as_mut().expect("retained own").gaze_heading = gaze;
            Ok(if self.view.protocol >= 5 {
                vec![252, (if increasing { count + 128 } else { count }) as u8]
            } else {
                vec![if increasing { 114 } else { 108 }, count as u8]
            })
        } else {
            self.view.left_steps = left;
            self.view.right_steps = right;
            Ok(Vec::new())
        }
    }
    /// Commit the boost attempt using state current after the host phase hook.
    pub fn boost_phase(&mut self, now_ms: f64) -> Result<Vec<u8>> {
        finite(now_ms, "Invalid timestamp")?;
        let Some(own) = &mut self.view.own else {
            return Ok(Vec::new());
        };
        if own.sent_boost != own.requested_boost && now_ms - self.view.last_boost_ms > 50.0 {
            own.sent_boost = own.requested_boost;
            self.view.last_boost_ms = now_ms;
            Ok(if self.view.protocol >= 5 {
                vec![if own.sent_boost { 253 } else { 254 }]
            } else {
                vec![109, u8::from(own.sent_boost)]
            })
        } else {
            Ok(Vec::new())
        }
    }
    /// Commit pointer gaze, cadence and quantization before any heading send.
    pub fn heading_phase(&mut self, now_ms: f64) -> Result<Vec<u8>> {
        finite(now_ms, "Invalid timestamp")?;
        let p = self.view.pointer;
        if p.x != self.view.compared_pointer.x || p.y != self.view.compared_pointer.y {
            self.view.want_heading = true;
        }
        let own = self.view.own.as_mut().ok_or_else(|| {
            error(
                ErrorKind::Type,
                "Cannot set properties of null (setting 'gazeHeading')",
            )
        })?;
        own.gaze_heading = crate::math::atan2(p.y, p.x);
        if self.view.want_heading && now_ms - self.view.last_heading_ms > 33.0 {
            self.view.want_heading = false;
            self.view.last_heading_ms = now_ms;
            self.view.compared_pointer = p;
            let mut angle = if p.x * p.x + p.y * p.y > 256.0 {
                crate::math::atan2(p.y, p.x)
            } else {
                own.wanted_heading
            };
            angle %= std::f64::consts::TAU;
            if angle < 0.0 {
                angle += std::f64::consts::TAU;
            }
            let quantized = ((if self.view.protocol >= 5 {
                251.0
            } else {
                16_777_215.0
            }) * angle
                / std::f64::consts::TAU)
                .floor();
            if quantized != self.view.last_quantized_heading {
                self.view.last_quantized_heading = quantized;
                let q = quantized as u32;
                return Ok(if self.view.protocol >= 5 {
                    vec![q as u8]
                } else {
                    vec![101, (q >> 16) as u8, (q >> 8) as u8, q as u8]
                });
            }
        }
        Ok(Vec::new())
    }
    /// Observe gaze after callbacks; clearing own during a heading send still fails.
    pub fn required_gaze(&self) -> Result<f64> {
        self.view.own.map(|own| own.gaze_heading).ok_or_else(|| {
            error(
                ErrorKind::Type,
                "Cannot read properties of null (reading 'gazeHeading')",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_coordinates_retain_blink_float_precision() {
        assert_eq!(mouse_client_position([-21.0, 21.0], 1.75), [-13.0, 12.0]);
        assert_eq!(
            mouse_client_position([0.999999999, -1.000000001], 1.0),
            [1.0, -1.0]
        );
        assert_eq!(
            mouse_client_position([5.0, -5.0], 1.2500000001),
            [4.0, -4.0]
        );
    }

    #[test]
    fn mouse_coordinates_match_authored_blink_float_sweep() {
        // Authored synthetic golden floors for physical positions -28..=28 in
        // steps of 7. Both axes traverse the same sweep in opposite directions.
        for (dpr, expected) in [
            (1.0, [-28, -21, -14, -7, 0, 7, 14, 21, 28]),
            (1.25, [-23, -17, -12, -6, 0, 5, 11, 16, 22]),
            (1.5, [-19, -14, -10, -5, 0, 4, 9, 14, 18]),
            (1.75, [-16, -13, -8, -4, 0, 4, 8, 12, 16]),
            (2.0, [-14, -11, -7, -4, 0, 3, 7, 10, 14]),
        ] {
            for (index, x) in (-28..=28).step_by(7).enumerate() {
                assert_eq!(
                    mouse_client_position([f64::from(x), f64::from(-x)], dpr),
                    [f64::from(expected[index]), f64::from(expected[8 - index])],
                    "DPR {dpr}, physical x {x}",
                );
            }
        }
    }

    #[test]
    fn mouse_coordinates_scale_before_flooring_without_clamping() {
        // Authored synthetic physical positions, expected CSS positions. Cover
        // negative fractions, exact edges and the adjacent interior/exterior.
        for (dpr, physical, expected) in [
            (1.0, [0.75, -0.25], [0.0, -1.0]),
            (1.0, [959.75, 539.75], [959.0, 539.0]),
            (1.0, [960.0, 540.0], [960.0, 540.0]),
            (1.0, [960.25, -1.25], [960.0, -2.0]),
            (1.25, [1.0, -1.0], [0.0, -1.0]),
            (1.25, [1199.75, 674.75], [959.0, 539.0]),
            (1.25, [1200.0, 675.0], [960.0, 540.0]),
            (1.25, [1200.25, -1.5], [960.0, -2.0]),
            (2.0, [1.75, -0.25], [0.0, -1.0]),
            (2.0, [1919.75, 1079.75], [959.0, 539.0]),
            (2.0, [1920.0, 1080.0], [960.0, 540.0]),
            (2.0, [1920.25, -2.25], [960.0, -2.0]),
        ] {
            assert_eq!(mouse_client_position(physical, dpr), expected, "DPR {dpr}");
        }
    }

    #[test]
    fn safe_integer_accumulation_boundary_is_atomic() {
        let mut input = DesktopInput::new(19.0).unwrap();
        input.view.connected = true;
        input.view.left_held = true;
        input.view.own = Some(OwnInput {
            wanted_heading: 0.0,
            gaze_heading: 0.0,
            dead: false,
            turn_rate: 0.03,
            size_turn_factor: 0.5,
            speed_turn_factor: 0.8,
            requested_boost: false,
            sent_boost: false,
        });
        input.view.left_steps = MAX_DEBT - 1;
        input.accumulate(1.0).unwrap();
        let before = input.view.clone();
        assert!(input.accumulate(1.0).is_err());
        assert_eq!(input.view, before);
        assert!(input.keyboard_phase(0.0, 1.0).is_err());
        assert_eq!(input.view, before);
    }
}
