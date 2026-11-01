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
The historical table reports lower-median/p95; the corrected capture uses the
conventional average of the middle two observations. Both report nearest-rank
p95 and the range of per-round medians, not a speedup claim.
Populations can use different detail policies: the 512-ID cell exceeds the old
live bound; both maximum-length cells exceed the old aggregate body bound.
Allocation counts requested alloc/zeroed/realloc bytes on the calling Rust thread,
excluding resident heap, deallocation, allocator overhead and GPU/driver memory.
The timing executable enables `feel-trace` with no recorder installed.

Builds and timing are separate. Before the approved quiet timing interval, wait
for all cargo/rustc processes without stopping them; the corrected window also
checks tar/zstd and monitors compiler/archiver absence throughout the run.
Preserve interrupted/overlapping attempts rather than counting
their fast or partial rows.

## Corrected results (15:45 quiet window)

The corrected benchmark binary is bound to shared-core/planner source `8b800bfd`,
rebased onto `d716697b` including #411 and #417. The later strict loopback probe
at `fe081b6a` is a separate executable and is not linked into this benchmark.
All 450 measured pictures completed across four cells and five alternating
rounds per cell, with every population present, a body for every snake, and no
draw/pixel quota failure.

The approved interval was 15:45–15:50 local on 2026-11-01. Window entry was
15:45:16.632; the initial process check took 0.031 seconds and found no blockers.
The capture ran **15:45:16.663–15:45:50.711 local** (34.05 seconds), starting
16.663 seconds after window opening. All **94** process-presence checks found
no cargo, rustc, tar, bsdtar, zstd, zstdmt or pzstd overlap. No process was stopped.
System CPU load across the whole harness interval was **24% median / 47% peak**
(25 samples); this is a compiler/archiver-free capture, not an idle-host claim.
Hardware/profile remain the Windows x86-64 Ryzen 7 9800X3D and Rust 1.99
release/thin-LTO setup also used by the historical capture.

These are **elapsed wall-planning** times, not thread CPU time or gameplay FPS.
The median averages the middle pair for even-sized cells; p95 uses nearest rank.
Allocation is requested calling-thread alloc/zeroed/realloc bytes per warm
picture, not resident or peak heap. The draw-staging full-client copy and private
sampling are included; ingress transaction copies and the other exclusions in
the method remain outside the bracket.

| Live snakes | Anchors/snake | Elapsed median / p95 ms | Round-median range ms | Requested MiB/frame | Draws | Measured frames |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 256 | 26 | 4.03 / 5.42 | 3.90–4.14 | 12.17 | 24,467 | 150 |
| 512 | 26 | 12.08 / 16.14 | 10.73–13.73 | 26.98 | 60,487 | 150 |
| 256 | 4096 | 41.90 / 46.53 | 39.02–44.41 | 116.79 | 104,913 | 75 |
| 512 | 4096 | 64.32 / 76.83 | 60.06–67.91 | 193.99 | 105,477 | 75 |

Maximum-length 512-snake pictures retained all 2,097,152 received anchors.
Visual-anchor payload peaked at **31,433,280 bytes**, within the 31,457,280-byte
pressure quota. The current draw maximum was **105,477**, below 262,144; picture
pixels peaked at **1,936,988 bytes**, below 64 MiB. These results demonstrate
bounded completion of the measured frozen worlds, not a low-latency guarantee
for ingress or an optimization claim. The historical and corrected tables are
not a controlled before/after comparison: planner geometry, shadow policy and
background conditions differ.

[Corrected selected samples](review-selected.csv), [summary](review-summary.json),
[source/binary provenance](review-source-provenance.json), and
[process-free receipt](review-timing-receipt.json) bind this capture separately.
Full raw CSV, stderr, process/wait samples and CPU-load samples remain outside Git.
The earlier 11:10 capture remains unchanged and explicitly historical below.

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
Battledome bots on an OS-assigned loopback port. The historical 20-second probe
(wheel/acknowledgement fixes applied, before the camera endpoint fix)
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

The [current quota audit and policy](https://github.com/NikitaMGrimm/slither-sim/blob/8b800bfd8a1c64ab65aa7f5d83f98b3bbd5255f3/client/docs/snake-capacity.md)
include the corrections below and the unchanged single presentation envelope.

The corrected compact representation assigns each retained anchor its incoming
logical interval count. Non-growth and tail removal consume one interval, not
an entire folded span. A reduced polyline sampler has no terminal extrapolation;
ingress pins the remaining endpoint to the received tail. The own body keeps
its complete legacy geometry and correction records. An exhausted reduced
body can accept a subsequent absolute growth anchor without requiring retired
visual ghosts. Seeded straight motion,
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
The [review controls](review-controls.json) bind the updated binaries, replay,
allocation, accounting and loopback results separately from the old timings.
The baseline and host were built from the retained `d716697b` archive. Earlier
JSON labels accidentally followed a later `origin/main` ref; schema 2 corrects
those metadata labels while retaining the superseded values and binary hashes.

Default stress fixtures separately cover 512 live IDs and a 17 × 4,096-anchor
world crossing the same pressure threshold. The latter sends 80 arrivals across
the 64-envelope deferred limit. Exhaustive 512 × 4,096 cases remain explicit
ignored variants. The default native capacity binary completes in **5.26–6.14 s**
in local Windows single-threaded debug runs (2.70 s with parallel tests).
The affected presentation-client, state and new selector binaries take **0.12 s**,
**0.10 s** and **0.00 s** respectively. The longer release compaction rules take
**0.02 s**. These are test-suite durations, not frame-time captures.

The corrected capture above was collected in the approved 15:45 window. The
11:10 historical capture predates the reported 13:40 disk-full incident; no
capture was taken during that incident.

## Reproduction

Corrected/rebased validation: **`npm run check` passed** on Windows, including
core/native tests and Clippy, real Canvas Wasm smoke, desktop compilation,
#411's feature-gated observer/shadow controls and WebGPU Wasm Clippy/build.
The final exhaustion edge fix additionally passed focused core/body/selector
tests, core Clippy, rebuilt Canvas smoke, release seeded compaction rules,
both exhaustive native cases, the fast native capacity suite, and fresh native
feel/allocation/loopback controls. The explicit real-Wasm 512 × 4,096 case passed.
The later strict loopback collector additionally passed its two Rust rules,
seven Node rules, strict bin Clippy, and the new wide/default-vision runs.
Root `uv run --locked --only-group tools python tools/test_documentation.py`
passed all seven tests. Hardware-GPU tests remain ignored; no desktop/browser
or outside game server was used.

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
node tools/e2e-local-host.mjs --host-bin <slither-sim> --client-bin <slither-client-e2e> --battledome 500 --seconds 20 --scratch <outside-repository-directory>
```

It binds only `127.0.0.1:0`, excludes reserved ports, requests full zoom-out and
measures every acknowledged post-wheel committed transform. It requires at least
one fit sample and worst overflow at most 0.000001 logical pixels, no picture skips,
no session failure, at least 500
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
Client wheel zoom is local and sends no subscription change. Untimed
same-fixture controls at `--vision 1500` reached only **5–8** received/prepared
snakes and the population assertion correctly rejected them. A fitted arena
transform does not establish 500-player delivery. This explains the handful-sized census in #411 without
attributing server subscriptions to the client cap. The simulator lane must
choose a whole-map interest policy for an ordinary host; this e2e opts into
40,000-unit coverage explicitly.

The strict post-wheel probe is a **different binary and run** from historical
attempt 4's 3.5604467877800516-pixel failure. After the camera endpoint fix, the
20-second wide-vision run reached **501 received / 501 prepared**, with zero skips
and a clean close. All **40** acknowledged post-wheel pictures fit: best overflow
0, worst **1.1368683772161603e-13 logical pixels**, within the 1e-6 tolerance. This
is headless transform acceptance for that run, not hardware-pixel evidence.

The new eight-second default-vision control measured **440** post-wheel pictures,
best/worst overflow **0/0**, but reached only **5** snakes and therefore failed
500-player acceptance. Earlier best-only probes could report `fullMap=true` after
one fitted picture and default an unmeasured best overflow to zero. They retained
neither post-wheel sample counts nor worst overflow; schema 2 marks their
continuous-fit acceptance **not measured**. Population/close failures remain
failed controls even when their raw best-only fields looked fitted. Their raw
records remain bound to the earlier published revision.

Battledome defaults to a 20-second client session after the existing fill delay.
The earlier eight-second wide run reached only 411; a longer low-vision run failed
close with `Socket is not open`. Both remain retained failures. The latest
collector rejects a late overflow even if an earlier or later picture fits, and
serializes absent fit measurements as `null`, not zero.
