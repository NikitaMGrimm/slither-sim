# Retained client processing workloads

`examples/perf_client_retained.rs` extends the short portable/protocol fixtures
in [the initial baseline](performance.md). It runs a seed-7 synthetic world for
up to 384 decisions, six 7 ms ticks per decision: 16.128 virtual seconds. Eight
slots retain their connections and decoders; slot zero reconnects immediately
after decision 192. Death does not reset the world. Native background seekers
may reconnect dead background slots in `next_frame` mode.

The explicit workload overrides are radius 6,000, 12,000 food placement attempts,
eight prey, vision 3,000, eight agents, zero added link/processing delays, wire
recording and a 136-square minimap. Broadcast/decision, minimap and leaderboard
cadences are six ticks, 2.5 seconds and three seconds. Other defaults remain
unchanged. Normal spawn bodies are used; available anchor counts are reported.
These are local engineering fixtures, not official-server presets.

```sh
cargo build --release --locked --example perf_client_retained -j 2
```

Copy the executable to `outputs/retained-client/bin/` in a new owned output
directory and record its SHA-256, actual runtime/harness source, toolchain,
platform and build command before use.
The embedded source label identifies fixture construction history only. The
caller must pin the actual binary and runtime separately.

```sh
./outputs/retained-client/bin/perf_client_retained step_all 384
./outputs/retained-client/bin/perf_client_retained next_frame 384
```

On Windows, use `./outputs/retained-client/bin/perf_client_retained.exe`.

Each invocation creates a fresh world. `--iterations N` permits one to eight
worlds with the same seed. `step_all` supplies controls for every playing slot;
`next_frame` supplies slot-zero controls and uses the actual built-in background
seekers. The modes produce different worlds and do different work. Compare a
given mode only against the same mode, configuration, seed and decision count.
There are no sockets, listeners, `BotRunner` or measured-compute feedback into
virtual time.

After every decision and lifecycle checkpoint, the harness drains records for
all eight slots, applies them to persistent independent public decoders,
materializes their frames, and checks equality with backend frames. Wire and
coverage counters retain array order and distinguish each connection generation.
Reports include actual living populations, food and available anchors, packet
codes, full/delta maps, leaderboards and reconnects. False coverage flags mean a
case did not occur; they do not establish an error or permission to change policy.
Sector-clear packets witness exits without revealing their private expiry reason.
Counts from available observations do not treat dead/waiting frames as empty worlds.

Wall intervals separately report backend operations, external action submission,
additional backend frames, record draining, replay application, replay frames and
manual reconnect. They exclude control calculation, equality checks, counters,
serialization and returned-output destruction. Process CPU/wall include those
costs. Internal physics/encoding/delivery/decoding remain combined inside the
operation; `next_frame` also includes background-seeker frame construction.
Agree a quiet interval before interpreting timing, retain every raw sample, and
report process CPU and peak memory separately from these inner wall intervals.

`--trace` requires one iteration and disables all measurement intervals. It emits
full available core state (including serialized core RNG/clocks), latest-tick
events, direct observations, ordered actions, frames and wire records. Only JSON
object keys and the internal unordered `prey_outward` set are canonicalized.
Full traces can be large; start with a few decisions, retain outputs and bound
execution when expanding. Use identical harness bytes and complete-byte
comparisons when checking a runtime optimization.

Private backend RNG, transport queues, subscription/presentation state and the
five intervening ticks' event buffers are inaccessible through these public
seams. The trace therefore does not establish full private-backend-state parity.
It does retain sent background commands rather than substituting another seeker.
The fixture does not stage 128-anchor bodies or assert that `V`, death or sector
expiration occurs; inspect the actual coverage report before claiming any of them.

The initial parent validation passed the complete Rust check group (default and
core-only all-target tests, doctests, Clippy with warnings denied, formatting and
basic example), the release build, and three-decision untimed smokes in both
modes. Each smoke checked 32 public replay frames. Independent read-only source
review found no issues and did not execute checks. Longer coverage and timing
results are recorded separately when measured.
