# Retained dense pellet batches at 1080p

## Result and boundary

On main f3c5b519, dense art spends most of its frame building and packing seven
generic records per visible pellet. Retaining source groups and instancing the
three additive pairs halves the measured callback time. It does **not** make
pellets close to free: art-period CPU stages still cost about 8 ms, GPU time
increases, p95 remains above 16.7 ms, and cold spikes remain. These synchronized
offscreen callbacks are not gameplay FPS or owner-play acceptance.

The halving applies to the default view after menu composition ends. The original
replay skipped native menu composition, and the join fade, spinner tail and any
wheel zoom still use the generic path; see the review-fix section for their
measured cost.

The original results and phase-profile tables below retain the pre-review
composition measured for 6293009a. The review-fix section adds final measurements
and makes the omitted native crossfade and wheel-zoom paths explicit.

Access: the ten-second community recording, images, packet/position journals,
source copies, binaries and full process/load samples are restricted local
material. They remain outside Git. Selected aggregate counts and hashes here
identify the experiment without publishing any server or player identifiers.
No new outside connection was used.

## Design

The replay's art has about 18,475 on-screen pellets and 133,534 logical image
records at its populated median. The authored control is a plain 25,000-pellet
rectangle, with no copied art or server layout.

The cache retains source grouping, atlas bindings and dense-slot mapping.
`FoodSourceIdentity` is an opaque membership/source epoch, independent of the
food array; a long-lived renderer cache must not force world-size COW on the
next packet. Queued pictures still retain frozen worlds. Animation alone keeps
the epoch; insertion, removal, expiry and rollback forks invalidate grouping.

Each visible pellet still needs its existing f64 animation/culling, exact body
and glow rectangles, pulse/distance/fade alpha and reference glow admission.
The compact path uploads 96 bytes of projected rectangles/alpha/UV and four
flag bytes per retained pellet per frame, rather than rebuilding seven generic
records and their vertices. This is retained grouping and instancing, **not**
an upload-only-on-change renderer. Visibility and alpha really change each frame.

Three instanced additive pairs keep the existing fragment math, atlas textures,
glow admission/targets/history and one MSAA world resolve. Original food/snakes/
late-glow barriers remain; names and antennas adjust those barriers. Black
outlines over the known black prefix remain omitted as in the existing renderer.
Logical draw-budget charges remain seven per pellet. Sparse/pressured/custom-zoom,
background-prefixed or unsupported frames retain the canonical path.

Shader-only fade/body attempts failed the pixel gate (including an earlier
21-byte body-edge error). The recovered phase-driven glow prototype later failed
one of 600 Native frames by three RGB bytes. Neither is proposed. The final path
uses already-computed exact CPU rectangles for every part, removing redundant
shader approximation, shader trigonometry and the extra position buffer.

The shader uses two instanced vertex buffers, seven attributes, uniform buffers
and texture arrays. The existing WebGL2 downlevel device limits admit it; no
compute/storage/f64/optional feature or CPU-specific code was added. The actual
repository browser backend is WebGPU, not a running WebGL backend. Native and
Wasm build/Clippy checks are distinct from browser/WebGL runtime or pixel proof.
Canvas drawing policy is unchanged; the shared core exposes opaque identity tracking.
Review fixes also move the existing glow alpha/centre, pulse, body centre/alpha
and image-rectangle formulas into inline shared-core helpers used by both paths;
the formula order and Canvas outputs remain exact. WGSL has no animation or
glow-policy formulas: its endpoint/UV and flag expansion is a GPU input-layout
adapter, while the existing shared quad fragments own paired-addition rounding.

## Measurement

Release builds, the same frozen input and virtual 600-callback schedule, physical
1920x1080, DPR 1, default view, MSAA4. Original uses the desktop's capped backing
projection plus blit; Native draws to 1920x1080. AMD Ryzen 7 9800X3D, Radeon iGPU,
DX12 driver 32.0.21001.9024. This is one machine and one retained join.

Five alternating pairs per mode. Pair order alternates A/B and B/A and mode order
also alternates. Every run remains in `runs.csv`; all raw journals remain local.
Machine load and cargo/rustc/link/test/tar/zstd processes were sampled about every
250 ms. Only an observed compiler overlap excludes a run; none occurred. No
quiet window was booked. Load peaks and the noisier fourth pair remain included.

Art is the fixed [2,7) virtual-second window, 300 callbacks; whole-run statistics
cover [0,10), 600 callbacks. p95 uses sorted index floor((n-1)*95/100). Tables show
the median of five per-run quantiles, with reductions computed within pairs.
The CPU-stages row is the per-callback sum of poll, ingress, preparation, resource
sync and submit, then its quantile; it is not a sum of separate medians.

`wall_ns` includes diagnostic GPU timestamp synchronization. Preparation includes
frame advance and snapshots. CPU submit includes compact pellet preparation,
packing, upload and encoding. GPU is timestamped passes. Phase hooks are disabled
for matched runs; separate diagnostic runs attribute stages. The driver also
collects density outside callback wall time; process elapsed includes that
instrumentation and is not a client-throughput claim. Cold renderer construction
is before connect; cold food buffers/bindings and scene/font preparation remain
inside callbacks. GPU pipelines are created before connect.

All GPU figures in this report (matched runs, profiles and measurement-only
counterfactuals) used **per-pass mode**: `Gpu::draw` in the retained private
`join_profile.rs` calls `begin_diagnostic_frame()` before the presenter, and
`Gpu::timestamp` resolves every recorded query pair and sums their durations.
Glow/world/blit/overlay passes are included. The aggregate 0/1-query overwrite
found during review did not produce any published figure here. The fix brackets
the first submitted glow/world pass and the last world pass in aggregate mode,
using optional pass-boundary writes without adding a device feature. Per-pass
query assignment is unchanged; aggregate spans can include gaps between passes,
whereas published per-pass totals sum only the recorded pass intervals.

<!-- BEGIN RESULTS -->
| Quality / median of per-run quantiles | Before ms | After ms | Paired reduction range |
| --- | ---: | ---: | ---: |
| original / first 10 s callback p50 | 28.635 | 13.734 | 49.3 to 53.0% |
| original / first 10 s callback p95 | 34.725 | 17.284 | 42.8 to 51.8% |
| original / first 10 s preparation p50 | 10.525 | 2.327 | 74.6 to 78.3% |
| original / first 10 s CPU submit p50 | 10.111 | 4.881 | 48.6 to 53.1% |
| original / first 10 s GPU p50 | 2.686 | 3.388 | -28.0 to -24.0% |
| original / art callback p50 | 30.121 | 14.950 | 47.6 to 52.7% |
| original / art callback p95 | 34.972 | 18.098 | 41.3 to 51.3% |
| original / art preparation p50 | 11.365 | 2.559 | 72.3 to 79.4% |
| original / art CPU submit p50 | 10.916 | 5.409 | 45.7 to 52.2% |
| original / art GPU p50 | 4.335 | 4.989 | -15.4 to -14.1% |
| original / art CPU stages p50 | 23.114 | 8.422 | 59.6 to 65.5% |
| original / first 10 s worst callback | 85.920 | 83.494 | -54.5 to 4.5% |
| native / first 10 s callback p50 | 28.619 | 14.037 | 49.1 to 52.3% |
| native / first 10 s callback p95 | 33.162 | 17.233 | 40.4 to 61.2% |
| native / first 10 s preparation p50 | 10.430 | 2.298 | 77.1 to 79.0% |
| native / first 10 s CPU submit p50 | 9.819 | 4.675 | 48.3 to 56.9% |
| native / first 10 s GPU p50 | 3.273 | 3.838 | -18.6 to -15.7% |
| native / art callback p50 | 30.005 | 15.023 | 48.0 to 53.7% |
| native / art callback p95 | 33.301 | 18.165 | 39.7 to 61.6% |
| native / art preparation p50 | 11.178 | 2.495 | 75.7 to 79.5% |
| native / art CPU submit p50 | 10.572 | 5.173 | 46.2 to 56.1% |
| native / art GPU p50 | 4.597 | 5.146 | -12.4 to -10.7% |
| native / art CPU stages p50 | 22.509 | 8.127 | 61.3 to 68.0% |
| native / first 10 s worst callback | 80.179 | 85.097 | -6.9 to -2.0% |

original baseline: 5 runs; art p50 range 29.881 to 31.299 ms; load 6.2 to 84.6%; observed cargo/test/archive samples 0/0/0; worst callback range 80.190 to 87.041 ms.

original candidate: 5 runs; art p50 range 14.338 to 16.403 ms; load 4.2 to 90.8%; observed cargo/test/archive samples 0/0/0; worst callback range 81.873 to 123.930 ms.

native baseline: 5 runs; art p50 range 29.253 to 32.240 ms; load 4.7 to 99.3%; observed cargo/test/archive samples 0/0/0; worst callback range 79.474 to 83.011 ms.

native candidate: 5 runs; art p50 range 14.807 to 15.483 ms; load 3.5 to 84.2%; observed cargo/test/archive samples 0/0/0; worst callback range 81.760 to 87.480 ms.

All runs: 20; accepted 20; excluded 0.
<!-- END RESULTS -->

## Review fixes and final remeasurement

The aggregate query fix, fallible menu batch expansion, non-batch single-copy
composition and shared-core food-policy extraction are included in the final
source. A public menu backdrop now canonicalizes before insertion, preserving
outlines over the page; merely shifting barriers is insufficient. A cover with
no overlay stays at the end and does not invalidate the black prefix. Both
orders and whole-picture rejection are authored regressions. A renderer guard
also falls back for any nonempty prefix before a batch. The rationale sits at
the instanced encode boundary. Timestamp-index tests cover zero to three glow
passes, and an offscreen validation test exercises aggregate query writes and
the non-black-prefix fallback without requiring an additional device feature.

The final release repeats exact canonical/numeric/vertex parity against main
on all three 600-callback scenes, and all 1,200 community pixel comparisons stay
within one RGB byte. The oracle test is retained after formula extraction.

Five alternating triples per resolution mode compare main, the pre-review
candidate and the final candidate on the same input. Triple and quality order
reverse on alternate repetitions. All 30 new runs remain in `review-runs.csv`,
including the high-load samples; the original 20 rows remain unchanged in
`runs.csv`. No compiler, test or archive overlap was observed. Small additions
to the diagnostic driver are disabled outside the menu diagnostic. The final
vs pre-review columns measure the combined fixes, not an isolated helper call.

<!-- BEGIN REVIEW RESULTS -->
| Review-fix comparison / median of per-run quantiles | Main ms | Before review ms | Final ms | Final vs before review paired range |
| --- | ---: | ---: | ---: | ---: |
| original / art callback p50 | 29.679 | 14.587 | 14.992 | -1.3 to +17.6% |
| original / art callback p95 | 33.425 | 17.852 | 18.115 | -4.6 to +34.5% |
| original / art preparation p50 | 10.731 | 2.483 | 2.562 | -7.0 to +45.1% |
| original / art CPU submit p50 | 10.807 | 5.274 | 5.426 | +0.5 to +21.1% |
| original / art GPU p50 | 4.324 | 4.956 | 4.978 | -1.7 to +0.8% |
| original / art CPU stages p50 | 22.851 | 8.214 | 8.516 | -1.2 to +30.7% |
| original / 10 s callback p50 | 28.546 | 13.491 | 13.935 | -0.2 to +7.8% |
| original / 10 s callback p95 | 33.425 | 17.354 | 17.433 | -5.2 to +26.9% |
| native / art callback p50 | 29.437 | 14.549 | 14.824 | -3.8 to +2.1% |
| native / art callback p95 | 33.150 | 17.382 | 18.007 | -5.9 to +4.5% |
| native / art preparation p50 | 10.840 | 2.380 | 2.377 | -18.8 to +0.4% |
| native / art CPU submit p50 | 10.556 | 5.055 | 5.156 | -1.2 to +4.2% |
| native / art GPU p50 | 4.599 | 5.113 | 5.076 | -1.3 to -0.1% |
| native / art CPU stages p50 | 21.988 | 7.835 | 7.971 | -8.4 to +1.9% |
| native / 10 s callback p50 | 28.279 | 13.861 | 14.046 | -2.7 to +1.7% |
| native / 10 s callback p95 | 32.985 | 17.150 | 17.272 | -7.1 to +2.7% |

original: 5 complete triples; load 0.7 to 90.4%; cargo/test/archive samples 0/0/0.

original callback: paired final-minus-before-review median +0.124 ms; range -0.212 to +2.602 ms.

original CPU submit: paired final-minus-before-review median +0.098 ms; range +0.024 to +1.110 ms.

native: 5 complete triples; load 0.7 to 94.5%; cargo/test/archive samples 0/0/0.

native callback: paired final-minus-before-review median +0.255 ms; range -0.586 to +0.301 ms.

native CPU submit: paired final-minus-before-review median +0.084 ms; range -0.064 to +0.211 ms.

Review-fix runs retained: 30; excluded: 0.
<!-- END REVIEW RESULTS -->

The inline shared-core extraction is not described as free. The combined
fixes show a small positive paired CPU-submit difference, with noisier callback
differences retained above. Main-to-final still roughly halves default-view
steady art cost. These figures do not fix the live-path gaps below.

### Native join crossfade is missing from the original replay comparison

The original comparison drives `LocalGame` and the world presenter directly;
it does **not** run `NativeLogin::step/observe_game/prepare_host`, menu composition
or egui painting. Therefore even its first-ten-second callback figures omit the
visible native login crossfade. The [2,7) art window begins after that fade.

The session's separate UI clock increases world alpha by 0.05 per 25 ms: a
500 ms fade, not a fixed frame count. On this retained 60 Hz join schedule it
remains outgoing for 31 callbacks, frames 60-90, at model times 1000-1500 ms;
floating-point accumulation reaches the fully-visible endpoint at frame 91,
1516.667 ms. The spinner keeps `needs_menu_pass` true for 11 more callbacks,
frames 91-101, about 183 ms. Menu composition first stops at 1700 ms, so this
recording has 42 callbacks spanning about 700 ms of generic-path composition
after the first gameplay callback. Real frame counts depend on callback timing.

A separate four-run diagnostic uses the same recording and frozen world clock
with NativeLogin's live ordering, actual `prepare_host`/`compose_optional`, and
offscreen egui paint. The direct controls observe the same menu clock but skip
menu preparation/composition/paint. Each row is one run, not a matched speed
claim; `menu-runs.csv` retains the per-run quantiles, clock/counts, identities and
load. There was no compiler/test overlap. Wall time includes egui submission and
completion; GPU query totals still cover the world-presenter passes, not egui.

<!-- BEGIN MENU RESULTS -->
| Single diagnostic / fade callbacks | Frame median ms | Frame p95 ms | Composition median ms | Spinner-tail median ms |
| --- | ---: | ---: | ---: | ---: |
| original direct world | 7.205 | 15.749 | 0.000 | 11.789 |
| original menu composed | 12.906 | 34.342 | 3.723 | 41.347 |
| native direct world | 7.521 | 15.008 | 0.000 | 13.775 |
| native menu composed | 13.345 | 36.066 | 3.711 | 42.242 |
<!-- END MENU RESULTS -->

The gap is material, particularly the 41-42 ms spinner-tail medians. Every fade
callback initially has a retained batch, but menu composition canonicalizes all
of them. It also expands the batch during the hidden-login/spinner-only tail.
This PR's five review fixes do not remove that fallback.

The smallest safe next step is to preserve batches when the menu has neither
backdrop nor cover, while keeping the spinner's egui pass. That removes needless
canonicalization in the tail without changing page composition. During the
actual fade, retain compact body/glow pairs but submit the original food outline
stream over the page backdrop in its original order. Simply deleting the page
backdrop or omitting outlines changes pixels. That second step needs composed
pixel parity before shipping; no visible or smoothing change is proposed here.

### A zoomed player still uses the previous food path

`user_zoom != 1` declines the retained batch. Modest wheel zoom therefore gets
the old generic per-pellet planning/packing cost, even when the art is dense.
Zooming out can admit more pellets; at extreme zoom the existing subpixel LOD
may instead remove them, so cost is not constant across zoom levels.

Four single-run diagnostics on the same input confirm zero retained-batch art
callbacks at one and four zoom-out notches, with counts and quantiles retained
in `zoom-runs.csv`. They are diagnostic observations, not an A/B claim.

<!-- BEGIN ZOOM RESULTS -->
| Single diagnostic / zoom-out notches | Visible pellet median | Art callback median ms | Art callback p95 ms | Retained art callbacks |
| --- | ---: | ---: | ---: | ---: |
| original / 1 | 19509.0 | 30.028 | 34.894 | 0 / 300 |
| original / 4 | 22631.5 | 33.505 | 36.770 | 0 / 300 |
| native / 1 | 19509.0 | 29.803 | 33.014 | 0 / 300 |
| native / 4 | 22631.5 | 34.872 | 38.779 | 0 / 300 |
<!-- END ZOOM RESULTS -->

Supporting wheel zoom safely requires applying the shared core's exact
zoom eligibility, source-overload and rectangle policy to compact records and
validating all modes again. That expansion is not part of these review fixes.

## Remaining work and owner's glow question

The answer is not just glow. Before this change, CPU stages cost roughly 23 ms
per art callback while GPU cost roughly 4.3-4.6 ms. After it, CPU still costs
roughly 8.1-8.4 ms and GPU roughly 5.0-5.1 ms. The GPU increase is retained in the
table, rather than hidden behind the overall CPU improvement.

Ranked remaining steady CPU costs from a separate diagnostic: compact exact
projection/flags/upload about 4.5 ms (inside submit), then food advance about
1.75 ms (inside preparation), then surrounding packing/encoding/resources/HUD.
Food source planning itself is only about 0.04 ms at its median, with membership
rebuilds still visible in its tail. Direct glow target passes cost about
2.4-2.6 GPU ms; the rest of the GPU costs about 2.3-2.7 ms and includes world
composition/unreduced glows. Component medians need not sum to total medians.

<!-- BEGIN DIAGNOSTICS -->
| Diagnostic / candidate art p50 ms | Original | Native |
| --- | ---: | ---: |
| join / redraw_ns | 2.464 | 2.365 |
| join / submit_ns | 5.272 | 5.047 |
| join / gpu_ns | 4.969 | 5.095 |
| join / pellet_prepare | 4.517 | 4.493 |
| join / food_advance | 1.766 | 1.754 |
| join / glow_passes | 2.625 | 2.395 |
| join / other_gpu | 2.293 | 2.668 |
| rectangle / redraw_ns | 1.516 | 1.482 |
| rectangle / submit_ns | 4.980 | 4.846 |
| rectangle / gpu_ns | 4.587 | 5.403 |
| rectangle / pellet_prepare | 4.726 | 4.649 |
| rectangle / food_advance | 1.239 | 1.222 |
| rectangle / glow_passes | 2.971 | 3.245 |
| rectangle / other_gpu | 1.614 | 2.160 |
<!-- END DIAGNOSTICS -->

The rectangle rows are single instrumented diagnostics, not a matched speed
claim. Its final art medians are around 13.3-13.5 ms, and p95 around 14.5-15.1 ms;
the community input is more demanding and remains the acceptance concern.

Cold preparation is unresolved. The matched Native worst-callback median rises
from 80.2 to 85.1 ms; the Original candidate has one 123.9 ms callback. A separate
instrumented Original run had a 133.3 ms cold callback, including 82.7 ms inside
compact preparation (which includes first buffers/bindings/uploads); that phase
is not split finely enough to assign the stall to a specific allocation or
driver action. The Native diagnostic's cold callback spends 73.5 ms in redraw,
including 29.8 ms font loading. Neither cold result is a smoothing/network claim.

A separate, unbuilt reduced-effect option could remove glow/pulse work above a
high visible-count threshold behind a default-off setting. A rough steady-art
estimate is 9-12 ms: subtract about 2-3 GPU ms and 1.5-3 CPU ms from the current
15 ms. Those savings are an inference from the measured pass/projection shares,
not an implemented or measured mode; no p95 or cold-spike estimate is justified.
The earlier main-based measurement-only outline/body probe was about 15.2 ms,
with p95 about 19.3 ms, showing that effects removal alone did not remove generic
command overhead. A visible change remains the owner's decision. No such setting
is shipped. No NTL timing was measured; fewer effects and less per-draw work are
an explanation consistent with the owner report, not proof about NTL internals.

## Parity, regressions and source identity

All 600 callbacks of default view, whole-map view and the authored rectangle
match main in canonical commands/resources/sends, projected vertex bits and
numeric-state digests, with zero skips. The digest path reconstructs the original
image stream through `Batch::canonical`; that alone cannot detect a retained
GPU packing/order bug. For the community pixel check, the private driver renders
each prepared picture twice at the same virtual time: one renderer instance
submits the retained batch, and a second submits `p.frame.canonical()` through
the generic image path, with the same overlay and resource list. The two share
the core world/animation inputs, authored texture bytes, quad fragment math,
glow admission/target policy and presenter; they have separate renderer objects
and GPU resources. This is a comparison of submission paths, not an independent
visual implementation. All 600 rendered community frames in each resolution
mode were compared: max RGB difference one byte, zero frames beyond the existing two-byte
paired-addition allowance. Overlapping pixels cannot identify an individual
pellet's contribution; the separate authored lifetime controls establish maxima
of zero for settled pellets, one while fading and zero for consumed flights.
No wider visual tolerance was accepted.

The committed authored pixel regression also compares retained submission with
the generic canonical submission path and shares textures and fragment math.
The separate reviewer inspected that committed test and the canonical-digest
construction; the private community driver and its per-frame outputs were not
inspected in that review. Their result above is implementation-run evidence.

Authored regressions cover exact projection/alpha bits, dense order, membership
replacement/rollback forks, flight/expiry, camera/animation reuse, frozen queued
pictures and static-cache lifetime. Intentionally restoring a retained
`FoodWorld` in `Sources` makes the lifetime test fail after picture release; the
mutation was reverted. Announcement, retry/resize and zoom golden tests compare
the canonical stream and retain their original expectations.

`identities.json` preserves the original measured production, built instrumented
and reviewed source hashes under their original keys. `review_fixes` records the
new measured production, instrumented binary and final runtime source identities;
`review-fixes-source.patch` retains their exact delta (an unused-parameter rename
and the outline-rationale URL, with no arithmetic or branching change). The
GPU pass-sum audit checks all 30,000 callbacks of the original and new timing
runs against their retained per-pass sums.

The rest of this paragraph describes the initial source snapshot, not the final
review-fix source. That measured source was an uncommitted candidate over
f3c5b519, frozen in local diagnostic copies; it was not a fabricated commit.
Only two documentation comments and a fresh-cache reset in the cfg(test)
regression differ between measured production and reviewed files. The exact
delta is retained in `reviewed-source.patch`; no production expression changed.
Historical study checks require neither a PR-branch commit nor current working
source hashes. Full passive diagnostic sources and binaries remain local.

Recompute every retained run/diagnostic table with stdlib Python:

```sh
python -B research/client-performance/2026-11-02-retained-pellets/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-02-retained-pellets -p test_study.py
```

`npm run check` passed on Windows on the final source, including strict native test Clippy, desktop
build, Canvas ABI and Wasm/browser checks. The study recomputation/tests and an
isolated study-only copy pass; the explicit authored offscreen GPU check passes.
Actual browser GPU/WebGL runtime, Vulkan/other hardware, owner play, physical input/
display latency and gameplay FPS remain unverified. Exact CPU/source parity and
the DX12 picture gate do not establish those acceptance axes.


## 2026-11-03: exact CPU projection specialization

This follow-up starts at merged main c58fc66b, including #445's review fixes.
`Frame::project` is shared by the retained and ordinary submission paths. Food
supplies a constant axis-aligned `Draw`, but a plain inline hint did not force
specialization. The adopted `inline(always)` attribute lets LLVM specialize
those calls. The checked f64 operations, f32 boundary, source selection, draw
order, animation and protocol code are unchanged. No intrinsics, target-feature
flags or per-CPU paths are added.

The isolated substage probe found about 70 ns per visible food for both glow
projections and 48 ns for body projection/packing, versus 29 ns for the two
shared glow-policy calls, 9 ns for pulse, and 4 ns for a snapshot copy. Stages
partly overlap (body and policy include pulse); these are diagnostic loop
measurements, not additive whole-frame claims. They motivated specialization
before a data-layout or threading rewrite.

The primary comparison is RTX 5070 Ti / Vulkan at physical 3840x2160, Original
and Native, DPR 1. The same retained offline join drives 600 virtual 60 Hz
callbacks over 10 seconds, 29,044 messages and zero skips. Art quantiles select
callbacks 120–419 ([2,7) seconds). This is about 18,475 visible foods and 30,522
retained foods at the window median. Every timing run includes the native login
step/observe/prepare/composition and egui paint path, with per-pass GPU queries.
Original uses 1500x845/MSAA4 plus blit; Native 4K uses the existing single-sample
memory-cap fallback. The AMD iGPU/DX12 guardrails also cover 4K and 1080p, both
qualities. This is the same 8-core/16-logical-processor desktop, not a four-core
hardware result.

Release builds use CARGO_BUILD_JOBS=2 and a lane-owned CARGO_TARGET_DIR on D:.
Runs alternate arms and reverse mode/configuration order. CPU/process and GPU
engine load are retained outside Git around every sample. Only observed
compiler/linker overlap excludes a run; a pair is used only if both arms are
accepted. After the first six primary pairs, collection continued until each
configuration had at least five complete accepted pairs, with a declared
maximum of twelve extra rounds. No quiet window, CPU pinning or worker pause
was used. Absolute times differ from the preceding busy-machine profile;
compare matched arms here rather than subtracting across studies.

`serial-runs.csv` retains every run of the adopted and rejected serial probes,
including excluded rows, binary/source labels, allocation requests, elapsed
and per-run quantiles. Recorded sent-byte control digests match across each pair, including
excluded samples. Canonical-parity mode was off in these timing runs; its
constant quad-digest accumulator is not world-state parity evidence. Actual
rendered-image controls are described separately below. `serial-identities.json` records the measured runtime
and passive-instrumentation hashes, timing/image binaries and image parity.
The measured candidate frame file is byte-identical to this follow-up's
production frame file. Historical revisions are labels, not `git show` test
dependencies; no historical check compares a current working-tree source hash.
Full captures, source trees, binaries, load and raw frame journals remain
local-only outside Git. Nothing identifying is included here.

<!-- BEGIN SERIAL RESULTS -->
| Adapter / physical / quality | Complete pairs | Art p50 before -> after ms | Art p95 before -> after ms | CPU stages before -> after ms | Pellet math before -> after ms | GPU before -> after ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rtx / 3840x2160 / original | 7 | 8.636 -> 7.185 | 11.847 -> 10.394 | 8.123 -> 6.751 | 3.762 -> 2.554 | 0.224 -> 0.224 |
| rtx / 3840x2160 / native | 5 | 8.560 -> 7.201 | 12.238 -> 10.516 | 8.005 -> 6.662 | 3.722 -> 2.515 | 0.243 -> 0.243 |
| igpu / 3840x2160 / original | 5 | 15.177 -> 13.954 | 18.005 -> 16.373 | 8.191 -> 6.742 | 3.784 -> 2.524 | 5.474 -> 5.427 |
| igpu / 3840x2160 / native | 5 | 18.206 -> 17.081 | 20.514 -> 19.288 | 7.830 -> 6.547 | 3.684 -> 2.533 | 8.318 -> 8.248 |
| igpu / 1920x1080 / original | 5 | 14.779 -> 13.306 | 17.559 -> 15.833 | 8.155 -> 6.906 | 3.782 -> 2.522 | 4.888 -> 4.985 |
| igpu / 1920x1080 / native | 5 | 14.775 -> 13.589 | 17.576 -> 16.351 | 7.828 -> 6.743 | 3.720 -> 2.509 | 5.081 -> 5.082 |

rtx / 3840x2160 / original: paired p50 difference median -1.451 ms, range -3.852 to -1.150 ms; baseline p50 range 8.310 to 10.929 ms; always p50 range 7.020 to 7.366 ms; load 8.2 to 82.3%; observed cargo/test samples 360/360.

rtx / 3840x2160 / native: paired p50 difference median -1.629 ms, range -2.680 to -0.994 ms; baseline p50 range 8.107 to 9.881 ms; always p50 range 6.860 to 7.424 ms; load 14.7 to 98.6%; observed cargo/test samples 258/258.

igpu / 3840x2160 / original: paired p50 difference median -1.202 ms, range -1.334 to -1.137 ms; baseline p50 range 15.099 to 15.281 ms; always p50 range 13.897 to 14.040 ms; load 7.0 to 54.3%; observed cargo/test samples 351/344.

igpu / 3840x2160 / native: paired p50 difference median -1.040 ms, range -1.176 to -0.916 ms; baseline p50 range 18.079 to 18.289 ms; always p50 range 17.061 to 17.291 ms; load 3.1 to 65.8%; observed cargo/test samples 319/318.

igpu / 1920x1080 / original: paired p50 difference median -1.316 ms, range -1.742 to -0.763 ms; baseline p50 range 14.329 to 15.048 ms; always p50 range 13.175 to 14.016 ms; load 0.8 to 95.2%; observed cargo/test samples 271/271.

igpu / 1920x1080 / native: paired p50 difference median -1.048 ms, range -1.682 to -0.788 ms; baseline p50 range 14.359 to 15.000 ms; always p50 range 13.311 to 13.987 ms; load 0.7 to 60.5%; observed cargo/test samples 144/138.

All serial-probe runs retained: 92; compile-overlap exclusions: 22. Incomplete accepted pairs are retained but excluded from aggregates.
<!-- END SERIAL RESULTS -->

The table gives medians of per-run quantiles, not pooled quantiles. CPU stages
include redraw, ingress, synchronization, submit and poll. CPU submit includes
projection/packing plus wgpu/driver calls; component medians need not sum.
Callbacks include synchronous diagnostic query resolution/completion and are
not presented intervals or gameplay FPS. GPU work is essentially unchanged;
the reduction is CPU work. The art frame remains over the 4.167 ms budget for
240 Hz, and this does not establish stable high-refresh delivery. Cold redraw,
ingress bursts, zoom's retained-path gap and menu-backdrop fallback remain.

Two earlier probes are retained rather than presented as wins. The plain
inline hint has four compiler-overlap pilot runs, so its timing is inconclusive.
A fixed-block byte-copy loop has two accepted pairs against the specialized
projection parent: packing is 0.577→0.627 and 0.580→0.613 ms. It was rejected;
the optimized iterator remains. Their source deltas are retained in the two
`serial-rejected-*.patch` files. Neither rejected probe is part of the final
runtime change.

### Validation and limits of this follow-up

All 600 individual output-image SHA-256 hashes match baseline in each of ten
cases (6,000 image pairs): the community replay on RTX/Vulkan 4K and AMD/DX12
4K and 1080p, both qualities, plus the authored 25,000-food rectangle on both
adapters at 4K in both qualities. Image drivers rasterize the actual old/new
world/menu stream, not just `Batch::canonical` text. They share the existing
materials and shader fragments; unchanged egui paint is omitted identically
from these image controls. No rounding allowance was needed for old/new images.
This does not assert cross-adapter image equality. Raw pixels were not retained
or published by this hash-only driver.

The authored compact-instance f32-bit oracle and explicit offscreen rectangle
GPU regression pass. `npm run check` passes on Windows, including native tests,
strict Clippy, Canvas/Wasm ABI checks, desktop and Wasm builds. Recompute all
new and historical tables and check the bounded parity counts with:

```sh
python -B research/client-performance/2026-11-02-retained-pellets/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-02-retained-pellets -p test_study.py
```

Actual browser GPU/WebGL runtime, other hardware/OSes, four-core hardware,
owner play, physical input latency and displayed intervals were not measured
by this follow-up. The separate visible benchmark owns pacing evidence.
No adapter default, motion smoothing, limiter, quality or visible effect changed.
