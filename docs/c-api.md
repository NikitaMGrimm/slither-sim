# C ABI: slither-sim-abi-v2

The `slither-sim-ffi` library runs [`serve` sessions](bot-api.md) inside the
caller's process for any language that can call C. It has the same worlds,
requests and frame values as `serve`. Steps take typed arrays and frames are read
as column pointers, so the decision loop never encodes or parses JSON. It wraps
the Rust [`transport::Session`](bot-api.md#in-process-sessions).

## Build and run

```sh
cargo build --release --locked -p slither-sim-ffi
python examples/ffi_bot.py --steps 200
```

The `ffi/` workspace member builds `slither_sim_ffi.dll` on Windows,
`libslither_sim_ffi.so` on Linux or `libslither_sim_ffi.dylib` on macOS in
`target/release`, plus a static library. It is not on crates.io. The
hand-written header is
[`ffi/include/slither_sim.h`](https://github.com/NikitaMGrimm/slither-sim/blob/main/ffi/include/slither_sim.h).

[Release archives](../README.md#download-a-release) include the shared library,
the header and, on Windows, the `slither_sim_ffi.dll.lib` import library, for
Linux (glibc 2.17+), macOS and Windows. They are unsigned. Pass the extracted
library to the example with `--library`.

The [Python example](../examples/ffi_bot.py) uses only `ctypes` from the standard
library. It makes the same decisions as the [JSON controller](../examples/bot.py)
and prints the same summary. Pass `--library PATH` or set `SLITHER_SIM_FFI` to
load another build.

## Calls

| Function | Purpose |
| --- | --- |
| `slither_abi_version()` | `SLITHER_ABI_VERSION` of the loaded library: 2 for `slither-sim-abi-v2`. Check it first. |
| `slither_session_new()`, `slither_session_free(s)` | Create and free a session; free ignores null. |
| `slither_request(s, line, len, &reply, &reply_len)` | Answers one `slither-sim-json-v3` request line as `serve` would. |
| `slither_step(s, headings, n, boosts, n)` | Steps every world like a JSON `step`, without JSON. |
| `slither_read_frame(s, world, &frame)` | Fills a `slither_frame` with one world's current frame. |
| `slither_worlds(s)`, `slither_agents(s)` | Worlds and slots per world; 0 before reset. |
| `slither_last_error(s)` | Why the latest request, step or frame read failed, or an empty string. |

Use `slither_request` for `info`, `reset` and `snapshot`. JSON errors, including
terminal failures, return `SLITHER_OK`; inspect `error` and `sessionEnded` in
the reply. The reply has no newline;
a NUL that `reply_len` excludes follows it.

`slither_step` takes `worlds × agents` entries per array, world-major: slot `s`
of world `w` is entry `w × agents + s`. Headings are `double` radians, with NaN
for no action, which only dead slots may give. Boosts are `uint8_t`; any nonzero
value boosts. Validation and error text match the JSON step, and a rejected step
changes nothing. JSON and typed steps act on the same worlds.
If advancement or frame collection fails, `SLITHER_ENDED` clears every world;
`slither_worlds` and `slither_agents` return 0, and frame reads require a reset.
The handle remains usable: reset it to continue.

| Code | Meaning |
| --- | --- |
| `SLITHER_OK` (0) | Success. |
| `SLITHER_REJECTED` (1) | The session rejected the call, for example a step before reset, and changed nothing. |
| `SLITHER_INVALID` (2) | A required pointer was null; nothing changed. Arrays may be null when their length is 0. |
| `SLITHER_PANICKED` (3) | An internal error. Panics never unwind into the caller; only `slither_last_error` and `slither_session_free` remain valid. |
| `SLITHER_ENDED` (4) | Advancement failed and cleared every world; reset to continue. |

## Frames

`slither_frame` holds the frame's `time`, `radius`, `vision`, `ticks` and
`living`; the counts `slots`, `snakes`, `body`, `food`, `prey` and `deaths`; and
one pointer per column. Columns hold the values and fixed types of the
[Rust frame columns](bot-api.md#in-process-sessions): `double` measures,
`uint32_t` slots, counts and row indices, `uint64_t` prey IDs and `uint8_t` codes.

- Snake row `r`'s anchors are entries `snake_body_start[r]` up to
  `snake_body_start[r + 1]` of `snake_body_x` and `snake_body_y`.
- `view_self[s]` is slot `s`'s own snakes row, or `SLITHER_NONE` when the slot is
  dead. Slot `s` sees the snakes rows `view_snakes[view_snakes_start[s]]` up to
  `view_snakes_start[s + 1]`, and likewise food and prey. Each `*_start` column
  has `slots + 1` entries.
- `food_size` is NaN without a size, `food_origin` is `SLITHER_NO_ORIGIN` without
  an origin, and `death_killer` is `SLITHER_NONE` without a killer; JSON prints
  `null` for each. `death_reason` is `SLITHER_DEATH_BODY` or
  `SLITHER_DEATH_BOUNDARY`.

Pointers of empty columns are not null, but must not be dereferenced.

## Lifetimes and threads

Pointers from `slither_request`, `slither_read_frame` and `slither_last_error`
remain valid until the next call that takes the same session. Copy what you keep.
Never use one session from two threads at once; separate sessions may run on
separate threads. Steps advance several worlds in parallel on the library's
[rayon](https://docs.rs/rayon) pool, as `serve` does; `RAYON_NUM_THREADS` sizes it.

## Compatibility

`slither-sim-abi-v2` covers the function signatures, the `slither_frame` layout,
the codes and the column meanings. An incompatible change needs a new identifier
and `SLITHER_ABI_VERSION`. Requests and replies follow `slither-sim-json-v3`;
check `protocol` in replies. As with `serve`, record the commit for reproducible
results.
Version 2 adds `SLITHER_ENDED`, distinguishing terminal failure from
`SLITHER_REJECTED`.
