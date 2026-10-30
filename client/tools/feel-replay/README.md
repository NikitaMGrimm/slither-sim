# Offline feel replay contract (v3)

Schedule/trace v3 and manifest v2 use ordinary finite-number JSON and one
`feel-perceptual-v1` comparator for Canvas, WebGPU, native, desktop and different
source revisions. [Perceptual parity](../../docs/perceptual-parity.md) defines the
budgets. Passing this CPU observation gate does not establish owner acceptance,
pixel parity, GPU completion, physical input or display latency.

Old v1/v2 evidence remains immutable and readable at its pinned old revision.
There is no compatibility reader or automatic upgrade. Raw packet envelopes stay
in separately hashed private schema-1 tapes; never put them in traces or Git.
`synthetic.mjs` and the browser fixtures are authored synthetic inputs.

Run from `client/`:

```sh
node --test tools/feel-replay*.test.mjs tools/feel-browser-replay.test.mjs tools/feel-native-replay.test.mjs tools/feel-desktop-replay.test.mjs
node tools/feel-replay.mjs /private/run/schedule.json /private/run/baseline.json /private/run/candidate.json
```

The CLI emits plain JSON and exits zero only for `perceptual-match`. Invalid
schemas fail with field paths; every comparison violation remains in the report.

## Files and validation

`io.mjs` exports `parse/stringify`, readers, SHA-256 and `buildManifest`.
Numbers must be finite. IDs/counts must be safe nonnegative integers; byte values
stay in range. Decimal u64 identities stay strings. Missing/unknown fields,
invalid lifecycle, failed sends, pending work and incomplete tails fail closed.
Signed zero has no separate encoding. Object keys have no ASCII/index restriction.
Rust uses serde_json with `float_roundtrip`; cadence-edge inputs round-trip in
both runtimes without a custom codec or serializer-identity gate.

Each trace has one small embedded manifest: source commit/staged tree, checksums
of generated bindings or the executable, tape/schedule hashes, peer geometry,
policy and completion. Source/artifact identities may differ between operands.
Each live run checks its own artifacts against its own staged source, before and
after replay; hashes are provenance, not perceptual proof. A completed valid
baseline trace can be consumed without an A2 summary or a special trace hash.
This does not authenticate a hand-edited trace as a capture.

Freeze the schedule before executing peers. It retains ordered unique source IDs,
packet/input/frame tie order, boundary IDs, virtual clocks, expected command
count, expected acknowledged draw IDs, eligibility and a required end sentinel at the final event/duration.
No missing/extra/reordered event, sample, update or draw is admitted, even if both
peers share the same truncation. No alignment, trimming or resampling is allowed.

## Observations and timing

Events retain admitted input coordinates/keys/buttons, semantic targets, effects
and scheduled/actual receipt clocks. Commands retain gameplay bytes, decision,
transport-send and acknowledgement clocks, send success and source event/frame IDs.
Login/handshake bytes are excluded. Samples retain raw/predicted head, headings,
prepared body centerline in CSS pixels, pupil positions, offsets, speed/size, spawn/death/boost
phases, body opacity, camera, zoom, quality, geometry and preparation status.
The body channel copies the actual own-body painter samples at draw preparation,
in original order, including every prepared point. The trace limit is 8,193
points per own-body draw; larger outputs mark the measuring run incomplete and
emit an explicit recorder error rather than omitting points. The production
65,536-sample rendering limit is unchanged. The flat ABI projects prepared world
positions to backing pixels; both decoders convert to CSS pixels. Updates,
culled/expired bodies and preparations without a body carry an empty list.
Anchors are not recorded, and no spatial subsampling is performed.
Production correction rings remain private; their cells, cursors and pending
counters are absent from this schema and both flat-ABI decoders.

Every preparation is retained, including Setup, resize, retry and recoverable
memory skips. Recorder identities bind acknowledgements to the original prepared
row; no resampling identifies a picture. `acknowledgedMs` is null until an actual
host receipt, independently of preparation `atMs`. Submitted/committed flags,
per-sample `quality.successfulDraws` and final `draws` count acknowledged pictures
only. Frozen schedule picture IDs check expected acknowledgements even if both peers
share a dropped receipt. Optional external receipt draw IDs authenticate these flags during browser
assembly; standalone files can establish only their internal consistency.

Packet receipt is the event clock; its update sample is core application. Desktop
sidecars retain transport/service and delayed picture lifecycle clocks. These
IDs/clocks preserve future input→send and packet→picture-ack measurement seams.
Synthetic clocks cannot establish physical latency; unrelated host clocks must
never be subtracted. CPU acknowledgement is neither GPU completion nor display.

## Comparator

Exact channels include event/admission semantics, command bytes/order/boost and
all command times (0 virtual ms), lifecycle/identity, packet-derived speed/mass/
segment decisions, requested quality, schedule and successful-picture counts.

Vectors use Euclidean CSS-pixel distance. Raw/corrected head, pupil and
camera diagnostics convert using declared zoom/backing geometry; prepared body
points are already CSS pixels. Camera and
projected head are independent channels. Positions use p95 ≤0.5 px/max ≤1 px;
headings use shortest wrapped differences with p95 ≤0.25°/max ≤0.5°.
Zero CSS/backing extents remain valid trace observations (for example minimized
windows), but positional comparisons require positive width and height in both
spaces. A zero extent makes the comparison invalid even when both peers have
identical geometry; no positional match is inferred for an undefined projection.
Surface-only zero extents do not affect these conversions. NaN or infinite
arithmetic errors saturate to a finite failing magnitude, never an accepted zero.
Replay zoom must be finite and within (0, 16], a generous instrumentation ceiling
above production camera targets. Together with positive safe-integer extents,
this bounds the projection scale. Per-axis/norm overflows still fail. Report means
and bias windows sum normalized components before restoring scale, so even extreme
finite failures remain valid, serializable JSON.
Zoom must pass absolute 0.0005, relative 0.001 and diagonal viewport-edge 1 px
limits independently. Spawn/death/boost phase and body opacity use p95 0.01/max
0.02; hidden/fully-shown endpoints and visibility remain exact. Spawn/death phase
also has an equivalent animation-time limit of one scheduled frame capped at
20 ms, using production rates at the same frozen clock. This does not shift
samples or loosen semantic enable/focus/send/hold decisions. Menu/text/antenna
images are outside these currently observed channels and need their own policy
evidence; this trace does not claim to measure them.

Every numeric channel reports count, unit, max, nearest-rank p95, RMS, signed mean,
thresholds, 500 ms sustained-bias flag and quarter RMS. Vector means retain both
signed components. Bias above half the hard maximum for at least 500 ms is a
flag. Last-quarter RMS minus first-quarter RMS above half the maximum fails when
both quarters have at least 10 eligible draws. Each metric's limits are evaluated
independently. Reports retain all violations and the first failing boundary. Exact endpoint
violations are attributed to their parent numeric channel as well.

See [browser peers](browser-peers.md) and [native/desktop peers](native-peer.md)
for production paths and executable commands. No exact-only report, platform-math
cause declaration, ULP cap, per-ring growth gate or mandatory repeat remains.
