# Exact ordered obstacle candidates for Battledome admission

**Rejected under the frozen study gate.** Final refused admission uses about
76% less CPU and successful admission about 75% less, with every paired ratio
below 1 in both labels. Both 80/160 controls pass. The old seven-ring row adds
103.027/102.786 microseconds to median tick CPU (19/13), exceeding this study's
additional frozen +10 microsecond old-row gate. No production change is proposed
by this research PR. The implementation and measured commits remain on the
[`perf/spawn-obstacle-index` reference branch](https://github.com/NikitaMGrimm/slither-sim/tree/perf/spawn-obstacle-index),
final runtime `c717b268`. The JSON `selected` arm means final measured revision,
not an accepted optimization.
This study profiles and counts complete admission on a frozen, naturally grown
synthetic ecology. It does not measure sockets, client latency or live capacity.
[Plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[tables](TABLES.md), [measurements](measurements.json),
[proof/retrieval receipts](verification.json).

## Why this fixture

The [live recheck after #272/#273](../2026-10-29-battledome-post-spawn/REPORT.md)
still spent 8.012 ms on spawn work in the candidate-320 slow tail. The old
seven-ring exhausted fixture cost under 1 ms and had only eight living obstacles.
It no longer represented the repeated traversal through hundreds of long snakes.

The frozen [warmup](live_fixture.rs) runs shared received-frame A4 with the
received-rule-gated constant-boost override, no sockets and no authoritative
bot inputs. The first eligible checkpoint is at 642.684 virtual seconds, with
318 living snakes and 99.8447% living-decision boost share. All 318 are boosting
at the snapshot. The frozen [census](fixture.json) and [geometry](geometry.json)
are independently recomputable with [census.py](census.py).

| Quantity | Frozen world | #275 candidate-320 |
| --- | --- | --- |
| Living snakes | 318 | mean 318.55, minimum 300 |
| Mean body points/length | 58.887 | 52.48 → 56.56 |
| Actual radius | 11,333.603 | about 11,232.048 |
| Food | 18,346 | 18,334 → 19,340 |
| Boost share | 99.8447% across warmup decisions | 99.82% measured live |

Body-point count min/p50/p90/p99/max is 26/26/150/234/252. Adjacent point spacing
has median 23.588 units and p99 42.000; nearest-head distance has median
531.519, p90 1,316.151 and minimum 82.454. The original warmup metadata uses
floor-rank p99 (221); the comparison census uses the stated ceil-rank convention
(234), without editing that frozen metadata. Configured base radius 3,584 is
distinct from actual grown radius 11,333.603.

This fixture has slightly longer bodies and a slightly larger radius than the
live row. Live point spacing was not retained, so matching its point cloud is
not claimed. Initial native placement, one reconnect retry per decision, absence
of sockets and a strict waited=0 probe differ from the live queue. The same
unmodified checkpoint yields refusal at ascending seed 40 (4,096 poses) and
success at ascending seed 0 (1,638 poses); no seed was selected on timing.
No geometry was moved to force those outcomes. Import and RNG reset are outside
counting; complete search, Safety construction, ring geometry and allocations
are inside. Returned-result serialization/drop is outside the counter. Every
call has fresh Safety, so index construction is included.

## Profile first

Baseline `9ad002c1` already contains #272/#273/#274. Full-DWARF diagnostic builds
retain portable release codegen. CPU-clock user sampling at 499 Hz yields:

| Exclusive phase | Refused % | Successful % |
| --- | ---: | ---: |
| Head predicate | 61.56 | 60.07 |
| Body predicate | 27.71 | 25.73 |
| Other admission | 3.97 | 3.76 |
| Ring geometry | 3.42 | 3.51 |
| Departure predicates | 1.42 | 3.00 |
| Safety construction | 1.36 | 3.01 |
| Allocator leaf | 0.56 | 0.92 |

These sampled categories sum to the whole admission phase; allocation is an
exclusive leaf category, with caller breakdown retained in [profile.json](profile.json).
The head/body `near` bounds paths account for 4,865/2,350 of 10,019 refused
samples (72.0% together, including nested bounds tests); repeated obstacle
screening is now the leading target.
Dynamic body-margin scoring is absent in this strict waited=0 row and stays
linear. Diagnostic per-search CPU is about 8.95/3.82 ms, not a selected timing.
The earlier line-info profile with unattributed inlined body work is retained.

## Exact broad phase

The prototype index uses 1,024-unit cells and dense obstacle bitsets. Each obstacle envelope
unions its body, projected path and inward-path bounds expanded by the largest
relevant fixed width. It is built lazily after 16 supported queries, only for
at least 32 obstacles. Short successful searches avoid construction. Storage
is bounded to 4,096 cells and 65,536 words; oversized grids fall back to linear.

Within the supported finite coordinate/width domain (absolute bounds at most
10⁷), original `near` slack is at most 0.01. An extra one-unit expansion covers
that slack and rounding. Both insertion and query use the same monotone cell
mapping, so an original qualifying bounds intersection retains a common cell.
This argument does not assume exact division for subnormal coordinates.
Invalid, non-finite, extreme or unsupported obstacles are always included at
their original positions; unsupported queries use the complete linear scan.

ORed bits are visited once in increasing original obstacle order. All retained
obstacles run the unchanged exact inner tests. Thus early rejection order,
inclusive thresholds and non-finite behavior are preserved. Dynamic-width
`body_margin` remains unchanged. No candidate pose, RNG draw, wait, refusal,
fallback or admission rule changes. A final revision isolated the grid path from small-world loops; that attempt
did not remove the old-row regression.

## Proof and measurement scope

The independent raw scalar oracle shares neither Safety nor the index. It
covers head/body/departure verdicts over random and adversarial geometry,
cell boundaries, adjacent thresholds, zero-length edges, invalid/non-finite
and huge coordinates. A direct inclusion oracle prevents an earlier obstacle
from masking a missing candidate. Lazy and eager indexed verdicts match, and
candidate order is checked. The wrong-skip mutation must fail by assertion,
then exact source restoration and the focused group must pass.

The selected proof receipts cover 18 focused Safety tests, all 15 Rust commands,
23 catalog traces, six host traces and two queued traces. Both protocol wire
mutations produce actual byte differences. Additional refused/successful traces
compare raw bytes including full result bodies/errors and RNG; the checkpoint
stays unchanged. No independent reviewer was used.

Timing uses default portable release with empty Rust flags on a dedicated
Frankfurt c7i.2xlarge Spot, four physical/logical CPUs with SMT disabled. Each
row discards one warmup and retains five alternating AB/BA pairs. Live-like
windows contain 512 fresh admission calls; old/control windows contain 360
complete host ticks from identical frozen forks. Both protocol labels are
independent repetitions for the protocol-independent pure admission rows;
old/control host work and wire proofs are protocol-specific. PMU scheduling
must be at least 99%. No builds, tests, transfers or reduction overlap selected
counting. Per-row bindings freeze binaries, runner and checkpoint/metadata.

All live-like CPU ratios are 0.2363–0.2388 refused and 0.2509–0.2548 successful.
The old-row paired means are 1.1576–1.1653. Its search ticks increase from about
0.633 to 0.739 ms and run 14.7% more instructions, while non-search ticks remain
about 0.022 ms with essentially unchanged instructions. The complete-tick
instruction ratio is 1.1401. The grid is disabled for this eight-obstacle world;
the instruction increase is in the changed small-world search code path. Symbol
inspection shows a larger generated admission function, but does not identify
the precise compiler transformation. The original 80/160 control gates and both
live-like gates pass; the extra old-row gate was frozen in the study before timing
and was not relaxed. There is no acceptable candidate under that complete plan.

The [generated tables](TABLES.md) give medians and paired ranges, instruction
ratios, CPU p50/p99, slowest-1% and successful search groups. All pairs and
all/search/exhausted/other summaries are retained in the selected JSON.
CPU is thread user+system time; user instructions are `instructions:u`.

## Attempts and limits

The first candidate made live-like searches much faster but added about
71–72 µs per median tick to the old seven-ring row. A direct small-head loop
revision still added 65–67 µs in a clean rerun. Both fail the study's frozen
extra +10 µs old-row gate and remain in the evidence. Another complete matrix
is excluded because proof-download completion had not been confirmed before
dispatch. All attempts are retained; no gates were loosened.

The marker-reuse reference branch `perf/pit-marker-reuse` at `58145ae` remains
parked: no PR and no timing, by the maintaining agent's instruction.

This fixture measures one deterministic ecology and two fixed admission seeds.
It establishes neither sustained capacity nor client-side latency improvement.
Waiting-age fallback behavior is covered by the older queued traces, not this
strict live-like timing row. Sampling attribution is descriptive and has inline
classification uncertainty. All measured commits remain in branch history;
merging current main adds release/study metadata without changing measured
production Rust sources.

## Cost and cleanup

Final conservative combined bound is $1.217, based on
140 rounded launch-to-final-scan minutes at the bid cap plus $0.33 for
transfer/storage/public IPv4; this is not an invoice. [Cost receipt](cost.json).
The [cleanup scan](cleanup.json) completed in all 17 enabled regions with zero
nonterminated instances, open/active Spot requests, volumes, Elastic IPs and
project/task key pairs or security groups. The task builder is terminated and
its key/security group deleted; protected resources were untouched. Budget is $1.50 combined, with a 180-minute
in-guest terminate timer, delete-on-termination volume and explicit limited
profile. Full-timer bid-cap bound is $1.483 including $0.33 noncompute allowance.
Archives must be closed/member-verified and downloaded with matching size/SHA
before source teardown. No local native work or shared SSH guest was used.

Full checkpoints, source snapshots, binaries, traces and diagnostics are local-only
private archives, unavailable at a public URL. Their digests identify the retained
material and do not grant download access. Public bounded summaries and helpers
are sufficient to recompute every reported table; the full replay needs that
external checkpoint/archive chain.
