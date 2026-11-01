# Distant snake shadows at whole-map zoom (2026-11-01)

Dropping interleaved body shadows below four logical pixels reduces CPU render
submission and GPU state changes in the authored Battledome workload. The native
wheel factor must also be below one. Body samples, palettes, tangents, other
passes, their order, and the shared distance cursor remain unchanged. The
retained cursor is important to the following boost/death passes and antenna
barrier. The planner counts the emitted commands before allocation.

## Reproduction and scope

- Base main: `276522f89667972a7134600696a8b89554c9e9dc`.
- Measured diagnostic baseline: `181bc01f21441d3447cad7acefec1f683b347e29`.
- Measured optimization candidate: `583c502478fce0e576c82af1de5103bd21bc46ff`.
  Later report/documentation and benchmark-comment edits do not change measured code.
- Frame benchmark SHA-256 before:
  `82fda4f9c33e4f56b0b23d4e81857a174856cd0202f9aef45dc9da59436559bd`;
  after: `42fe9aa38ab6effd10abf32d83b7053bc0af03675e3aa3665bfdaabff173eb32`.
- Final capture: 06:12:29–06:24:12 Europe/Berlin, 2026-11-01.
  All ten timing runs and two separate profile runs completed without compiler overlap.
  Timing-run median system CPU loads were 11.05–19.00%; the one-second samples
  peaked at 60.8%. Samples and 200 ms compiler monitor receipts remain in scratch.


Windows, Ryzen 7 9800X3D (16 logical processors), Rust 1.99.0 release/thin LTO,
`feel-trace` enabled with no feel recorder in the timed runs. Hardware DX12
adapters: RTX 5070 Ti and AMD Radeon integrated graphics. Both use the same
authored packets and procedural placeholder assets. No external artwork, private
reference, socket, window, simulation advance or ingress occurs in the frozen
comparison. This is an offscreen workload, not gameplay FPS or display latency.

The fixture models 120- and 500-bot Battledome arena sizes using
`3584 × sqrt((bots + 1) / 32)`, 26-anchor circular bodies at the 400-point floor,
half boosting, and constant-density pit food. Names, a leaderboard row and a
synthetic minimap are included; Battledome has no prey, but the prey/eater planning
path still runs. The 120 model retains 121 live snakes; the 500 model retains the
maximum valid 256 live snakes including own. At full zoom every retained snake
is visible. Retained food is a conservative full-pit set, not a claim that one
real server subscription supplies all food in the arena. Default and mid views
cull subsets. Mid is 12 wheel notches out. Physical 1080p/4K are direct targets
at logical scale one; this does not select a window/DPR resolution policy.

The finite driver is `frame_perf --battledome NEW_DIRECTORY`. Set
`FRAME_PERF_PHASES=1` for its separate opt-in render observer;
`FRAME_PERF_PLAN_ONLY=1` performs a no-timing workload/budget preflight;
`FRAME_PERF_NO_PNG=1` retains CSVs and default draw snapshots without redundant
PNG captures. Ordinary render observers are unset. They add no latency-trace-v1
vocabulary, and observation-enabled pictures/resources have an exact regression.
GPU queries are explicitly opt-in at renderer construction; ordinary hosts
request no additional features or query storage.

Build sequentially with `CARGO_TARGET_DIR=D:/CargoTargets/zoomout-perf` and
`CARGO_BUILD_JOBS=2`, retaining each side's executable. A/B uses the same driver.
Before timing, wait until **all** cargo/rustc processes have exited. The study
monitor sampled their presence every 200 ms and system CPU load every second;
any overlap invalidates the run and retains its receipt. Run five alternating
A/B pairs. Each scene settles zoom for 110 CPU pictures, then warms 10 rendered
frames and measures 30, giving 150 measured frames per side/cell. Two additional
observer-enabled profile runs are excluded from the reported timing matrix.

CPU is the per-frame sum of world preparation, HUD/final validation, resource
sync and render serialization/submission. GPU timestamps bracket the render
pass; uploads, queue idle waits, query resolve/readback, PNG/debug serialization,
and logging are outside CPU timings. These CPU/GPU numbers are separate and
must not be added to infer FPS. Allocations are requested Rust allocation and
reallocation bytes on the caller thread, excluding live/resident heap, GPU/driver
memory and asynchronous workers. Vertices are the six serialized vertices per
quad. Quad area is an unclipped geometric area proxy; no hardware fragment or
overdraw counter was captured.

## Selected results

In the 500-bot arena-size model (256 admitted snakes), full-map 1080p on AMD
integrated graphics at 4x MSAA drops CPU **44.24 → 16.56 ms** and GPU
pass **15.04 → 4.95 ms**. Per-run CPU medians range
43.05–45.85 → 15.52–17.70 ms; GPU medians 15.00–15.08 → 4.59–5.74 ms.
Draw calls fall 52,628 → 23,822 and requested allocations 49.32 → 26.06 MiB/frame.
This is a reduction for the threshold-eligible workload, with the unaffected
control regression below still unresolved. It does not establish 500 live snakes
in a client or gameplay FPS.

The [selected CSV](selected.csv) includes median/p95, per-run median ranges,
separate prepare/HUD/sync/submit times, all allocation phases, vertices, and
1x/4x MSAA for every case. Table values below are pooled medians at 4x MSAA.
The bot count is an arena-size model; the second count is prepared/cull-eligible snakes (including padded bounds),
not necessarily literal visible pixels, total received or server population.

| GPU | Model bots / prepared | Pixels | Zoom | CPU ms before → after | GPU ms before → after | Draw calls before → after | Alloc MiB before → after |
| --- | ---: | --- | --- | ---: | ---: | ---: | ---: |
| AMD integrated | 120 / 4 | 1920×1080 | default | 1.50 → 1.47 | 2.44 → 2.48 | 915 → 915 | 5.41 → 5.41 |
| AMD integrated | 120 / 22 | 1920×1080 | mid | 6.25 → 6.16 | 4.85 → 4.82 | 6,853 → 6,853 | 9.44 → 9.44 |
| AMD integrated | 120 / 121 | 1920×1080 | out | 27.94 → 14.07 | 9.34 → 3.98 | 28,084 → 14,593 | 26.45 → 14.84 |
| AMD integrated | 120 / 11 | 3840×2160 | default | 2.87 → 3.04 | 13.61 → 14.40 | 2,527 → 2,527 | 6.19 → 6.19 |
| AMD integrated | 120 / 56 | 3840×2160 | mid | 16.54 → 16.91 | 24.38 → 25.79 | 22,168 → 22,168 | 14.92 → 14.92 |
| AMD integrated | 120 / 121 | 3840×2160 | out | 35.10 → 35.12 | 31.38 → 33.48 | 39,843 → 39,843 | 27.75 → 27.75 |
| AMD integrated | 500 / 3 | 1920×1080 | default | 2.36 → 2.36 | 2.43 → 3.49 | 686 → 686 | 13.20 → 13.20 |
| AMD integrated | 500 / 15 | 1920×1080 | mid | 4.85 → 4.67 | 3.82 → 4.73 | 3,471 → 3,471 | 15.23 → 15.23 |
| AMD integrated | 500 / 256 | 1920×1080 | out | 44.24 → 16.56 | 15.04 → 4.95 | 52,628 → 23,822 | 49.32 → 26.06 |
| AMD integrated | 500 / 6 | 3840×2160 | default | 3.21 → 3.21 | 12.70 → 13.64 | 1,400 → 1,400 | 13.56 → 13.56 |
| AMD integrated | 500 / 42 | 3840×2160 | mid | 15.05 → 14.20 | 22.12 → 23.27 | 16,548 → 16,548 | 24.60 → 24.60 |
| AMD integrated | 500 / 256 | 3840×2160 | out | 59.63 → 28.89 | 31.96 → 22.25 | 59,289 → 30,737 | 53.76 → 30.50 |
| RTX 5070 Ti | 120 / 4 | 1920×1080 | default | 1.33 → 1.35 | 0.18 → 0.18 | 915 → 915 | 5.41 → 5.41 |
| RTX 5070 Ti | 120 / 22 | 1920×1080 | mid | 5.61 → 5.52 | 0.98 → 0.98 | 6,853 → 6,853 | 9.44 → 9.44 |
| RTX 5070 Ti | 120 / 121 | 1920×1080 | out | 26.65 → 14.22 | 5.25 → 0.17 | 28,084 → 14,593 | 26.45 → 14.84 |
| RTX 5070 Ti | 120 / 11 | 3840×2160 | default | 2.64 → 2.56 | 0.56 → 0.56 | 2,527 → 2,527 | 6.19 → 6.19 |
| RTX 5070 Ti | 120 / 56 | 3840×2160 | mid | 15.65 → 15.18 | 2.85 → 2.84 | 22,168 → 22,168 | 14.92 → 14.92 |
| RTX 5070 Ti | 120 / 121 | 3840×2160 | out | 33.81 → 32.91 | 5.74 → 5.75 | 39,843 → 39,843 | 27.75 → 27.75 |
| RTX 5070 Ti | 500 / 3 | 1920×1080 | default | 2.27 → 2.30 | 0.14 → 0.14 | 686 → 686 | 13.20 → 13.20 |
| RTX 5070 Ti | 500 / 15 | 1920×1080 | mid | 4.45 → 4.65 | 0.66 → 0.66 | 3,471 → 3,471 | 15.23 → 15.23 |
| RTX 5070 Ti | 500 / 256 | 1920×1080 | out | 40.76 → 15.56 | 10.98 → 0.20 | 52,628 → 23,822 | 49.32 → 26.06 |
| RTX 5070 Ti | 500 / 6 | 3840×2160 | default | 2.74 → 2.79 | 0.34 → 0.34 | 1,400 → 1,400 | 13.56 → 13.56 |
| RTX 5070 Ti | 500 / 42 | 3840×2160 | mid | 13.14 → 13.21 | 2.16 → 2.17 | 16,548 → 16,548 | 24.60 → 24.60 |
| RTX 5070 Ti | 500 / 256 | 3840×2160 | out | 53.99 → 30.20 | 11.17 → 0.64 | 59,289 → 30,737 | 53.76 → 30.50 |


## What scales

The separate observer runs expose the following AMD 500-model/1080p/full-map
4x phase medians. Observer-enabled runs are excluded from the main timings.
The [phase CSV](phases.csv) contains all views, sizes, adapters and MSAA settings.

| CPU phase | Before ms | After ms |
| --- | ---: | ---: |
| Core clone/culling/curve preparation | 1.956 | 1.719 |
| Background | 0.003 | 0.001 |
| Food planning | 0.244 | 0.198 |
| Eater collection/prey planning | 0.184 | 0.181 |
| Body/face planning and draw insertion | 2.305 | 1.916 |
| Arena edge | 0.007 | 0.006 |
| Name planning/text | 0.001 | 0.001 |
| Resource collection/frame assembly | 0.680 | 0.369 |
| Announcement/minimap | 0.024 | 0.019 |
| Remaining HUD/final validation | 2.554 | 1.623 |

In unobserved runs the CPU submit phase falls 35.88 → 9.78 ms and HUD/final
validation 2.58 → 1.66 ms. The improvement is mostly serialization/submission,
not faster food, minimap or curve math. At full-map 120/1080p, names still take
about 1.2 ms in the profiled RTX baseline; the larger arena's smaller names exit
earlier. These phase brackets include adjacent work as labelled, not a sampled
CPU call-stack profile.

The body shadow and colored body use different pipelines. At distant scales,
one shadow per segment forces repeated black/color pipeline changes while
retaining a fine shading detail. Omitting that layer saves command construction,
final validation, vertex/upload work and pipeline switches. This does not
resample the curve or compact ordinals. Names below one pixel already stop
before rasterization; warmed name/HUD caches are measured here, not cold font
startup or newly arriving-name spikes. Culling, transactional world copies and
curve sampling are combined in the core-preparation phase rather than reported
as independently isolated timings.

For the AMD 500 model, 1x versus 4x GPU pass medians are
13.67 / 15.04 ms before
and 3.29 / 4.95 ms after at 1080p;
at 4K they are 20.29 / 31.96 ms before
and 10.51 / 22.25 ms after.
MSAA cases run sequentially: ingress and simulation updates are frozen, but the
draw clock continues. A/B histories match within each case; 1x and 4x are not
byte-identical scene histories, so this is sensitivity evidence, not a pure
MSAA-only experiment. The final after/4K/1x run-median range is 9.56–23.30 ms.

At 1080p the 500-model serialized vertices fall 399,132 → 247,830, while the
unclipped quad-area/surface-area proxy only falls 3.198 → 3.115. At 4K these are
667,026 → 515,724 vertices and 3.317 → 3.235. The much larger draw-call saving
than area saving is consistent with pipeline alternation being the main distant
body cost. No actual fragment count, fill-rate or hardware overdraw measurement
was available; the proxy cannot establish those quantities.

At 120 bots and 4K the body diameter stays above the four-pixel threshold, so the
shadow layer is retained and the workload has no measured benefit. The iGPU's
4K fill/MSAA cost remains substantial. World resolution and quality policy are
outside this change.

## Default behavior, images and limits

Factor-one draw snapshots are exact in all **40** final timing-run pairs
(five runs × two GPUs × two populations × two extents). The final profile
readbacks are byte-identical for all **four AMD default cases**, and for every
AMD unaffected mid/out control. The preliminary run separately retained 20
identical AMD default pairs. Body-painter regressions cover strict threshold,
default factor, mixed palettes/flags/fades, boost/death cursor and exact budget.
Pinned-main default command/frame hashes pass.

The final native authored two-session replay is byte-identical: **148 samples,
72 acknowledged pictures, seven gameplay command sends**, zero errors and zero
`feel-perceptual-v1` mismatches. This checks native CPU history and draw lists,
not Canvas/browser GPU output.

**RTX pixel repeatability remains unresolved.** Repeated unchanged baseline
captures in the preliminary window differed at sparse RGB bytes by up to eight
levels. Final unaffected A/B profile images also show sparse differences of up
to eight levels despite exact default draw lists. No RTX byte-equal pixel claim
or driver-cause conclusion is made. Changed full-map shading intentionally
differs on both GPUs; small global mean errors do not prove perceptual parity.

The full-map crop in the PR is a matched native-size crop of the authored
256-snake fixture, with procedural placeholder art. It deliberately shows the
changed shading. Core centerlines, face/eye input, other effect passes and paint
ordering are retained. Zoomed-out pixel identity, official/Canvas perceptual
acceptance, owner play, physical input and display pacing are not established.

**Open merge blocker: unexplained AMD control increase persists.** In the preliminary window the
unaffected 500-model/default/1080p/4x control GPU median increased
2.4046 → 3.3605 ms, with disjoint per-run ranges. In the final window it is
**2.4324 → 3.4860 ms**, again disjoint: 2.3137–2.4871 versus 3.3047–3.8704 ms.
CPU is 2.3626 → 2.3641 ms; draw lists, counts, allocation bytes and AMD RGBA are
identical. This has not been explained and is not dismissed as noise. Other
unaffected GPU controls also increase in the table, including 120/4K/full-map
31.38 → 33.48 ms. The final candidate cannot claim zero GPU regressions.
The AMD integrated adapter is the only weak-GPU measurement here; other iGPUs,
thermal states, asynchronous gameplay and sustained frame pacing are unmeasured.
The [GPU-input audit and A/A plan](CONTROL-AUDIT.md) record the unchanged pipelines,
resource metadata, buffer sizes and derived upload work. They do not yet explain
the timing increase or replace the requested A/A and reversed-order comparison.

## Local host cross-check

Two scratch-only `slither-sim host --config` runs used the Battledome preset,
seed 7, one remote slot, 120/500 background bot slots and two bot workers. The
host kept its loopback port-0 bind; reserved ports 8766, 18884 and 18885 were
excluded. After 15 seconds of fill, the finite headless client observed five
seconds at maximum wheel zoom-out. Received/visible peaks were 6/6 and 7/7,
with 316/561 accepted pictures and zero skipped pictures. The playable circle
fit; the shaded exterior band can extend past the viewport. These are actual
server subscriptions, not an assertion that all server snakes reach one client.
No live CPU/GPU timing enters the A/B table. Both hosts were reaped. Recordings
remain outside Git.

## The 257th live snake

Today `state::MAX_SNAKES` is 256. A new appearance when the live map already
contains 256 IDs returns `StateError::Limit` (`scene collection limit exceeded`)
from `ClientState`'s appearance path. Replacement of an existing ID is allowed.
The entire received frame is staged and committed only after successful parsing;
the existing `synthetic_limits_hold_across_frames_and_duplicate_replacement_is_allowed`
test rejects ID 257 and checks that received state is unchanged.

This is **not** a silent drop in the native product: `LocalGame::apply_message`
propagates the error, the native `Event::Binary` handler calls `game.fail`, and
`stop_as` queues `EffectKind::Close` for the native transport. The session stops
and the socket closes; the admitted received world is not partially corrupted.
The shared WebGPU wrapper's checked-error path also calls `game.fail`.

A separate fix should raise the bounded live allowance to at least 501 (512
covers 500 bots plus the player), audit the derived retained-snake and aggregate
body/sample/text budgets, and add session tests admitting the 257th and 501st
snakes without a fail/Close effect. Keep complete-picture draw/pixel guards and
fail-soft rendering. Silently discarding appearances would leave later movement,
eat and lifecycle messages without their entities and is not the proposed fix.
Raising the client ceiling alone does not change server interest coverage or
make unreceived snakes visible when zooming out.
**This PR does not change admission.**

## Retained attempts and verification

An initial 501-live-snake synthetic fixture hit the existing state limit and
stopped; it was not counted as a fast frame. The final fixture uses the valid
maximum subscription, with the distinction above. An early name-field encoding
error and a stale Cargo artifact caused by preserved restore timestamps also
stopped preflight and were excluded. Earlier binaries/failed receipts remain in
scratch. Restored source timestamps were refreshed and rebuilt binaries were
hashed before use.
The preliminary fixture rounded fractional mass just below the 400-point score
boundary (displaying 399); the final fixture asserts score 400 explicitly.

The first full check caught profiling phase names being sent into the closed
latency schema. The final observer is independent; latency serialization/join
tests and the new exact-picture observer regression cover the correction.
The first measurement window is retained as [preliminary selected data](selected-preliminary.csv),
including its control-time discrepancy; the final table uses the corrected
observer's rebuilt baseline/candidate pair.

- Full `npm run check` reached the native studio-cosmetics tests before its
  measurement-window guard stopped that attempt. A fresh check was paused at
  the owner's next quiet-window instruction. Full validation will resume after
  08:40; no complete full-check pass is claimed yet.
- Final all-target format/Clippy, 33 body-painter tests, nine release zoom tests,
  issued-picture feel regression, two latency tests and observer regression passed.
- Final baseline/candidate workload preflights completed, score-400 assertions
  passed, and no timed case was rejected or skipped.
- Final native A/B feel replay and exact default draw-list comparisons passed.
- No independent review or owner-play acceptance is claimed.
