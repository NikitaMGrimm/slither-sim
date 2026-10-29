# Exact pit minimap projection cache

On the synthetic 80-viewer fixed-work row, the warmed cache reduces map-due
tick instructions by **52.11% (protocol 19) / 52.15% (13)**, and all-tick
instructions by **0.60% / 0.63%**. Five alternating pairs per protocol retain
complete byte equality. Map-tick CPU ranges separate, but overall p99 ranges
overlap. The first cache construction instead costs **12.7% more map-tick
instructions**. Both cold and warm rows are retained; no live capacity,
latency or human-play improvement is claimed.

The backend caches only the pit's outside/edge cell indices, leaving each
snake raster, protocol-13/19 packet, recipient baseline and delivery cadence
live. Reuse is allowed only within the exact interval defined by the original
per-cell nearest/farthest comparisons. Equality remains strict for outside and
interior cells and inclusive for edge cells. This avoids quantizing a radius
whose floating representation can move during proportional arena growth.
The public uncached rasterizer remains an independent reference. The cache
belongs to one backend and is rebuilt for a different side or threshold range.

Baseline `cf67b80e`, implementation `af420c84`; native measurements use these exact
runtime pins. The final branch also integrates main's subsequent food-listing
optimization (#264); integrated-head checks are recorded separately. [Plan](PLAN.md), [attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md), [selected correctness](correctness.json).
The unrun natural-warm code-fixture proposal is replaced by the explicitly
synthetic, qualified [fixed-work amendment](PLAN-ADDENDUM.md); no historical
plan or AWS result is rewritten.

## Current evidence

- All 23 deterministic catalog rows and four historical host traces compare
  complete bytes and terminal records exactly. This includes stock, mixed and
  protocol-13 500-seat fixtures and the 2,281-seat fixture.
- Two supplementary eight-seat Battledome traces preserve full bytes through
  growth, shrink, deaths and reconnects, one per protocol. The original final
  world checkpoints, spawn/latency RNG and every outgoing frame/time agree.
- A projected-radius multiplier of 0.98 changes wire-frame bytes in both
  supplementary protocols. A focused strict-to-inclusive threshold mutation
  fails the independent cell oracle; restoring source passes all 398 active
  library tests, with one pre-existing ignored test.
- Cell oracles cover odd/even sides, all cell thresholds for selected small
  grids, adjacent floats, nonfinite radii, changing side, real world growth and
  shrink, ordinary worlds and invalid-side errors. The test also verifies that
  adjacent radii inside one exact interval retain the mask allocations.
- Existing Battledome and protocol-13 tests and strict default all-target
  Clippy pass locally. All 15 required full Rust-group commands pass; see
  [verification](verification.json) and the attempt notes.

These are untimed local Windows debug-build checks with one Cargo job and one
Rust test thread, not the Linux performance experiment. The earlier 16-seat
supplementary initialization failure and failed helper copy are retained,
including original sources/binaries/logs. No incomplete trace qualifies.
Full recordings are local-only; selected hashes identify them without making
them publicly retrievable.

## Native method and gates

After the maintaining agent released the bench, built locked/offline release
arms serially on one x86-64 Linux guest (AMD EPYC family, Rust 1.99.0,
`-C target-cpu=native`). Worker/fork children use logical CPU 0; the controller
uses CPU 1. The parent is single threaded, complete host/backend/RNG and wire
histories are frozen together, and held-pose resets and trace serialization
stay outside the counted command/admission + simulation/broadcast/hand-over
tick. There are no sockets in this fixture. All 80 viewers are alive and
stock-decorated, with nonzero routing generations and no waiting joins.

Native gates repeat all 23 catalog, six host and two queued full-byte traces,
detect both protocol wire mutations, and compare repeated-fork, cross-arm and
direct-parent bytes. Each protocol then has one discarded AB warmup and five
AB/BA/AB/BA/AB pairs of 360 ticks. All counted PMU time is scheduled (100%).
Private inventories show no competing benchmark/build workload; ordinary guest
services remain, so this is a controlled VM comparison, not physical isolation.

The [original amendment](PLAN-ADDENDUM.md) freezes after 300 ticks (2.1 world
seconds), before any viewer receives a map. Fork warmup cannot populate the
immutable parent's cache. That valid cold-construction result is preserved.
The separately frozen [warm amendment](PLAN-WARM.md) changes only the minimap
fixture warmup to 600 ticks (4.2 seconds), after the first map delivery. A fresh
three-arm build and the same gates/pairs measure reuse at the next map tick.
Every counted window contains one map and five food epochs; there is no
admission or marker-update event. This is held-body synthetic work with free
boosting, not a naturally grown 80-player policy workload.

## Warm reuse

| Protocol | Tick group | Ticks/window | Instructions change, median [min, max] | CPU p50 / p99 / worst, ms: baseline to candidate |
| --- | --- | ---: | ---: | --- |
| 19 | all | 360 | -0.601914% [-0.601915, -0.601912] | 0.10859 / 0.18074 / 0.95107 to 0.10585 / 0.18134 / 0.63584 |
| 19 | foodEpoch | 5 | +0.066149% [+0.066142, +0.066155] | 0.18021 / 0.21534 / 0.29747 to 0.17029 / 0.21933 / 0.24766 |
| 19 | minimap | 1 | -52.107754% [-52.107763, -52.107749] | 0.81825 / 0.81825 / 0.95107 to 0.48672 / 0.48672 / 0.63584 |
| 19 | slowestCpu1Percent | 4 | -27.957927% [-28.428525, -26.824394] | 0.48429 / 0.81825 / 0.95107 to 0.46557 / 0.48672 / 0.63584 |
| 13 | all | 360 | -0.634766% [-0.634766, -0.634765] | 0.10962 / 0.23803 / 0.98276 to 0.10661 / 0.22468 / 0.60344 |
| 13 | foodEpoch | 5 | -0.012344% [-0.012357, -0.012331] | 0.23801 / 0.28116 / 0.29117 to 0.21698 / 0.32824 / 0.36037 |
| 13 | minimap | 1 | -52.152353% [-52.152358, -52.152344] | 0.92755 / 0.92755 / 0.98276 to 0.34788 / 0.34788 / 0.38785 |
| 13 | slowestCpu1Percent | 4 | -28.358707% [-29.376649, -27.930813] | 0.61905 / 0.92755 / 0.98276 to 0.34788 / 0.54107 / 0.60344 |


## Cold construction

| Protocol | Tick group | Ticks/window | Instructions change, median [min, max] | CPU p50 / p99 / worst, ms: baseline to candidate |
| --- | --- | ---: | ---: | --- |
| 19 | all | 360 | +0.127323% [+0.127300, +0.127323] | 0.10900 / 0.28437 / 0.56868 to 0.10875 / 0.26467 / 0.61814 |
| 19 | foodEpoch | 5 | -0.068267% [-0.068287, -0.068267] | 0.17572 / 0.19121 / 0.28808 to 0.17953 / 0.18401 / 0.20230 |
| 19 | minimap | 1 | +12.712675% [+12.712662, +12.712686] | 0.53559 / 0.53559 / 0.56868 to 0.60097 / 0.60097 / 0.61814 |
| 19 | slowestCpu1Percent | 4 | +5.189058% [+5.189058, +18.277496] | 0.52763 / 0.53559 / 0.56868 to 0.49106 / 0.60097 / 0.61814 |
| 13 | all | 360 | +0.126565% [+0.126523, +0.126571] | 0.10840 / 0.31336 / 0.70713 to 0.10728 / 0.30652 / 0.61499 |
| 13 | foodEpoch | 5 | -0.000463% [-0.000463, -0.000450] | 0.18001 / 0.18752 / 0.19655 to 0.17814 / 0.19929 / 0.20235 |
| 13 | minimap | 1 | +12.689115% [+12.689028, +12.689137] | 0.54244 / 0.54244 / 0.55597 to 0.60721 / 0.60721 / 0.61499 |
| 13 | slowestCpu1Percent | 4 | +4.858301% [+4.858218, +4.858309] | 0.55019 / 0.56591 / 0.70713 to 0.51333 / 0.60721 / 0.61499 |

CPU p50/p99 are medians of five per-window quantiles; worst is the largest
counted tick across those windows. Each slowest-1% group contains its arm's own
four slowest CPU ticks; the identities may differ. Groups overlap and their
quantiles must not be added. Complete per-pair values retain all ranges and the
discarded warmup. Historical local [verification](verification.json) remains
the pre-guest receipt; native identities and gates are separate below.

## Interpretation and decision

The cache removes repeated per-cell pit-distance calculations, leaving live
snake rasterization, encoding and recipient work. The baseline map tick is
only about 1.2% of total warm-window instructions, bounding its contribution to
the whole row. The 52% saving therefore produces about 0.6% total saving,
not a 52% server speedup. Startup construction adds vectors and exact interval
extrema to the original cell traversal, explaining the cold regression by
source inspection. Cache invalidation can incur that cost again; the growth
and threshold traces establish equality, not the frequency of misses in play.

Warm map-tick CPU median [five-window range] is 0.81825 [0.74419, 0.95107] to
0.48672 [0.47209, 0.63584] ms in protocol 19, and 0.92755 [0.87579, 0.98276]
to 0.34788 [0.33394, 0.38785] ms in protocol 13. Overall p99 ranges overlap;
other phases include CPU movement without corresponding instruction movement.
Use the instruction counts and bounded map-event result, not those incidental
CPU changes, as the performance claim. Each row has its own matched baseline;
different world ages make cold-versus-warm absolute timing comparisons invalid.

This is an **exact** candidate with a small whole-window instruction saving and
a documented cold cost. It changes no rule, cadence or feel. Submit it for
maintaining-agent review with that tradeoff. Prioritize the independent
spawn-search candidate for the high-boost overload identified in #263; marker
reuse remains separately prepared and is not measured in this collection.

## Selected evidence, reproduction and resources

- [Warm pair values](minimap-warm-pairs.json) and
  [cold pair values](minimap-pairs.json) retain every pair's instruction means,
  event counts and CPU quantiles, including warmup. [summarize.py](summarize.py)
  recomputes these tables and all quantile ranges.
- [Warm native evidence](minimap-warm-evidence.json) and
  [cold native evidence](minimap-evidence.json) pin runtime archives, native
  binaries, instrumented sources, trace receipts, gates, work and raw archives.
  Full traces, sources, inventories and logs are **local-only**; digests are not
  public downloads. [Reproduction](REPRODUCE.md) links the common harness.
- Each native collection completed all ten recorded steps with exit zero.
  Archives were closed, tar-read and hashed before ready receipts; each download
  was size/hash-checked and all retained files verified before deleting its
  remote source. Earlier attempts remain in [ATTEMPTS](ATTEMPTS.md).
- After integrating #264, all 15 [Rust checks](integrated-checks.json) and a
  fresh [31-trace matrix](integrated-traces.json) pass against main `cc1bb1e9`.
  Both protocol wire mutations are detected. This untimed Windows run checks
  source `79b616df`; subsequent changes are evidence/docs only. It does not
  relabel the earlier native timing pins as a post-#264 measurement.
- The task used a bounded 4.18 GB scratch filesystem, below the approximately
  11 GB guest limit; observed total filesystem use including archives stayed
  below 2.46 GB. Targets were removed before counting and again checked absent
  at completion. [Cleanup receipt](cleanup.json) verifies our task directory
  and processes are absent; unrelated retained work was untouched.
- Reused the released guest; no new AWS resources or incremental AWS collection
  spend. No play hold, service, persistent guest change or human seat.
