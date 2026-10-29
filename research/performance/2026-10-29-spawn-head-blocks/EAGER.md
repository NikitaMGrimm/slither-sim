# Exact head edge blocks in Battledome spawn searches

Keep the exact block guard. On the portable synthetic exhausted-search row,
total user instructions fall **63.42%** in both protocols across five
alternating pairs each. Median per-window CPU p99 falls **1.512 → 0.676 ms
(protocol 19)** and **1.521 → 0.675 ms (13)**, with separated five-window
ranges. Exhaustion ticks use 64.47–64.48% fewer instructions; the independently
selected slowest 1% uses about 60% fewer.

The cache is not free. Complete construction of the preferred/floor Safety
pair rises from about **7 to 14 µs** on overload, **28 to 51 µs** at 80 viewers
and **58 to 106 µs** at 160. Candidate block construction alone costs roughly
3, 9–10 and 19 µs per Safety obstacle set, respectively. All construction is
included in the main tick rows. The 80/160 controls use **0.07–0.10% more total
instructions** and their five successful search ticks use **4.5–6.0% more**.
Control CPU p50 rises about **1–8 µs**; protocol-13/80 and both 160-viewer p50
ranges separate, while protocol-19/80 p50 ranges overlap. All control p99
ranges overlap. Parent/layout effects are not isolated. No ordinary-tick CPU
improvement or live-capacity gain is claimed.

Baseline `3c55991d` contains #272; merged main `e860ec23` has identical Rust/
Cargo/check-runner source. Candidate `ea5869fa` adds consecutive eight-edge
boxes to each immutable Safety obstacle. Later main integration retains the
candidate's exact Rust source. The box guard skips only provably distant
blocks; retained edges run the existing exact predicate in original order.
Candidates, RNG draws, admitted poses, refusals and delays are unchanged.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[profile](profile.json), [correctness](correctness.json),
[timing](measurements.json), [construction](construction.json),
[verification](verification.json).

## Profile before implementation

The portable post-#272 baseline was sampled at 499 Hz using `cpu-clock:u`
and DWARF stacks. FIFO acknowledgements enable sampling after the 300-tick
warmup and disable it after a wholly blocked 3,600-tick window. The fixture
qualifies 1,800 exhausted searches, 7,372,800 poses and 72 remaining waiters.
An earlier mistaken release-half profile is retained and excluded.

Of 1,333 samples, 998 (74.9%) contain the head predicate, including 833 (62.5%
of all samples) classified as edge projection/comparison and 165 (12.4%) as
other head work. Safety construction receives seven samples (0.5%); ring
geometry receives 136 (10.2%). These exclusive stack-membership sample counts
are diagnostic, not exact phase clocks or live-tail shares. The profile also
includes fixture trace writing between ticks; it is separate from selected
uninstrumented timing. No lost samples are reported by perf.

Head edge scans are therefore the selected target. The earlier
[failed-search census](../2026-10-29-spawn-head-distance/REPORT.md)
observed about 246k edge tests and 9.4k obstacle visits per failed search;
these counts describe that earlier diagnostic, not a fresh census here.

## Exact guard and proof scope

Cache bounds over consecutive groups of eight edges when an obstacle has
more than eight edges. Every endpoint, query coordinate and nonnegative width
must lie in the finite `1e7` domain; unsupported data uses the original linear
edge loop. A partial last block includes every remaining edge. The initial
head edge remains present. No bounds or arithmetic in body/departure checks
change.

The query box expands by `head_width + 1`. In this domain, differences/products
in the original projection cannot overflow, and its clamped coefficient stays
in `[0,1]`; projection and distance roundoff is far below one unit. A strict
axis-box separation therefore proves the original distance exceeds the
inclusive width. Boxes inside that conservative margin retain every exact
test. The extra unit is a guard against skipping, not a changed clearance or
allowed output tolerance. This follows the finite-domain approach in
[visibility blocks](../2026-10-28-mature-covered-bounds/REPORT.md).

The independent raw-geometry oracle freezes scalar projection, hypot and
whole-obstacle admission arithmetic without calling Safety or its block
helpers. The candidate adapter alone constructs cached bounds. It covers
1,223,404 whole head verdicts, 50,159 direct multi-block verdicts, 409,996
standalone segment comparisons and 19,992 independent swept comparisons.
Cases include signed-zero exhaustive coordinates, random ordinary/full-bit
floats, first/middle/partial-last blocks, repeated and zero-length edges,
adjacent thresholds, invalid obstacles, non-finite/extreme edges and bounds,
and both sides of the finite guard domain. The original `>` and `<=` swept
comparisons remain separately checked, including NaN.

All six focused tests pass. The deliberately wrong skip fails the new block
boundary test; exact restoration passes again. All 15 Rust commands pass on
this builder. All 23 catalog, six host and two queued traces retain complete
bytes and terminal records; both protocol-13/19 wire mutation controls change
actual frames. Frozen-parent repetition, cross-arm and direct-parent
continuation proofs pass. Every counted tick qualifies at a 1.0 PMU scheduled
fraction. The final source equals the proved candidate; the later main merge
changes only research/docs relative to that source pin.

## Matched portable fixed-work results

| Row | Protocol | Instructions change median [range] | CPU p50 base → candidate ms | CPU p99 base → candidate ms | CPU max base → candidate ms |
| --- | --- | ---: | ---: | ---: | ---: |
| spawn | 19 | -63.42% [-63.42, -63.42] | 1.393 → 0.618 | 1.512 → 0.676 | 1.642 → 1.023 |
| spawn | 13 | -63.42% [-63.42, -63.42] | 1.394 → 0.619 | 1.521 → 0.675 | 1.654 → 1.011 |
| seats80 | 19 | +0.10% [+0.10, +0.10] | 0.148 → 0.149 | 0.423 → 0.445 | 0.836 → 0.841 |
| seats80 | 13 | +0.09% [+0.09, +0.09] | 0.146 → 0.149 | 0.407 → 0.411 | 0.876 → 0.859 |
| seats160 | 19 | +0.10% [+0.10, +0.10] | 0.455 → 0.462 | 0.899 → 0.915 | 1.401 → 1.505 |
| seats160 | 13 | +0.07% [+0.07, +0.07] | 0.456 → 0.464 | 1.096 → 1.112 | 1.448 → 1.520 |

| Row / protocol | Group | Instructions change median [range] | CPU p99 base → candidate ms |
| --- | --- | ---: | ---: |
| spawn / 19 | search | -64.48% [-64.48, -64.48] | 1.593 → 0.679 |
| spawn / 19 | exhausted | -64.48% [-64.48, -64.48] | 1.593 → 0.679 |
| spawn / 19 | other | -0.02% [-0.02, -0.02] | 0.047 → 0.048 |
| spawn / 19 | slowestCpu1Percent | -60.60% [-60.72, -60.24] | 1.642 → 1.023 |
| spawn / 13 | search | -64.47% [-64.47, -64.47] | 1.619 → 0.679 |
| spawn / 13 | exhausted | -64.47% [-64.47, -64.47] | 1.619 → 0.679 |
| spawn / 13 | other | -0.01% [-0.01, -0.01] | 0.046 → 0.046 |
| spawn / 13 | slowestCpu1Percent | -60.38% [-60.48, -59.78] | 1.654 → 1.011 |
| seats80 / 19 | search | +5.99% [+5.99, +5.99] | 0.423 → 0.435 |
| seats80 / 19 | other | -0.00% [-0.00, -0.00] | 0.355 → 0.353 |
| seats80 / 19 | slowestCpu1Percent | +0.96% [-2.60, +0.96] | 0.836 → 0.841 |
| seats80 / 13 | search | +5.92% [+5.92, +5.92] | 0.315 → 0.331 |
| seats80 / 13 | other | -0.01% [-0.01, -0.01] | 0.407 → 0.411 |
| seats80 / 13 | slowestCpu1Percent | -0.20% [-0.20, +12.75] | 0.876 → 0.859 |
| seats160 / 19 | search | +4.66% [+4.66, +4.66] | 0.719 → 0.878 |
| seats160 / 19 | other | +0.03% [+0.03, +0.03] | 0.899 → 0.915 |
| seats160 / 19 | slowestCpu1Percent | +1.38% [-8.07, +12.07] | 1.401 → 1.505 |
| seats160 / 13 | search | +4.53% [+4.53, +4.53] | 0.823 → 0.840 |
| seats160 / 13 | other | +0.00% [+0.00, +0.00] | 1.091 → 1.112 |
| seats160 / 13 | slowestCpu1Percent | +0.06% [+0.06, +8.50] | 1.448 → 1.520 |

| Row / protocol | CPU p50 range base / candidate ms | CPU p99 range base / candidate ms |
| --- | ---: | ---: |
| spawn / 19 | 1.386–1.396 / 0.615–0.618 | 1.505–1.540 / 0.670–0.746 |
| spawn / 13 | 1.387–1.398 / 0.615–0.620 | 1.510–1.623 / 0.673–0.679 |
| seats80 / 19 | 0.147–0.152 / 0.149–0.150 | 0.391–0.449 / 0.429–0.468 |
| seats80 / 13 | 0.146–0.147 / 0.149–0.150 | 0.394–0.608 / 0.403–0.419 |
| seats160 / 19 | 0.454–0.456 / 0.461–0.463 | 0.890–0.935 / 0.896–0.952 |
| seats160 / 13 | 0.455–0.458 / 0.462–0.464 | 1.087–1.290 / 1.101–1.141 |

| Row / protocol | Whole two-Safety CPU median base → candidate µs | Paired CPU ratio median [range] | Candidate blocks alone / one Safety µs |
| --- | ---: | ---: | ---: |
| spawn / 19 | 6.998 → 13.873 | 1.978 [1.800, 2.006] | 3.086 |
| spawn / 13 | 6.955 → 13.877 | 2.003 [1.791, 2.011] | 3.085 |
| seats80 / 19 | 27.727 → 51.214 | 1.838 [1.687, 1.852] | 9.311 |
| seats80 / 13 | 27.627 → 50.613 | 1.832 [1.749, 1.864] | 9.536 |
| seats160 / 19 | 57.984 → 106.011 | 1.828 [1.802, 1.883] | 18.700 |
| seats160 / 13 | 58.522 → 105.406 | 1.801 [1.792, 1.915] | 19.530 |

CPU cells are medians of five per-window quantiles, including maxima; they
are not pooled quantiles. Ranges retain all five windows. Each arm's slowest
1% is selected independently by its own CPU clock, so tick membership may
differ. Full-window work and complete bytes must match. Phase quantiles are
not added together. The discarded warmup remains in selected JSON.

Reuse #272's frozen full-host fixture: seven held synthetic rings, an observer
and 72 queued requests, warm 300 and count 360 ticks. Every overload window
must make 180 exhausted 4,096-pose searches and leave all 72 waiting. The
80/160 controls use separated synthetic held viewers, a seat-sized arena and
five successful reconnects/poses per measured window, with no backlog and all
viewers alive. They are not naturally grown bots or real sockets. Held-pose
resets, reconnect insertion and trace writing are outside per-tick counters;
actual queue/admission, world/broadcast and delivery hand-over are inside.
Safety and new block construction are inside the complete search/tick timing.

Separate construction diagnostics freeze each fixture after warmup and
repeatedly construct/drop the preferred and departure-floor Safety objects.
Sixteen pairs of objects per sample amortize counter/clock overhead; 64
samples form a window. Five alternating whole-build pairs per protocol/row
follow a discarded warmup. World state must remain unchanged. The descriptive
blocks-only column builds/drops just block bounds over existing edges for
one Safety's obstacle set, also 16 repeats × 64 samples, in one separate
window. It excludes edge/path construction and is not a measured phase
subtraction from main ticks. No extra diagnostic clocks perturb main pairs.

## Execution and limits

Both arms use the repository's default portable release optimization settings
(`lto=thin`, one codegen unit), locked/offline dependencies and Rust 1.99,
without `target-cpu=native` or other Rust flags. Profile binaries alone retain
debug information; counted binaries use the default release debug setting.
One pinned AWS virtualized Intel core runs children, with a separate physical
core for the controller. SMT is disabled at launch. No concurrent builds,
tests, polling commands or reduction run during counted work. Hardware
`instructions:u` must have at least 99% scheduled time per tick; no kernel or
security setting is changed. Fork repetition, cross-arm and direct-parent
continuation prove the frozen workload. This is Frankfurt Spot evidence,
not dedicated physical-host timing or a comparison to Broadwell.

There is no real-network jitter, spawn-wait or live 320/480-seat capacity
claim. Control rows bound ordinary fixture overhead, not all live worlds.
No independent agent review was performed. Full raw sources, traces, binaries
and logs remain local-only outside Git; archive and manifest digests do not
grant access. Selected JSON is sufficient to recompute these tables with
[tables.py](tables.py), reusing earlier reducers and harnesses by reference.

## Retrieval, cost and cleanup

The combined conservative bound is **$0.35**, comprising 30 rounded
launch-to-final-scan minutes at the $0.20/h bid cap plus a shared $0.25 allowance
for storage, transfer and IPv4. This is a bound, not an invoice. The prelaunch
full-timer bound was $0.657, under the $1 cap. [Cost receipt](cost.json).

The 120-minute in-guest poweroff timer was installed at bootstrap, with
terminate shutdown behavior and DeleteOnTermination verified. After complete
retrieval, the builder was explicitly terminated before its timer. The task key
and security group were deleted; protected resources were untouched. A first
all-region scan found one active implicit Spot request tied to the terminated
builder. Explicit cancellation was acknowledged. The final scan at 18:06:19 UTC
covered all 17 enabled regions and found zero nonterminated instances,
open/active Spot requests, volumes (including available), Elastic IPs or
project/task keys and groups. [Cleanup receipt](cleanup.json).

Every archive was closed, tar/gzip-read and fully member-hash-verified remotely
before readiness. Downloads were size/SHA-verified locally; selected JSON and
remote-generated tables were independently bound to the final manifest. The
immutable source/byte-proof archive was retrieved before counting; the timing
and results delta references that archive. Sources survived until verified
retrieval. Lean pins and the method appear once in verification. The SSH guest
was untouched. All native builds, Rust tests, traces, timing and reduction ran
on AWS; local work was limited to editing, Git and short metadata/I/O controllers.
