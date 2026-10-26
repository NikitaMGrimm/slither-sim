# C ABI: slither-sim-abi-v3

`slither-sim-ffi` wraps Rust [`transport::Session`](bot-api.md#in-process-sessions)
for C callers, with the worlds, requests and frame values of [`serve`](bot-api.md).
Typed-array steps and column pointers avoid JSON in the decision loop.

## Build and run

```sh
cargo build --release --locked -p slither-sim-ffi
python examples/ffi_bot.py --steps 200
```

The `ffi/` workspace member (not on crates.io) builds a static library plus
`slither_sim_ffi.dll` (Windows), `libslither_sim_ffi.so` (Linux) or
`libslither_sim_ffi.dylib` (macOS) in `target/release`. The hand-written header is
[`ffi/include/slither_sim.h`](https://github.com/NikitaMGrimm/slither-sim/blob/main/ffi/include/slither_sim.h).

[Release archives](../README.md#download-a-release) include the shared library,
the header and, on Windows, the `slither_sim_ffi.dll.lib` import library, for
Linux (glibc 2.17+), macOS and Windows. They are unsigned.

The [example](../examples/ffi_bot.py) uses standard-library `ctypes`, makes the
[JSON controller](../examples/bot.py)'s decisions and prints its summary. Load
an extracted library or another build with `--library PATH` or `SLITHER_SIM_FFI`.

## Calls

| Function | Purpose |
| --- | --- |
| `slither_abi_version()` | `SLITHER_ABI_VERSION` of the loaded library: 3 for `slither-sim-abi-v3`. Check it first. |
| `slither_session_new()`, `slither_session_free(s)` | Create and free a session; free ignores null. |
| `slither_request(s, line, len, &reply, &reply_len)` | Answers one `slither-sim-json-v5` request line as `serve` would. |
| `slither_step(s, headings, n, boosts, n)` | Steps every world like a JSON `step`, without JSON. |
| `slither_read_frame(s, world, &frame)` | Fills a `slither_frame` with one world's current frame. |
| `slither_worlds(s)`, `slither_agents(s)` | Worlds and slots per world; 0 before reset. |
| `slither_last_error(s)` | Diagnostic text for the latest request, step or frame error, including embedded JSON errors. |
| `slither_read_error(s, &error)` | Read stable code, constraint and locations without clearing them. |

Use `slither_request` for `info`, `reset` and `snapshot`. JSON errors, including
terminal failures, return `SLITHER_OK`; inspect `error` and `sessionEnded`.
Replies have no newline; a trailing NUL is excluded from `reply_len`.
`error` is the geometric v5 structured object. A JSON failure still returns
`SLITHER_OK`; both that object and `slither_read_error` retain its typed cause.

`slither_step` takes `worlds × agents` entries per array, world-major: slot `s`
of world `w` is entry `w × agents + s`. Headings are `double` radians, with NaN
for no action, which only dead slots may give. Boosts are `uint8_t`; any nonzero
value boosts. JSON and typed steps share worlds, validation and error text.
Rejection changes nothing. Advancement or frame-collection failure returns
`SLITHER_ENDED` and clears every world: `slither_worlds` and `slither_agents`
return 0; reset the usable handle before reading frames or stepping again.

| Code | Meaning |
| --- | --- |
| `SLITHER_OK` (0) | Success. |
| `SLITHER_REJECTED` (1) | The session rejected the call, for example a step before reset, and changed nothing. |
| `SLITHER_INVALID` (2) | A required pointer was null; nothing changed. Arrays may be null when their length is 0. |
| `SLITHER_PANICKED` (3) | An internal error. Panics never unwind into the caller; only `slither_last_error` and `slither_session_free` remain valid. |
| `SLITHER_ENDED` (4) | Advancement failed and cleared every world; reset to continue. |

## Frames

`slither_frame.radius` is the live boundary, independent of the initial
`snapshot.config.radius`. `slither_request` snapshots expose a separate live
`radius` field. ABI v3 appends an inline rules record to the frame; rebuild v2 callers.

`slither_frame` holds the frame's `time`, `radius`, `vision`, `ticks` and
`living`; the counts `slots`, `snakes`, `body`, `food`, `prey` and `deaths`; and
one pointer per column. Columns hold the values and fixed types of the
[Rust frame columns](bot-api.md#in-process-sessions): `double` measures,
`uint32_t` slots, counts and row indices, `uint64_t` prey IDs and `uint8_t` codes.

- `snake_kill_score` maps to JSON [`killScore`](bot-api.md#frames).
- Snake row `r`'s anchors are entries `snake_body_start[r]` up to
  `snake_body_start[r + 1]` of `snake_body_x` and `snake_body_y`.
- `view_self[s]` is slot `s`'s own snakes row, or `SLITHER_NONE` when the slot is
  dead. Slot `s` sees the snake rows in
  `view_snakes[view_snakes_start[s]:view_snakes_start[s + 1]]` (half-open slice),
  and likewise food and prey. Each `*_start` column
  has `slots + 1` entries.
- `food_size` is NaN without a size, `food_origin` is `SLITHER_NO_ORIGIN` without
  an origin, and `death_killer` is `SLITHER_NONE` without a killer; JSON prints
  `null` for each. `death_reason` is `SLITHER_DEATH_BODY`,
  `SLITHER_DEATH_BOUNDARY` or `SLITHER_DEATH_PIT`.

Pointers of empty columns are not null, but must not be dereferenced.

### Inline rules and error inspection

`slither_frame.rules` is a `slither_world_rules` value. Boundary mode is
`SLITHER_BOUNDARY_SLIDING` (0) or `SLITHER_BOUNDARY_HARD` (1). Its flags are
`SLITHER_RULE_HAS_PIT` (1), `SLITHER_RULE_HAS_MIN_SCORE` (2),
`SLITHER_RULE_HAS_KILL_WINDOW` (4) and `SLITHER_RULE_FREE_BOOST_AT_FLOOR` (8).
Absent numeric fields are zero and ignored unless their presence flag is set.
The header and Rust use native C layout; compiler assertions check every rules
member, the appended frame member and the error structure.

`slither_error.code` uses the [geometric cause codes](bot-api.md#structured-errors),
with 0 for no error, 18 for a null-pointer boundary and 19 for an internal panic.
`rule` is a constraint code or 0 when absent. `SLITHER_ERROR_HAS_WORLD` (1) and
`SLITHER_ERROR_HAS_SLOT` (2) indicate location presence independently of the full
`size_t` values, including `SIZE_MAX`. An absent location's numeric member is 0;
`field` is null when absent. `message` is diagnostic text. Inspection succeeds
without clearing the cause; the next request, step or frame read replaces it,
and success clears it. Inspection remains available after a panic.

## Lifetimes and threads

Pointers from `slither_request`, `slither_read_frame` and `slither_last_error`
remain valid until the next call that takes the same session. Copy what you keep.
Never use one session from two threads at once; separate sessions may run on
separate threads. Steps advance several worlds in parallel on the library's
[rayon](https://docs.rs/rayon) pool, as `serve` does; `RAYON_NUM_THREADS` sizes it.

## Compatibility

`slither-sim-abi-v3` covers function signatures, `slither_frame`/rules/error layouts, codes
and column meanings. Incompatible changes need a new identifier and
`SLITHER_ABI_VERSION`. Rebuild v1/v2 callers; see [migration notes](../CHANGELOG.md#unreleased).
Requests and replies follow `slither-sim-json-v5`; check `protocol` and record
the commit for reproducibility.
