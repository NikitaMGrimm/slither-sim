# Live constant-boost Battledome after #272/#273

**320 still falls behind; 480 falls substantially behind in both arms.**
At 320, the candidate records 66 deferred ticks versus 231 for base, with
lower CPU/jitter/excess tails. At 480, CPU p99/max is slightly lower in the
candidate window, but median CPU, deferrals, delivery and occupancy are worse.
Spawn search still leads the slowest 1% in every row. These single windows
do not establish a repeatable speed ratio or sustainable capacity.
640 was not measured because candidate-480 failed the zero-deferral gate.

Base `3a223676` is main immediately before #272 and already contains #265/
#266; candidate `a1535902` also contains the exact head-distance and lazy
edge-block fixes. No production code changes are made in this study.
[Plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[selected results](selected-results.json), [verification](verification.json).

## Observed rows

| Seats | Arm (order) | CPU p50 / p99 / max ms | Deferred | Gap jitter p99 ms | Excess p99 / max ms | Stalls >20 / >100 / >250 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 320 | base (1) | 4.109 / 13.931 / 17.543 | 231 | 8.371 | 1536.633 / 1838.012 | 2390 / 0 / 0 |
| 320 | candidate (2) | 4.130 / 12.787 / 14.797 | 66 | 6.291 | 638.255 / 723.636 | 695 / 0 / 0 |
| 480 | base (4) | 6.661 / 20.788 / 25.740 | 4197 | 27.844 | 26066.872 / 29130.960 | 80576 / 1 / 0 |
| 480 | candidate (3) | 12.234 / 20.083 / 23.406 | 5067 | 32.930 | 29876.821 / 35550.963 | 95995 / 569 / 0 |
| 640 | both arms | Not measured | — | — | — | — |

| Seats | Arm | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Alive mean / min | Boost % / births | Length first → last | Food first → last |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 320 | base | 11.691 / 620.280 / 1157.570 (1039) | 18 / 1086.446 | 318.01 / 299 | 99.71 / 70643 | 53.23 → 55.66 | 18662 → 21168 |
| 320 | candidate | 10.082 / 663.839 / 1587.946 (1026) | 6 / 1482.757 | 318.55 / 300 | 99.82 / 72733 | 52.48 → 56.56 | 18334 → 19340 |
| 480 | base | 1120.196 / 3450.166 / 3656.555 (951) | 2 / 3603.738 | 467.03 / 433 | 99.46 / 67415 | 49.48 → 51.76 | 31153 → 28986 |
| 480 | candidate | 2705.863 / 3450.576 / 4140.961 (912) | 39 / 4083.869 | 452.23 / 432 | 99.30 / 53933 | 53.72 → 52.92 | 25010 → 29213 |

| Seats | Arm | Tail n / mean CPU ms | Spawn ms / % | Sockets+broadcast ms / % | Minimap ms / % | Other ms / % | Search / exhausted ticks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 320 | base | 125 / 14.432 | 8.801 / 60.98 | 4.563 / 31.62 | 0.308 / 2.13 | 0.760 / 5.27 | 125 / 122 |
| 320 | candidate | 127 / 13.181 | 8.012 / 60.78 | 4.145 / 31.45 | 0.310 / 2.35 | 0.713 / 5.41 | 127 / 125 |
| 480 | base | 85 / 21.353 | 12.398 / 58.06 | 7.214 / 33.78 | 0.485 / 2.27 | 1.256 / 5.88 | 85 / 72 |
| 480 | candidate | 77 / 20.959 | 11.946 / 56.99 | 7.389 / 35.26 | 0.462 / 2.21 | 1.162 / 5.54 | 77 / 47 |

| Order | Arm / seats | Canonical ticks / seconds | Full-run deferred | Calibration deferred | Outside all marked windows |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | base / 320 | 12500 / 89.369 | 277 | 0 | 46 |
| 2 | candidate / 320 | 12700 / 89.470 | 66 | 0 | 0 |
| 3 | candidate / 480 | 7700 / 89.376 | 7066 | 0 | 1999 |
| 4 | base / 480 | 8500 / 88.881 | 7056 | 343 | 2516 |

Each arm/count has one measured window. These are descriptive comparisons of
fresh, independently evolving worlds, not an isolated causal speed ratio or a
capacity maximum. Scheduled prehistory is matched in wall time; overload
changes virtual age, received inputs, body growth, food, deaths and spawn work.
The alive counts, actual boost duty, pending waits and complete deferral ledger
are part of the result, not filters for choosing a favorable ecology.

At **320**, CPU p99 changes from 13.931 to 12.787 ms and max from
17.543 to 14.797 ms, while median CPU is almost unchanged. Jitter p99 changes
from 8.371 to 6.291 ms and excess p99 from 1,536.633 to 638.255 ms. Respawn
p50 is slightly lower, but p95/max are higher: 663.839/1,587.946 ms versus
620.280/1,157.570 ms. This is an improvement in the observed server/delivery
tail, with remaining overload and no general spawn-wait improvement.

At **480**, the candidate records 5,067 deferred ticks over 7,700 measured
ticks; base records 4,197 over 8,500. Their canonical windows span 89.376 and
88.881 wall seconds, respectively. Candidate CPU p99/max is lower, but p50
is 12.234 versus 6.661 ms. Candidate jitter/excess p99 reaches
32.930/29,876.821 ms versus 27.844/26,066.872 ms. Its mean alive population
is 452.23 versus 467.03, and completed respawn p50 is 2.706 versus 1.120 s.
Both respawn p95s are about 3.45 s. The candidate has 39 pending requests at
the last reports versus 2 for base; the maximum observed pending ages are
4.084 and 3.604 s. The different virtual ages and ecologies prevent treating
these mixed differences as an isolated regression estimate.

**Spawn search remains first, sockets/broadcast second.** In the candidate's
slowest 1%, search takes 8.012 ms (60.78%) at 320 and 11.946 ms (56.99%) at
480. Sockets/broadcast takes 4.145 ms (31.45%) and 7.389 ms (35.26%). Minimap
is only 2.35% and 2.21%. All 127 candidate-320 tail ticks include a search;
125 are refused. At 480 all 77 tail ticks include a search, but only 47 are
refused, so successful searches also occur among the slowest ticks. These
are whole admission-phase costs, including more than the optimized head
predicate; no current per-function hotspot is inferred from this split.

All four primary rows and ten calibration windows pass the unchanged
client/queue gates. The largest primary worker CPU p99 is 26.84% of its
assigned core, and worst primary input lateness p99 is 28.740 ms, below the
42-ms gate. No human seat is present. Every logged action requests boost;
actual living-tick duty is 99.71/99.82% at 320 and 99.46/99.30% at 480
(base/candidate). The lower 480 duty is retained explicitly; the frozen
qualification floor was 99%, and no gate was relaxed. The same 16-core,
16-vCPU c8a.4xlarge Spot generator serves every row.

## Method and metric boundaries

Reuse [#267's method](../2026-10-29-battledome-live-recheck/REPRODUCE.md): one
c7a.medium server vCPU and a separate same-AZ generator, private-IP bot traffic,
six workers on CPUs 0–5 and control on CPU 7. Generator topology/market is
recorded per segment in selected evidence. All target host tasks remain on
CPU 0. The shipped Battledome delivery/arena preset is unchanged: seed 7,
641 socket slots, zero in-process bots, no official delivery override.

Real protocol-19 clients decode every received frame and run shared received-
frame A4 steering. The study-only override requests boost when the received
`freeBoostAtFloor` rule is true. It preserves heading bits and the rule gate;
its regression passes for both arm builds. The candidate client serves both
hosts. Shared client/policy source hashes match. No debug/authoritative bot
inputs, thin clients or additional policy are introduced.

Both hosts use Rust 1.99 and the repository's default portable release profile,
without native CPU flags. Unlike #267's debug-info build, release debug info
is left at its default. Source manifests and copied binaries are checked and
pinned once per arm. Builds and reduction run only on AWS, outside measured
client/server windows; no local native work or SSH guest is used.

Every primary row starts fresh, warms 300 seconds at 80, then settles 60 and
counts 90 scheduled wall seconds at 80, 160 and subsequent stages through its
target. Earlier stages are retained calibration windows. Order is base-320,
candidate-320, candidate-480, base-480; 640 is conditional on candidate-480's
zero deferrals and remaining time/budget. No play hold or human seat is planned.

CPU is the aligned sum of simulation and socket task clocks **per tick**, then
quantiles are computed. It is not a sum of independent task quantiles. Canonical
selection uses complete interior 100-tick blocks from final logs and whole
client-report boundaries. Independent task counters must reconcile. Full-run,
calibration and out-of-window deferrals remain visible.
This world-task CPU excludes the telemetry collector and interrupt processing
outside those tasks; collector CPU and per-core context are retained in selected
results, rather than treating world-task CPU as total core utilization.

Jitter subtracts protocol elapsed-header increments from receipt gaps; excess
subtracts cumulative header time relative to a reset running minimum. Stalls
count individual residual gaps above 20/100/250 ms. These include server slowdown
and client delivery/scheduling; they are not absolute one-way or input-to-effect
latency. Accumulated excess can grow while individual stall gaps stay small.

Completed respawn waits cover the login-to-Playing interval defined by the
unchanged collector, excluding fixed reconnect delay and the preceding TCP/
upgrade handshake. Pending requests are censored, with last count and maximum
observed age reported separately. Alive minima are periodic census samples,
not per-tick guarantees. Length means body segments. Boost births are physical
pellets; free boosting at the permanent floor creates none.

The slow-tail table selects each window's own largest aligned-CPU 1%. Four
exclusive means over those same ticks partition search, sockets+broadcast,
minimap and other. Nested minimap is subtracted before combining broadcast
with sockets. That minimap scope includes recipient gating/shared work, not
only outline rasterization. Search/refused columns count ticks with events.
No per-function explanation is inferred from a broad phase alone.

Generator input-lateness, CPU headroom, decoding progress and queue gates are
unchanged from #267. Every calibration and primary must qualify, including
at least 99% living-tick boosting. Server overload alone does not discard a row.
One window cannot establish repeatability, long-uptime behavior, a Broadwell
comparison or a sustainable capacity beyond the observed rows. Later main
changes, including #274's compact recipient histories, are outside these pins.

## Retrieval, cost and cleanup

All four closed row archives were downloaded and SHA/size-verified before
reduction; canonical selection matches each row to its own complete,
source-verified retrieval receipt. Both complete instrumented sources and
binaries were already retrieved in a member-verified build archive before
counting. Source manifests are pinned once per arm, and the spawn-safety and
physics files exactly match their declared runtime archives.

The inherited authored controls pass for aligned clocks, cadence, histogram
ranks, headroom rejection, canonical/whole-report boundaries, concurrent log
tails, archive readiness/failure and cleanup guards. Four assembler mutations
reject a changed receipt digest, incomplete status, unverified required sources
and a selected-row/archive mismatch. See [verification](verification.json).
No production changes or full Rust check group are needed for this study;
both portable release builds and boost-treatment regressions passed on AWS.

Cleanup is complete: both instances terminated, their delete-on-termination
volumes are gone, and the task key pair/security group were deleted. The final
scan completed in all 17 enabled regions: zero nonterminated instances,
open/active Spot requests, volumes, Elastic IPs and task keys/groups. Protected
resources were untouched. See [cleanup receipt](cleanup.json).
The conservative launch-to-final-scan cost bound is **$1.378**:
$0.728 compute at the launch bid caps plus the $0.65 noncompute
allowance, including every launch attempt. See [cost calculation](cost.json).

The launch-time full-timer bound is $1.976 including a $0.65 noncompute
allowance, below the $3 combined cap. It is a conservative bound, not an
invoice. One insufficient-capacity launch created no instance; there were
no Spot interruptions or generator substitutions in the measured session.
The [attempt ledger](ATTEMPTS.md) also retains the post-launch print error
and asynchronous setup timeout.

Raw logs, sources, binaries, inventories and receipts are local-only outside
Git. Digests identify them without granting public download. Only bounded
aggregates needed for the tables and single manifest/archive pins are published;
[tables.py](tables.py) recomputes the tables from selected JSON. No host/cloud
identifiers, addresses, account IDs, key/group names or credentials are published.
