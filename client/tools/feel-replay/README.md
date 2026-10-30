# Offline feel replay contract (v2)

Schedule/trace v2 adds explicit retained-own expiry and a declared `between-lives`
state. Manifests and raw replay-input tapes remain schema 1. V1 artifacts remain immutable;
read them with their pinned earlier revision, without rewriting or auto-upgrading
them. The bit-preserving file encoding is unchanged.

This pure Node contract supports later Canvas/WebGPU (A2) and native CPU (A3)
peers. It does not launch peers, build artifacts, read a capture format, schedule
real timers, connect to a server or establish gameplay/rendering/latency parity.
`synthetic.mjs` is authored synthetic data, including its placeholder hashes.
Never replace it with private recordings. Historical tapes and tools stay intact.

The [browser peers](browser-peers.md) now drive the production Canvas runtime and
WebGPU LocalGameplay offline. The contract/comparator below remains pure Node.
The [native CPU peer](native-peer.md) drives production LocalGame and compares
against the same frozen Canvas Wasm schedule in cross-target mode.

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

The [native Rust example](native-peer.md) supports an explicit subset of general
A1 documents: every object key must be ASCII and must not be a canonical array
index (decimal `0` through `4294967294`, without leading zeros). Both reading and
writing reject unsupported keys recursively. This avoids JS's numeric ordering
of index keys and UTF-16 versus Rust ordering of non-ASCII keys. String values
may contain any Unicode scalar values; lone UTF-16 surrogates accepted by the JS
reader fail native decoding and cannot inhabit the writer's Rust strings. The
subset restriction changes no A1 schema or policy and does not normalize keys or
strings. Admitted documents retain byte-compatible output.

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
ordinals start at zero and are consecutive. Non-draw references may precede their
draw, but every referenced ordinal must be realized by a draw in the schedule.
The last boundary must name the final
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
- `final`: duration, exact event/command/sample/acknowledged-draw counts and final boundary ID,
  completion, pending effect/frame counts and unexpected network status.

Draw IDs number observed preparation boundaries, including recoverably skipped
pictures. A skipped core projection remains `prepared: true` with `submitted`
and `committed` false; those flags and `quality.successfulDraws` require a picture
acknowledgement. `final.draws` counts acknowledged draw samples, rather than all
draw observations. These existing v2 fields need no new status value or shape.

Only explicitly scheduled both-peer `pre-appearance` boundaries may initially
have `own: null`; that exception cannot resume after appearance. A scheduled
`between-lives` interval also permits null, but only after an observed expired
own, and the next own must have a token never previously retired or expired in
the trace. Completion while still between lives is invalid; v2 has no declared
end-between-lives schedule exception. Missing own elsewhere,
missing fields/times, duplicate IDs, wrong order/counts, unfinished work,
unexpected network or send/present failures fail completeness before comparison.
Do not resample or insert draws to obtain an observation.

An expired retained own remains an `own` object with raw `deathProgress >= 1`
and `expired: true`. The production shared draw preparation removes it from the
draw collection; Canvas and WebGPU therefore skip its head/face instead of
clamping death progress. Trace v2 records `projected: null` for that skip, retaining
all stored head/pupil/correction channels. Only expired own may have null
projection, expired must agree with death progress, and an expired token cannot
revive. Repeated observations of the same retained expired own remain valid.
Replacing an own retires its token even if it never expired; retired tokens
cannot return in any state after later lives. Live/fading own still requires
the complete projected geometry.
Unsupported face families continue to fail closed, including after expiry.

## Match criteria

The comparator policy is **`feel-cross-target-v2`**. Schedule/trace v2, manifest
v1 and `feel-f64-v1` schema/encoding bytes are unchanged: this revision changes
only cross-target comparison rules for existing ring floats. Reports identify
their policy version and `exactOnly` setting. Historical reports retain their
pinned tool revision; do not rewrite them under the new policy.

`compareTraces(left, right, schedule, { mode })` defaults to `same-core`, which
accepts two Wasm peers or repeats of the same native peer. All numbers use
`Object.is`, including signed zero; command bytes, timing/order, discrete state
and rings remain exact. Native-versus-Wasm requires `cross-target` explicitly.

Cross-target still reports every non-exact float with both bit patterns, signed
delta and whether it was tolerated. Numerical acceptance requires identical,
predeclared `numericalPolicy` in both manifests, with an identified platform-math
`cause`. Maximum absolute caps are `worldCap: 1e-9` for head/camera world positions,
`angleCap: 1e-9` radians for heading/gaze, `zoomCap: 1e-12` for zoom, and
`f32Ulps: 1` for current entity offsets and f32 ring storage. Camera offsets and
camera ring elements remain f64 world values, bounded by `worldCap`.
These are limits, not adjustable CLI tolerances.
Smaller caps are allowed. f32 offset/ring observations must already be exactly f32
representable; the comparator never rounds a double to make it pass. Signed-zero
differences, ring indices/pending counts, branch choices, speed/size/delta, pixels,
commands and all times stay exact. Non-exact values without a declared cause fail.

Ring floats can retain platform-transcendental rounding from their construction
and later injections. Requiring exact cross-target ring storage would reject
these values while permitting the corresponding current offset; it would also
tie native behavior to a particular browser/runtime's math implementation.
Cross-target therefore applies the existing storage-appropriate caps to every
head/angle/length f32 ring element and camera f64 ring element. This changes no
production math and adds no tolerance in **same-core** mode: Canvas/WebGPU Wasm
comparisons and same-native repeats still require bit-exact rings and all other
numbers, even when their manifests declare a numerical policy.

Cause taxonomy uses `host-transcendental` for target/runtime math differences,
such as native platform libm `f64::cos` versus browser `Math.cos` when constructing
camera smoothing weights. Identify the actual functions, targets, browser/tool
versions and an independent reproduction alongside the predeclared policy;
`Math.sin`, `Math.pow` and `Math.atan2` use the same Wasm/native dispatch boundary.
The cause string classifies evidence, rather than proving it or granting a new
cap. Authored synthetic perturbations must be labelled `SYNTHETIC` and cannot
substantiate a measured cause. No generic unexplained-drift allowance exists.

As a conservative accumulation guard, three nonzero same-sign growing error
levels in one channel's ordered sample sequence fail even within its individual
cap. Equal-magnitude plateaus retain the growth count without incrementing it,
including interleaved update/draw observations. A sign change, strict decrease
or exact observation resets that channel. This guard screens a declared
numerical result; it cannot prove a platform-math explanation scientifically.
The owner must substantiate that cause with independent evidence. A numerical
match is always separate from exact parity and from any perceptual/visual claim.
Each physical ring element has its own accumulation history in sample order;
moving ring cursors do not pool elements or reset that history. Array shape,
indices and pending counts remain exact. `compareTraces(..., { mode:
'cross-target', exactOnly: true })` produces a separate exact-parity report from
the same traces and declarations, disabling all numerical allowances without
rewriting either manifest.
