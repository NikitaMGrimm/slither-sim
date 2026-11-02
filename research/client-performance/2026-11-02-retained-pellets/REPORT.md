# Retained dense pellet batches at 1080p

## Result and boundary

On main f3c5b519, dense art spends most of its frame building and packing seven
generic records per visible pellet. Retaining source groups and instancing the
three additive pairs halves the measured callback time. It does **not** make
pellets close to free: art-period CPU stages still cost about 8 ms, GPU time
increases, p95 remains above 16.7 ms, and cold spikes remain. These synchronized
offscreen callbacks are not gameplay FPS or owner-play acceptance.

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
Canvas drawing is unchanged; the shared core adds only opaque identity tracking.

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
numeric-state digests, with zero skips. Every one of 600 rendered community
frames in each resolution mode was compared to an independent canonical
renderer: max RGB difference one byte, zero frames beyond the existing two-byte
paired-addition allowance. Overlapping pixels cannot identify an individual
pellet's contribution; the separate authored lifetime controls establish maxima
of zero for settled pellets, one while fading and zero for consumed flights.
No wider visual tolerance was accepted.

Authored regressions cover exact projection/alpha bits, dense order, membership
replacement/rollback forks, flight/expiry, camera/animation reuse, frozen queued
pictures and static-cache lifetime. Intentionally restoring a retained
`FoodWorld` in `Sources` makes the lifetime test fail after picture release; the
mutation was reverted. Announcement, retry/resize and zoom golden tests compare
the canonical stream and retain their original expectations.

`identities.json` records separate measured production, built instrumented and
reviewed source hashes. The measured source was an uncommitted candidate over
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

`npm run check` passed on Windows, including strict native test Clippy, desktop
build, Canvas ABI and Wasm/browser checks. The study recomputation/tests and an
isolated study-only copy pass; the explicit authored offscreen GPU check passes.
Actual
browser GPU/WebGL runtime, Vulkan/other hardware, owner play, physical input/
display latency and gameplay FPS remain unverified. Exact CPU/source parity and
the DX12 picture gate do not establish those acceptance axes.
