# Bounded 512-snake client capacity (2026-11-01)

Authored synthetic capacity validation and headless elapsed planning measurements.
No outside server, private reference, external artwork, browser, window or GPU
submission is part of the synthetic workload. The optional loopback cross-check
is separate from the frozen-world planning workload.

Base: `3669ac9a` (current main at task start). The
[quota audit and presentation policy](https://github.com/NikitaMGrimm/slither-sim/blob/a0a4579e3cdcd51972be5c4f58e9d7cde383d6d9/client/docs/snake-capacity.md)
describe the original capacity implementation. The review corrections below
describe the revised behavior and its separate validation.

## Question and method

Does raising received admission from 256 to 512 permit complete full-map pictures
without multiplying corrected visual storage with received-anchor capacity?
What elapsed planning time and requested allocation do 256/512-snake full-map scenes
cost at a 26-anchor Battledome-like length and at the existing 4,096-anchor
client retention ceiling?

The authored fixture places circular bodies inside a radius-15,000 arena, with
empty names and shared procedural placeholder materials. It includes exactly the
reported live-ID count, including own. This is a retained whole-map workload,
not a server subscription or a reconstruction of official-game rules. Logical
body length and transmitted anchor count stay intact when rival visuals compact.
Every reported picture must prepare every live snake with body commands, remain
inside draw/pixel limits and return no error.

`snake_capacity --measure` uses the production shared planner and HUD finisher at
1920×1080, logical scale one, without a renderer/device. Each cell settles the
production camera for 110 numeric draws at deliberately coarse sampling, then
uses normal applied quality for all warm and measured pictures. Source
materialization and validation complete inside the measured planner/HUD result.
GPU serialization/submission, simulation, networking, ingress, animation updates,
display pacing, output serialization and retired-picture destruction are outside
the elapsed bracket. The world is otherwise frozen.

The HUD input is the frozen ingress view before its first scored readout, not a
timed `LocalGame::begin_frame` lifecycle. In particular, the 4,096-anchor own
appearance exceeds this workload's `N=430` score-table domain. The separate full
session maximum-length regression advertises `N=4096`; it covers HUD binding and
frame completion without changing these pinned planning measurements.

Five rounds alternate population order. The 26-anchor cells have 10 warm + 30
measured pictures per round; maximum-length cells have 5 warm + 15 measured.
Report pooled lower-median/p95 and the range of per-round medians, not a speedup claim.
Populations can use different detail policies: the 512-ID cell exceeds the old
live bound; both maximum-length cells exceed the old aggregate body bound.
Allocation counts requested alloc/zeroed/realloc bytes on the calling Rust thread,
excluding resident heap, deallocation, allocator overhead and GPU/driver memory.
The timing executable enables `feel-trace` with no recorder installed.

Builds and timing are separate. Before the approved quiet timing interval, wait
for all cargo/rustc processes without stopping them; monitor their absence during
the retained run. Preserve interrupted/overlapping attempts rather than counting
their fast or partial rows.

## Historical results (before rebase/review corrections)

These timings describe the pinned 11:10 artifact, not the corrected, rebased
implementation. `Instant::elapsed` measures wall time; it is not thread CPU time.
For even-sized cells the reported value is the lower middle observation.
The selected CSV renames only that clock column; numeric observations are unchanged.

All four cells completed; no rejected picture, missing body or compiler overlap.
The retained interval was 11:10:21–11:10:47 local, Windows x86-64 on an AMD Ryzen
7 9800X3D (8 cores / 16 logical processors), pinned Rust 1.99 release/thin LTO.
This is one 26-second measurement attempt with five
alternating rounds per cell. System CPU load was 38% median and 54% peak
(18 samples): compiler-free does not mean idle. These are descriptive numbers
under that observed background load, not isolated-host or gameplay-FPS claims.

| Live snakes | Anchors/snake | Elapsed lower-median / p95 ms | Round-median range ms | Requested MiB/frame | Draws | Measured frames |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 256 | 26 | 4.86 / 6.19 | 4.77–5.13 | 12.85 | 46,739 | 150 |
| 512 | 26 | 9.94 / 12.47 | 9.54–11.13 | 26.99 | 60,574 | 150 |
| 256 | 4096 | 32.52 / 40.93 | 29.04–35.36 | 162.70 | 105,399 | 75 |
| 512 | 4096 | 47.03 / 60.70 | 44.96–52.19 | 284.43 | 106,483 | 75 |

Maximum-length 512-ID pictures retain 2,097,152 received anchors, while visual
anchor payload peaks at 31,433,280 bytes (29.98 MiB), below the 30 MiB pressured
ceiling. Picture pixels peak at 1,936,988 bytes; draws peak at 106,483, below
262,144. No maximum-population result is substituted with a 256-ID model.

The timed draw preparation includes its draw-staging full-client copy, including
received state, and private full-curve sampling before rival density is reduced.
The ingress transaction copies in presentation/client.rs and state.rs are outside
the measured bracket. No profiler attributes a particular cost fraction to them. This is bounded
capacity support, not an optimization of those copies or a low-latency claim for
4,096-anchor snakes. No CPU profiler or GPU timing was collected.

[Selected samples](selected.csv), [summary](summary.json),
[source/binary provenance](source-provenance.json), and
[compiler-free receipt](timing-receipt.json) bind this exact retained run.
Full raw outputs and failed attempts remain outside Git. Provenance preserves
the measured inputs and the pre-review post-measurement exceptions. The rebase
and review corrections subsequently changed native planning and camera behavior;
the historical samples must not be read as measurements of the corrected code.

## Historical functional controls

The [untimed loopback attempts](loopback-summary.json) used 500 configured
Battledome bots on an OS-assigned loopback port. The corrected 20-second probe
received **501 live snakes**, prepared up to 503 retained visual appearances,
recorded **zero picture skips** and closed cleanly. Its strict whole-circle fit
assertion still failed during camera motion (best excess 3.56 logical pixels),
so the command exited nonzero. This is partial loopback validation, not exact
whole-map acceptance. Earlier failures, including early wheel admission and
pre-acknowledgement camera observation, remain retained separately.

- Native session regression admits IDs 257, 501 and 512. ID 513 rejects the
  complete transaction, then the checked-error policy queues Close. Existing-ID
  replacement remains legal at the cap.
- Maximum-length native and real Canvas Wasm cases retain 2,097,152 received
  anchors and all 4,096 own anchors. Rival visuals fit fixed aggregate storage.
  The connected reduced planner obeys even zero optional draw allowance.
- Real Wasm standard Canvas submission resolves all 512 live IDs in a whole-map
  view below zoom 0.05, stays inside the pressure image pool and preserves own
  anchors. This uses an authored Canvas recorder, not browser or raster evidence.
- Native feel replay: **byte-identical**, 148 samples, 72 acknowledged pictures,
  seven commands and **zero `feel-perceptual-v1` mismatches**. This is native CPU
  observation/draw parity, not Canvas or GPU pixel parity.
- Identical-harness 256-ID full-map control: ordered Draw snapshot SHA-256
  `97cf577579ab1943614999a722532f03442fbf4457d5dd4f9b34a58f956047aa`
  matches byte for byte; cold requested allocation matches at **175,037,594
  bytes** and 46,739 draws. This cold functional control is separate from the
  warm timing table. Baseline core/renderer source stays at the base revision;
  only the identical authored workload driver/fixture are added.
- Scalar relationships and the working-storage envelope have executable
  assertions. These bound requested storage, not measured RSS.

## Review corrections and current controls

The corrected compact representation assigns each retained anchor its incoming
logical interval count. Non-growth and tail removal consume one interval, not
an entire folded span. A reduced polyline sampler has no terminal extrapolation;
ingress pins the remaining endpoint to the received tail. The own body keeps
its complete legacy geometry and correction records. Seeded straight motion,
growth, removal and fade sequences require extent/length error at most 0.02 world
units (including Float32 rounding near 60,000). Smooth curved sequences require
the same bounds epsilon and at most 10% chord-length loss. Sharp bends can lose
more arc length through visual simplification; received anchors remain complete.

Canvas and WebGPU now call the same food/prey pressure selector. Visibility,
including independently visible interpolated prey glows, precedes quota
selection. Below pressure it borrows the original slices. The own reservation
counts the ordinary standard painter conservatively; reduced rivals omit shadows
already, so #411's shadow savings are not deducted a second time. A pressured
own-body draw-count control equals the unpressured standard painter.

The whole-map fit deficit came from smoothing a moving camera-fit endpoint,
not aspect rounding or the zoom floor. The explicit endpoint now follows the
fit immediately; intermediate wheel positions retain interpolation. The rule
test covers moving cameras/radii, four aspect ratios and DPR 1, 1.25, 2 and 4.

The current accounting assertion is **1,579,766,632 <= 1,610,612,736 bytes**,
including bounded entity-selection and retained-polyline scratch. `BodyPoint`
stays 960 bytes. This remains a requested-payload bound, not measured RSS.

Against main `d716697b` (including #411 and #417), the authored native feel replay
is byte-identical: 148 samples, 72 pictures, seven commands and zero
`feel-perceptual-v1` mismatches. The 256-ID cold full-map allocation control
matches at **151,384,002 bytes**, 24,467 draws, with ordered Draw SHA-256
`ec0724478151f26df1797a2685a4e1fba0c7267553e9956b06da70c0fd3c1551`.
Main's shadow cut explains why these totals differ from the historical control.

Default stress fixtures separately cover 512 live IDs and a 17 × 4,096-anchor
world crossing the same pressure threshold. The latter sends 80 arrivals across
the 64-envelope deferred limit. Exhaustive 512 × 4,096 cases remain explicit
ignored variants. The default native capacity binary completes in **2.70 s**
in a local Windows debug run; core compaction rules take **0.01 s** and state
rules **0.09 s**. These are test-suite durations, not frame-time captures.

Corrected frame-time measurements require a new agreed quiet window. The
11:10 historical capture predates the reported 13:40 disk-full incident; no
corrected capture was taken during that incident.

## Reproduction

Pre-review local validation: **`npm run check` passed** on Windows with Rust 1.99,
Node 24 and wasm-bindgen 0.2.129. This includes repository/history guards, Node
tests, core tests/Clippy, real Wasm smoke, native tests/Clippy/desktop build, and
WebGPU Wasm Clippy/build. The native capacity suite passed all four rule tests;
the added maximum-length full-session/burst case also passed in release mode.
Hardware-GPU tests remained ignored. No outside game server was contacted.

The earlier full-check attempt was stopped by the compiler-free-window guard
before 12:00, then rerun to completion after 12:40. Resolved fixture, Clippy and
missing-tool attempts remain in task-local logs; none is counted as a successful
full check.

From the repository root, with a per-command target directory and two build jobs:

```text
cargo build --release --locked -p slither-wgpu-slice --features feel-trace --example snake_capacity --example feel-replay
cargo test --release --locked -p slither-wgpu-slice --features feel-trace --test snake_capacity
```

Outside timing, the retained executable accepts `--functional` (both nominal
populations) and `--functional-256` (matched legacy allocation/frame control).
`SNAKE_CAPACITY_SNAPSHOT` optionally writes the functional ordered Draw debug
representation. Neither functional mode reads a frame timer. Run `--measure`
only in the agreed quiet interval. Retain CSV, stderr, source/binary hashes and
the compiler-presence receipt. Failed runs and clipped subscriptions cannot
stand in for completed full-population pictures.

The loopback e2e uses prebuilt binaries, from `client/`:

```text
node tools/e2e-local-host.mjs --host-bin <slither-sim> --client-bin <slither-client-e2e> --battledome 500 --seconds 8 --scratch <outside-repository-directory>
```

It binds only `127.0.0.1:0`, excludes reserved ports, requests full zoom-out and
asserts a whole-map camera, no picture skips, no session failure, at least 500
received/prepared snakes and a clean requested close. Full
recordings and failed attempts stay outside Git.

The Battledome cross-check uses an explicit 40,000-unit snake/prey coverage radius
and 15 seconds of bot fill before login. The browser does not transmit its zoom;
this test configuration requests server coverage separately. Food sector interest
is unchanged. Actual admitted/received counts, not configured seats, determine
the strength of that cross-check.

The host's default snake/prey coverage is 1,500 units around the player's head
(`src/simulator.rs`, `src/client/simulation/broadcast.rs`). A 501-seat Battledome
scales its arena radius to roughly 14,181 units and spreads spawns across it.
Client wheel zoom is local and sends no subscription change. An untimed
same-fixture control at `--vision 1500` reached only **5** received/prepared
snakes despite `fullMap=true` and zero skips; the stricter population assertion
correctly rejects it. This explains the handful-sized census in #411 without
attributing server subscriptions to the client cap. The simulator lane must
choose a whole-map interest policy for an ordinary host; this e2e opts into
40,000-unit coverage explicitly.
