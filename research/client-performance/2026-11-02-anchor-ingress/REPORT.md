# Non-growing anchor ingress

After the consume and per-snake snapshot fixes, the retained whole-map diagnostic
still attributes about 27 ms of its worst callback to received-state copies.
Ordinary anchors are the largest remaining packet family. This study tests a
narrow fast path while leaving growth, batches and pressure transitions staged.

## Change and semantic boundary

A singleton non-growing anchor selects the canonical reader, then invokes the
existing received reducer with its complete snake-local context: Setup, own ID,
the touched received snake and its protocol-13 absolute-front cache. It stages
that snake and its presentation object/camera, validates the existing bounds,
and commits only after all fallible work. This avoids copying unrelated received
food/maps and presentation snakes for ordinary motion anchors.

No new movement arithmetic or smoothing is introduced. Unknown IDs, detached own
objects, relative legacy coordinates, received/presentation counters and elapsed
headers use the existing reducer/event semantics. Growth, multi-command packets,
detail-pressure worlds and potential cross-object compaction/reclamation decline
without mutation and use the full path. A pressure threshold crossed by the
candidate also declines. Batch rollback and the ordinary resource limits remain.
The fast lane conservatively declines worlds with more than 256 received IDs
or more than 65,536 received body points. The replay peaks at 31,467 received
food keys, 40 snakes and zero prey; counts are retained in identities.json.

The original measurements used an independent anchor diff on main 1b09894d and
a separately retained diagnostic composition with consume and snake sharing.
All timing numbers, allocation totals and stage durations below come from those
earlier sources, not the rebased composition. No timings were rerun for the rebase.

The production branch is now rebased onto main a23952d1 after the consume and
snake-sharing merges (#436 and #437). Anchor staging explicitly clones the
shared snake's value and publishes a new Arc only after validation. Named
received-pressure thresholds and one shared predicate keep anchor admission and
ordinary pressure transitions aligned. Fresh composed validation is described
below; it does not relabel the historical measurements as new results.

## Workload and evidence

- Timing source base: 1b09894d3d11ba819474381fc33c68bd705da933.
- Restricted input SHA-256: 84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82.
- Same 6.1 s history, release profiles, Original quality, 960x540 procedural CPU
  presentation and fixed 60 Hz virtual schedule. Zero/-120 wheel notches select
  default/whole-map views. Replay opens no socket, window or GPU and has no
  pacing sleeps. No new outside-server connection was made.
- Six alternating pairs per view/comparison, no booked quiet time. CPU load and
  compiler/Cargo/test/archive processes are sampled around runs and about every
  250 ms. Observed rustc overlap excludes a run individually; other load remains.
  None observed is not continuous proof of absence; no selection uses speed.
- [runs.csv](runs.csv) and [combined-runs.csv](combined-runs.csv) retain all
  pair/view/arm decisions, quantiles, requested allocations, elapsed/process CPU,
  process/load counts and output/journal hashes. [identities.json](identities.json)
  binds source base, binaries, driver, overlays and full-output digests.
  [profiles.csv](profiles.csv) retains bounded attribution totals. Raw recordings,
  journals, source overlays and executables stay external under D:. No server
  address, player identities or positions are published.
- All runs complete 366 callbacks/32,106 messages without skips. Quantiles select
  302 populated callbacks at model time >=1 s and >=10,000 received food keys.
  Per-run p50 is the median; p95 is sorted index floor(.95*(N-1)). Tables take
  medians of per-run summaries. Requested bytes sum all callbacks and are not
  live heap usage. Launcher elapsed includes startup/output/exit observation;
  callback clocks exclude loading/final serialization. Host: Ryzen 7 9800X3D.

From repository root, Python 3.11+ and only its standard library recompute all tables:

    python research/client-performance/2026-11-02-anchor-ingress/recompute.py --check

## Results

<!-- BEGIN SUMMARY -->
**Independent: main / anchor fast path**

View **default**, accepted control/candidate **6/6**; load **5-73%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 6.561 (5.739-8.215) s | 5.643 (5.333-6.285) s |
| Process CPU, median (range) | 6.312 (5.688-7.609) s | 5.523 (5.281-6.188) s |
| Allocation requests | 33.231 GB | 25.643 GB |
| callback p50 / p95 | 13.887 / 40.470 ms | 13.123 / 33.553 ms |
| ingress p50 / p95 | 0.000 / 26.392 ms | 0.000 / 20.677 ms |
| redraw p50 / p95 | 12.225 / 15.527 ms | 11.977 / 14.038 ms |
| Largest callback, median (range) | 248.213 (191.999-311.667) ms | 195.562 (152.996-243.662) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 42.442 / 36.079 ms | 15.0% | 8.6% |
| 2 | 41.931 / 31.028 ms | 26.0% | 17.7% |
| 3 | 39.008 / 37.091 ms | 4.9% | 3.9% |
| 4 | 33.923 / 30.245 ms | 10.8% | 6.7% |
| 5 | 34.749 / 29.932 ms | 13.9% | 7.2% |
| 6 | 48.924 / 36.788 ms | 24.8% | 29.0% |

Paired p95 reduction median (range): **14.4% (4.9-26.0%)**.

View **whole map**, accepted control/candidate **6/6**; load **2-71%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 3.226 (3.156-4.151) s | 2.795 (2.736-4.561) s |
| Process CPU, median (range) | 3.180 (3.047-3.891) s | 2.727 (2.500-4.047) s |
| Allocation requests | 32.104 GB | 24.507 GB |
| callback p50 / p95 | 3.858 / 24.859 ms | 3.888 / 20.809 ms |
| ingress p50 / p95 | 0.000 / 21.270 ms | 0.000 / 17.420 ms |
| redraw p50 / p95 | 3.151 / 4.021 ms | 3.106 / 4.020 ms |
| Largest callback, median (range) | 199.788 (182.970-211.284) ms | 175.686 (150.117-232.684) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 24.546 / 20.240 ms | 17.5% | 14.0% |
| 2 | 32.950 / 20.903 ms | 36.6% | 31.7% |
| 3 | 25.173 / 20.714 ms | 17.7% | 14.2% |
| 4 | 24.103 / 19.686 ms | 18.3% | 14.7% |
| 5 | 34.164 / 32.719 ms | 4.2% | -9.9% |
| 6 | 24.060 / 25.905 ms | -7.7% | -9.7% |

Paired p95 reduction median (range): **17.6% (-7.7-36.6%)**.

**Incremental: consume + snake snapshots / all three fixes**

View **default**, accepted control/candidate **6/6**; load **2-37%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 4.717 (4.598-4.808) s | 4.375 (4.264-4.735) s |
| Process CPU, median (range) | 4.586 (4.344-4.781) s | 4.281 (4.219-4.719) s |
| Allocation requests | 17.615 GB | 13.626 GB |
| callback p50 / p95 | 12.838 / 19.919 ms | 12.376 / 17.522 ms |
| ingress p50 / p95 | 0.000 / 7.572 ms | 0.000 / 5.008 ms |
| redraw p50 / p95 | 11.446 / 13.572 ms | 11.398 / 13.334 ms |
| Largest callback, median (range) | 68.564 (62.332-75.138) ms | 44.635 (42.186-47.280) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 19.921 / 17.364 ms | 12.8% | 6.8% |
| 2 | 19.916 / 17.510 ms | 12.1% | 7.3% |
| 3 | 19.871 / 17.503 ms | 11.9% | 6.6% |
| 4 | 20.995 / 19.510 ms | 7.1% | 1.1% |
| 5 | 19.844 / 17.533 ms | 11.6% | 5.7% |
| 6 | 21.738 / 17.627 ms | 18.9% | 8.5% |

Paired p95 reduction median (range): **12.0% (7.1-18.9%)**.

View **whole map**, accepted control/candidate **6/6**; load **5-65%**.

| Medians of accepted per-run summaries | Control | Candidate |
| --- | ---: | ---: |
| Elapsed, median (range) | 2.070 (2.044-3.050) s | 1.886 (1.772-2.338) s |
| Process CPU, median (range) | 2.047 (1.906-2.266) s | 1.805 (1.656-2.078) s |
| Allocation requests | 16.483 GB | 12.486 GB |
| callback p50 / p95 | 3.894 / 10.369 ms | 3.836 / 8.291 ms |
| ingress p50 / p95 | 0.000 / 6.629 ms | 0.000 / 4.624 ms |
| redraw p50 / p95 | 3.050 / 3.822 ms | 3.057 / 3.981 ms |
| Largest callback, median (range) | 48.102 (47.313-57.578) ms | 30.177 (29.018-33.392) ms |

| Complete accepted pair | Control / candidate callback p95 | p95 reduction | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 9.896 / 8.630 ms | 12.8% | 8.8% |
| 2 | 10.371 / 9.407 ms | 9.3% | -12.9% |
| 3 | 11.090 / 7.864 ms | 29.1% | 8.6% |
| 4 | 10.366 / 7.728 ms | 25.5% | 12.5% |
| 5 | 12.578 / 10.240 ms | 18.6% | 27.6% |
| 6 | 9.831 / 7.952 ms | 19.1% | 14.4% |

Paired p95 reduction median (range): **18.9% (9.3-29.1%)**.

Separate whole-map diagnostic attribution (excluded from timing):

| Interval/family | Main | Consume + snapshots | Anchor | All three |
| --- | ---: | ---: | ---: | ---: |
| Host snapshots | 0.3085 s | 0.1219 s | 0.2141 s | 0.0817 s |
| Received snapshots | 0.3467 s | 0.3189 s | 0.2364 s | 0.1052 s |
| Core snapshots | 0.3113 s | 0.1151 s | 0.1945 s | 0.0653 s |
| 976 anchors | 0.5019 s | 0.3729 s | 0.0195 s | 0.0194 s |
| 225 growth anchors | 0.1146 s | 0.1017 s | 0.1391 s | 0.0906 s |
| 639 consumes | 1.1068 s | 0.0265 s | 0.9390 s | 0.0198 s |

Remaining all-three whole-map diagnostic families:

| Family | Complete-history time | Populated callback contribution p95 / max | In largest callback |
| --- | ---: | ---: | ---: |
| F | 0.1375 s | 0.000 / 0.000 ms | 0.000 ms |
| n | 0.0906 s | 1.365 / 5.644 ms | 5.644 ms |
| W | 0.0725 s | 0.000 / 0.000 ms | 0.000 ms |
| s | 0.0469 s | 0.609 / 8.113 ms | 8.113 ms |
| b | 0.0440 s | 1.426 / 1.649 ms | 0.033 ms |
| h | 0.0428 s | 0.854 / 5.499 ms | 5.499 ms |
| f | 0.0266 s | 0.000 / 4.416 ms | 1.545 ms |
| c | 0.0198 s | 0.258 / 3.126 ms | 3.126 ms |

<!-- END SUMMARY -->

Profiles are separate diagnostic runs with passive timestamps enabled, excluded
from timing. Snapshot intervals are disjoint; family totals include snapshots
and must not be added to them. Opcode labels classify a frame by its first command.

## Validation and limits

Fresh release parity after rebasing onto a23952d1 compares current main against
the actual composed candidate, including the Arc adaptation and shared pressure
predicate. Each arm completes 366 callbacks and 32,106 messages without skips in
each view. Every callback's numeric observations match byte-for-byte, and the
complete prepared frame/overlay/resource/send consistency digests match current
main and the historical controls: default `b9f2870246df9495`, whole map
`78b36afd816d2773`. [rebased-parity.json](rebased-parity.json) binds the source,
driver, diagnostic shim, binaries and bounded results. Raw journals stay outside
Git. The unchanged retained driver runs in CPU-only parity mode; both diagnostic
copies add only its unused transaction-observer registration seam. These are
correctness controls, excluded from timing; the 64-bit output digests are
consistency checks, not cryptographic proofs or GPU pixel comparisons.

The new authored `staged_anchor_rejection_preserves_live_and_frozen_state` test
covers both protocols. It preserves a frozen snapshot through a successful
anchor, prepares visible geometry, then repeats a valid anchor. Canonical
received application accepts the repeated anchor, but presentation rejects its
zero-distance edge. Owned full-state strings, a cloned SnakePresentation and a
cloned camera verify the live client and frozen snapshots after failure. A
temporary mutation committed the staged received snake/caches/counters/elapsed
header on that presentation error before returning it. The test compiled and
failed at its independent live-state assertion. The production source was
restored byte-for-byte, and all four anchor transaction tests then passed.

- Exact authored comparisons against ordinary application cover both protocols,
  anchor shapes, explicit/compact targets, absent/unknown/reused own IDs, elapsed
  headers, malformed lengths and zero-distance error boundaries. Growth/batches
  and pressure worlds decline without mutation. A native allocation regression
  sends 100 moving anchors with 64 versus 6,000 retained foods and compares bytes.
- All 366 callback state observations and complete prepared draw/resource/send
  consistency digests match in both views for independent and all-three binaries.
  Every timing run also matches the reference numeric-observation SHA-256.
- Full npm run check passes on the actual rebased composition: core/native tests,
  strict Windows native/profiling/Wasm Clippy, desktop/Wasm builds and real Canvas
  ABI. The focused 224 core test executions also pass on this source. The original
  independent full suite and extra feature-gated feel/geometry checks remain
  historical evidence. Builds use the dedicated D: target, two jobs, dev opt-level
  1 and disabled debug info. Fresh composed parity is release-built separately;
  no timing samples are collected from it.
- Both comparisons retain six complete accepted pairs per view: all 48 runs
  have no observed compiler overlap. Standalone and incremental regressions
  remain in the tables, including largest-callback and elapsed regressions.
- An initial allocation fixture repeated zero-distance anchors until canonical
  presentation rejected the degenerate input. It now uses moving anchors, with a
  separate exact zero-distance/consecutive error-boundary test. The failed log
  remains external; no production behavior or expectation was changed to admit it.
- All rows were independently checked against retained frame/process receipts
  and executable hashes. Recomputed report blocks, invalid-summary rejection,
  documentation, LF/whitespace and repository guards pass before publication.

This is CPU main-thread processing, not iGPU display-pacing or owner-play
acceptance. No GPU pixels, stock-art fidelity, physical input/scanout or live
rubber-banding conclusion follows. Even improved p95 does not prove a 16.7 ms
budget pass while worst callbacks and GPU/display work remain.

## Stopping decision

Further packet specialization is not justified in this pass. In the separate
all-three whole-map profile, growth is 0.0906 s for the history and contributes
1.365 ms at populated-callback p95 (5.644 ms maximum). Supporting growth would
add own-length/HUD scheduling and cross-object pressure/rollback handling to the
narrow path. Appearances total 0.0469 s and contribute 0.609 ms p95; their rare
8.113 ms burst remains. Listings F (0.1375 s) and W (0.0725 s) occur outside the
selected populated interval in this recording, so their totals do not explain
its later crowded-frame tail. These are diagnostic bounds on the opportunity,
not predictions that all cost can be removed.

The rare mixed burst still runs in one callback. Deferring due messages changes
the state shown by the fixed frame schedule; this series does not trade that
semantics for a timing cap. The remaining main-thread tail and unverified iGPU
display pacing stay explicit rather than claiming the owner's lag is resolved.
