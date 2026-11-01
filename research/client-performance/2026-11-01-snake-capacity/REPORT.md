# Bounded 512-snake client capacity (2026-11-01)

Authored synthetic capacity validation and headless CPU planning measurements.
No outside server, private reference, external artwork, browser, window or GPU
submission is part of the synthetic workload. The optional loopback cross-check
is separate and does not establish subscription coverage of 500 bots.

Base: `3669ac9a` (current main at task start). The
[quota audit and presentation policy](../../../client/docs/snake-capacity.md)
own the current behavior; this report records this bounded validation.

## Question and method

Does raising received admission from 256 to 512 permit complete full-map pictures
without multiplying corrected visual storage with received-anchor capacity?
What CPU planning time and requested allocation do 256/512-snake full-map scenes
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
the CPU bracket. The world is otherwise frozen.

The HUD input is the frozen ingress view before its first scored readout, not a
timed `LocalGame::begin_frame` lifecycle. In particular, the 4,096-anchor own
appearance exceeds this workload's `N=430` score-table domain. The separate full
session maximum-length regression advertises `N=4096`; it covers HUD binding and
frame completion without changing these pinned planning measurements.

Five rounds alternate population order. The 26-anchor cells have 10 warm + 30
measured pictures per round; maximum-length cells have 5 warm + 15 measured.
Report pooled median/p95 and the range of per-round medians, not a speedup claim.
Populations can use different detail policies: the 512-ID cell exceeds the old
live bound; both maximum-length cells exceed the old aggregate body bound.
Allocation counts requested alloc/zeroed/realloc bytes on the calling Rust thread,
excluding resident heap, deallocation, allocator overhead and GPU/driver memory.
The timing executable enables `feel-trace` with no recorder installed.

Builds and timing are separate. Before the approved quiet timing interval, wait
for all cargo/rustc processes without stopping them; monitor their absence during
the retained run. Preserve interrupted/overlapping attempts rather than counting
their fast or partial rows.

## Results

All four cells completed; no rejected picture, missing body or compiler overlap.
The retained interval was 11:10:21–11:10:47 local, Windows x86-64 on an AMD Ryzen
7 9800X3D (8 cores / 16 logical processors), pinned Rust 1.99 release/thin LTO.
This is one 26-second measurement attempt with five
alternating rounds per cell. System CPU load was 38% median and 54% peak
(18 samples): compiler-free does not mean idle. These are descriptive numbers
under that observed background load, not isolated-host or gameplay-FPS claims.

| Live snakes | Anchors/snake | CPU median / p95 ms | Round-median range ms | Requested MiB/frame | Draws | Measured frames |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 256 | 26 | 4.86 / 6.19 | 4.77–5.13 | 12.85 | 46,739 | 150 |
| 512 | 26 | 9.94 / 12.47 | 9.54–11.13 | 26.99 | 60,574 | 150 |
| 256 | 4096 | 32.52 / 40.93 | 29.04–35.36 | 162.70 | 105,399 | 75 |
| 512 | 4096 | 47.03 / 60.70 | 44.96–52.19 | 284.43 | 106,483 | 75 |

Maximum-length 512-ID pictures retain 2,097,152 received anchors, while visual
anchor payload peaks at 31,433,280 bytes (29.98 MiB), below the 30 MiB pressured
ceiling. Picture pixels peak at 1,936,988 bytes; draws peak at 106,483, below
262,144. No maximum-population result is substituted with a 256-ID model.

The growing allocation/CPU cost includes transactional raw-state copies and
private full-curve sampling before rival density is reduced. This is bounded
capacity support, not an optimization of those copies or a low-latency claim for
4,096-anchor snakes. No CPU profiler or GPU timing was collected.

[Selected samples](selected.csv), [summary](summary.json),
[source/binary provenance](source-provenance.json), and
[compiler-free receipt](timing-receipt.json) bind this exact retained run.
Full raw outputs and failed attempts remain outside Git. After timing, `cfg(test)` memory-accounting/quota assertions, Canvas-only
Wasm preflight and the standalone loopback probe changed; provenance lists their
final hashes. Canvas is excluded from native builds, and the probe is separate
from the measured workload. Measured native planner and workload code are unchanged.

## Functional controls

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

## Reproduction

Final local validation: **`npm run check` passed** on Windows with Rust 1.99,
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
asserts a settled whole-map camera, no picture skips, no session failure and a
clean requested close. It reports actual peak received/prepared counts. Full
recordings and failed attempts stay outside Git.

The Battledome cross-check uses an explicit 40,000-unit snake/prey coverage radius
and 15 seconds of bot fill before login. The browser does not transmit its zoom;
this test configuration requests server coverage separately. Food sector interest
is unchanged. Actual admitted/received counts, not configured seats, determine
the strength of that cross-check.
