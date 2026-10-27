//! Calls the exported functions as a C caller would and checks them against
//! `include/slither_sim.h` and an in-process `Session`.
use slither_sim::transport::{FrameRef, NO_ORIGIN, NONE, ORIGINS, REASONS, Session};
use slither_sim_ffi::*;
use std::collections::HashMap;
use std::ffi::CStr;
use std::mem::{align_of, offset_of, size_of};
use std::ptr;

const HEADER: &str = include_str!("../include/slither_sim.h");

fn without_comments(mut text: &str) -> String {
    let mut out = String::new();
    while let Some(start) = text.find("/*") {
        out.push_str(&text[..start]);
        text = &text[start + text[start..].find("*/").expect("closed comment") + 2..];
    }
    out + text
}

/// `(name, C type)` of each `slither_frame` member, in declaration order.
fn header_fields() -> Vec<(String, String)> {
    let start = HEADER.find("typedef struct slither_frame {").unwrap();
    let body = &HEADER[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("} slither_frame;").unwrap()];
    without_comments(body)
        .split(';')
        .map(str::trim)
        .filter(|declaration| !declaration.is_empty())
        .map(|declaration| {
            let split = declaration.rfind([' ', '*']).unwrap() + 1;
            let name = declaration[split..].to_owned();
            let ty = declaration[..split].split_whitespace().collect::<Vec<_>>();
            (name, ty.join(" "))
        })
        .collect()
}

macro_rules! rust_fields {
    ($($field:ident),* $(,)?) => {{
        // SAFETY: every member is a number or a raw pointer.
        let frame: SlitherFrame = unsafe { std::mem::zeroed() };
        vec![$((
            stringify!($field),
            offset_of!(SlitherFrame, $field),
            std::any::type_name_of_val(&frame.$field),
        )),*]
    }};
}

#[test]
fn frame_layout_matches_the_header() {
    let rust = rust_fields!(
        time,
        radius,
        vision,
        ticks,
        living,
        slots,
        snakes,
        body,
        food,
        prey,
        deaths,
        snake_slot,
        snake_x,
        snake_y,
        snake_heading,
        snake_speed,
        snake_radius,
        snake_length,
        snake_segments,
        snake_fam,
        snake_score,
        snake_kills,
        snake_kill_score,
        snake_body_start,
        snake_body_x,
        snake_body_y,
        food_x,
        food_y,
        food_value,
        food_size,
        food_origin,
        prey_id,
        prey_x,
        prey_y,
        prey_heading,
        prey_target_heading,
        prey_speed,
        prey_size,
        view_self,
        view_snakes_start,
        view_snakes,
        view_food_start,
        view_food,
        view_prey_start,
        view_prey,
        death_time,
        death_victim,
        death_killer,
        death_reason,
        death_food,
        rules,
    );
    let types: HashMap<&str, (&str, usize, usize)> = [
        ("double", ("f64", size_of::<f64>(), align_of::<f64>())),
        ("uint32_t", ("u32", 4, align_of::<u32>())),
        ("size_t", ("usize", size_of::<usize>(), align_of::<usize>())),
        (
            "slither_world_rules",
            (
                std::any::type_name::<SlitherWorldRules>(),
                size_of::<SlitherWorldRules>(),
                align_of::<SlitherWorldRules>(),
            ),
        ),
        (
            "const double *",
            ("*const f64", size_of::<usize>(), align_of::<usize>()),
        ),
        (
            "const uint32_t *",
            ("*const u32", size_of::<usize>(), align_of::<usize>()),
        ),
        (
            "const uint64_t *",
            ("*const u64", size_of::<usize>(), align_of::<usize>()),
        ),
        (
            "const uint8_t *",
            ("*const u8", size_of::<usize>(), align_of::<usize>()),
        ),
    ]
    .into();
    let header = header_fields();
    assert_eq!(header.len(), rust.len());
    // Lay the header's members out by C rules and compare with the Rust struct.
    let (mut offset, mut alignment) = (0usize, 1usize);
    for ((name, c_type), (rust_name, rust_offset, rust_type)) in header.iter().zip(&rust) {
        let &(expected_type, size, align) = types.get(c_type.as_str()).expect(c_type);
        offset = offset.next_multiple_of(align);
        alignment = alignment.max(align);
        assert_eq!((name.as_str(), offset), (*rust_name, *rust_offset));
        assert_eq!(expected_type, *rust_type, "{name}");
        offset += size;
    }
    assert_eq!(
        offset.next_multiple_of(alignment),
        size_of::<SlitherFrame>()
    );
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        size_of::<SlitherFrame>(),
        32 + 6 * 8 + 39 * 8 + size_of::<SlitherWorldRules>()
    );
    compile_layout_assertions(&rust);
}

/// Has a real C compiler check the header against the Rust layout, so a bug in
/// the parser above cannot hide a mismatch. Hosts without `cc` (or `$CC`), such
/// as plain Windows shells, skip this part unless `SLITHER_SIM_REQUIRE_CC` is
/// set, as it is in the Linux and macOS CI jobs.
fn compile_layout_assertions(rust: &[(&str, usize, &str)]) {
    use std::process::Command;
    let mut source = String::from("#include <stddef.h>\n#include \"slither_sim.h\"\n");
    source.push_str(&format!(
        "_Static_assert(sizeof(slither_frame) == {}, \"size\");\n\
         _Static_assert(_Alignof(slither_frame) == {}, \"align\");\n",
        size_of::<SlitherFrame>(),
        align_of::<SlitherFrame>()
    ));
    for (name, offset, _) in rust {
        source.push_str(&format!(
            "_Static_assert(offsetof(slither_frame, {name}) == {offset}, \"{name}\");\n"
        ));
    }
    for (c_type, size, align, fields) in [
        (
            "slither_world_rules",
            size_of::<SlitherWorldRules>(),
            align_of::<SlitherWorldRules>(),
            vec![
                (
                    "boundary_mode",
                    offset_of!(SlitherWorldRules, boundary_mode),
                ),
                ("flags", offset_of!(SlitherWorldRules, flags)),
                ("pit_radius", offset_of!(SlitherWorldRules, pit_radius)),
                ("min_score", offset_of!(SlitherWorldRules, min_score)),
                (
                    "kill_score_seconds",
                    offset_of!(SlitherWorldRules, kill_score_seconds),
                ),
            ],
        ),
        (
            "slither_error",
            size_of::<SlitherError>(),
            align_of::<SlitherError>(),
            vec![
                ("code", offset_of!(SlitherError, code)),
                ("rule", offset_of!(SlitherError, rule)),
                ("flags", offset_of!(SlitherError, flags)),
                ("world", offset_of!(SlitherError, world)),
                ("slot", offset_of!(SlitherError, slot)),
                ("field", offset_of!(SlitherError, field)),
                ("message", offset_of!(SlitherError, message)),
            ],
        ),
    ] {
        source.push_str(&format!(
            "_Static_assert(sizeof({c_type}) == {size}, \"{c_type} size\");\n\
             _Static_assert(_Alignof({c_type}) == {align}, \"{c_type} align\");\n"
        ));
        for (name, offset) in fields {
            source.push_str(&format!(
                "_Static_assert(offsetof({c_type}, {name}) == {offset}, \"{c_type}.{name}\");\n"
            ));
        }
    }
    let path = std::env::temp_dir().join(format!("slither_sim_layout_{}.c", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let compiler = std::env::var("CC").unwrap_or_else(|_| {
        if cfg!(target_env = "msvc") {
            "cl".into()
        } else {
            "cc".into()
        }
    });
    let mut command = Command::new(&compiler);
    let msvc = std::path::Path::new(&compiler)
        .file_stem()
        .is_some_and(|name| {
            let name = name.to_string_lossy();
            name.eq_ignore_ascii_case("cl") || name.eq_ignore_ascii_case("clang-cl")
        });
    if msvc {
        command.args(["/nologo", "/std:c11", "/TC", "/Zs", "/I"]);
    } else {
        command.args(["-std=c11", "-fsyntax-only", "-I"]);
    }
    let result = command
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/include"))
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    match result {
        Ok(output) => assert!(
            output.status.success(),
            "{compiler} rejected the header layout:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::env::var_os("SLITHER_SIM_REQUIRE_CC").is_none_or(|v| v.is_empty()) =>
        {
            eprintln!("skipped the C compiler layout check: {compiler} not found");
        }
        Err(error) => panic!("could not run {compiler}: {error}"),
    }
}

#[test]
fn constants_and_functions_match_the_header() {
    let defines: HashMap<&str, &str> = HEADER
        .lines()
        .filter_map(|line| line.strip_prefix("#define "))
        .filter_map(|line| line.split_once(' '))
        .collect();
    let number = |name: &str| -> u64 {
        match defines[name].trim_end_matches('u') {
            "UINT32_MAX" => u32::MAX.into(),
            value => value.parse().unwrap(),
        }
    };
    assert_eq!(
        number("SLITHER_ABI_VERSION"),
        u64::from(SLITHER_ABI_VERSION)
    );
    assert_eq!(slither_abi_version(), SLITHER_ABI_VERSION);
    for (name, code) in [
        ("SLITHER_OK", SLITHER_OK),
        ("SLITHER_REJECTED", SLITHER_REJECTED),
        ("SLITHER_INVALID", SLITHER_INVALID),
        ("SLITHER_PANICKED", SLITHER_PANICKED),
        ("SLITHER_ENDED", SLITHER_ENDED),
    ] {
        assert_eq!(number(name), code as u64, "{name}");
    }
    for code in [
        slither_sim::ErrorCode::InvalidRequest,
        slither_sim::ErrorCode::InvalidConfig,
        slither_sim::ErrorCode::ActionShape,
        slither_sim::ErrorCode::InvalidAction,
        slither_sim::ErrorCode::UnknownSlot,
        slither_sim::ErrorCode::UnknownWorld,
        slither_sim::ErrorCode::UnknownEntity,
        slither_sim::ErrorCode::DuplicateId,
        slither_sim::ErrorCode::InvalidState,
        slither_sim::ErrorCode::PlacementExhausted,
        slither_sim::ErrorCode::NotReset,
        slither_sim::ErrorCode::WorldStopped,
        slither_sim::ErrorCode::UnsupportedCheckpoint,
        slither_sim::ErrorCode::InvalidCheckpoint,
        slither_sim::ErrorCode::NumericOverflow,
        slither_sim::ErrorCode::RepresentationLimit,
        slither_sim::ErrorCode::AllocationFailed,
    ] {
        assert_eq!(
            number(&format!("SLITHER_CAUSE_{}", code.as_str().to_uppercase())),
            code as u64
        );
    }
    for (name, rule) in [
        ("MISSING", slither_sim::Constraint::Missing),
        ("NON_FINITE", slither_sim::Constraint::NonFinite),
        ("NEGATIVE", slither_sim::Constraint::Negative),
        ("NON_POSITIVE", slither_sim::Constraint::NonPositive),
        ("OUT_OF_RANGE", slither_sim::Constraint::OutOfRange),
        ("LENGTH_MISMATCH", slither_sim::Constraint::LengthMismatch),
        ("DUPLICATE", slither_sim::Constraint::Duplicate),
        ("ORDERING", slither_sim::Constraint::Ordering),
        ("OUTSIDE_ARENA", slither_sim::Constraint::OutsideArena),
        (
            "INCONSISTENT_STATE",
            slither_sim::Constraint::InconsistentState,
        ),
    ] {
        assert_eq!(
            number(&format!("SLITHER_CONSTRAINT_{name}")),
            u64::from(rule.code())
        );
    }
    for (name, code) in [
        ("SLITHER_ERROR_HAS_WORLD", SLITHER_ERROR_HAS_WORLD),
        ("SLITHER_ERROR_HAS_SLOT", SLITHER_ERROR_HAS_SLOT),
        ("SLITHER_RULE_HAS_PIT", SLITHER_RULE_HAS_PIT),
        ("SLITHER_RULE_HAS_MIN_SCORE", SLITHER_RULE_HAS_MIN_SCORE),
        ("SLITHER_RULE_HAS_KILL_WINDOW", SLITHER_RULE_HAS_KILL_WINDOW),
        (
            "SLITHER_RULE_FREE_BOOST_AT_FLOOR",
            SLITHER_RULE_FREE_BOOST_AT_FLOOR,
        ),
    ] {
        assert_eq!(number(name), u64::from(code));
    }
    assert_eq!(number("SLITHER_NONE"), u64::from(NONE));
    assert_eq!(number("SLITHER_NO_ORIGIN"), u64::from(NO_ORIGIN));
    for (code, origin) in ORIGINS.iter().enumerate() {
        let name = format!("SLITHER_ORIGIN_{}", origin.to_uppercase());
        assert_eq!(number(&name), code as u64);
    }
    for (code, reason) in REASONS.iter().enumerate() {
        let name = format!("SLITHER_DEATH_{}", reason.to_uppercase());
        assert_eq!(number(&name), code as u64);
    }
    for function in [
        "uint32_t slither_abi_version(void)",
        "slither_session *slither_session_new(void)",
        "void slither_session_free(slither_session *session)",
        "int32_t slither_request(slither_session *session, const uint8_t *line, size_t line_len,",
        "int32_t slither_step(slither_session *session, const double *headings, size_t headings_len,",
        "int32_t slither_read_frame(slither_session *session, size_t world, slither_frame *frame)",
        "size_t slither_worlds(const slither_session *session)",
        "size_t slither_agents(const slither_session *session)",
        "const char *slither_last_error(const slither_session *session)",
        "int32_t slither_read_error(const slither_session *session, slither_error *error)",
    ] {
        assert!(HEADER.contains(function), "{function}");
    }
}

struct Handle(*mut SlitherSession);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { slither_session_free(self.0) };
    }
}
impl Handle {
    fn new() -> Self {
        let session = slither_session_new();
        assert!(!session.is_null());
        Handle(session)
    }
    fn request(&self, line: &str) -> Vec<u8> {
        let (mut reply, mut len) = (ptr::null(), usize::MAX);
        let code =
            unsafe { slither_request(self.0, line.as_ptr(), line.len(), &mut reply, &mut len) };
        assert_eq!(code, SLITHER_OK);
        let bytes = unsafe { std::slice::from_raw_parts(reply, len + 1) };
        assert_eq!(bytes[len], 0, "replies end with a NUL");
        bytes[..len].to_vec()
    }
    fn step(&self, headings: &[f64], boosts: &[u8]) -> i32 {
        unsafe {
            slither_step(
                self.0,
                headings.as_ptr(),
                headings.len(),
                boosts.as_ptr(),
                boosts.len(),
            )
        }
    }
    fn frame(&self, world: usize) -> Result<SlitherFrame, (i32, String)> {
        // SAFETY: every member is a number or a raw pointer.
        let mut frame: SlitherFrame = unsafe { std::mem::zeroed() };
        match unsafe { slither_read_frame(self.0, world, &mut frame) } {
            SLITHER_OK => Ok(frame),
            code => Err((code, self.error())),
        }
    }
    fn error(&self) -> String {
        let error = unsafe { CStr::from_ptr(slither_last_error(self.0)) };
        error.to_str().unwrap().to_owned()
    }
}

/// # Safety
/// `data` points to `len` elements, or `len` is 0.
unsafe fn column<'a, T>(data: *const T, len: usize) -> &'a [T] {
    assert!(!data.is_null());
    if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
    }
}

fn assert_same(frame: &SlitherFrame, expected: FrameRef) {
    let rules = SlitherWorldRules::from(expected.rules);
    assert_eq!(frame.rules.boundary_mode, rules.boundary_mode);
    assert_eq!(frame.rules.flags, rules.flags);
    assert_eq!(frame.rules.pit_radius, rules.pit_radius);
    assert_eq!(frame.rules.min_score, rules.min_score);
    assert_eq!(frame.rules.kill_score_seconds, rules.kill_score_seconds);
    let (s, f, p, v, d) = (
        expected.snakes,
        expected.food,
        expected.prey,
        expected.views,
        expected.deaths,
    );
    assert_eq!(
        (
            frame.time,
            frame.radius,
            frame.vision,
            frame.ticks,
            frame.living
        ),
        (
            expected.time,
            expected.radius,
            expected.vision,
            expected.ticks,
            expected.living
        )
    );
    assert_eq!(frame.slots, v.me.len());
    let (n, slots) = (frame.snakes, frame.slots);
    unsafe {
        assert_eq!(column(frame.snake_slot, n), s.slot);
        assert_eq!(column(frame.snake_x, n), s.x);
        assert_eq!(column(frame.snake_y, n), s.y);
        assert_eq!(column(frame.snake_heading, n), s.heading);
        assert_eq!(column(frame.snake_speed, n), s.speed);
        assert_eq!(column(frame.snake_radius, n), s.radius);
        assert_eq!(column(frame.snake_length, n), s.length);
        assert_eq!(column(frame.snake_segments, n), s.segments);
        assert_eq!(column(frame.snake_fam, n), s.fam);
        assert_eq!(column(frame.snake_score, n), s.score);
        assert_eq!(column(frame.snake_kills, n), s.kills);
        assert_eq!(column(frame.snake_kill_score, n), s.kill_score);
        assert_eq!(column(frame.snake_body_start, n + 1), s.body_start);
        assert_eq!(column(frame.snake_body_x, frame.body), s.body_x);
        assert_eq!(column(frame.snake_body_y, frame.body), s.body_y);
        let n = frame.food;
        assert_eq!(column(frame.food_x, n), f.x);
        assert_eq!(column(frame.food_y, n), f.y);
        assert_eq!(column(frame.food_value, n), f.value);
        let sizes = column(frame.food_size, n);
        assert!(
            sizes
                .iter()
                .zip(f.size)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert_eq!(column(frame.food_origin, n), f.origin);
        let n = frame.prey;
        assert_eq!(column(frame.prey_id, n), p.id);
        assert_eq!(column(frame.prey_x, n), p.x);
        assert_eq!(column(frame.prey_y, n), p.y);
        assert_eq!(column(frame.prey_heading, n), p.heading);
        assert_eq!(column(frame.prey_target_heading, n), p.target_heading);
        assert_eq!(column(frame.prey_speed, n), p.speed);
        assert_eq!(column(frame.prey_size, n), p.size);
        assert_eq!(column(frame.view_self, slots), v.me);
        for (start, rows, expected_start, expected_rows) in [
            (
                frame.view_snakes_start,
                frame.view_snakes,
                v.snakes_start,
                v.snakes,
            ),
            (frame.view_food_start, frame.view_food, v.food_start, v.food),
            (frame.view_prey_start, frame.view_prey, v.prey_start, v.prey),
        ] {
            let start = column(start, slots + 1);
            assert_eq!(start, expected_start);
            assert_eq!(column(rows, start[slots] as usize), expected_rows);
        }
        let n = frame.deaths;
        assert_eq!(column(frame.death_time, n), d.time);
        assert_eq!(column(frame.death_victim, n), d.victim);
        assert_eq!(column(frame.death_killer, n), d.killer);
        assert_eq!(column(frame.death_reason, n), d.reason);
        assert_eq!(column(frame.death_food, n), d.food);
    }
}

#[test]
fn sessions_match_the_rust_api() {
    let handle = Handle::new();
    let mut expected = Session::new();
    assert_eq!(
        unsafe { (slither_worlds(handle.0), slither_agents(handle.0)) },
        (0, 0)
    );
    assert_eq!(
        handle.frame(0).unwrap_err(),
        (SLITHER_REJECTED, "Reset first".into())
    );
    assert_eq!(handle.step(&[], &[]), SLITHER_REJECTED);
    assert_eq!(handle.error(), "Reset first");

    // A synthetic pit and a short kill-score window fill the newer columns.
    let reset = r#"{"op":"reset","seed":11,"worlds":2,"agents":5,"config":{"radius":400,"dynamicRadius":false,"boundaryMode":"hard","foodCount":200,"pit":{"radius":120},"spawnRadius":200,"killScoreSeconds":0.2}}"#;
    for line in [r#"{"op":"info"}"#, "not json", reset] {
        assert_eq!(handle.request(line), expected.request(line.as_bytes()));
    }
    assert_eq!(
        unsafe { (slither_worlds(handle.0), slither_agents(handle.0)) },
        (2, 5)
    );
    assert_eq!(
        handle.frame(2).unwrap_err(),
        (SLITHER_REJECTED, "World 2: Unknown world".into())
    );
    let (mut reasons, mut scored) = ([false; 3], false);
    for t in 0..80 {
        let mut headings = Vec::new();
        for w in 0..2 {
            let frame = expected.frame(w).unwrap();
            assert_same(&handle.frame(w).unwrap(), frame);
            headings.extend(frame.views.me.iter().enumerate().map(|(s, &me)| {
                if me == NONE {
                    f64::NAN
                } else {
                    (s as f64 * 1.3 + w as f64 + t as f64 * 0.1) % 6.0
                }
            }));
        }
        // Any nonzero byte boosts.
        let boosts: Vec<u8> = (0..10).map(|i| [0, 1, 7][(i + t) % 3]).collect();
        assert_eq!(handle.step(&headings, &boosts), SLITHER_OK);
        assert_eq!(handle.error(), "");
        let flags: Vec<bool> = boosts.iter().map(|&b| b != 0).collect();
        expected.step(&headings, &flags).unwrap();
        for w in 0..2 {
            let frame = handle.frame(w).unwrap();
            for &reason in unsafe { column(frame.death_reason, frame.deaths) } {
                reasons[reason as usize] = true;
            }
            scored |= unsafe { column(frame.snake_kill_score, frame.snakes) }
                .iter()
                .any(|&score| score > 0);
        }
    }
    // Body and pit deaths, and a kill score above zero.
    assert!(reasons[0] && reasons[2] && scored, "{reasons:?} {scored}");
    let snapshot = r#"{"op":"snapshot","world":1}"#;
    assert_eq!(
        handle.request(snapshot),
        expected.request(snapshot.as_bytes())
    );
}

#[test]
fn terminal_failure_is_distinct_from_rejection_and_reset_recovers() {
    // Synthetic stationary catches: individually finite gains overflow in one tick.
    let reset = r#"{"op":"reset","worlds":2,"agents":1,"decisionTicks":1,"config":{"radius":100,"dynamicRadius":false,"foodCount":0,"ambientReplenishPerSecond":0,"prey":{"count":100,"size":18.8,"maxSize":18.8,"nutritionFactor":1.6128984555936463e308,"cruiseSpeed":0,"escapeSpeed":0,"sprintSpeed":0,"turnRate":0}}}"#;
    let handle = Handle::new();
    let reply = String::from_utf8(handle.request(reset)).unwrap();
    assert!(reply.contains("\"result\":"), "{reply}");
    assert_eq!(unsafe { slither_worlds(handle.0) }, 2);
    assert_eq!(
        handle.step(&[std::f64::consts::PI; 2], &[0; 2]),
        SLITHER_ENDED
    );
    assert!(handle.error().contains("mass would overflow"));
    assert_eq!(
        unsafe { (slither_worlds(handle.0), slither_agents(handle.0)) },
        (0, 0)
    );
    assert_eq!(
        handle.frame(0).unwrap_err(),
        (SLITHER_REJECTED, "Reset first".into())
    );
    assert_eq!(handle.step(&[], &[]), SLITHER_REJECTED);
    assert_eq!(handle.error(), "Reset first");
    handle.request(r#"{"op":"reset","agents":1}"#);
    assert_eq!(handle.step(&[0.0], &[0]), SLITHER_OK);
    assert!(handle.frame(0).unwrap().time > 0.0);

    // JSON calls still return SLITHER_OK; the reply carries the failure kind.
    handle.request(reset);
    let step = r#"{"op":"step","actions":[{"heading":[3.141592653589793],"boost":[false]},{"heading":[3.141592653589793],"boost":[false]}]}"#;
    let terminal = String::from_utf8(handle.request(step)).unwrap();
    assert!(terminal.contains("mass would overflow"), "{terminal}");
    assert!(terminal.contains("\"sessionEnded\":true"), "{terminal}");
    assert_eq!(unsafe { slither_worlds(handle.0) }, 0);
    assert_eq!(
        handle.request(step),
        br#"{"error":{"code":11,"field":null,"message":"Reset first","rule":null,"slot":null,"world":null},"protocol":"slither-sim-json-v6","sessionEnded":false}"#
    );
    handle.request(r#"{"op":"reset","agents":1}"#);
    assert_eq!(handle.step(&[0.0], &[0]), SLITHER_OK);
}

#[test]
fn rejected_and_null_arguments_change_nothing() {
    let handle = Handle::new();
    handle.request(r#"{"op":"reset","seed":3,"worlds":2,"agents":2}"#);
    let snapshot =
        || [0, 1].map(|w| handle.request(&format!(r#"{{"op":"snapshot","world":{w}}}"#)));
    let before = snapshot();

    assert_eq!(
        handle.step(&[0.0, 1.0, 0.0, f64::NAN], &[0; 4]),
        SLITHER_REJECTED
    );
    assert_eq!(handle.error(), "World 1: Missing or invalid action for 1");
    assert_eq!(handle.step(&[0.0; 3], &[0; 3]), SLITHER_REJECTED);
    assert!(
        handle
            .error()
            .starts_with("One heading and one boost per agent")
    );
    let s = handle.0;
    let mut frame: SlitherFrame = unsafe { std::mem::zeroed() };
    let (mut reply, mut len) = (ptr::null(), 0);
    unsafe {
        assert_eq!(
            slither_step(s, ptr::null(), 4, [0u8; 4].as_ptr(), 4),
            SLITHER_INVALID
        );
        assert_eq!(handle.error(), "headings is null");
        assert_eq!(
            slither_step(s, [0.0; 4].as_ptr(), 4, ptr::null(), 4),
            SLITHER_INVALID
        );
        assert_eq!(handle.error(), "boosts is null");
        // Null arrays of length 0 are empty arrays.
        assert_eq!(
            slither_step(s, ptr::null(), 0, ptr::null(), 0),
            SLITHER_REJECTED
        );
        assert_eq!(slither_read_frame(s, 0, ptr::null_mut()), SLITHER_INVALID);
        assert_eq!(handle.error(), "frame is null");
        assert_eq!(
            slither_request(s, ptr::null(), 3, &mut reply, &mut len),
            SLITHER_INVALID
        );
        assert_eq!(handle.error(), "line is null");
        assert!(reply.is_null() && len == 0);
        assert_eq!(
            slither_request(s, ptr::null(), 0, ptr::null_mut(), &mut len),
            SLITHER_INVALID
        );
        assert_eq!(handle.error(), "reply is null");
        assert_eq!(
            slither_request(s, ptr::null(), 0, &mut reply, ptr::null_mut()),
            SLITHER_INVALID
        );
        assert_eq!(handle.error(), "reply_len is null");
        // An empty line is a malformed request, answered like any other.
        assert_eq!(
            slither_request(s, ptr::null(), 0, &mut reply, &mut len),
            SLITHER_OK
        );
        let empty = std::slice::from_raw_parts(reply, len);
        let decoded: serde_json::Value = serde_json::from_slice(empty).unwrap();
        assert_eq!(decoded["error"]["code"], 1);
        assert!(
            decoded["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("EOF while parsing")
        );
    }
    assert_eq!(snapshot(), before);

    let null = ptr::null_mut();
    unsafe {
        assert_eq!(
            slither_step(null, ptr::null(), 0, ptr::null(), 0),
            SLITHER_INVALID
        );
        assert_eq!(slither_read_frame(null, 0, &mut frame), SLITHER_INVALID);
        assert_eq!(
            slither_request(null, ptr::null(), 0, &mut reply, &mut len),
            SLITHER_INVALID
        );
        assert_eq!((slither_worlds(null), slither_agents(null)), (0, 0));
        assert_eq!(CStr::from_ptr(slither_last_error(null)), c"session is null");
        slither_session_free(null);
    }
}

#[test]
fn sessions_run_on_separate_threads() {
    fn send<T: Send>() {}
    send::<Session>();
    let replies: Vec<_> = (0..4)
        .map(|seed| {
            std::thread::spawn(move || {
                let handle = Handle::new();
                handle.request(&format!(r#"{{"op":"reset","seed":{seed},"agents":3}}"#));
                for _ in 0..5 {
                    assert_eq!(handle.step(&[0.0, 2.0, 4.0], &[0, 1, 0]), SLITHER_OK);
                }
                handle.request(r#"{"op":"snapshot"}"#)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    for (seed, reply) in replies.iter().enumerate() {
        let mut session = Session::new();
        session.request(format!(r#"{{"op":"reset","seed":{seed},"agents":3}}"#).as_bytes());
        for _ in 0..5 {
            session
                .step(&[0.0, 2.0, 4.0], &[false, true, false])
                .unwrap();
        }
        assert_eq!(*reply, session.request(br#"{"op":"snapshot"}"#));
    }
}

#[test]
fn c_frame_and_snapshot_expose_live_radius_without_changing_initial_config() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/serve-radius-v5.json")).unwrap();
    let session = Handle::new();
    session.request(&serde_json::to_string(&fixture["reset"]).unwrap());
    for radius in fixture["liveRadii"].as_array().unwrap() {
        let snapshot: serde_json::Value =
            serde_json::from_slice(&session.request(r#"{"op":"snapshot"}"#)).unwrap();
        assert_eq!(fixture["protocol"], "slither-sim-json-v5");
        assert_eq!(snapshot["protocol"], "slither-sim-json-v6");
        assert_eq!(
            snapshot["result"]["config"]["radius"],
            fixture["initialRadius"]
        );
        assert_eq!(snapshot["result"]["radius"], *radius);
        assert_eq!(session.frame(0).unwrap().radius, radius.as_f64().unwrap());
        assert_eq!(session.step(&[0.], &[0]), SLITHER_OK);
    }
}

fn assert_cause_matches(handle: &Handle, expected: &serde_json::Value) {
    let mut error = std::mem::MaybeUninit::<SlitherError>::uninit();
    assert_eq!(
        unsafe { slither_read_error(handle.0, error.as_mut_ptr()) },
        SLITHER_OK
    );
    let error = unsafe { error.assume_init() };
    assert_eq!(u64::from(error.code), expected["code"].as_u64().unwrap());
    assert_eq!(
        (error.rule != 0).then_some(u64::from(error.rule)),
        expected["rule"].as_u64()
    );
    assert_eq!(
        (error.flags & SLITHER_ERROR_HAS_WORLD != 0).then_some(error.world as u64),
        expected["world"].as_u64()
    );
    assert_eq!(
        (error.flags & SLITHER_ERROR_HAS_SLOT != 0).then_some(error.slot as u64),
        expected["slot"].as_u64()
    );
    let field = if error.field.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(error.field) }.to_str().unwrap())
    };
    assert_eq!(field, expected["field"].as_str());
    assert_eq!(
        unsafe { CStr::from_ptr(error.message) }.to_str().unwrap(),
        expected["message"].as_str().unwrap()
    );
}

#[test]
fn c_error_inspection_matches_json_rust_and_preserves_rejected_ended() {
    let handle = Handle::new();
    let mut oracle = Session::new();
    let cause: serde_json::Value =
        serde_json::from_slice(&oracle.request(br#"{"op":"step","actions":[]}"#)).unwrap();
    assert_eq!(handle.step(&[], &[]), SLITHER_REJECTED);
    assert_cause_matches(&handle, &cause["error"]);
    let reset =
        r#"{"op":"reset","agents":2,"worlds":2,"config":{"preset":"sandbox","foodCount":0}}"#;
    handle.request(reset);
    oracle.request(reset.as_bytes());
    let cause = oracle
        .step(&[0., 0., 0., f64::NAN], &[false; 4])
        .unwrap_err();
    assert_eq!(
        handle.step(&[0., 0., 0., f64::NAN], &[0; 4]),
        SLITHER_REJECTED
    );
    assert_cause_matches(&handle, &serde_json::to_value(cause.details()).unwrap());
    let cause: serde_json::Value =
        serde_json::from_slice(&handle.request(r#"{"op":"reset","config":{"radius":-1}}"#))
            .unwrap();
    assert_cause_matches(&handle, &cause["error"]);
    let huge = usize::MAX;
    let cause = oracle.frame_checked(huge).unwrap_err();
    assert!(handle.frame(huge).is_err());
    assert_cause_matches(&handle, &serde_json::to_value(cause.details()).unwrap());
    let cause = oracle.frame_checked(17).unwrap_err();
    assert!(handle.frame(17).is_err());
    assert_cause_matches(&handle, &serde_json::to_value(cause.details()).unwrap());
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/serve-errors-v5.json")).unwrap();
    let reset = serde_json::to_string(&fixture["reset"]).unwrap();
    handle.request(&reset);
    oracle.request(reset.as_bytes());
    let cause: serde_json::Value =
        serde_json::from_slice(&oracle.request(&serde_json::to_vec(&fixture["step"]).unwrap()))
            .unwrap();
    assert_eq!(
        handle.step(&[std::f64::consts::PI; 2], &[0; 2]),
        SLITHER_ENDED
    );
    assert_cause_matches(&handle, &cause["error"]);
    assert_eq!(unsafe { slither_worlds(handle.0) }, 0);
    handle.request(r#"{"op":"reset","agents":1}"#);
    let mut error = std::mem::MaybeUninit::<SlitherError>::uninit();
    unsafe {
        slither_read_error(handle.0, error.as_mut_ptr());
    }
    assert_eq!(unsafe { error.assume_init() }.code, 0);
}

#[test]
fn inline_c_rules_match_json_and_absent_fields_use_flags() {
    let handle = Handle::new();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/serve-rules-v5.json")).unwrap();
    handle.request(&serde_json::to_string(&fixture["reset"]).unwrap());
    for _ in 0..3 {
        let rules = handle.frame(0).unwrap().rules;
        assert_eq!(rules.boundary_mode, SLITHER_BOUNDARY_HARD);
        assert_eq!(rules.flags, 15);
        assert_eq!(rules.pit_radius, 550.);
        assert_eq!(rules.min_score, 400);
        assert_eq!(rules.kill_score_seconds, 0.014);
        assert_eq!(handle.step(&[0., 0.], &[1, 1]), SLITHER_OK);
    }
    handle.request(r#"{"op":"reset","agents":1,"config":{"preset":"sandbox","foodCount":0}}"#);
    let rules = handle.frame(0).unwrap().rules;
    assert_eq!(rules.boundary_mode, SLITHER_BOUNDARY_SLIDING);
    assert_eq!(rules.flags, 0);
    assert_eq!(
        (rules.pit_radius, rules.min_score, rules.kill_score_seconds),
        (0., 0, 0.)
    );
}
