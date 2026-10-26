//! `slither-sim-abi-v3`: a C ABI over [`slither_sim::transport::Session`].
//!
//! `include/slither_sim.h` declares the API, and `tests/abi.rs` checks it
//! against this crate. A panicking call returns [`SLITHER_PANICKED`] instead of unwinding;
//! its session then accepts only [`slither_last_error`], [`slither_read_error`] and
//! [`slither_session_free`].
//!
//! A session must not be used by two threads at once; separate sessions may
//! run on separate threads.
use slither_sim::transport::{FrameRef, Session, StepError};
use slither_sim::{BoundaryMode, Constraint, SimError, WorldRules};
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{ptr, slice};

/// Increments with every incompatible change of this interface.
pub const SLITHER_ABI_VERSION: u32 = 3;

pub const SLITHER_OK: i32 = 0;
/// The session rejected the call and changed nothing.
pub const SLITHER_REJECTED: i32 = 1;
/// A required pointer was null; nothing changed.
pub const SLITHER_INVALID: i32 = 2;
/// An internal error; only [`slither_last_error`], [`slither_read_error`] and [`slither_session_free`]
/// remain valid.
pub const SLITHER_PANICKED: i32 = 3;
/// Advancement failed and cleared all worlds; reset to continue.
pub const SLITHER_ENDED: i32 = 4;

/// An opaque session handle.
pub struct SlitherSession {
    session: Session,
    /// The latest reply, followed by a NUL.
    reply: Vec<u8>,
    boosts: Vec<bool>,
    error: CString,
    error_field: CString,
    cause: Option<SimError>,
    boundary_code: u32,
    poisoned: bool,
}

pub const SLITHER_BOUNDARY_SLIDING: u32 = 0;
pub const SLITHER_BOUNDARY_HARD: u32 = 1;
pub const SLITHER_RULE_HAS_PIT: u32 = 1;
pub const SLITHER_RULE_HAS_MIN_SCORE: u32 = 2;
pub const SLITHER_RULE_HAS_KILL_WINDOW: u32 = 4;
pub const SLITHER_RULE_FREE_BOOST_AT_FLOOR: u32 = 8;
pub const SLITHER_ERROR_HAS_WORLD: u32 = 1;
pub const SLITHER_ERROR_HAS_SLOT: u32 = 2;
pub const SLITHER_CAUSE_INVALID_POINTER: u32 = 18;
pub const SLITHER_CAUSE_PANIC: u32 = 19;

/// Inline, fixed-size geometric rules. Absent numeric members are zero.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct SlitherWorldRules {
    pub boundary_mode: u32,
    pub flags: u32,
    pub pit_radius: f64,
    pub min_score: u32,
    pub kill_score_seconds: f64,
}
impl From<WorldRules> for SlitherWorldRules {
    fn from(r: WorldRules) -> Self {
        Self {
            boundary_mode: match r.boundary_mode {
                BoundaryMode::Sliding => SLITHER_BOUNDARY_SLIDING,
                BoundaryMode::Hard => SLITHER_BOUNDARY_HARD,
            },
            flags: (u32::from(r.pit_radius.is_some()) * SLITHER_RULE_HAS_PIT)
                | (u32::from(r.min_score.is_some()) * SLITHER_RULE_HAS_MIN_SCORE)
                | (u32::from(r.kill_score_seconds.is_some()) * SLITHER_RULE_HAS_KILL_WINDOW)
                | (u32::from(r.free_boost_at_floor) * SLITHER_RULE_FREE_BOOST_AT_FLOOR),
            pit_radius: r.pit_radius.unwrap_or(0.),
            min_score: r.min_score.unwrap_or(0),
            kill_score_seconds: r.kill_score_seconds.unwrap_or(0.),
        }
    }
}

/// Error inspection; flags preserve full-range locations; zero rule/null field mean absent.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SlitherError {
    pub code: u32,
    pub rule: u32,
    pub flags: u32,
    pub world: usize,
    pub slot: usize,
    pub field: *const c_char,
    pub message: *const c_char,
}

/// One world's current frame; see `slither_frame` in the header. Pointers of
/// empty columns are dangling but non-null.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SlitherFrame {
    pub time: f64,
    pub radius: f64,
    pub vision: f64,
    pub ticks: u32,
    pub living: u32,
    pub slots: usize,
    pub snakes: usize,
    pub body: usize,
    pub food: usize,
    pub prey: usize,
    pub deaths: usize,
    pub snake_slot: *const u32,
    pub snake_x: *const f64,
    pub snake_y: *const f64,
    pub snake_heading: *const f64,
    pub snake_speed: *const f64,
    pub snake_radius: *const f64,
    pub snake_length: *const f64,
    pub snake_segments: *const u32,
    pub snake_fam: *const f64,
    pub snake_score: *const f64,
    pub snake_kills: *const u32,
    pub snake_kill_score: *const u32,
    pub snake_body_start: *const u32,
    pub snake_body_x: *const f64,
    pub snake_body_y: *const f64,
    pub food_x: *const f64,
    pub food_y: *const f64,
    pub food_value: *const f64,
    pub food_size: *const f64,
    pub food_origin: *const u8,
    pub prey_id: *const u64,
    pub prey_x: *const f64,
    pub prey_y: *const f64,
    pub prey_heading: *const f64,
    pub prey_target_heading: *const f64,
    pub prey_speed: *const f64,
    pub prey_size: *const f64,
    pub view_self: *const u32,
    pub view_snakes_start: *const u32,
    pub view_snakes: *const u32,
    pub view_food_start: *const u32,
    pub view_food: *const u32,
    pub view_prey_start: *const u32,
    pub view_prey: *const u32,
    pub death_time: *const f64,
    pub death_victim: *const u32,
    pub death_killer: *const u32,
    pub death_reason: *const u8,
    pub death_food: *const u32,
    pub rules: SlitherWorldRules,
}

impl From<FrameRef<'_>> for SlitherFrame {
    fn from(f: FrameRef<'_>) -> Self {
        let (s, food, p, v, d) = (f.snakes, f.food, f.prey, f.views, f.deaths);
        SlitherFrame {
            time: f.time,
            radius: f.radius,
            vision: f.vision,
            ticks: f.ticks,
            living: f.living,
            slots: v.me.len(),
            snakes: s.slot.len(),
            body: s.body_x.len(),
            food: food.x.len(),
            prey: p.id.len(),
            deaths: d.time.len(),
            snake_slot: s.slot.as_ptr(),
            snake_x: s.x.as_ptr(),
            snake_y: s.y.as_ptr(),
            snake_heading: s.heading.as_ptr(),
            snake_speed: s.speed.as_ptr(),
            snake_radius: s.radius.as_ptr(),
            snake_length: s.length.as_ptr(),
            snake_segments: s.segments.as_ptr(),
            snake_fam: s.fam.as_ptr(),
            snake_score: s.score.as_ptr(),
            snake_kills: s.kills.as_ptr(),
            snake_kill_score: s.kill_score.as_ptr(),
            snake_body_start: s.body_start.as_ptr(),
            snake_body_x: s.body_x.as_ptr(),
            snake_body_y: s.body_y.as_ptr(),
            food_x: food.x.as_ptr(),
            food_y: food.y.as_ptr(),
            food_value: food.value.as_ptr(),
            food_size: food.size.as_ptr(),
            food_origin: food.origin.as_ptr(),
            prey_id: p.id.as_ptr(),
            prey_x: p.x.as_ptr(),
            prey_y: p.y.as_ptr(),
            prey_heading: p.heading.as_ptr(),
            prey_target_heading: p.target_heading.as_ptr(),
            prey_speed: p.speed.as_ptr(),
            prey_size: p.size.as_ptr(),
            view_self: v.me.as_ptr(),
            view_snakes_start: v.snakes_start.as_ptr(),
            view_snakes: v.snakes.as_ptr(),
            view_food_start: v.food_start.as_ptr(),
            view_food: v.food.as_ptr(),
            view_prey_start: v.prey_start.as_ptr(),
            view_prey: v.prey.as_ptr(),
            death_time: d.time.as_ptr(),
            death_victim: d.victim.as_ptr(),
            death_killer: d.killer.as_ptr(),
            death_reason: d.reason.as_ptr(),
            death_food: d.food.as_ptr(),
            rules: f.rules.into(),
        }
    }
}

struct Failure {
    status: i32,
    message: String,
    cause: Option<Box<SimError>>,
}
fn invalid(what: &str) -> Failure {
    Failure {
        status: SLITHER_INVALID,
        message: format!("{what} is null"),
        cause: None,
    }
}
fn rejected(cause: SimError) -> Failure {
    Failure {
        status: SLITHER_REJECTED,
        message: cause.to_string(),
        cause: Some(Box::new(cause)),
    }
}
impl SlitherSession {
    fn clear_error(&mut self) {
        if !self.error.is_empty() {
            self.error = CString::default();
        }
        if !self.error_field.is_empty() {
            self.error_field = CString::default();
        }
        self.cause = None;
        self.boundary_code = 0;
    }
    fn record_error(&mut self, message: String, cause: Option<SimError>, boundary_code: u32) {
        self.error = CString::new(message.replace('\0', " ")).expect("NULs replaced");
        if let Some(field) = cause.as_ref().and_then(|e| e.field.as_deref()) {
            self.error_field = CString::new(field.replace('\0', " ")).expect("NULs replaced");
        }
        self.cause = cause;
        self.boundary_code = boundary_code;
    }
}

/// `len` elements at `data`, which may be null only when `len` is 0.
///
/// # Safety
/// A non-null `data` must point to `len` initialized elements that outlive `'a`.
unsafe fn elements<'a, T>(data: *const T, len: usize, what: &str) -> Result<&'a [T], Failure> {
    match (data.is_null(), len) {
        (true, 0) => Ok(&[]),
        (true, _) => Err(invalid(what)),
        // SAFETY: the caller's contract.
        (false, _) => Ok(unsafe { slice::from_raw_parts(data, len) }),
    }
}

/// Runs `call` on `session`, recording its error and catching any panic.
///
/// # Safety
/// `session` is null or a live pointer from [`slither_session_new`].
unsafe fn with_session(
    session: *mut SlitherSession,
    call: impl FnOnce(&mut SlitherSession) -> Result<(), Failure>,
) -> i32 {
    // SAFETY: the caller's contract; the header forbids concurrent use.
    let Some(s) = (unsafe { session.as_mut() }) else {
        return SLITHER_INVALID;
    };
    if s.poisoned {
        return SLITHER_PANICKED;
    }
    s.clear_error();
    match catch_unwind(AssertUnwindSafe(|| call(s))) {
        Ok(Ok(())) => SLITHER_OK,
        Ok(Err(failure)) => {
            let boundary = if failure.status == SLITHER_INVALID {
                SLITHER_CAUSE_INVALID_POINTER
            } else {
                0
            };
            s.record_error(failure.message, failure.cause.map(|cause| *cause), boundary);
            failure.status
        }
        Err(_) => {
            s.poisoned = true;
            s.record_error(
                "Internal panic; free this session".into(),
                None,
                SLITHER_CAUSE_PANIC,
            );
            SLITHER_PANICKED
        }
    }
}

/// [`SLITHER_ABI_VERSION`] of this library.
#[unsafe(no_mangle)]
pub extern "C" fn slither_abi_version() -> u32 {
    SLITHER_ABI_VERSION
}

/// A new empty session, or null if it cannot be created. Free it with
/// [`slither_session_free`].
#[unsafe(no_mangle)]
pub extern "C" fn slither_session_new() -> *mut SlitherSession {
    catch_unwind(|| {
        Box::into_raw(Box::new(SlitherSession {
            session: Session::new(),
            reply: Vec::new(),
            boosts: Vec::new(),
            error: CString::default(),
            error_field: CString::default(),
            cause: None,
            boundary_code: 0,
            poisoned: false,
        }))
    })
    .unwrap_or(ptr::null_mut())
}

/// Frees `session`; null is ignored.
///
/// # Safety
/// `session` is null or a live pointer from [`slither_session_new`], which this
/// call invalidates.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_session_free(session: *mut SlitherSession) {
    if !session.is_null() {
        // SAFETY: the caller's contract. A panicking drop leaks the rest.
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(session) })));
    }
}

/// Answers one JSON request line without its newline, as `serve` would.
///
/// # Safety
/// `session` is null or live; `line` is null with `line_len` 0 or points to
/// `line_len` bytes; `reply` and `reply_len` are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_request(
    session: *mut SlitherSession,
    line: *const u8,
    line_len: usize,
    reply: *mut *const u8,
    reply_len: *mut usize,
) -> i32 {
    unsafe {
        with_session(session, |s| {
            if reply.is_null() {
                return Err(invalid("reply"));
            }
            if reply_len.is_null() {
                return Err(invalid("reply_len"));
            }
            // SAFETY: both are non-null and writable by the caller's contract.
            reply.write(ptr::null());
            reply_len.write(0);
            let line = elements(line, line_len, "line")?;
            let (reply_bytes, cause) = s.session.request_with_cause(line);
            s.reply = reply_bytes;
            if let Some(error) = cause {
                s.record_error(error.to_string(), Some(error.into_cause()), 0);
            }
            let len = s.reply.len();
            s.reply.push(0);
            reply.write(s.reply.as_ptr());
            reply_len.write(len);
            Ok(())
        })
    }
}

/// Steps every world from world-major headings and boosts; see the header.
///
/// # Safety
/// `session` is null or live; each array is null with length 0 or points to
/// that many elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_step(
    session: *mut SlitherSession,
    headings: *const f64,
    headings_len: usize,
    boosts: *const u8,
    boosts_len: usize,
) -> i32 {
    unsafe {
        with_session(session, |s| {
            let headings = elements(headings, headings_len, "headings")?;
            let boosts = elements(boosts, boosts_len, "boosts")?;
            s.boosts.clear();
            s.boosts.extend(boosts.iter().map(|&boost| boost != 0));
            s.session.step(headings, &s.boosts).map_err(|error| {
                let status = match &error {
                    StepError::Rejected(_) => SLITHER_REJECTED,
                    StepError::Ended(_) => SLITHER_ENDED,
                };
                Failure {
                    status,
                    message: error.to_string(),
                    cause: Some(Box::new(error.into_cause())),
                }
            })
        })
    }
}

/// Fills `*frame` with world `world`'s current frame.
///
/// # Safety
/// `session` is null or live; `frame` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_read_frame(
    session: *mut SlitherSession,
    world: usize,
    frame: *mut SlitherFrame,
) -> i32 {
    unsafe {
        with_session(session, |s| {
            if frame.is_null() {
                return Err(invalid("frame"));
            }
            let columns = s.session.frame_checked(world).map_err(rejected)?;
            // SAFETY: non-null and writable by the caller's contract.
            frame.write(columns.into());
            Ok(())
        })
    }
}

/// Worlds of the current session; 0 before reset or for a null session.
///
/// # Safety
/// `session` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_worlds(session: *const SlitherSession) -> usize {
    // SAFETY: the caller's contract.
    unsafe { session.as_ref() }.map_or(0, |s| s.session.worlds())
}

/// Agent slots per world; 0 before reset or for a null session.
///
/// # Safety
/// `session` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_agents(session: *const SlitherSession) -> usize {
    // SAFETY: the caller's contract.
    unsafe { session.as_ref() }.map_or(0, |s| s.session.agents())
}

/// Why the latest request, step or frame read failed, or "" if it succeeded.
///
/// # Safety
/// `session` is null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_last_error(session: *const SlitherSession) -> *const c_char {
    const NULL_SESSION: &CStr = c"session is null";
    // SAFETY: the caller's contract.
    match unsafe { session.as_ref() } {
        Some(s) if s.poisoned => c"Internal panic; free this session".as_ptr(),
        Some(s) => s.error.as_ptr(),
        None => NULL_SESSION.as_ptr(),
    }
}

/// Inspect the latest request/step/frame cause without clearing it.
/// JSON requests retain SLITHER_OK status; their embedded errors are inspectable here.
///
/// # Safety
/// `session` is null or live; `error` is null or writable. Strings obey frame lifetimes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn slither_read_error(
    session: *const SlitherSession,
    error: *mut SlitherError,
) -> i32 {
    if error.is_null() {
        return SLITHER_INVALID;
    }
    let Some(s) = (unsafe { session.as_ref() }) else {
        unsafe {
            error.write(SlitherError {
                code: SLITHER_CAUSE_INVALID_POINTER,
                rule: 0,
                flags: 0,
                world: 0,
                slot: 0,
                field: c"session".as_ptr(),
                message: c"session is null".as_ptr(),
            });
        }
        return SLITHER_INVALID;
    };
    let cause = s.cause.as_ref();
    let value = SlitherError {
        code: cause.map_or(s.boundary_code, |e| e.code() as u32),
        rule: cause.and_then(SimError::rule).map_or(0, Constraint::code),
        flags: (u32::from(cause.is_some_and(|e| e.world.is_some())) * SLITHER_ERROR_HAS_WORLD)
            | (u32::from(cause.is_some_and(|e| e.slot.is_some())) * SLITHER_ERROR_HAS_SLOT),
        world: cause.and_then(|e| e.world).unwrap_or(0),
        slot: cause.and_then(|e| e.slot).unwrap_or(0),
        field: if cause.is_some_and(|e| e.field.is_some()) {
            s.error_field.as_ptr()
        } else {
            ptr::null()
        },
        message: s.error.as_ptr(),
    };
    unsafe {
        error.write(value);
    }
    SLITHER_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_calls_reuse_the_empty_error() {
        let session = slither_session_new();
        let mut frame = std::mem::MaybeUninit::<SlitherFrame>::uninit();
        let code = unsafe { slither_read_frame(session, 0, frame.as_mut_ptr()) };
        assert_eq!(code, SLITHER_REJECTED);
        let request = br#"{"op":"reset","agents":2}"#;
        let (mut reply, mut reply_len) = (ptr::null(), 0);
        let code = unsafe {
            slither_request(
                session,
                request.as_ptr(),
                request.len(),
                &mut reply,
                &mut reply_len,
            )
        };
        assert_eq!(code, SLITHER_OK);
        let empty = unsafe { slither_last_error(session) };
        assert_eq!(unsafe { CStr::from_ptr(empty) }, c"");
        for _ in 0..3 {
            let code = unsafe { slither_read_frame(session, 0, frame.as_mut_ptr()) };
            assert_eq!(code, SLITHER_OK);
            assert_eq!(unsafe { slither_last_error(session) }, empty);
        }
        unsafe { slither_session_free(session) };
    }

    #[test]
    fn panics_poison_the_session_instead_of_unwinding() {
        let session = slither_session_new();
        let code = unsafe { with_session(session, |_| panic!("deliberate test panic")) };
        assert_eq!(code, SLITHER_PANICKED);
        let error = unsafe { CStr::from_ptr(slither_last_error(session)) };
        assert_eq!(error, c"Internal panic; free this session");
        let mut frame = std::mem::MaybeUninit::<SlitherFrame>::uninit();
        let code = unsafe { slither_read_frame(session, 0, frame.as_mut_ptr()) };
        assert_eq!(code, SLITHER_PANICKED);
        unsafe { slither_session_free(session) };
    }
}
