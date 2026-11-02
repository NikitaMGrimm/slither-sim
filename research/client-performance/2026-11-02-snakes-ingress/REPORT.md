# Share unchanged snake snapshots

Generic ingress still copies every snake's animation state in host/core
transaction snapshots. This study remeasures main after #428, #429 and #433,
then tests sharing untouched objects, including the incremental effect after
the separately published consume fix.

## Change

Each private retained snake entry owns an Arc of its animation object. A packet
snapshot shares that object; mutation detaches only the touched snake. Setup,
packet updates, advance, draw preparation, gaze, death and detail-pressure changes
explicitly obtain independent mutable objects. Singleton motion still stages its
changed snake before committing. Public getter types, received state, camera,
event order and quotas stay unchanged. No smoothing, interpolation, visual, GPU,
frame-scheduling or quality policy changes; no CPU-specific path or dependency.

The production diff is independent of the consume fix and based on main.
A diagnostic-only combined binary measures the added effect after consumes
stop staging whole worlds; that source is not included in this PR.

## Method and provenance

- Source base: 1b09894d3d11ba819474381fc33c68bd705da933.
- Input SHA-256: 84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82.
- The same restricted 6.1 s history, release profiles, 60 Hz virtual schedule,
  Original quality and 960x540 procedural CPU presentation run on both arms.
  Default view uses zero wheel notches; whole-map uses -120. Replay opens no
  socket, window or GPU and has no pacing sleep. No new live session was collected.
- The independent comparison is main versus snake sharing; the incremental one
  is the retained consume binary versus consume plus snake sharing. Each uses
  six alternating pairs per view, without reserved quiet time. Process lists
  are sampled around each run and about every 250 ms; CPU load is recorded.
  Observed rustc overlaps exclude individual runs; other load is retained.
  No selection depends on speed. None sampled is not continuous proof of absence.
- [runs.csv](runs.csv) and [combined-runs.csv](combined-runs.csv) retain every
  pair/view/arm, acceptance/reason, allocations, elapsed/process CPU, callback
  quantiles, process counts and output/artifact hashes. [identities.json](identities.json)
  binds binaries, source base, retained overlays/driver and full-output digests.
  [profiles.csv](profiles.csv) contains bounded attribution totals. Recordings,
  raw journals, executables and source overlays stay external under D:. No
  endpoint, player identity or coordinates are published.
- Runs complete 366 callbacks and 32,106 messages, without skips. Quantiles
  select 302 callbacks at model time >=1 s and >=10,000 received food keys.
  Per-run p50 is the median; p95 is sorted index floor(.95*(N-1)). The tables
  take medians of run summaries, not pooled percentiles. Requested bytes sum all
  callbacks on the calling thread; they are not live heap usage. Launcher elapsed
  includes startup/output/exit observation; callback clocks exclude loading and
  final serialization. The host is a Ryzen 7 9800X3D desktop.

Python 3.11+ and only its standard library recompute every table from repository root:

    python research/client-performance/2026-11-02-snakes-ingress/recompute.py --check

## Results

<!-- BEGIN SUMMARY -->
**Independent: main / snake snapshots**

View **default**, accepted control/candidate **6/6**; load **4-85%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 5.831 (5.757-7.640) s | 6.181 (5.295-7.851) s |
| Process CPU, median (range) | 5.750 (5.688-7.156) s | 5.266 (5.000-5.969) s |
| Allocation requests | 33.231 GB | 25.000 GB |
| callback p50 / p95 | 12.726 / 34.543 ms | 12.566 / 29.805 ms |
| ingress p50 / p95 | 0.000 / 22.329 ms | 0.000 / 17.955 ms |
| redraw p50 / p95 | 11.651 / 13.568 ms | 11.345 / 13.240 ms |
| Largest callback, median (range) | 201.630 (190.156-260.061) ms | 154.044 (149.850-203.828) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 48.547 / 35.934 ms | 26.0% | -2.8% |
| 2 | 34.604 / 29.893 ms | 13.6% | -7.0% |
| 3 | 34.198 / 29.516 ms | 13.7% | -5.6% |
| 4 | 34.483 / 29.835 ms | 13.5% | 8.8% |
| 5 | 34.058 / 29.706 ms | 12.8% | -7.6% |
| 6 | 39.533 / 29.774 ms | 24.7% | 2.9% |

Paired p95 reduction median (range): **13.7% (12.8-26.0%)**.

View **whole map**, accepted control/candidate **6/6**; load **6-70%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 3.200 (3.086-4.144) s | 2.766 (2.730-2.823) s |
| Process CPU, median (range) | 3.188 (3.047-3.531) s | 2.727 (2.703-2.781) s |
| Allocation requests | 32.104 GB | 23.873 GB |
| callback p50 / p95 | 3.786 / 25.050 ms | 3.850 / 20.588 ms |
| ingress p50 / p95 | 0.000 / 21.616 ms | 0.000 / 16.909 ms |
| redraw p50 / p95 | 3.081 / 3.928 ms | 3.011 / 3.812 ms |
| Largest callback, median (range) | 184.267 (174.162-198.684) ms | 141.036 (135.033-166.000) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 27.337 / 20.222 ms | 26.0% | 34.1% |
| 2 | 24.988 / 20.658 ms | 17.3% | 11.4% |
| 3 | 27.868 / 20.874 ms | 25.1% | 26.8% |
| 4 | 23.641 / 20.519 ms | 13.2% | 10.4% |
| 5 | 24.154 / 20.757 ms | 14.1% | 12.1% |
| 6 | 25.112 / 19.615 ms | 21.9% | 15.5% |

Paired p95 reduction median (range): **19.6% (13.2-26.0%)**.

**Incremental: consume fix / consume + snake snapshots**

View **default**, accepted control/candidate **6/6**; load **4-56%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 5.285 (4.858-5.831) s | 4.565 (4.536-4.883) s |
| Process CPU, median (range) | 4.914 (4.719-5.312) s | 4.531 (4.344-4.578) s |
| Allocation requests | 23.662 GB | 17.615 GB |
| callback p50 / p95 | 12.625 / 23.460 ms | 12.524 / 19.858 ms |
| ingress p50 / p95 | 0.000 / 10.536 ms | 0.000 / 7.361 ms |
| redraw p50 / p95 | 11.531 / 13.435 ms | 11.233 / 13.375 ms |
| Largest callback, median (range) | 100.122 (82.080-106.372) ms | 70.105 (62.730-80.719) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 22.509 / 20.515 ms | 8.9% | 6.8% |
| 2 | 25.320 / 19.826 ms | 21.7% | 16.8% |
| 3 | 23.700 / 19.890 ms | 16.1% | 22.2% |
| 4 | 24.070 / 19.756 ms | 17.9% | 21.6% |
| 5 | 22.547 / 22.307 ms | 1.1% | -0.5% |
| 6 | 23.220 / 19.768 ms | 14.9% | 7.6% |

Paired p95 reduction median (range): **15.5% (1.1-21.7%)**.

View **whole map**, accepted control/candidate **6/6**; load **4-56%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 2.425 (2.355-2.818) s | 2.146 (2.060-2.517) s |
| Process CPU, median (range) | 2.398 (2.312-2.547) s | 2.109 (2.016-2.297) s |
| Allocation requests | 22.530 GB | 16.483 GB |
| callback p50 / p95 | 3.866 / 13.491 ms | 3.781 / 10.722 ms |
| ingress p50 / p95 | 0.000 / 9.807 ms | 0.000 / 6.937 ms |
| redraw p50 / p95 | 3.093 / 3.908 ms | 3.045 / 3.824 ms |
| Largest callback, median (range) | 94.819 (76.626-110.505) ms | 59.203 (49.520-67.016) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 13.399 / 10.465 ms | 21.9% | 25.9% |
| 2 | 15.405 / 11.054 ms | 28.2% | 13.5% |
| 3 | 13.071 / 10.274 ms | 21.4% | 12.3% |
| 4 | 13.583 / 10.978 ms | 19.2% | 10.1% |
| 5 | 13.066 / 13.344 ms | -2.1% | -6.9% |
| 6 | 13.659 / 10.107 ms | 26.0% | 13.2% |

Paired p95 reduction median (range): **21.6% (-2.1-28.2%)**.

Separate whole-map diagnostic attribution (excluded from timing):

| Interval/family | Main | Consume | Snake snapshots | Combined |
| --- | ---: | ---: | ---: | ---: |
| Host snapshots | 0.3085 s | 0.2335 s | 0.1418 s | 0.1219 s |
| Received snapshots | 0.3467 s | 0.2690 s | 0.4993 s | 0.3189 s |
| Core snapshots | 0.3113 s | 0.2373 s | 0.1296 s | 0.1151 s |
| 976 anchors | 0.5019 s | 0.4897 s | 0.4197 s | 0.3729 s |
| 225 growth anchors | 0.1146 s | 0.1309 s | 0.1100 s | 0.1017 s |
| 639 consumes | 1.1068 s | 0.0211 s | 1.0298 s | 0.0265 s |

<!-- END SUMMARY -->

Profile runs are attribution only, excluded from timing, with passive timestamps
enabled and uncontrolled load. Snapshot intervals are disjoint; packet-family
totals include them and must not be added to them. Opcode labels classify frames
by their first command. Absolute times are not comparisons with older studies'
binaries/configurations or host load.

All 48 runs have no observed compiler overlap; none is excluded. The independent
default launcher-elapsed median regresses despite lower process CPU and callback
p95; the incremental whole-map series also includes one p95/elapsed regression.
These remain in the per-run tables. Host load/exit observation limit precision;
this is not a uniform wall-time improvement. The incremental data and lower
worst-callback medians support using this bounded ownership change for bursts.

## Validation

- All 366 callback observations match main in both views for independent and
  combined binaries. Every prepared world/overlay draw, resource and send
  contributes to matching consistency digests. Each timing run also matches
  the reference numeric observations and retains their SHA-256.
- An authored regression holds snapshots across packet mutation, failed batch,
  advance, redraw, death and reset, verifying unchanged-object sharing and complete
  isolation. Three production snapshots with 129 snakes request 349,794 bytes in
  both protocols, below half the full snake-payload subtotal of 1,040,256 bytes.
  This catches deep-copy regressions without a wall-time gate.
- Full npm run check passes on the independent source: core/native tests, strict
  core/native/profiling/Wasm Clippy, desktop/Wasm builds and real Canvas ABI tests.
  Additional feature-gated feel/geometry suites pass. Builds use the dedicated
  D: target and two jobs, dev opt-level 1 and disabled debug info. A new test first
  supplied an extra draw argument; correcting that authored call resolved it.
  The failed log is retained externally; no production expectation was relaxed.
- Combined binaries are release-built and exact-output checked; the full client
  sequence applies to the independent PR source, not the diagnostic composition.
- All 48 rows were independently checked against retained receipts, frames,
  process observations and binary hashes. Recomputed aggregates, invalid-summary
  rejection, documentation tests and repository guards pass before publication.

## Remaining costs and limits

Received maps still need one rollback candidate for ordinary packets. Food
listings, growth and other generic families remain. The combined whole-map profile
attributes 0.3189 s to received snapshots; they occupy about 27 ms of the worst
profiled callback. Non-growing anchor specialization warrants a separate experiment.
Extra small heap requests
replace some inline payload copies; allocation regressions and whole-history
totals expose this cost rather than assuming every allocation declines.

These are main-thread CPU measurements, without iGPU display pacing, physical
input/scanout, stock-art pixel, live rubber-banding or owner-play acceptance.
A CPU p95 below 16.7 ms in one cell does not establish a 60 Hz pass: worst
callbacks and GPU/display work remain separate.
