# Singleton motion transactions

The food snapshot COW change removes the largest dense-client copying cost, but
its candidate still has a 25.14 ms median and 134.06 ms p95 CPU callback in the
retained 6.1-second replay. Ingress accounts for 87.5% of its slowest 16 callbacks.
A separate diagnostic replay attributes 6.44 seconds to 2,616 motion `3` messages
and 2.18 seconds to 919 motion `5` messages. These are the next p95 target.

## Change

Singleton motion messages now stage only the affected snake presentation object
and decoded received scalars. Canonical protocol-13/19 decoding, target resolution,
angle corrections, speed arithmetic, remote gaze, counters and elapsed-header
retention are shared with ordinary transactions. Both received and presentation
updates publish after the last fallible change. Unknown/detached own IDs keep their
existing semantics. Other messages, batches and unsupported protocol families use
the existing path. Detail-pressure tail synchronization remains in both paths.
Session admission still precedes application; the fast lane produces no HUD/Setup
journal and consumes no randomness.

The motion implementation is independent of the food COW source. Its branch was
rebased onto main `0aa1fa2201e92148afcdda8810fd144f107d3fdf`, after #420 merged.
The matched incremental benchmark temporarily applied the measured pre-review
food COW source to its measurement binary. The final branch inherits the reviewed
COW source from main; this change adds only the motion optimization.
This measures what the motion fix adds to the already measured COW candidate.
No visuals, smoothing, quality settings or protocol contracts change.

## Verification

- Authored exact comparisons cover both protocols, motion opcode/field shapes,
  explicit and compact targets, removed own objects, reused IDs, unknown IDs,
  invalid commands, two-byte elapsed headers and batches.
- The production LocalGame allocation regression sends 100 motion messages with
  64 and 6,000 foods: both sizes request **578,900 bytes** in each protocol.
  Allocation is independent of retained food. No timing gate, GPU or socket.
- The combined diagnostic binary processes all 32,106 messages and 366 callbacks
  of the restricted community capture. Received counts, draws, clocks, zoom,
  lag and own motion observations have zero numeric mismatches against the
  retained COW control. Every prepared draw/resource and sent packet contributes
  to matching 64-bit consistency digest `6d77e7c537dbfdd1`. This is a consistency
  check, not cryptographic or physical visual/feel acceptance.
- Focused motion/protocol-13 tests passed (11), native food/motion ingress tests
  passed (four at the initial run, plus the later fallback/decode-rollback case),
  strict core Clippy passed, and 17 feature-gated feel replay tests passed.
- Full `npm run check` passed on the independent main-based motion tree with
  per-command `CARGO_PROFILE_DEV_OPT_LEVEL=1`, debug info disabled, two build jobs
  and the required dedicated target. It covers JavaScript, core/native tests,
  strict core/native/profiling/Wasm Clippy, Canvas ABI, frame-profile checks,
  desktop build and Wasm bindings. The final log ends `All checks passed`.
  Earlier linker/stale-artifact failures and a corrected test-byte-slice lint
  are retained separately. The final check follows a client-package-only clean.

After the full check passed, the branch was rebased onto merged #420. Conflicts
were limited to the shared food-ingress test additions and research index; both
sets were preserved. `git range-diff` showed only those integration contexts.
The combined tree then passed 37 focused core tests, all seven food/motion ingress
regressions (two manual timing cases ignored), formatting and strict native Clippy.
The rebased diff contains the same motion code; its inherited COW source is
covered by these combined tests. The full multi-feature sequence is the
pre-rebase run, not relabelled as an execution on the combined head.

## Matched incremental timing

[runs.csv](runs.csv) contains one bounded summary row for each of the 56
retained runs, including exclusions. It was reduced from the existing frame CSVs,
launcher receipts and process observations; no replay or new measurement was run.
It contains no packets, player identities, positions, server address or recordings.

From the repository root, Python 3.11+ and only its standard library recompute all
rows, counts, medians, ranges and reductions in the timing block below, and check
that the published block matches:

```sh
python research/client-performance/2026-11-01-motion-transactions/recompute.py --check
```

The command also reproduces the pair-9 tail diagnostic used under Remaining cost.
`recompute.py` reads only the adjacent CSV and report. Each arm is bound to its
measured binary SHA-256 and source base commit; the base commit does not include
the diagnostic overlays described under Provenance. Each row includes the retained
frame/process-log hashes, per-run compiler-observation count, acceptance reason,
callback count/quantiles, complete-history requested allocation bytes, launcher
elapsed/process CPU and before/after CPU-load snapshots. Raw outputs stay external.
The script rejects missing/duplicate pair-arm rows, arm/binary mismatches,
incomplete workloads and acceptance flags inconsistent with compiler observations.

Per-run p50 is the median of the 302 selected callback values. Per-run p95 is the
sorted value at zero-based index `floor(0.95 * (N - 1))`; the CSV stores both in
nanoseconds. Selection is model time >=1 second and >=10,000 received food keys.
The table uses medians of accepted per-run values; its p95 is not a pooled
percentile. Exclusion is individual: a clean arm remains in its arm's aggregates
even when its paired arm overlapped compilation. The paired table includes only
pairs with both arms accepted. A rustc observation means any sampled process list
contained rustc; a zero count means none was observed, not continuous proof of
absence between samples.

<!-- BEGIN RECOMPUTED RUN AGGREGATES -->
Twenty-eight pairs: accepted control/motion **5 / 12**; excluded **23 / 16** for observed rustc.
CPU-load snapshot range: **29-100%**.

| Fixed 6.1 s history | COW control | Motion + COW |
| --- | ---: | ---: |
| Launcher elapsed, median (range) | 20.675 s (16.875-28.856) | 12.325 s (11.520-16.590) |
| Process CPU, median (range) | 19.109 s (16.219-25.297) | 11.695 s (10.844-15.109) |
| Requested allocation bytes | 100.64 GB | 50.47 GB |
| Populated callback wall, median / p95 | 29.99 / 159.15 ms | 24.98 / 78.36 ms |
| Populated ingress, p95 | 133.92 ms | 54.91 ms |
| Populated redraw preparation, median / p95 | 24.24 / 33.97 ms | 21.58 / 26.58 ms |

Reductions: callback p95 **50.8%**; median launcher elapsed **40.4%**; requested allocation bytes **49.8%**.

| Pair | COW control | Motion + COW | Elapsed reduction |
| --- | ---: | ---: | ---: |
| 1 | 23.807 s | 16.590 s | 30.3% |
| 9 | 20.675 s | 12.433 s | 39.9% |
| 19 | 28.856 s | 14.413 s | 50.1% |
| 28 | 16.875 s | 12.750 s | 24.4% |

Accepted cow_control pairs: 1, 2, 9, 19, 28.
Accepted motion_cow pairs: 1, 3, 4, 5, 7, 9, 12, 19, 21, 26, 27, 28.

Motion pair 9: ingress is **83.5%** of the slowest 16 populated callbacks' wall time.
Its largest populated callback has **724 messages**, **816.7 ms** wall and **788.0 ms** ingress.

<!-- END RECOMPUTED RUN AGGREGATES -->

More pairs were added because background compilation invalidated many controls.
The same exclusion rule applied to both sides; no sample was removed for being
slow. The unbalanced accepted counts and broad host-load range limit precision;
these figures must not be compared directly with the earlier, quieter COW-only
series. Launcher elapsed includes startup/output and exit-observation delay in
the external process monitor. Callback durations are measured inside the replay
process; process CPU comes from the Windows process counter. All 56 processes
completed the same 32,106 messages without skips and with matching command digest.

Both binaries use release profiles and the same fixed 60 Hz model schedule,
Original/default quality, 960x540 procedural presentation and CPU completion.
Replay opens no socket, window or GPU. Loading, output serialization and teardown
are outside callback timing. Primary runs disable trace/hash observers; requested
allocation bytes are counted on the calling thread. The same recording and
protocol-13 world reach 31,467 received food keys, 40 snakes, zero prey and
118,750 prepared quads. The old recording has no recurring ping responses;
its lag schedule is held identical, not used as live-network evidence.

The control is the retained COW executable built at `3209d873`; the new binary
uses main `ada0f7b2` plus motion and diagnostic-only COW. Main's intervening egui
server-picker change is outside the exercised game path; exact output control
above covers its prepared frames. The binaries differ in passive ingress hooks
and code layout as well as the motion optimization. Allocation reduction is
independent of those clocks; timing is bounded to these executables and workload.

No quiet interval was reserved. Runs alternate on the same desktop; process lists
are sampled around each run and about every 500 ms. Samples with observed rustc
overlap are excluded individually and retained with their rejection flags. CPU
load and test/Cargo/archive processes are recorded. Concurrent tests remain a
limitation; timings are not owner gameplay FPS.

## Remaining cost

The fix removes full-world copying from the dominant motion family, but frame
preparation remains about 25 ms per populated callback and p95 remains 78 ms.
In one accepted motion run (pair 9), ingress still accounts for 83.5% of the
slowest 16 callbacks. Its largest callback has 724 messages and takes 816.7 ms,
including 788.0 ms ingress. That fixed input burst includes 109 food consumes,
72 anchors, 27 appearances and 22 growth anchors in addition to the now-cheap
motion messages. The remaining largest candidates are:

1. Food-consume/anchor/growth/appearance transactions still copy received maps,
   unrelated snake state and, when needed, the food presentation array. The prior
   diagnostic COW profile spends 3.34 seconds on consume `c` and 2.45 seconds on
   anchor `g`; these durations are attribution from that earlier profile, not
   newly measured post-motion totals.
2. Repeated frame advance/snapshots, food draw planning and minimap/resource
   preparation remain even on callbacks with no ingress. Their aggregate keeps
   the median above the 16.7 ms budget.
3. Default-view GPU drawing was previously observed at 11.20 ms p95 on the
   integrated GPU; this CPU replay does not measure or change it.

There is no claim of a 60 Hz budget pass, physical input/scanout latency, live
steering response, stock-art pixel equivalence or high-resolution GPU performance.
No new live session was collected for this follow-up. Restricted raw recordings,
raw traces, executables, all samples and failed attempts remain outside Git.

## Provenance

- Main source: `ada0f7b2d11d0bb3dc1e5157f8c0dd87cd3b533c`.
- Community input SHA-256: `84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82`.
- COW control executable SHA-256: `0b919c09e3c622c3b206c7067ad5cda379222e0710dbcb87138de63d54aab0fd`.
- Motion+COW executable SHA-256: `e817bd39bf7754876a65fefbe66f12be187440b5f34ba237083187970a058105`.
- Retained harness SHA-256: `306af00f26dd5f9d52f7c6e9c6941936860592a2c29744b4c4d8237dbc18f3ed`.
- Rust 1.99.0, Windows x86-64, AMD Ryzen 7 9800X3D, two Cargo build jobs and
  dedicated D: target directory; repository release profiles.
- An initial authored test incorrectly expected protocol-13 `d` to be a motion
  message and a separate test expected the heading byte to be wanted-heading.
  Canonical decoders and the full transaction exposed those fixture mistakes;
  corrected tests retain the ignored-family fallback and second-field semantics.

### Earlier-study evidence gap

The merged [food snapshot study from #420](../2026-11-01-food-snapshots/REPORT.md)
has the same retained-data gap: it publishes rounded elapsed pairs but does not
include the per-run callback quantiles, allocation/CPU summaries or a command to
recompute its aggregate table. Its underlying files remain owner-local. That
study and its historical claims are unchanged here. Figures cited from it and
its diagnostic profiles are prior-study context, not results recomputed from
this motion-series CSV. Test counts and fixed workload constants are validation
or provenance facts, rather than aggregates of these 56 timing runs.
