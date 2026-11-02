# Pellet-art join and sector refresh

The owner reports several seconds of bad frame pacing while the community
server's pellet art is visible. Current main still spends substantial CPU time
building and submitting those pictures, and it also blocks on a large sector
clear/reload burst. This change fixes the latter: stage only new food records
for a multi-food `F`, and apply `w`/ignored `W` without whole-world transactions.
It does **not** establish that the art-period frame rate is fixed.

## Access, source and method

Restricted community inputs and all raw journals remain local-only under
`D:/slither-scratch/client-lane-20261101/community-lag/join-20261102`.
No address, player identity, captured position, art image or layout is published.
Two authorized 12-second wire captures and two 12-second native CPU/iGPU
diagnostic joins (baseline and candidate) were collected. No official server,
visible window or external artwork was used. Four short joins total; live joins
have different traffic and are not matched performance comparisons.

Baseline: main `fde706f1600e7d0917bc467a9b578de1b3d3b724`, including #439.
Windows, release, Rust 1.99, two Cargo jobs, portable code. Original quality,
960x540 backing and offscreen output, MSAA4, procedural source rasters, DX12 AMD
Radeon integrated GPU. The owner’s physical display/scanout is not reproduced.

The old 6.1-second capture **does** include challenge/Setup and the initial food
flood, but ends before the art clears. Its accelerated replay drains every
packet due at a virtual 60-Hz callback. Production transport polls at most 64
messages at once; the interactive host additionally polls on readiness events.
Thus neither the old replay nor this deterministic schedule proves actual
desktop backlog duration. The new capture covers the removal and replacement.
The complete-history replay uses 600 callbacks over ten virtual seconds and
29,044 actual envelopes; it has no pacing sleeps, socket, window or GPU pixels
in CPU-only parity mode. GPU timings use offscreen passes plus synchronous
timestamp readback per picture; readback/diagnostic overhead is not scanout.

Separate diagnostic spans record each callback and packet opcode/byte length,
host/core/received snapshots, preparation stages, CPU raster creation, GPU
resource/pipeline creation and uploads, submit, and individual GPU pass times.
They are excluded from performance comparisons. Source overlays, driver,
executables and raw per-callback/per-packet journals remain external.

## Measured diagnosis

Both fresh captures negotiate **protocol 13**. The initial art comprises
25,451–26,360 ordinary `f` commands, size 0.2 and palette 9, with no repeated
coordinates in those initial listings. The densest 16x16 world-unit cell holds
16 such pellets. These are normal animated food instances, not a downloaded
image or a special protocol object. The ordinary fade/phase/wobble machinery
and high-quality body/glow passes still run. The first sector clears arrive at
7.515–7.625 seconds; 225 `w` commands clear the covered sectors and `W`/`F`
repopulate ordinary food. This is not a mass sequence of eat commands.

The initial raw inspection mistakenly used protocol 19 to summarize colour
and spacing. The canonical protocol selection and unchanged first candidate
exposed that mistake. The incorrect colour/spacing interpretation and the
protocol-19-only candidate were rejected, not used as evidence for the fix.
Their raw logs and binaries remain local. All published characterizations use
the negotiated reader. An empty-`F` optimization was also discarded to preserve
the existing fast-API fallback contract. An authored opaque-`W` check caught
stricter-than-canonical validation; both readers now keep `W` opaque.

On `fde706f1`, the diagnostic art interval (model seconds 2–7) has median total
callback 27.31 ms, redraw preparation 12.16 ms, CPU submission 8.94 ms and GPU
passes 2.33 ms. After removal (seconds 8–10), total callback median is 3.41 ms.
About 10,000–15,000 pellet centres are on screen during the art, generating
roughly 80,000–113,000 complete draw records. These are one diagnostic sequence,
not matched performance acceptance. At this resolution the sustained cost is
primarily CPU preparation/submission, not an iGPU fill-rate stall.

The deterministic removal/reload callback takes 743.00 ms, of which 738.66 ms
is ingress. Across that diagnostic history, `F` costs 410.39 ms, `w` 366.73 ms
and `W` 98.95 ms. Per-pellet `f` insertion already uses its fast lane and costs
28.89 ms across 25,451 commands. The real headless join divides the refresh into
several 64-message callbacks: worst total 145.28 ms, ingress 123.39 ms. This
explains a visible hitch and delayed later traffic even though GPU passes stay
near 1–2 ms in those callbacks. Actual event-loop pacing varies with join/load.

The final candidate's separate live smoke completes twelve seconds, 29,738
messages and 654 callbacks without skips. Its worst callback is 55.29 ms during
cold preparation; the later clearing/reload has no comparable 120 ms ingress
callback. This is an unmatched diagnostic, not a live before/after speed claim.
Both native joins use the production transport/core/presenter in a finite
diagnostic loop, not a visible interactive window.

First-use work is present but shorter-lived: 18 instrumented renderer pipelines are created
before Connect; cold HUD/source preparation produces roughly 80 ms spikes.
The complete ten-second diagnostic's renderer requests 285 GPU textures and 5,118 texture
writes, taking about 7 ms and 67 ms of CPU API time respectively across the
whole history. This does not account for hidden driver work, but resource
creation is not the repeated multi-second bottleneck in this trace. No optional
cosmetics pack was loaded, so its decode cost is not verified.
The observer covers renderer/array/presenter APIs; the diagnostic offscreen
target and two timestamp readback buffers per picture are harness work, outside
those production resource counters. Blitter-internal API calls and hidden driver
allocations are not separately counted. [timeline.csv](timeline.csv),
[packets.csv](packets.csv) and [diagnostics.csv](diagnostics.csv) retain bounded
derived summaries; the full callback/packet/phase journals remain local-only.

## Change and invariants

Both canonical readers prepare `F` records in order, including absolute
protocol-13 keys, duplicates, capacity and coordinate checks. Presentation
stages only the new records and runs the same factories/serial sequence. A later
factory or numeric error discards the appended candidate before any publication.
An externally shared food snapshot still detaches with a fallible reservation.
Only after the whole listing succeeds are received state and food published.
Empty/single-entry listings keep the existing lane/fallback; multi-command
packets, mode changes and unsupported protocols keep full staging.

Sector clears retain the canonical received scan, reverse presentation scan,
swap-removal order, token retirement, counters, elapsed header and visual-tail
sync. `W` retains its existing ignored counter and opaque-body semantics. Session
and HUD admission precede native publication. No smoothing, protocol output,
draw planning, quality default or GPU code changes.

## Matched results

See [runs.csv](runs.csv). Alternating A/B pairs reverse order on even pairs;
load and Cargo/rustc/test/archive processes are sampled around each run and
approximately every 250 ms. Only observed compiler overlap excludes a sample;
all runs and exclusions remain retained. Fresh processes preserve cold CPU
resource creation. Timings leave phase observers and full-output formatting off.
Numbers measure this restricted workload, not browser-versus-native FPS.

From the repository root, standard-library Python recomputes every result table:

    python research/client-performance/2026-11-02-join-sectors/recompute.py --check

<!-- BEGIN RESULTS -->
**default**: accepted control/candidate 5/5; excluded 0. Observed load 0.0–37.9%.

| Median of accepted per-run summaries (range) | Before | After |
| --- | ---: | ---: |
| Ten-second processing elapsed | 12.865 (12.335–13.129) s | 11.799 (11.797–12.073) s |
| Process CPU | 10.406 (9.828–10.781) s | 9.547 (9.391–9.844) s |
| Requested allocation | 21.383 (21.383–21.383) GB | 12.690 (12.690–12.690) GB |
| All callbacks p50 | 25.443 (24.341–25.849) ms | 24.589 (24.447–25.218) ms |
| All callbacks p95 | 31.009 (29.999–32.305) ms | 30.110 (29.489–30.967) ms |
| Worst callback | 599.731 (552.662–763.342) ms | 96.487 (92.395–107.672) ms |
| Sector-refresh callback | 599.731 (552.662–763.342) ms | 25.125 (23.948–28.096) ms |
| Sector-refresh ingress | 595.316 (547.455–758.980) ms | 20.738 (19.846–22.937) ms |
| First two seconds: largest callback | 140.807 (136.807–146.187) ms | 96.487 (92.395–107.672) ms |
| Art interval: callback p50 | 27.125 (25.692–27.305) ms | 26.034 (25.812–26.497) ms |
| Art interval: callback p95 | 31.694 (30.344–32.669) ms | 30.200 (29.773–31.746) ms |
| Recovered interval: callback p50 | 3.240 (3.165–3.441) ms | 3.182 (3.108–3.409) ms |
| Art interval: callbacks over 16.7 ms (of 300) | 300.000 (300.000–300.000) callbacks | 300.000 (300.000–300.000) callbacks |
| Recovered: callbacks over 16.7 ms (of 120) | 0.000 (0.000–0.000) callbacks | 0.000 (0.000–0.000) callbacks |
| Longest consecutive over-budget virtual interval | 6183.333 (6183.333–6183.333) ms | 6183.333 (6183.333–6183.333) ms |
| Complete-history ingress | 0.993 (0.914–1.166) s | 0.285 (0.276–0.292) s |
| Complete-history redraw | 4.914 (4.648–5.024) s | 4.680 (4.634–4.842) s |
| Complete-history CPU submit | 3.375 (3.349–3.458) s | 3.368 (3.332–3.449) s |
| Complete-history GPU passes | 0.944 (0.942–0.946) s | 0.941 (0.938–0.947) s |

| Complete pair | Before / after refresh callback | Reduction |
| --- | ---: | ---: |
| 1 | 599.731 / 27.602 ms | 95.4% |
| 2 | 580.675 / 23.948 ms | 95.9% |
| 3 | 552.662 / 25.125 ms | 95.5% |
| 4 | 763.342 / 24.102 ms | 96.8% |
| 5 | 635.094 / 28.096 ms | 95.6% |

Paired refresh reduction: median 95.6%, range 95.4–96.8%.

**whole_map**: accepted control/candidate 5/5; excluded 0. Observed load 0.0–43.8%.

| Median of accepted per-run summaries (range) | Before | After |
| --- | ---: | ---: |
| Ten-second processing elapsed | 4.728 (4.718–4.990) s | 4.195 (3.683–4.201) s |
| Process CPU | 3.812 (3.688–4.062) s | 3.109 (2.609–3.141) s |
| Requested allocation | 19.886 (19.886–19.886) GB | 11.192 (11.192–11.192) GB |
| All callbacks p50 | 5.717 (5.627–5.865) ms | 5.852 (4.950–5.996) ms |
| All callbacks p95 | 8.841 (8.172–9.346) ms | 9.545 (7.504–9.780) ms |
| Worst callback | 715.459 (706.045–744.823) ms | 58.257 (55.896–62.337) ms |
| Sector-refresh callback | 715.459 (706.045–744.823) ms | 25.062 (20.864–26.656) ms |
| Sector-refresh ingress | 711.408 (701.999–739.891) ms | 21.376 (17.465–22.350) ms |
| First two seconds: largest callback | 130.340 (117.289–132.683) ms | 58.257 (55.896–62.337) ms |
| Art interval: callback p50 | 6.169 (5.994–6.351) ms | 6.219 (5.472–6.443) ms |
| Art interval: callback p95 | 9.060 (8.497–9.396) ms | 9.926 (7.716–10.632) ms |
| Recovered interval: callback p50 | 2.491 (2.431–2.696) ms | 2.449 (2.035–2.569) ms |
| Art interval: callbacks over 16.7 ms (of 300) | 0.000 (0.000–1.000) callbacks | 0.000 (0.000–0.000) callbacks |
| Recovered: callbacks over 16.7 ms (of 120) | 0.000 (0.000–0.000) callbacks | 0.000 (0.000–0.000) callbacks |
| Longest consecutive over-budget virtual interval | 33.333 (33.333–33.333) ms | 16.667 (16.667–16.667) ms |
| Complete-history ingress | 1.060 (1.038–1.114) s | 0.282 (0.211–0.311) s |
| Complete-history redraw | 1.534 (1.470–1.612) s | 1.587 (1.303–1.631) s |
| Complete-history CPU submit | 0.163 (0.162–0.174) s | 0.166 (0.161–0.171) s |
| Complete-history GPU passes | 0.319 (0.318–0.324) s | 0.320 (0.287–0.328) s |

| Complete pair | Before / after refresh callback | Reduction |
| --- | ---: | ---: |
| 1 | 715.459 / 25.511 ms | 96.4% |
| 2 | 720.370 / 25.062 ms | 96.5% |
| 3 | 744.823 / 26.656 ms | 96.4% |
| 4 | 706.045 / 24.188 ms | 96.6% |
| 5 | 709.030 / 20.864 ms | 97.1% |

Paired refresh reduction: median 96.5%, range 96.4–97.1%.

<!-- END RESULTS -->

## Validation and remaining work

- Exact numeric callback observations and complete prepared world/overlay/
  resource/send consistency digests are compared in default and whole-map views.
  These 64-bit digests are consistency controls, not cryptographic proofs or GPU
  readback comparisons. Bounded identities and final parity results accompany
  the per-run CSV; no identifying trace columns are committed.
- Authored comparisons force the ordinary multi-command path and normalize only
  the added ignored-command counters. They cover split listings, duplicate keys,
  both protocols, cached spawns, eaten duplicates, detached own objects, repeated
  clears, elapsed headers, malformed bodies and opaque `W` lengths.
- A late factory failure checks the live client and frozen snapshot against
  independently owned full-state strings. Deliberately publishing the staged
  prefix on that error makes the test fail; the mutation was restored. A
  separately authored 25,000-pellet white rectangle animates/prepares and clears
  without removing other-sector food. It contains no copied art or layout.
- Allocation regression compares the same repeated sector refresh at 64 and
  6,000 unrelated foods. It measures allocation growth, not wall-clock speed.
- Requested bytes for five warmed 32-pellet list/clear cycles are
  634,530 / 632,010 (protocol 13) and 634,530 / 629,490 (protocol 19), at
  64 / 6,000 unrelated foods. Existing vector capacity is warmed equally first;
  legitimate one-time capacity growth is not a whole-world snapshot regression.
- Full `npm run check` passes on the final source, including strict native,
  profiling and Wasm Clippy, desktop/Wasm builds, core/native regressions and
  real Canvas ABI checks. Seven documentation checks, LF/whitespace and source
  guards pass. Bounded evidence verification is recorded on the PR.
  Earlier failed compile/lint/contract probes remain external.

All 20 alternating runs are accepted; none has observed Cargo/rustc/test/archive
overlap. The worst-callback reduction is distinct from overall p95: whole-map
p95 rises from 8.841 to 9.545 ms, within the retained run spread, and is not
claimed as an improvement. Default-view art remains above 16.7 ms for all 300
callbacks in the measured seconds 2–7 on both arms; the longest over-budget
virtual sequence remains 6.183 seconds. The sector fix does not shorten that
sustained drawing interval in this deterministic history.

Ranked remaining cost: (1) sustained art-period CPU preparation/submission, still
above 16.7 ms in the diagnostic; (2) cold CPU HUD/raster preparation; (3) remaining
received/dense sector scans and non-sector ingress bursts; (4) GPU pass time at
the measured resolution. This PR removes repeated transaction copies from the
largest ingress burst; it does not claim owner-play acceptance or resolve the
sustained FPS complaint. NTL smoothness is owner-reported; NTL was not timed here.
No GPU pixels, stock-art fidelity, physical input/scanout, live rubber-banding,
higher-resolution iGPU budget or owner-play acceptance is established. The live
candidate smoke establishes connection/processing only, not acceptance of FPS.
