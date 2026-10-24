//! `slither-sim-abi-v1`: a C ABI over [`slither_sim::transport::Session`].
//!
//! `include/slither_sim.h` declares these functions and [`SlitherFrame`];
//! `tests/abi.rs` checks that the two agree. No panic unwinds across the
//! boundary: a panicking call returns [`SLITHER_PANICKED`] and leaves its
//! session usable only for [`slither_session_free`].
//!
//! A session must not be used by two threads at once; separate sessions may
//! run on separate threads.
use slither_sim::transport::{FrameRef, Session};
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::{ptr, slice};

/// Increments with every incompatible change of this interface.
pub const SLITHER_ABI_VERSION: u32 = 1;

pub const SLITHER_OK: i32 = 0;
/// The session rejected the call and changed nothing.
pub const SLITHER_REJECTED: i32 = 1;
/// A required pointer was null; nothing changed.
pub const SLITHER_INVALID: i32 = 2;
/// An internal error; only [`slither_session_free`] remains valid.
pub const SLITHER_PANICKED: i32 = 3;

/// An opaque session handle.
pub struct SlitherSession {
    session: Session,
    /// The latest reply, followed by a NUL.
    reply: Vec<u8>,
    boosts: Vec<bool>,
    error: CString,
    poisoned: bool,
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
        }
    }
}

type Failure = (i32, String);

fn invalid(what: &str) -> Failure {
    (SLITHER_INVALID, format!("{what} is null"))
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
    let (code, message) = match catch_unwind(AssertUnwindSafe(|| call(s))) {
        Ok(Ok(())) => (SLITHER_OK, String::new()),
        Ok(Err(failure)) => failure,
        Err(_) => {
            s.poisoned = true;
            (SLITHER_PANICKED, "Internal panic; free this session".into())
        }
    };
    s.error = CString::new(message.replace('\0', " ")).expect("NULs were replaced");
    code
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
            s.reply = s.session.request(line);
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
            s.session
                .step(headings, &s.boosts)
                .map_err(|error| (SLITHER_REJECTED, error))
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
            let columns = s.session.frame(world).ok_or_else(|| {
                let error = if s.session.worlds() == 0 {
                    "Reset first"
                } else {
                    "Unknown world"
                };
                (SLITHER_REJECTED, error.to_owned())
            })?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
