# Received snapshots in packet transactions

After #420 and #423, dense packet bursts still exceed a 60 Hz CPU callback
budget. This study isolates remaining consume, anchor, growth and appearance
ingress on the same restricted 6.1-second history. It collects no live session.
Frame advance, draw planning, minimap/resources and GPU changes belong to a
separate lane; this change does not edit those production paths.

## Profile and decision

The separate passive-attribution runs below split host/core snapshots, received
reduction and presentation events. Nested snapshots dominate the control's
ingress; received mutation itself is small. Consumes are also expensive because
their presentation event detaches the retained food array. Ordinary anchors
spend little time folding their presentation event.

Before this change a generic native packet copies received state in the host
world clone, the presentation reducer's pre-copy, the received reducer's rollback
clone, and the presentation candidate clone. The largest common cause is these
four full received-world copies, including unrelated food keys and snakes.
The fix retains one received candidate per transaction. It leaves the existing
snake/presentation staging and food-consumption semantics intact.

The profile tables are diagnostic attribution from one run per arm, with passive
timestamps enabled and uncontrolled background load. They are excluded from the
alternating performance series and do not establish independent per-opcode speed
ratios. The snapshot intervals are disjoint; the mutation interval begins after
the received rollback snapshot. Publication, teardown and unresolved overhead
are grouped in the residual receive bucket. Opcode rows classify each received
frame by its first command.

## Change and semantics

AnimatedClient retains its private received state through an immutable Arc.
Ingress clones share the committed state until mutation; the public received
view and ClientState's public field types are unchanged. Generic presentation
application asks the existing received reducer to return its candidate and
ordered events directly, avoiding a second defensive copy of that candidate.
The public received-state APIs still stage and commit atomically through the
same decoder and command loop.

Factories and ordered Setup callbacks continue observing the pre-transaction
received state. Received state publishes only after all fallible presentation
work succeeds. Singleton food/motion commits detach shared received state before
mutation, so retained snapshots remain independent. Reset replaces the snapshot.
Score/minimap admission, HUD command boundaries, trace geometry captures,
elapsed headers, counters and saturation remain in the canonical reducer.

This does not split packet bursts across callbacks. Deferring due packets would
change which state the fixed schedule presents, so it cannot retain identical
per-frame output. Ordinary ingress already prepares full pictures only for Setup;
it does not rebuild a complete draw list after every consume or anchor. Repeated
event folding also preserves correction arithmetic, random order and rollback
boundaries. The selected change removes redundant copying within those boundaries.

Sharing received storage can incidentally reduce cloning in existing redraws.
No frame-advance, animation, renderer, quality, smoothing, GPU or scheduling code
is changed, and this lane makes no additional redraw optimization.

## Replay and reduction method

Both executables use main source base
7ce9569285e2b6aec22f725675865ee5b3fe9a08 (main at the study's start), the same diagnostic driver, release
profiles, Original/default quality, a 960x540 procedural CPU presentation and
the same 60 Hz virtual schedule. No socket, window, GPU or wall-clock sleep is
created. Every run completes 366 callbacks and 32,106 messages without picture
skips. Loading, output serialization and teardown are outside callback timing.

[runs.csv](runs.csv) receives one row immediately after every run, including
exclusions. It contains pair/arm, acceptance reason, process-observation counts,
CPU-load snapshots, binary/source/driver hashes, complete-history requested
allocation bytes, elapsed/process CPU, per-run callback quantiles and the shared
all-frame numeric-observation digest. Raw frame
and process journals remain external; their hashes bind the reduced rows.
[profile-stages.csv](profile-stages.csv) and
[profile-messages.csv](profile-messages.csv) retain bounded attribution totals.
No packets, player identities, coordinates or recordings are published.

From the repository root, Python 3.11+ and only its standard library recompute
every timing/attribution table, selection count, range and reduction below:

    python research/client-performance/2026-11-01-received-snapshots/recompute.py --check

Per-run callback statistics select model time at least 1 second and at least
10,000 received food keys: 302 populated callbacks. p50 is the ordinary median;
p95 is the sorted value at zero-based floor(0.95 * (N - 1)). The aggregate uses
medians of accepted per-run quantiles, not pooled callback percentiles. Requested
allocation bytes sum all callbacks on the calling thread and describe allocation
requests, not live heap size. Launcher elapsed includes startup/output and the
monitor's exit-observation delay; process CPU uses the Windows process counter.

The Ryzen 7 9800X3D desktop has 16 logical processors. Runs alternate immutable
control/candidate executables without a reserved quiet interval. Process lists
are sampled around each run and approximately every 250 ms. Only observed rustc
overlap excludes an otherwise complete run; Cargo/test/archive observations and
CPU load remain recorded. A zero compiler count means none was sampled, not
continuous proof of absence. No sample is removed merely for being slow. Arms
are accepted individually; the paired table requires both arms accepted.
This is a matched comparison on that pinned main revision; absolute times must not be
compared directly with #423's earlier binaries and background load.

## Retained results

<!-- BEGIN RECOMPUTED RUN AGGREGATES -->
64 alternating pairs: accepted control/candidate **6 / 10**; excluded **58 / 54** for observed rustc.
CPU-load snapshot range: **37-97%**.

| Fixed 6.1 s history | Main control | Received-snapshot candidate |
| --- | ---: | ---: |
| Launcher elapsed, median (range) | 11.921 s (10.895-17.248) | 9.910 s (7.944-15.987) |
| Process CPU, median (range) | 11.531 s (10.734-15.750) | 9.344 s (7.781-12.531) |
| Requested allocation bytes, median | 50.13 GB | 34.06 GB |
| Populated callback wall, p50 / p95 | 25.93 / 81.46 ms | 24.04 / 62.41 ms |
| Populated ingress, p50 / p95 | 0.00 / 57.14 ms | 0.00 / 40.16 ms |
| Populated redraw preparation, p50 / p95 | 21.82 / 26.75 ms | 21.32 / 26.13 ms |
| Largest populated callback, median (range) | 542.91 ms (442.84-969.72) | 280.77 ms (212.40-833.68) |
| Ingress share of slowest 16 callbacks, median (range) | 80.3% (80.1-83.5) | 70.9% (67.8-73.9) |

Reductions: callback p95 **23.4%**; median launcher elapsed **16.9%**; requested allocation bytes **32.1%**.

| Accepted pair | Control elapsed | Candidate elapsed | Elapsed reduction | Control / candidate callback p95 |
| --- | ---: | ---: | ---: | ---: |
| 16 | 11.920 s | 8.836 s | 25.9% | 77.56 / 50.55 ms |
| 17 | 11.748 s | 9.489 s | 19.2% | 74.60 / 61.33 ms |
| 22 | 17.248 s | 15.327 s | 11.1% | 124.45 / 91.93 ms |
| 55 | 10.895 s | 9.311 s | 14.5% | 72.02 / 62.80 ms |
| 56 | 11.923 s | 13.152 s | -10.3% | 85.37 / 94.79 ms |
| 57 | 13.694 s | 11.611 s | 15.2% | 88.71 / 68.40 ms |

Complete-pair reduction, median (range): callback p95 **20.3% (-11.0-34.8%)**; launcher elapsed **14.9% (-10.3-25.9%)**.

Accepted control pairs: 16, 17, 22, 55, 56, 57.
Accepted candidate pairs: 11, 16, 17, 18, 21, 22, 54, 55, 56, 57.

Separate passive-attribution runs (excluded from the alternating timing series):

| Ingress interval | Main control | Candidate |
| --- | ---: | ---: |
| Host world snapshot | 0.8625 s | 0.4006 s |
| Core received pre-copy | 0.5102 s | 0.0000 s |
| Received rollback snapshot | 0.6228 s | 0.5685 s |
| Core presentation snapshot | 0.7780 s | 0.3888 s |
| Received decode/mutation after snapshot | 0.0067 s | 0.0064 s |
| Consume presentation events | 0.6002 s | 0.8463 s |
| Sum of the four snapshot intervals | 2.7735 s | 1.3580 s |
| Other presentation events | 0.0606 s | 0.0806 s |
| Other receive work, including validation/commit/teardown | 1.4433 s | 0.8261 s |

| Leading opcode | Messages | Control receive | Candidate receive | Control / candidate requested bytes |
| --- | ---: | ---: | ---: | ---: |
| c | 639 | 1.8257 s | 1.5082 s | 13.215 / 9.225 GB |
| g | 976 | 1.6652 s | 0.7231 s | 13.942 / 7.847 GB |
| n | 225 | 0.3991 s | 0.1800 s | 3.130 / 1.725 GB |
| s | 156 | 0.2648 s | 0.1030 s | 2.129 / 1.183 GB |
| F | 225 | 0.2145 s | 0.2021 s | 1.418 / 1.278 GB |
| h | 128 | 0.2024 s | 0.0863 s | 1.776 / 0.977 GB |
| W | 225 | 0.1386 s | 0.1027 s | 0.593 / 0.463 GB |
| b | 483 | 0.0794 s | 0.1236 s | 1.146 / 1.146 GB |

<!-- END RECOMPUTED RUN AGGREGATES -->

The initial eight pairs all overlapped sampled compilation and remain excluded.
The series expanded on accepted sample counts, reaching its minimum at pair 56.
One complete pair regressed under background load, so a fixed eight further
pairs were retained to characterize that noise. The final series has six
complete accepted pairs; pair 56's regression remains in every applicable table.
These are descriptive timings on a busy host, with an unbalanced accepted set
and substantial variation, rather than a reliable p95 improvement across loads.
The allocation reduction is identical in every run. Profiling and full-output
hashing runs never enter the timing tables.

## Validation

- Separate replay controls compare every callback's received/draw counts, model
  clock, correction, lag, zoom and own raw/corrected motion fields: zero mismatches
  over all 366 callbacks. Every prepared world/overlay draw, resource and sent
  packet contributes in order to matching 64-bit digest 46fdb4b9136c161c. This is
  a consistency control, not cryptographic, pixel or physical-feel acceptance.
- Authored regressions bound snapshot allocation against retained food history,
  and preserve frozen received/presentation state across singleton motion/food,
  consume, anchor, growth, mass, appearance, reset and rejected batch updates in
  both protocols. Existing shared-food allocation-failure coverage still passes.
- Focused core library/state/event/motion suites pass, as do native library and
  food-ingress suites. The first new regression used an invalid protocol-19
  consume opcode; the canonical reader rejected it. The corrected authored
  eater-token packet uses the existing less-than opcode. The failed run is
  retained externally and did not change production decoding or expectations.
- Full npm run check passes on the production source: core/native tests, strict
  core/native/profiling/Wasm Clippy (including the Windows test targets), Canvas
  Wasm/ABI suites, profiling checks, desktop build and WebGPU bindings. Commands
  use the dedicated D: target, two build jobs, development opt-level 1 and no
  debug info. No stale-artifact cleanup or bypass was needed in this run.
- Additional feel-trace checks pass: 84 core library tests, four core trace
  tests, one native trace test and all 17 native feel-replay/geometry cases.
  They include ordered Setup packet prefixes and retained draw observations.
- All 128 CSV rows were cross-checked against retained frame/process journals,
  launcher receipts and executable hashes. Eleven deliberately invalid summary
  cases and a changed report aggregate were rejected. Recomputed report tables,
  all seven documentation tests, whitespace and LF checks pass.

## Remaining cost and limits

The remaining covered ingress costs rank as follows in the candidate profile:

1. The snapshot subtotal remains largest: one received rollback candidate plus
   the host/core snake and presentation channel snapshots. Sharing received state
   does not eliminate these remaining copies.
2. Consume presentation work still detaches the food array, scans for the newest
   duplicate key and retires entries while retaining eater tokens.
3. The residual receive bucket includes validation, session/HUD work, publication
   and teardown. Its subcomponents were not resolved further in this profile.

These are disjoint interval categories; the per-opcode totals include those
intervals and must not be added to them. Among packet families, consumes remain
largest, followed by ordinary anchors, food listings and growth anchors.

Large due-message bursts still occupy one callback. This is not a 16.7 ms budget
pass or a gameplay-FPS claim. Redraw preparation and GPU work are owned separately;
the CPU replay neither measures nor accepts GPU pixels, high-resolution drawing,
physical input/scanout latency, live steering or owner gameplay. Concurrent tests
and the unbalanced accepted counts limit timing precision. No live-network or
rubber-banding conclusion follows from this old recording's virtual schedule.

## Provenance

- Restricted input SHA-256:
  84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82.
- Each CSV row identifies its binary, source base, retained source-overlay patch
  and shared driver by hash. The control patch contains passive stage markers;
  the candidate patch includes this implementation and equivalent markers.
  Those source snapshots, driver, executables, raw outputs and failed attempts
  remain owner-local outside Git. Final production source removes only passive
  diagnostic hooks/copy and applies formatting/comments after the measured build.
- Timing disables stage/trace/hash observers. Separate profiling enables stage
  timestamps; separate consistency controls enable complete output hashing.
  These hooks/code layout and this single workload bound the measured claim.
- Full recordings remain outside slither-sim. This report publishes selected
  numeric summaries and hashes only, under the owner's authorized diagnostics.
