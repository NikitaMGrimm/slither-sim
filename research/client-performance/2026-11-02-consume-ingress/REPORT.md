# Singleton consume ingress

This follow-up remeasures main after #428 received snapshots, #429 frame
preparation reuse and #433 GPU bandwidth changes. Whole-map CPU redraws are
shorter, but due packet bursts still cause main-thread callback spikes. The
separate current-main profile identifies consumes as the largest packet family.

## Change

Singleton consumes use the canonical protocol-13/19 readers and cache resolution,
then commit the validated food removal/flight and received caches without the
generic whole-world transaction copies. No fallible model operation follows
validation. Existing COW detaches when an external snapshot is retained; ordinary
owned ingress avoids copying the food presentation array and unrelated snakes.
Session admission and its empty HUD journal commit are validated before the core
transition. Mode changes and batches retain the existing staged transaction.

The ordinary decoder shares prepare/commit helpers with the fast path. Duplicate
keys still select the newest presentation instance; repeat consumes reset its
progress; unknown eaters, cached sectors/eaters, tokens across ID reuse and
immediate removals retain their semantics. Counters saturate and elapsed headers
are retained. Food retirement and detail-pressure tail synchronization still run.
No motion smoothing, frame schedule, renderer, GPU or quality setting changes.

## Method and provenance

- Source base: 1b09894d3d11ba819474381fc33c68bd705da933, current main at start.
- Input SHA-256: 84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82.
- The same restricted 6.1 s history, fixed 60 Hz virtual schedule, Original quality
  and 960x540 procedural CPU presentation run on both release binaries. Both views
  complete 366 callbacks and 32,106 messages with zero picture skips. Default view
  uses zero wheel notches; whole-map view uses -120. Replay creates no socket,
  window, GPU work or pacing sleeps. No new live connection was made.
- Both binaries use the same retained diagnostic driver and passive stage hooks.
  Stage/hash observers are disabled during timing. Separate full-output hash and
  attribution runs are excluded. Executable, driver, source-overlay and input
  identities are retained in [identities.json](identities.json); raw source
  overlays, executables, recordings and journals remain outside Git under D:.
- Each timing run is reduced immediately to [runs.csv](runs.csv), including its
  pair/view/arm, exclusion and reason, compiler/Cargo/test/archive observations,
  CPU load, elapsed/process CPU, allocation requests and per-callback quantiles.
  Raw frame/process journal hashes bind those summaries. Only numeric summaries
  and hashes are published, with no endpoint, player identity or positions.
- The host is a Ryzen 7 9800X3D desktop. A/B runs alternate within each view with
  load/process snapshots around each sample and approximately every 250 ms. No
  quiet interval is booked. An observed rustc overlap excludes a run individually;
  other background load is retained and limits precision. None observed is not
  continuous proof of compiler absence. No exclusion depends on timing values.
- Populated rows select model time >=1 s and >=10,000 received food keys: 302
  callbacks. Per-run p50 is the median, p95 is sorted index floor(.95*(N-1)).
  Aggregate entries are medians of per-run summaries, not pooled quantiles.
  Requested bytes sum every callback and describe allocator requests, not live
  heap size. Launcher elapsed includes startup/output/exit observation; callback
  clocks exclude loading and final serialization. Main-thread callback wall,
  ingress, redraw preparation and the largest callback are reported separately.

From the repository root, Python 3.11+ and only its standard library recompute
every timing and profile table:

    python research/client-performance/2026-11-02-consume-ingress/recompute.py --check

## Results

<!-- BEGIN RUN TABLES -->
View: **default**. Accepted control/candidate **6/6** of 6 per arm; load snapshots **8-71%**.

| Per-run summary; medians across accepted runs | Control | Candidate |
| --- | ---: | ---: |
| Elapsed median (range) | 6.389 (6.036-7.454) s | 5.626 (5.208-6.878) s |
| Process CPU median (range) | 6.250 (5.891-6.578) s | 5.305 (4.859-6.000) s |
| Requested allocations | 33.231 GB | 23.662 GB |
| Callback p50 / p95 | 13.655 / 37.509 ms | 14.198 / 26.694 ms |
| Ingress p50 / p95 | 0.000 / 25.110 ms | 0.000 / 12.175 ms |
| Redraw preparation p50 / p95 | 12.126 / 15.075 ms | 12.582 / 15.041 ms |
| Largest callback median (range) | 246.847 (216.260-280.035) ms | 101.952 (91.715-114.072) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 35.494 / 23.948 ms | 32.5% | 19.3% |
| 2 | 38.102 / 26.574 ms | 30.3% | 13.3% |
| 3 | 41.071 / 30.421 ms | 25.9% | 7.7% |
| 4 | 42.426 / 26.815 ms | 36.8% | 15.5% |
| 5 | 36.916 / 27.217 ms | 26.3% | 10.4% |
| 6 | 35.618 / 23.276 ms | 34.7% | -8.8% |

Paired p95 reduction median (range): **31.4% (25.9-36.8%)**. Paired elapsed reduction: **11.8% (-8.8-19.3%)**.
Requested-byte reduction: **28.8%**.

View: **whole map (-120 wheel notches)**. Accepted control/candidate **6/6** of 6 per arm; load snapshots **3-74%**.

| Per-run summary; medians across accepted runs | Control | Candidate |
| --- | ---: | ---: |
| Elapsed median (range) | 3.976 (3.764-4.150) s | 2.874 (2.473-3.772) s |
| Process CPU median (range) | 3.883 (3.203-4.047) s | 2.812 (2.453-3.141) s |
| Requested allocations | 32.104 GB | 22.530 GB |
| Callback p50 / p95 | 4.713 / 31.983 ms | 4.986 / 16.172 ms |
| Ingress p50 / p95 | 0.000 / 27.242 ms | 0.000 / 11.510 ms |
| Redraw preparation p50 / p95 | 3.918 / 4.932 ms | 3.951 / 5.057 ms |
| Largest callback median (range) | 238.484 (187.722-278.799) ms | 100.535 (86.229-115.949) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 33.428 / 16.086 ms | 51.9% | 30.6% |
| 2 | 29.843 / 15.674 ms | 47.5% | 28.7% |
| 3 | 33.712 / 16.786 ms | 50.2% | 29.3% |
| 4 | 32.186 / 16.259 ms | 49.5% | 27.7% |
| 5 | 31.781 / 20.242 ms | 36.3% | 4.8% |
| 6 | 24.336 / 14.607 ms | 40.0% | 34.3% |

Paired p95 reduction median (range): **48.5% (36.3-51.9%)**. Paired elapsed reduction: **29.0% (4.8-34.3%)**.
Requested-byte reduction: **29.8%**.

Separate diagnostic runs, excluded from timing:

| View | Family/interval | Control | Candidate |
| --- | --- | ---: | ---: |
| default | 639 consumes | 1.0544 s | 0.0308 s |
| default | 976 anchors | 0.5250 s | 0.5464 s |
| default | 225 growth anchors | 0.1282 s | 0.1434 s |
| default | Host snapshots | 0.2932 s | 0.2616 s |
| default | Received snapshots | 0.3845 s | 0.3200 s |
| default | Core snapshots | 0.3025 s | 0.2638 s |
| zoom | 639 consumes | 1.1068 s | 0.0211 s |
| zoom | 976 anchors | 0.5019 s | 0.4897 s |
| zoom | 225 growth anchors | 0.1146 s | 0.1309 s |
| zoom | Host snapshots | 0.3085 s | 0.2335 s |
| zoom | Received snapshots | 0.3467 s | 0.2690 s |
| zoom | Core snapshots | 0.3113 s | 0.2373 s |

<!-- END RUN TABLES -->

The single-run profile rows are attribution only, with uncontrolled background
load. They are not per-opcode speed-ratio benchmarks. Snapshot intervals are
disjoint, while packet-family totals include their snapshots: do not add those
categories. Raw opcode labels classify frames by their first command.

## Validation and limits

- All 366 callback state/count/clock/correction/lag/zoom/own-motion observations
  match in both views. Separate ordered hashes of every prepared world/overlay
  draw, resource and sent packet match; their values are in identities.json.
  This is exact CPU output consistency, not cryptographic, GPU pixel, stock-art,
  physical-input/scanout or owner-play acceptance.
- Authored exact comparisons use the ordinary batch path as an oracle for both
  protocols, cached forms, duplicate keys, repeat consumes, unknown/reused eater
  IDs, elapsed headers, truncations and rejected-batch rollback. Native allocation
  regression sends 100 consumes with 64 and 6,000 foods; both request 16,100 bytes.
- Full npm run check passes on this production source, including native/core
  tests, strict Windows native/profiling/Wasm Clippy, desktop/Wasm builds and
  real Canvas ABI checks. It uses two jobs and the dedicated D: target, dev
  opt-level 1 and disabled debug info. Tests open no window/GPU or game connection.
- All 24 timing summaries match retained receipt/frame/process/binary checks;
  every run also matches the reference's all-frame numeric-observation SHA-256.
  Six complete pairs per view remain, with no compiler exclusions. Background
  load is recorded; one default elapsed pair regresses and remains included.
- The recomputation/report check, deliberately invalid-summary checks, all seven
  documentation tests and repository guards pass. The first documentation check
  caught the report's missing index link; adding that link resolves it.

Remaining packet costs are ranked again after this change rather than carrying
forward the older 0.85/1.36-second figures. The largest remaining family is
anchors; generic received and presentation snapshots still copy unrelated state.
The CPU replay does not establish iGPU display pacing, high-resolution GPU cost,
live rubber-banding or a 60 Hz budget pass. Even with the whole-map p95 near
16.7 ms, worst callbacks remain far above it. Bursts still execute in one callback;
deferring due packets would change the state visible to the fixed frame schedule.
