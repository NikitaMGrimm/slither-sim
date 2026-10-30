# Offline feel replay contract (v1)

This pure Node contract supports later Canvas/WebGPU (A2) and native CPU (A3)
peers. It does not launch peers, build artifacts, read a capture format, schedule
real timers, connect to a server or establish gameplay/rendering/latency parity.
`synthetic.mjs` is authored synthetic data, including its placeholder hashes.
Never replace it with private recordings. Historical tapes and tools stay intact.

Run from `client/`:

```sh
node --test tools/feel-replay.test.mjs
node tools/feel-replay.mjs /private/run/schedule.json /private/run/canvas.json /private/run/wgpu.json
node tools/feel-replay.mjs /private/run/schedule.json /private/run/canvas.json /private/run/native.json cross-target
```

The CLI prints a complete bit-preserving report to stdout, and exits 0 only for
`exact-match` or explicitly labelled `numerical-match`. Invalid schemas exit 1
with a field path on stderr; comparison failures exit 1 with all differences in
the report. No mismatches are sampled or capped. Output files, input hashes and
private paths belong in a fresh private run directory outside Git.

## Writing and reading files

`schema.mjs` exports the closed, required-field schemas and validators. Unknown
fields fail, so packet bodies cannot accidentally enter a trace as extra fields.
All times are finite, nonnegative milliseconds relative to the same run origin.
All IDs are semantic IDs, independent of a host's object addresses. Event
coordinates are CSS pixels in the declared peer target; `null` means no pointer
coordinate/code for that event, never an absent observation. Effect IDs must
include their generation to remain unique for the complete run.

`io.mjs` exports `stringify`/`parse`, `readSchedule`, `readTrace`, `scheduleHash`,
`sha256` and `buildManifest`. Use `stringify` on the original numeric observations
BEFORE any ordinary JSON serialization. It sorts object keys and encodes every
number (including IDs and counts) as `{ "$f64": "8000000000000000" }` for `-0`,
with big-endian IEEE 754 binary64 bits and a `feel-f64-v1` document wrapper.
The reader rejects bare JSON numbers, invalid encodings and non-finite numbers.
Rust peers can emit this same format directly. A normal JSON round trip before
this writer irreversibly destroys signed-zero evidence.

Build a manifest with `buildManifest({ schedule, artifactFiles, inputFile,
...fields })`: `artifactFiles` maps semantic artifact names to existing files;
the optional `inputFile` hashes private bytes without copying or parsing them.
`fields` supplies the manifest fields shown in `synthetic.mjs` except artifacts,
schema and schedule hash. With no input file, supply an input label and either
its known SHA-256 or `null`. The builder computes SHA-256 over exact artifact/input
bytes and the canonical bit-preserving schedule serialization. It never invokes
Git or builds; the caller must verify product commit/tree and shared core identity.
List all relevant prebuilt artifact hashes and tool versions; an artifact name
is a label rather than a filesystem path. Seed AND request-tagged random-stream
hash bind entropy: the same seed alone is insufficient. Record ordered startup,
initial geometry/quality and GPU/present settings (`null` where inapplicable).
Product commit/tree/core, input, schedule, entropy, startup, initial geometry,
quality and numerical policy must agree. Same-peer repeats also require identical
artifact/tool/present declarations; different peers retain their own declarations.

## Schedule and observations

Freeze the schedule independently before running either peer. It contains ordered
`events` with unique source IDs, kind and scheduled time, ordered `boundaries`,
the expected command count, label and duration. `orderEvents` returns a new array
with stable ties ordered packet, input, frame; equal-time events of one kind
retain source order. Validation rejects an unordered schedule. Payloads remain
in the separately hashed private input, with unsplit packet envelopes retained
by A2/A3; this tool neither splits nor transforms packet data.

Each expected boundary names its source event ordinal, phase, frame, optional draw
ordinal, virtual time and eligibility. Enumerate every update/draw and lifecycle
boundary being compared, including startup, Setup, resize and retry draws. Draw
ordinals start at zero and are consecutive. The last boundary must name the final
event and occur at the declared duration. This explicit expectation is essential:
two truncated peers must not pass by sharing the same missing tail. A future
adapter that cannot predict these boundaries needs a separately frozen expected
history; it must not derive expectations from the candidate trace after execution.

Trace arrays contain:

- `events`: sequence/source ID, kind, scheduled/actual times, observed target/type,
  CSS coordinates/key code, trusted/repeat/buttons, focus/visibility and effect IDs.
- `commands`: global sequence, source event/frame/effect IDs, safe gameplay control
  bytes and exact decision/send/ack times. Protocol >=5 one/two-byte gameplay
  controls are allowed. Login, handshake and packet bodies are excluded; their
  lifecycle is represented by semantic event/effect IDs. Legacy wire controls
  require a future explicit contract revision.
- `samples`: boundary identity, raw/lagged delta and whole/doubled steps, lag/ping,
  own token, raw/corrected head/heading, gaze/pupils, f32 offsets, speed/visual
  speed/boost, size and complete correction rings, camera state, projected head
  and pupils in backing/CSS pixels, quality, CSS/backing/surface/DPR geometry and
  prepare/submit/commit/retry status. Ring `values` are complete flattened stored
  channels in channel-major order; indices and pending counts are separate.
  Head storage has 3 x 53 f32 values, angle has 26, length has 53; pending
  counts admit the core's `-1` cleanup sentinel. The camera ring has 2 x 62 f64
  values and nonnegative pending counts (see `src/presentation.rs`,
  `src/presentation/length.rs` and `src/presentation/camera.rs`).
- `final`: duration, exact event/command/sample/draw counts and final boundary ID,
  completion, pending effect/frame counts and unexpected network status.

Only explicitly scheduled both-peer `pre-appearance` boundaries may have `own:
null`; that exception cannot resume after appearance. Missing own elsewhere,
missing fields/times, duplicate IDs, wrong order/counts, unfinished work,
unexpected network or send/present failures fail completeness before comparison.
Do not resample or insert draws to obtain an observation.

## Match criteria

`compareTraces(left, right, schedule, { mode })` defaults to `same-core`, which
accepts two Wasm peers or repeats of the same native peer. All numbers use
`Object.is`, including signed zero; command bytes, timing/order, discrete state
and rings remain exact. Native-versus-Wasm requires `cross-target` explicitly.

Cross-target still reports every non-exact float with both bit patterns, signed
delta and whether it was tolerated. Numerical acceptance requires identical,
predeclared `numericalPolicy` in both manifests, with an identified platform-math
`cause`. Maximum absolute caps are `worldCap: 1e-9` for head/camera world positions,
`angleCap: 1e-9` radians for heading/gaze, `zoomCap: 1e-12` for zoom, and
`f32Ulps: 1` for current entity offsets. Camera offsets remain f64 world values.
These are limits, not adjustable CLI tolerances.
Smaller caps are allowed. f32 offset observations must already be exactly f32
representable; the comparator never rounds a double to make it pass. Signed-zero
differences, ring storage, branch choices, speed/size/delta, pixels, commands and
all times stay exact. Non-exact values without a declared cause fail.

As a conservative accumulation guard, three consecutive nonzero same-sign,
strictly growing errors in one sample channel fail even within its individual
cap; an exact observation resets that channel. This guard screens a declared
numerical result; it cannot prove a platform-math explanation scientifically.
The owner must substantiate that cause with independent evidence. A numerical
match is always separate from exact parity and from any perceptual/visual claim.
