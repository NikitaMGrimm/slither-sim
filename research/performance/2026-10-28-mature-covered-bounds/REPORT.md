# Mature host coverage bounds, 2026-10-28

**#249 reduces mature host-tick instructions by 23.563% with stock clients and
23.206% with protocol 13.** Conservative bounds for blocks of eight body edges
save a further **22.916% / 22.465%**, including construction. Short fixtures use
0.60–0.96% more instructions with bounds; this is a mature-scene tradeoff.
[Selected results](results.json), [frozen plan](PLAN.md), [attempts](ATTEMPTS.md).

## Full-host construction and fork qualification

The preceding [checkpoint attempt](../2026-10-28-mature-broadcast/REPORT.md)
could restore the core but lacked host food-presentation history. This fixture
never restores a checkpoint. It advances `RawSimulator` from tick zero with all
500 recipients and broadcasts active, so food births, pickups, palette lifetimes
and reconnects pass through the ordinary backend. Core checkpoints are exported
only as complete trace witnesses, excluding producer provenance, never loaded.

Seed 7, `Config::official(500)`, explicit Official delivery, 500 remote slots and
A4-mean direct-core observations/actions every six 7 ms ticks. Admission,
control generation and frame draining stay outside the counted
`advance_tick_with` interval, as in the original host/wire fixture. No client
decoding or native-bot observation work is counted. No outside servers are used.

Through tick 180,000, 100 slots remain resident while 400 retire/rejoin through
the ordinary backend paths, one every 50 ticks. At that boundary only the
external admission RNG is reset to seed 7; forced churn stops. Continue to the
frozen 193,000-tick endpoint, preserving authentic food-presentation history.
No geometry, mass or food provenance is edited. The unchanged screen is living
>=490, mean segments 55–90, p90 150–230, p99 220–350. The state qualifies with
**499 living, mean 55.3307, p90 176, p99 247, maximum 271**, 27,610 anchors,
28,897 food and 20 prey. The fixed 360-tick window ends with 500 living, mean
55.884, p90 176, p99 247. It approximates #248's 2,281-seat length distribution
63/174/250, not its complete ecology or its separate 501-seat distribution.
No mature 2,281-seat measurement or stationarity claim is made.

The matched short row uses the same harness at ticks 0–1,080. Its staggered
retire/rejoin construction is active; the mature window has no forced churn.
Length is therefore not the only difference. The original circle/boost host
fixture and `client-official-500` retain separate identities.

One OS thread is asserted before every fork and window. Phase brackets and
libm caller recording are disabled during warmup. Every child opens a fresh
current-thread `perf_event_open`; the shim closes inherited events and resets
calibration. Warm stdout is flushed before forking. Children write distinct
files; the parent waits and remains unchanged until its final direct reference.
Kernel/COW work is outside the user-instruction metric.

Two repeated child windows and direct parent continuation match complete bytes,
including canonical core states, actions, every ordered frame/time and host
RNG/session/lifecycle summaries. The first #249 stock cohort uses three baseline
children and a direct candidate-parent reference; later cohorts validate direct
parent continuation on both arms. This qualifies this single-threaded fixture,
not a general host snapshot API.

Warm streams are compared live byte for byte using bounded buffers. Stock warm
streams contain **5,876,276,676 bytes/arm**; protocol 13 contains
**10,132,210,980 bytes/arm**. They are not stored on the small disk. Matching
length/hash receipts, full window traces, logs and build/source receipts stay
local. Mismatches retain offsets and both differing chunks. Hashes identify
results; acceptance uses complete byte comparison, not hashes alone.

## Attribution: short and mature

Separate diagnostic builds, **million user instructions per tick**, include
instrumentation and the coverage eligibility vector. Coverage, encoding, food,
prey and new bounds construction nest inside broadcast; compaction and collision
nest inside physics. Do not add nested rows to their parent totals.

| Scope | Short, post-#249 | Mature, pre-#249 | Mature, post-#249 | Mature, bounds |
| --- | ---: | ---: | ---: | ---: |
| Complete host advance | 7.7090 | 23.1861 | 17.9292 | 14.2372 |
| World physics | 2.4557 | 4.4912 | 4.4912 | 4.4912 |
| Food-interest advance | 0.3040 | 0.2970 | 0.2970 | 0.2970 |
| Broadcast inclusive | 4.8991 | 18.3501 | 13.0933 | 9.4013 |
| Snake coverage | 1.0290 | 14.1464 | 8.9090 | 3.6613 |
| Snake encoding/history | 1.0460 | 1.2572 | 1.2572 | 1.2571 |
| Recipient food | 0.5602 | 0.4723 | 0.4723 | 0.4723 |
| Recipient prey | 0.5649 | 0.2772 | 0.2578 | 0.2579 |
| Whole food compaction | 0.5588 | 0.1722 | 0.1722 | 0.1722 |
| Collision bounds/index build | 0.2726 | 0.8478 | 0.8478 | 0.8477 |
| Collision query/exact tests | 0.2647 | 2.1962 | 2.1962 | 2.1962 |
| New body-block construction | — | — | — | 1.5478 |

Coverage remains the top supported target after #249: 8.909M inside a 13.093M
broadcast parent. Candidate #1 therefore stays first. The complete encoding or
compaction parent is not assigned to a guessed memoizable/rotation subset.

Calibrated shared-libm **callee** instructions as a share of the corresponding
plain matched-row median, not of an interposed/phase-build denominator:

| Caller | Short pre-#249 | Mature pre-#249 | Mature post-#249 |
| --- | ---: | ---: | ---: |
| `covered`: head `hypot` | 2.1794% | 0.9707% | 0.0000% |
| `covered`: segment `hypot` | 1.8036% | 29.6970% | 0.0000% |
| Prey visibility `hypot` | 1.1995% | 0.1142% | 0.0000% |
| Incremental anchor math | 0.1299% | 0.0422% | 0.0552% |
| Appearance math | 0.0100% | 0.0013% | 0.0018% |
| Food-interest heading | 0.9449% | 0.3132% | 0.4097% |
| Physics `advance_directed` | 2.4989% | 0.8942% | 1.1699% |
| Physics head-tip | 0.9449% | 0.3132% | 0.4097% |
| Physics collision distance | 0.0106% | 0.3664% | 0.4794% |
| Physics prey math | 0.3360% | 0.0356% | 0.0466% |
| Other tick math | 0.1309% | 0.0431% | 0.0563% |
| **All shared libm** | 10.1886% | 32.7912% | 2.6287% |

The mature pre-#249 baseline makes **45,780,359 segment-visibility `hypot`
calls / 360 ticks**. #249 eliminates these calls; projection and traversal remain.
All nine dynamic imports are wrapped: `asin`, `atan2`, `cos`, `exp`, `hypot`,
`log`, `pow`, `sin`, `sincos`. Outer driver math is untagged. Compiler-builtins
`fmod` is static and outside this shared-libm total. The 32.79% does not reproduce
#248's 45.1% IP weight: population, history, window and method differ. No rejected
partial IP sample is used as a budget.

Common no-IPA measurement routines use a one-ret control per signature, 64 stable
calibration repetitions and 32-NOP-plus-ret probes recovering exactly 33
instructions. Callee counts are raw minus control plus one. Return offsets
resolve through inline `addr2line` frames; shared `Point::distance` callers use
actual source stacks. Accepted tagged mature headers have enabled==running.
The separate plain-binary mature cross-check matches call counts for every
function; corrected instructions differ by at most **3.04 ppm**, within the
predeclared 20 ppm gate. Full window bytes match the plain #249 references.
The failed initial supplemental startup and successful retry remain recorded.

## Candidate and exactness evidence

Build a flat block-bounds array and per-slot ranges once per broadcast for living
bodies with at least 16 edges. Blocks cover eight consecutive body edges, sharing
an endpoint. Keep the original head and head/front tests and original exact
edge predicates/order within surviving blocks. Short bodies keep the full walk.
Bounds are rebuilt for every broadcast, including external pose edits and
diagnostic broadcasts. All construction is counted in advance/broadcast.

Only finite body/query coordinates and radius bounded by 1e7 permit rejection.
At this scale a one-unit margin exceeds projection/subtraction/hypot roundoff;
outside it, retain the original full walk. This follows the existing admission
bounds pattern. Extra candidate edges are harmless; exact visibility thresholds
are unchanged.

An independent oracle freezes the original scalar projection and complete
coverage expression. Over 160,000 random/adjacent-radius queries cover empty
and short bodies, long crossing/final edges, degeneracy, signed zero,
underflow/overflow/nonfinite values, safe-domain borders and slot reuse.
Debug/release pass. A deliberately narrowed block margin fails immediately;
restoring source passes. The complete offline Rust check group passes.

All **23 catalog and four original host/wire traces** match complete decompressed
bytes and terminal records. Coverage-radius perturbations are detected with stock
and protocol-13 clients. Warm-stream bit flips are detected in both protocols,
with retained mismatch witnesses. Both mature protocols match complete warm
streams, repeated-child windows and direct-parent references. Mature mutations
are detected with identical canonical physical states in both protocols.
The interrupted first protocol-13 cohort had parity but no plain pairs; the
accepted retry supplies all five. No sample is replaced in an accepted cohort.

## Fixed-work instruction results

One discarded AB warmup, then five adjacent pairs in AB/BA/AB/BA/AB order for
every row. Positive reductions mean fewer instructions. Host rows count complete
host advances; the client row counts the complete fixed command. All per-pair
counts, build identities, diagnostic counters and controls are in results.json.

| Change / row | Before → after, M instructions/tick | Median reduction | Pair range |
| --- | ---: | ---: | ---: |
| #249 mature stock | 22.0645 → 16.8655 | 23.562898% | 23.562861–23.562907% |
| #249 mature protocol 13 | 22.4028 → 17.2040 | 23.206047% | 23.206044–23.206084% |
| #249 matched short | 6.9117 → 6.6433 | 3.883414% | 3.883348–3.883439% |
| Bounds mature stock | 16.8659 → 13.0009 | 22.915825% | 22.915791–22.915880% |
| Bounds mature protocol 13 | 17.2041 → 13.3392 | 22.465194% | 22.465185–22.465195% |
| Bounds matched short | 6.6432 → 6.7069 | -0.959075% | -0.959099–-0.959027% |

| Bounds, original short row | Median reduction | Pair range |
| --- | ---: | ---: |
| `official500` | -0.894281% | -0.896020–-0.892094% |
| `official500-legacy` | -0.834216% | -0.836869–-0.831387% |
| `official2281` | -0.621460% | -0.622177–-0.620169% |
| `client-official-500` | -0.599694% | -0.600144–-0.599367% |

The mature stock plain delta is **3.865M instructions/tick**. Separate diagnostic
coverage falls by 5.248M while new construction costs 1.548M; diagnostic overhead
and code layout prevent treating that subtraction as a plain-delta identity.
Remaining plain host work is 13.001M instructions/tick. Five fork pairs repeat
one warm state, not five independent world histories. No elapsed-time or
real-socket capacity improvement is inferred from instructions.

Next candidates ranked by remaining mature parent budgets, not predicted savings:

1. Tighter conservative rejection in remaining coverage: **3.661M** instructions
   per tick. Measure which exact edge tests could be skipped; the parent is an
   upper bound.
2. Collision query geometry: **2.196M** per tick (0.265M short). Preserve exact
   predicates and collision ordering.
3. Fuse validation/bounds construction passes: the new **1.548M** parent is worth
   investigating. No removable subset is established. Encoding/history is 1.257M
   and whole food compaction 0.172M; the earlier short food-copy ranking changes.

## Pins, reproduction, budget and limits

The retained [stream comparer](compare.py) checks complete raw bytes, including
mutation controls. The [preload probes](preload-probes.py) preserve the short
launch/calibration validation before the long retry.

Base is merged #251 main `6e02fefefb447991d24cf0c3385d5eb8ac6c62ff`.
Bounds runtime is `50220958c900782d53cb225949bf25165709c371`.
#249 cohorts use `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506` and
`9c3b29da359586341af83f0fa45aef9f63b30967`; runtime source/manifests/lockfile
match pre-#249 main and post-#251 main respectively. Later report/harness-only
commits do not change those measured runtimes.

A **4-vCPU Zen 5 EPYC KVM guest**, Linux x86-64, Rust 1.99, portable CPU release,
thin LTO, one codegen unit, debuginfo=1, locked offline dependencies. Builds and
checks precede counted windows. Warmers can overlap, then all wait at a barrier;
windows run sequentially on CPU 0 with the guest otherwise idle. Full process
command lines at boundaries stay local, account redacted before writing. No
system/security changes, installs, outside servers or existing play servers.

[Reproduction](REPRODUCE.md) covers archives/builds/cohorts. All generated Rust
source hashes match across eight measured arms. The LF fixture's historical
terminal CRLF is reconstructed explicitly by build.py. The new individual-cohort
controller also passes a 120-warm-tick pilot with complete fork/parent parity;
that pilot is not a performance result. Historical wave scripts are audit
records, not the simplest fresh-run entry point.

Completed mature warmups total **14,349.685 CPU seconds (3.986 hours)**, including
interrupted protocol-13 parents and retries. Available process/fork/controller
receipts plus warm-only fallbacks total **4.109 CPU hours**. Earlier controllers
lack complete child CPU receipts; that sum omits their windows and is not an
exact all-task total. Full traces and boundary inventories remain. Construction
and pairs finished within the approximately six-hour allowance, without a
long-running hold. The single bounded Cargo target was removed after verification (2.094 GB);
copied binaries, sources, raw windows and failed attempts remain local-only.

Synthetic scenes establish this stated parity/performance scope, not official
capture fidelity, universal parity or gameplay qualification. No independent
review or real-socket capacity qualification is claimed.
