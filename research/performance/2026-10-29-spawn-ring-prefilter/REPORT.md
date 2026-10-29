# Reject blocked spawn heads before constructing rings

The exact prefilter reduces total user instructions by **36.73%** in both
protocols on the synthetic exhausted-search row, with **36.93%** less on search
ticks. Five alternating pairs per protocol preserve complete bytes. All-tick
CPU p99 medians fall from **3.66 to 2.47 ms (19)** and **3.64 to 2.46 ms (13)**;
the five-window ranges separate. The saving is directly present on the slowest
1% of ticks. Admission outcomes and waits in world time are unchanged. This
is fixed-work evidence, not a live capacity or network-latency result.

Baseline `23eddca8`; implementation `bd8915c8`. The selected `edab3287` archive
adds only harness qualification, with the same production source. This branch
is independent of the minimap cache and marker reuse candidates. The final
branch integrates main's subsequent food-listing optimization (#264); its
untimed checks are recorded separately from these native timing pins.
[Plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[selected correctness](correctness.json).

The high-boost [Battledome study](../2026-10-29-battledome-boost/REPORT.md) found
admission dominating the slowest ticks at 160 and 320 seats. Every rejected
pose still built a complete circular body before testing its head. After the
first original body initializes the strict query, this candidate skips that
construction only when the head already fails, ring geometry is valid and
directional fallback is ineligible. All three random draws still occur in
their original order. The first 256 fallback-eligible poses keep the full
original path, including exact scores/ties. Invalid geometry keeps the original
construction/error path. No candidate budget, admission outcome, refusal,
retry delay, floor, clearance, population or delivery rule changes.
The original ring constructor keeps `head` bit-identical as `body[0]`, and
`head_is_clear` is a pure query, so moving a guaranteed rejection before
construction does not replace its geometric predicate.

## Correctness and stress qualification

The frozen pre-change full-search oracle compares bitwise admitted poses and
headings, errors, subsequent RNG and unchanged world state for three seeds,
open/blocked scenes, floor/no-floor and request ages below/at/above two seconds.
The focused fallback-gate mutation fails that oracle; restored source passes.
The ring-head unit control also rejects hiding invalid/nonfinite ring geometry.

All 23 deterministic catalog rows, four historical host fixtures and two
supplementary Battledome protocol traces compare complete bytes with terminal
records. Two more traces run the actual host queue against held synthetic
barriers, then remove the barriers halfway through to check successful
admissions and bytes. Their checkpoint/RNG, queue state, frames and delivery
times agree completely between baseline and candidate.

| Protocol | Blocked-phase pending seats throughout | Refused searches | Candidate poses tested | Pending after release | Mutation pending after release |
| --- | ---: | ---: | ---: | ---: | ---: |
| 19 | 72 | 90 | 368640 | 54 | 57 |
| 13 | 72 | 90 | 368640 | 54 | 57 |

These values come from `fixed[*].base.summary` in the selected JSON; the
candidate's entire corresponding receipt is identical. The uncompressed bytes
are also compared directly. Each refused search tests exactly 4,096 poses.
Both wire mutations change encoded frames, not merely a summary or hash field.
The wrong fallback gate admits 15 seats after release, versus the original 18.

The stress row guarantees the overload work pattern with the real host's
waiting list, request-age/fallback clock and refusal retry methods. Its seven
held replay barriers and one observer are explicitly synthetic; dynamic radius
growth is disabled for that row. This is not an 80-live-player, naturally grown
or capacity workload. The counted primary version holds all 72 joins waiting
throughout, while the uncounted correctness version includes the release phase.

Additional no-backlog 80-viewer and shrinking-pit controls are prepared to
measure the minimap and marker paths independently. Their held bodies are also
synthetic. No local timing or PMU value is used as performance evidence.
Full raw sources, copied binaries, traces and logs are local-only, with selected
hashes identifying them but no public download.

## Native fixed-work method

After explicit bench release, measured the minimap candidate first and this
candidate next, each against its own pinned baseline. Native builds are locked,
offline release builds on one x86-64 Linux guest (AMD EPYC family, Rust 1.99.0,
`-C target-cpu=native`). All three arms rebuild serially with package-artifact
clearing and distinct executable hashes. Worker/fork children use logical CPU
0; the controller uses CPU 1. The frozen parent has exactly one thread.

The primary row ages the real host queue for 300 ticks, then every fork counts
360 ticks of command/admission + simulation/broadcast/hand-over. It holds the
barriers throughout: **72 pending joins, 180 refused searches and 737,280 poses
per window**, exactly 4,096 per search, in both arms/protocols. One observer and
seven replay barriers are live; this is not 80 living players. Pose resets and
trace serialization stay outside counters. The uncounted release-phase trace
above separately checks successful poses, RNG and outgoing bytes.

Native gates pass all 23 catalog, six host and two queued full-byte traces and
detect both wire mutations. Repeated-fork, cross-arm and direct-parent traces
also match completely. Each protocol then runs one discarded AB warmup and
five AB/BA/AB/BA/AB pairs. All counted PMU time is scheduled (100%). Private
inventories show no competing build/benchmark workload; ordinary guest services
remain, so this is a controlled VM comparison, not physical isolation.

## Instructions, CPU and slow ticks

| Protocol | Tick group | Ticks/window | Instructions change, median [min, max] | CPU p50 / p99 / worst, ms: baseline to candidate |
| --- | --- | ---: | ---: | --- |
| 19 | all | 360 | -36.727016% [-36.727017, -36.727016] | 3.41778 / 3.66114 / 3.96532 to 2.28433 / 2.46946 / 2.88001 |
| 19 | search | 180 | -36.933866% [-36.933868, -36.933866] | 3.52023 / 3.70856 / 3.96532 to 2.36709 / 2.56899 / 2.88001 |
| 19 | other | 180 | +0.000998% [+0.000991, +0.001005] | 0.00958 / 0.04738 / 0.44261 to 0.00948 / 0.04741 / 0.44650 |
| 19 | foodEpoch | 5 | +0.023913% [+0.023815, +0.023913] | 0.01070 / 0.04738 / 0.05078 to 0.01054 / 0.04741 / 0.05309 |
| 19 | minimap | 1 | +0.000000% [-0.000015, +0.000015] | 0.43150 / 0.43150 / 0.44261 to 0.42315 / 0.42315 / 0.44650 |
| 19 | slowestCpu1Percent | 4 | -36.958751% [-37.273165, -36.654314] | 3.70856 / 3.89346 / 3.96532 to 2.56899 / 2.66640 / 2.88001 |
| 13 | all | 360 | -36.726992% [-36.726993, -36.726992] | 3.42538 / 3.64129 / 3.91164 to 2.27949 / 2.46290 / 2.78008 |
| 13 | search | 180 | -36.933871% [-36.933871, -36.933870] | 3.50391 / 3.69107 / 3.91164 to 2.36426 / 2.59821 / 2.78008 |
| 13 | other | 180 | -0.000030% [-0.000034, -0.000027] | 0.00962 / 0.04836 / 0.44386 to 0.00958 / 0.04669 / 0.43086 |
| 13 | foodEpoch | 5 | -0.001367% [-0.001367, -0.001367] | 0.01063 / 0.04836 / 0.05601 to 0.01033 / 0.04669 / 0.05776 |
| 13 | minimap | 1 | +0.000000% [-0.000015, +0.000015] | 0.43154 / 0.43154 / 0.44386 to 0.42847 / 0.42847 / 0.43086 |
| 13 | slowestCpu1Percent | 4 | -36.706552% [-36.821494, -36.504411] | 3.69107 / 3.85994 / 3.91164 to 2.59821 / 2.70443 / 2.78008 |

CPU p50/p99 are medians of the five per-window quantiles; worst is the
largest counted tick across those windows. Each slowest-1% group selects its
arm's own four slowest CPU ticks, so the identities can differ. All four are
exhausted 4,096-pose search ticks in every counted arm/window; selected event
receipts are in `spawn-evidence.json`'s `slowTailWork`. Search and
exhausted groups contain exactly the same ticks here. Groups overlap; their
quantiles must not be added. Complete per-pair values retain all ranges and the
discarded warmup. Each window also has five food epochs, one map opportunity
and no marker update; those groups are shown even though this optimization
does not target them.

All-tick CPU p99 median [five-window range] is 3.66114 [3.64007, 3.73856] to
2.46946 [2.46672, 2.50653] ms in protocol 19, and 3.64129 [3.59823, 3.66215]
to 2.46290 [2.45223, 2.51787] ms in protocol 13. On the four slowest ticks,
instruction savings span 36.65–37.27% in protocol 19 and 36.50–36.82% in 13.
These are whole admission-containing ticks, not an isolated helper benchmark.

## Interpretation and decision

Search ticks account for about 99% of this stress row's baseline instructions.
Skipping allocation and trigonometric construction for guaranteed-rejected
heads removes about 37% of that work. The remaining strict head queries,
fallback-eligible rings and ordinary world/broadcast work still execute. The
result is below the bound of eliminating the search-containing ticks entirely;
it does not remove the geometric admission limit. Other ticks are effectively
unchanged in instructions, with small CPU movement that is not a claimed gain.
Outside the fallback window, a head that passes the pretest still follows the
original construction/head-query path, so it can pay an extra head query.
The first candidate retains the original path. This deliberately blocked row
does not measure accepted-search performance or the tradeoff in an open band.

This is an **exact** optimization: same poses/draws/order, fallback and errors,
same admissions, refusals, queue ordering and retry clocks. It needs no feel or
rule change. Submit it for maintaining-agent review as the priority optimization
suggested by the high-boost study. The row reproduces repeated full-budget
admission work, but its static synthetic blockers and smaller living population
do not predict natural 160/320-player CPU, real spawn waits, deferred ticks,
socket delivery or human experience. Those remain unmeasured after this fix.

## Selected evidence, reproduction and resources

- [Pair values](spawn-pairs.json) retain all per-pair means, CPU quantiles and
  event groups, including the discarded warmup. The shared
  [table summarizer](https://github.com/NikitaMGrimm/slither-sim/blob/f1f426784354c9691f2c74c95a75d97d7e37c1d7/research/performance/2026-10-29-pit-minimap-cache/summarize.py)
  recomputes this table and every CPU range; no duplicate helper is added here.
- [Native evidence](spawn-evidence.json) pins source archives, instrumented
  sources, native binaries, complete-byte receipts, gates, counted work and raw
  archive digest/size. Full sources, binaries, traces, logs and inventories
  remain **local-only**; hashes do not make them publicly downloadable.
  [Reproduction](REPRODUCE.md) links the earlier helpers rather than copying them.
- All ten native execution steps finish with exit zero. The archive is closed,
  tar-read and hashed before the atomic ready receipt. Its downloaded size/hash
  and all 3,994 retained file hashes were verified before remote source deletion.
  Every earlier failure remains in [ATTEMPTS](ATTEMPTS.md).
- After integrating #264, all 15 [Rust checks](integrated-checks.json) and a
  fresh [31-trace matrix](integrated-traces.json) pass against main `cc1bb1e9`.
  Both protocol wire mutations are detected. This untimed Windows run checks
  source `25a40fa9`; subsequent changes are evidence/docs only. It does not
  relabel the earlier native timing pins as a post-#264 measurement.
- The task stays in a bounded 4.18 GB scratch filesystem, below the approximately
  11 GB guest limit. Observed total filesystem use including this archive is
  2.45 GB. The target is removed before counting and checked absent after the
  run. Final [cleanup](cleanup.json) removes our task directory and confirms no
  task process or target remains; unrelated retained data is untouched.
- The guest is released for handoff. No new AWS resources or incremental AWS
  collection spend, persistent service, guest setting change or play hold.
