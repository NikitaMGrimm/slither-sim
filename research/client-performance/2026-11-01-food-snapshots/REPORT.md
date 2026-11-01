# Dense community-client food snapshots

The largest removable cost in the retained dense workload was copying every
retained food presentation object while staging unrelated world transactions.
`FoodWorld` now shares its private array until food changes. Every mutating path
detaches before writing; reverse traversal, swap removal, duplicate identity,
serials, animation arithmetic and rollback remain unchanged. No quality or
smoothing setting changes.

This is a client performance study, not a server-mechanics claim or owner-play
acceptance. Inputs are restricted community-server recordings. This note publishes
aggregate measurements; recordings, the diagnostic harness, raw traces and failed
attempts remain in owner-retained external storage.

## Matched replay

Ten baseline/candidate pairs were alternated without a reserved quiet window.
Process lists were sampled before/after each run and approximately every 500 ms;
CPU load and Cargo, rustc, test, archive-tool and (later runs) Node processes were
retained. Individual samples with observed rustc overlap were excluded. Five
baseline and six candidate default-view runs remained. Background tests continued;
CPU-load snapshots ranged from 24% to 85%, including the zoom pair.

| Matched 6.1 s history | Baseline | Food COW |
| --- | ---: | ---: |
| Process elapsed, median (range) | 43.650 s (38.550-47.294) | 16.953 s (15.030-17.844) |
| Process CPU, median (range) | 43.094 s (37.813-46.188) | 16.516 s (14.641-17.516) |
| Requested allocation bytes, complete history | 265.60 GB | 100.64 GB |
| Populated callback wall, median / p95 | 33.44 / 388.40 ms | 25.14 / 134.06 ms |
| Populated redraw preparation, median / p95 | 28.54 / 35.93 ms | 21.20 / 26.07 ms |

Elapsed time falls **61.2%**, allocation bytes **62.1%**. Callback entries are
medians of the accepted per-run quantiles, not pooled percentiles or a sum of
stage percentiles. Three complete adjacent pairs survived exclusion on both
sides: reductions were 61.0%, 59.1% and 58.2%. The per-side medians above also use
clean individual samples whose adjacent partner overlapped compilation.

| Pair | Before (s) | After (s) | Samples excluded for compilation |
| --- | ---: | ---: | --- |
| 1 | 38.550 | 15.030 | none |
| 2 | 41.881 | 16.310 | before |
| 3 | 43.952 | 16.098 | after |
| 4 | 43.994 | 17.488 | before, after |
| 5 | 40.818 | 17.405 | before |
| 6 | 43.650 | 17.844 | none |
| 7 | 47.294 | 17.416 | after |
| 8 | 47.838 | 17.187 | before, after |
| 9 | 48.501 | 16.514 | before |
| 10 | 41.646 | 17.393 | none |

All samples completed the full input without picture skips and returned the same
sent-command digest. Excluded samples remain in the table and external receipts.
One additional zoom-out pair had no observed compile overlap: 50.195 -> 15.296 s;
it is a single, noisy diagnostic pair, not the headline comparison.


The input contains 32,106 received WebSocket messages over 6.012 seconds, including
25,451 compact food-spawn messages. The protocol-13 world reaches 31,467 unique
received food keys, 40 simultaneous received snakes and zero prey. Default views
reach 118,750 world/overlay quads. The driver schedules this input on 366 fixed
60 Hz callbacks through production `LocalGame`, using default Original policy,
procedural materials and the normal effect/presentation acknowledgement fences.
Replay opens no socket. Input loading and process setup are outside frame timing.

The processing comparison uses CPU presentation completion without GPU submission
or sleeps. It is not gameplay FPS. Both sides retain the same virtual clock and
quality feedback. The old capture has no recurring ping responses: 266 callbacks
have the same lag flag on both sides. It is a deterministic load reproducer,
not a claim about steady network latency.

Separate validation compares every callback's received counts, draw counts, zoom,
lag state, own raw/corrected head, heading and speed: zero mismatches. A separate
run hashes the complete Debug representation of every prepared world/overlay draw,
resource image and sent packet in order; both return `6d77e7c537dbfdd1`. This is a
64-bit consistency digest, supported by exact authored regression assertions;
it is not a cryptographic proof or a visual/feel acceptance test.

## Live diagnostics

Four manual offscreen attempts were bounded to 60 seconds each. Two earlier
attempts ended with socket errors before the probe serialized its frame journal;
their captures and error logs remain retained. The completed baseline zoom-out
run ended naturally at death after 27.0 seconds. The candidate default-view run
retained 11.86 seconds before a Windows socket-abort error; its trace is explicitly
incomplete. These different live inputs are descriptive, not a before/after pair.
No further live retries were made.

The GPU path uses `WorldPresenter`, 960x540, Original, DX12, MSAA4, on the AMD
Radeon integrated GPU. The ordinary renderer requests a low-power adapter.
Pass timestamps sum world, blit and overlay GPU intervals. GPU completion and
query readback are serialized outside CPU prepare/submit intervals. The diagnostic
readback itself adds overhead to callback cadence; there is no surface or scanout.

| Observation | Baseline, zoomed out | Candidate, default (partial) |
| --- | ---: | ---: |
| Received messages | 32,661 | 25,175 |
| Peak received food / snakes / prey | 31,041 / 10 / 0 | 31,163 / 7 / 0 |
| Populated-frame CPU stage sum, median / p95 | 14.50 / 310.81 ms | 19.37 / 27.58 ms |
| Socket poll, median / p95 | 0.014 / 0.022 ms | 0.013 / 0.019 ms |
| Ingress, median / p95 | 1.34 / 285.36 ms | 1.73 / 2.67 ms |
| Redraw preparation, median / p95 | 12.60 / 21.74 ms | 11.18 / 15.60 ms |
| Resource sync, median / p95 | 0.008 / 0.270 ms | 0.019 / 0.232 ms |
| GPU submission CPU, median / p95 | 0.440 / 0.620 ms | 5.707 / 10.091 ms |
| GPU pass time, median / p95 | 1.48 / 1.69 ms | 8.63 / 11.20 ms |
| Callback interval, median / p95 / max | 17.77 / 32.69 / 609.69 ms | 31.31 / 40.01 / 239.05 ms |
| Admitted input to flush, p95 / max | 435.07 / 589.97 ms (48 commands) | 3.89 / 26.45 ms (24 commands) |
| Message receipt to apply, p95 / max | 53.02 / 579.58 ms | 2.67 / 246.83 ms |

Populated-frame rows select model time >=1 second and >=10,000 received food keys.
The callback statistics use adjacent selected callbacks; sparse unselected gaps
are retained. Message/input joins use the complete baseline trace and the observed
candidate prefix; neither dropped records. Candidate counts exclude its final
uncompleted callback from frame totals. Input begins at a synthetic callback before
the ingress drain and ends at the actual native transport flush. Physical input,
remote steering response and input-to-photon latency were not measured.

Preparation observer medians/p95 across all diagnostic callbacks separate food,
snake geometry/body and minimap work:

| CPU layer | Baseline zoom-out | Candidate default (partial) |
| --- | ---: | ---: |
| Initial frame decode/session header | 0.0055 / 0.0222 ms | 0.0043 / 0.0074 ms |
| Snake curve preparation, including its transaction snapshot | 0.72 / 3.09 ms | 0.27 / 0.58 ms |
| Food draw planning | 0.16 / 0.79 ms | 1.65 / 3.98 ms |
| Prey draw planning and eater projection | 0.28 / 0.36 ms | 0.39 / 0.58 ms |
| Snake body/face assembly | 0.024 / 0.040 ms | 0.14 / 0.57 ms |
| Minimap composition | 3.05 / 3.67 ms | 2.73 / 3.70 ms |
| HUD completion and final validation (derived span remainder) | 0.055 / 0.078 ms | 0.76 / 2.57 ms |

The decode interval covers the first borrowed frame read and session header;
subsequent full command decoding is included in ingress. HUD completion is derived
by subtracting the observer's build-to-minimap span from the matching latency
build-to-completion span (811/811 and 438/438 ordered builds matched). It includes
final validation, rather than claiming an isolated text-rasterization cost.

Position corrections were also observed. Across all callbacks, own head-correction magnitude peaked at 1004.0 world units in the baseline zoom-out run and 0.0 in the partial candidate default run. Corrected-head displacement per callback had p95/max 29.2/144.3 and 3.1/7.2 world units, respectively. These are local
presentation measurements under synthetic steering. They do not establish a
separate server/network rubber-banding cause, and smoothing was not changed.

## Remaining candidate attribution on the same replay

A separate trace-enabled candidate replay completed all 32,106 messages and 366
callbacks, without dropped trace records or picture skips. It took 26.32 seconds
under concurrent checks; these diagnostic durations are excluded from the primary
performance comparison. Of 26.0 seconds inside callbacks, ingress accounted for
16.8 seconds and redraw preparation for 9.2 seconds. The outer ingress snapshot
observer accounted for 3.46 seconds, and core apply after that snapshot for 11.00
seconds. Initial borrowed frame decode/session-header work was only 5.8 ms total.

The highest packet-family receive costs were motion `3` (2,616 messages,
6.44 seconds), food consume `c` (639, 3.34 seconds), anchor `g` (976,
2.45 seconds), motion `5` (919, 2.18 seconds), and growth `n` (225,
0.66 seconds). Packet-family intervals include receipt and core application, not
socket waiting. In an accepted untraced candidate run, ingress accounted for
**87.5%** of the slowest 16 populated callbacks; those callbacks contained 53-724
messages each. One 724-message callback took 784.0 ms, including 762.6 ms ingress
and 20.4 ms redraw preparation. This identifies per-message world work as the
next p95 target, ahead of optimizing food draw work.

Observer totals for the same diagnostic replay were 1.68 seconds for entity
advance after its outer snapshot, 1.56 seconds food planning, 1.14 seconds minimap,
0.78 seconds resource planning, 0.67 seconds bodies/faces, 0.36 seconds curves,
and 0.19 seconds the outer advance snapshot. Spans are disjoint within each
observer lane; these totals do not include every callback operation and must not
be summed with the encompassing ingress/redraw intervals.

## Remaining cost and limits

1. **Food-array transaction copies (fixed):** the isolated source change removes
   about 165 GB of requested allocation per history and 26.70 s from the medians.
   Existing single-food admission already avoids these copies; unrelated packet
   and frame transactions were still copying the entire food presentation array.
2. **Remaining per-message world transactions:** the candidate diagnostic replay
   still spends most of its ingress interval in outer world snapshotting and
   the core transaction. Received food maps and snake/body state still copy;
   food-changing transactions also detach the retained food array. Sparse packet
   bursts dominate the remaining 134 ms callback p95 and must be profiled before
   choosing the next optimization.
3. **Per-callback preparation:** the candidate's matched populated-frame redraw
   median/p95 remains 21.20/26.07 ms even with no GPU. Food planning, minimap
   composition and remaining snapshots are measurable contributors. Default views
   prepare up to 118,750 quads.
4. **GPU/submit cost:** the partial default-view diagnostic has 11.20 ms p95 GPU
   time, plus resource synchronization/submission CPU. This remains material but
   cannot explain multi-hundred-millisecond ingress/input stalls.

The 16.7 ms frame budget is still exceeded. This change removes the largest
verified copying cost; it does not establish smooth 60 Hz gameplay on this server.
Owner play, physical input/scanout, stock-art pixel parity, 1080p/4K and the discrete
GPU were not tested. Windows socket aborts remain unattributed.

## Provenance and checks

- Base: `3209d873d318b102c19521acaea6f8799a1bee4b`.
- Measured candidate food source blob: `fda137e0dae23d098998a00133059ea393445d0c`.
- Capture SHA-256: `84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82`.
- Diagnostic harness SHA-256: `306af00f26dd5f9d52f7c6e9c6941936860592a2c29744b4c4d8237dbc18f3ed`.
- Baseline executable SHA-256: `e5eb51e458ec83d08f41a30087511ea6e1673b2042313661ad1c45af4ce1a1fd`.
- Candidate executable SHA-256: `0b919c09e3c622c3b206c7067ad5cda379222e0710dbcb87138de63d54aab0fd`.
- Rust 1.99.0, release, repository profiles, two build jobs, dedicated target
  directory; AMD Ryzen 7 9800X3D. Both builds include the same passive
  `frame-profile` hooks and calling-thread System allocation counter. Primary
  timing disables stage/latency tracing and output hashing. Allocation figures
  are requested bytes/calls, not live heap, worker-thread heaps or GPU memory.
- A stale Cargo artifact caused by copying an older-mtime source was detected
  before measurement and retained separately. Final executable identities above
  come from forced-fresh builds; the stale artifact is excluded.
- Focused authored food tests: 18 food-world + 8 client-food tests passed.
- Native food-ingress tests: 4 passed; 2 manual timing cases intentionally ignored.
  The new 6,000-food/three-snapshot allocation regression requests 1,395,471 bytes,
  below one full 2,544,000-byte food array, with exact client-value equality.
- `npm test` and seven documentation checks passed. Full `npm run check` is
  running at PR opening; its result will be recorded before handoff.
