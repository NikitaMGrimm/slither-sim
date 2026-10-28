# Official-density single-core host capacity, 2026-10-28

**Current main does not keep up at 2,281 seats on this guest, even after bot
compute is excluded.** Three one-core holds deliver **10.07–10.57 ticks/s**
with all server work included. Simulation plus the one socket alone spends
**32.3–33.9 CPU ms/tick**, about **461–485% of a one-core 7 ms budget**.
Its same-work CPU-budget equivalent is **29.5–31.0 ticks/s**, before paying
the socket cost of 2,281 real connections.

At the same observed work mix, the median world-work budget would need about
**78.9% less CPU per tick**; the all-work equivalent about **92.8% less**.
These are descriptive VM measurements
and conditional CPU-budget arithmetic, not measured optimization savings.
The exact profile puts **75.7% of simulation-thread user instructions in
broadcast/encoding**. No qualified set of two or three optimizations is claimed
to close that gap; the ranked candidates below target measured remaining costs.

The 500-bot reference delivers 72.45–86.99 ticks/s with bots sharing the core.
Its world-work projection uses 69.5–77.8% of the 7 ms budget, before the missing
real-socket cost. A pristine, cold-start target check also misses full speed.

[Protocol](PLAN.md), [retained attempts](ATTEMPTS.md), [collector](run.py),
[archived-copy build overlays](prepare.py), [public result selection](select_results.py),
[selected results](results.json), [source/build receipts](source-pins.json),
[execution environment](environment.json).

## Target holds and accounting splits

Each CPU hold lasts 300 seconds. Tick throughput, deferred counts and duration
percentiles use the exact complete-block counter window inside that hold;
its duration and tick denominator are shown explicitly. End fragments are not
silently labelled as full-hold counts. Boundary-overlapping duration blocks
are retained separately to bound the whole-hold maximum.

| Target hold | Tick window, s / ticks | Ticks/s | Deferred ticks in that window | Server CPU | External client CPU | Tick p50 / p90 / p95 / p99 / max, ms |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 1 | 291.30 / 3,000 | 10.299 | 38,616 | 99.92% | 0.51% | 93.76 / 110.31 / 116.58 / 194.06 / 234.95 |
| 2 | 288.02 / 2,900 | 10.069 | 38,298 | 99.91% | 0.52% | 95.43 / 114.27 / 119.33 / 201.12 / 276.35 |
| 3 | 293.18 / 3,100 | 10.574 | 38,831 | 99.91% | 0.47% | 91.18 / 107.56 / 113.09 / 199.64 / 235.07 |

The whole-hold duration maxima are bounded above by 236.03, 276.35 and
235.07 ms from the boundary-overlapping blocks. Holds 2 and 3 contain those
maxima in their interior, so those two maxima are exact for the complete holds;
hold 1's exact maximum lies between 234.95 and 236.03 ms. Interior host
`--stats` reports span 8.6–11.4 ticks/s and 86.1–113.3% `simulation_load`.

Peak server RSS over the complete target run, warmup and profile included:
**990.05 MiB**. All three quiet screens pass; the largest other surviving
process uses 1.98–2.03 CPU seconds per 300-second hold, below the predeclared
3-second screen. Full command-line inventories remain local-only. There is
no competing compiler or simulation worker in the recorded boundaries.

| Same target hold | (a) All server CPU ms/tick | (a) 7 ms budget used | (b) Simulation + one socket CPU ms/tick | (b) 7 ms budget used | (b) CPU-budget-equivalent solo ticks/s |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 97.02 | 1,386% | 33.14 | 473% | 30.18 |
| 2 | 99.22 | 1,417% | 33.94 | 485% | 29.47 |
| 3 | 94.49 | 1,350% | 32.27 | 461% | 30.99 |

At the actual slowed tick rate, (b) occupies only about **34.1% of the core**.
That is not its cost at full speed: normalizing by completed ticks gives the
461–485% target-rate budget above. Each bot worker uses 32.85–32.90%, the
simulation thread 32.52–32.56%, sockets 1.56–1.65%, and CLI statistics about
0.02%. These are full-hold process/task CPU deltas, not lifetime `ps` averages.
Socket kernel CPU is 4.32–4.44 seconds per hold; simulation kernel CPU is
0.50–0.77 seconds. Kernel instruction counts are unavailable.

The target first passes the frozen mean/p90 screen at 105,000 ticks: mean
63.39 segments, p90 174, p99 250 and maximum 293. Across its last three
checkpoints, mean max/min is 1.01517 and p90 max/min 1.01149. Food spans
232,336–241,313. This is a pass of the named screen, not all-quantile stationarity:
the median moves 37 -> 36 -> 34 over those checkpoints.

After re-pinning, mean length declines from 62.28 at the first retained hold
edge to 58.82 at the last, while p90 stays near 170–172. Within individual
holds mean changes -2.64%, -2.76%, -0.81%; median changes -6.25%, -9.68%,
-3.45%. Living sample minima are 2,278, 2,278 and 2,279; mean living counts
2,280.13, 2,280.13 and 2,280.47. Occupied seats remain 2,281. Normal deaths,
respawns and growing death food remain included. The final phase profile
describes the continued world after hold 3, not an assumed stationary copy of
hold 1 or indefinitely long uptime.

## 500-bot reference and pristine target control

The reference has 500 server bots **plus one player**, not 500 total seats.
It passes the same frozen screen at 120,000 ticks: mean length 105.02,
median 117 and p90 185. The three screening endpoints have mean max/min
1.03593 and p90 max/min 1.04520. This pre-pin screen does not survive the
scheduling transition unchanged: mean length falls from 94.20 to 78.67 in
hold 1 (-16.49%), with median -56.25% and p90 +10.11%. Mean changes in
holds 2 and 3 are -0.24% and +2.07%. All three holds remain reported; the
reference is not claimed to be stationary after re-pinning.

| Reference hold | Tick window, s / ticks | Ticks/s | Deferred ticks | Server CPU | External client CPU | Tick p50 / p90 / p95 / p99 / max, ms |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| 1 | 298.13 / 21,600 | 72.451 | 20,993 | 99.91% | 0.57% | 13.50 / 18.55 / 20.20 / 24.59 / 40.68 |
| 2 | 298.63 / 24,200 | 81.036 | 18,462 | 99.92% | 0.67% | 11.76 / 16.82 / 18.47 / 22.85 / 40.98 |
| 3 | 298.88 / 26,000 | 86.990 | 16,694 | 99.92% | 0.64% | 11.15 / 16.22 / 18.05 / 22.64 / 41.25 |

| Same reference hold | (a) All CPU ms/tick / 7 ms budget | (b) World + one socket CPU ms/tick / 7 ms budget | (b) Conditional scheduled ticks/s |
| --- | ---: | ---: | ---: |
| 1 | 13.79 / 197.0% | 5.44 / 77.8% | 142.857 |
| 2 | 12.33 / 176.2% | 5.10 / 72.9% | 142.857 |
| 3 | 11.49 / 164.1% | 4.86 / 69.5% | 142.857 |

Median reference throughput is 81.04 ticks/s; median world CPU cost is
5.10 ms/tick. Its uncapped CPU equivalents are 183.7–205.6 ticks/s, capped
above at the configured 142.857 ticks/s. These are conditional budget
projections, not demonstrated 501-remote-player holds. Peak server RSS is
175.95 MiB. Per-thread full-hold shares are simulation 37.91–40.79%, bot
worker 0 28.92–30.57%, bot worker 1 28.69–29.88%, sockets 1.52–1.55%,
and CLI 0.01–0.02%. Socket kernel CPU is 4.12–4.22 seconds per hold.
All quiet screens pass; the largest other surviving process uses 1.92–1.95
CPU seconds per hold. Boundary-overlapping maxima equal the interior maxima
in all three reference holds. Living minima are 499, 500, 500; occupied
seats remain 501. The target/reference comparison changes population and
ecology; it is not a paired optimization measurement.

After both primary cases, the pristine target CLI uses the same 2,280 bots
and external player, with all server threads on CPU 0 and the player on CPU 1.
A 30-second cold warmup precedes one 300-second CPU hold. Its 58 retained
interior five-second `--stats` reports have median **16.6 ticks/s**, range
**15.2–17.6**, deferred counts 573–688 per report and `simulation_load`
90.3–108.3%. Server CPU is 99.94%, client CPU 0.40%, peak server RSS
799.15 MiB. The quiet screen passes (largest other process 2.47 CPU seconds).
No research census, duration or PMU hooks exist in this binary. Lengths are unknown;
this is evidence that the basic shortfall also occurs without hooks, not a
mature-length qualification, paired throughput comparison or hook-overhead
estimate. No panic/error/failure lines appear in any final host/client log;
collectors verify liveness and the census verifies actual world advancement.

## Scope

Real offline `slither-sim host --stats 5` at main
`d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`, including the spawn-bounds (#244)
and client-frame materialization (#245) optimizations. No production source
changes are delivered. A 4-vCPU Zen 5 EPYC KVM guest runs Linux x86-64,
Rust 1.99.0, glibc 2.41 and Python 3.13.5. Locked native-CPU release builds
use thin LTO, one codegen unit, debuginfo=1 and one Cargo/Rayon worker.

The stock-compatible backend permits at most 2,281 seat-following seats.
`670 * sqrt(2281)` is about 31,991.9 world units; the next seat exceeds the
32,000 supported radius. The host requires at least one socket player slot
and validates every configured seat, even an empty socket slot. Therefore its
practical maximum is **2,280 server bots plus one player**, rather than 2,281
bots with a spare player slot. The pristine CLI rejects the latter setup.

The actual radius is retained in every census. The normal seat-following
controller has a dead zone, so it need not equal the ideal target bit for bit;
the target run's observed radius is about 31,992.06, less than 0.2 units from
the ideal. No controller override or density-increasing cap is added.

The target uses 2,280 default A4 Foragers and one loopback A4 player through
the public `NetworkBackend` and real WebSocket protocol. The reference uses
500 server bots and the same player, hence **501 occupied seats**. Both use
seed 7, the official world preset, official delivery, seat-following arena,
default six-tick bot decisions and default zero artificial link/processing
delay. During each measured hold, the two normal server bot workers,
simulation and socket threads all use CPU 0. The loopback client and low-rate collectors use CPU 1; its CPU
is reported separately. CPU affinity is checked throughout collection.
No outside game server, freezing, unloading, lower physics rate, replacement
policy or authoritative observation shortcut is used. Overload defers ticks
and slows world time; every executed tick advances the complete world.

The maintaining agent later authorized multi-core warmup on the owner's behalf. After roughly 20,000 primary
ticks on CPU 0, the same running host's allowed set became CPUs 0, 2 and 3;
the reference uses that warmup set from startup. Warmup is excluded from the
capacity windows. Before holds, all host tasks are re-pinned with
`taskset -a -p -c 0 PID`, every task's affinity is verified and a 60-second
single-core settling interval is discarded. Pin timestamps, task readbacks
and full process-list hashes are retained. The live all-thread pin/readback
procedure first passed an idempotent CPU-0 test. This changes scheduling,
not game state; hosted trajectories remain clock-driven, not deterministic
replays. Hold length drift is reported rather than assumed absent.
The target re-pin completes at **08:47:00.600 UTC** and the reference at
**09:18:01.166 UTC**, 2026-10-28. Every one of the five host threads reads
back CPU 0; transition receipts and per-thread affinities are in the results.

The configured decision interval is 42 ms, but an overloaded bot worker may
be late and the existing host makes at most one catch-up decision per seat.
The instruction profile therefore retains actual decision counts; requested
intervals do not prove that all policies ran at their nominal wall rate.
Neither the harness nor the policy reduces any configured rate.

The loopback client uses protocol-19 framing/decoding and SlitherSim's normal
physical-food/rules opt-in, as do server bots. It is not a stock browser or
rendering qualification. Its real socket I/O cost is included in server CPU.

## Measurement boundaries

Elapsed times are **descriptive VM timings**. They are not a bare-metal
throughput guarantee or a comparison with an outside server. User-instruction
counts are a separate experiment after the holds, in the same continued world.
Kernel instruction access returns permission denied under the existing PMU
policy; kernel CPU seconds remain available from `/proc`. No sysctl, limit,
service or system configuration is changed.

The measured CLI copy adds read-only length/population census and duration
hooks to the archived source, plus phase hooks that are inactive during warmup
and timed holds. It retains the real host, policy, decoder and wire scheduling.
The pristine CLI and client are built first from the exact archive. Hook checks,
clocks and one census per 100 ticks remain part of the measured copy's cost;
the counting events remain open, and no uninstrumented elapsed-time equivalence
is asserted. IP sampling is inactive until the separate profile. The task-owned build
target is removed before collection and no compilation runs alongside holds.

Warmup requires at least 30,000 completed ticks and three successive 5,000-tick
endpoint blocks whose mean living length and p90 each span at most 5%.
All warmup blocks and food-stock drift are retained. This screen is a bounded
length-stability test, not proof of ecological stationarity. The initial
90-minute ceiling was extended on the owner's behalf before any hold;
the amended ceiling is three elapsed warmup hours. A case
that fails the screen still runs its holds, labelled **unsettled**.
Three consecutive 300-second holds then preserve each window separately;
they are repeated windows of one evolving world, not independent seeds.

CPU is user plus system seconds divided by wall seconds, where 100% is one
logical CPU. Per-thread counters read the explicit
`/proc/PID/task/TID/stat` path and must sum to process CPU within clock
quantization. Peak memory is process `VmHWM`, including warmup, not allocator
usage or total guest memory. The external player's CPU is excluded.
Asynchronous interrupt/softirq work outside the process is not charged to its
CPU counters. Aggregate guest CPU fields retain that context without attributing
every kernel operation exclusively to this listener.

Tick duration spans command application/admission, complete world advance,
broadcast and handover. It excludes sleep, includes preemption and excludes
the subsequent census/reporting hook. It is distinct from the host's own
`simulation_load`, whose busy-wall batch interval includes preemption and
reporting and can straddle the five-second statistics interval. Values near
or above 100% in one statistics line do not mean more than one CPU was used.
Percentiles are empirical order statistics over full 100-tick blocks inside
each hold. The complete CPU hold is slightly wider than the tick window;
both denominators are retained.

Two splits are computed from each hold. **(a) Everything** includes both bot
workers and the host's small CLI thread. **(b) World work** includes the
simulation thread's full physics, admission, per-recipient frame encoding for
all 2,281 recipients and handover, plus the socket/I/O thread. Bot decode,
owned frame materialization and policy are outside (b), as they would execute
on remote clients. The socket thread here services only one real connection;
in-process bots exchange their encoded bytes through host channels.

For budget arithmetic, cumulative process/thread CPU counters are linearly
interpolated from approximately one-second samples to the retained tick-window
edges. CPU ms/tick divided by 7 ms gives the required one-core share at full
rate; its reciprocal gives the implied standalone tick rate if that same work
and execution cost were unchanged. This **projection is not a separate
remote-bot experiment or a 2,281-connection capacity qualification**. Removing
competition can change scheduling, client decisions, ecology and execution
cost. Full-hold CPU percentages remain separate from these aligned estimates.

Bot decisions follow wall time while overloaded physics slows world time.
Consequently policies can execute more often per completed physics tick even
while late workers miss their nominal wall cadence. The all-work per-tick
equivalent is an observed-mix budget, not a minimum physical-core count or an
exact required optimization factor for a different input schedule. Socket CPU
also mixes per-wall-time polling with per-message work; those parts are not
isolated here. The implied rate is a conditional CPU-cost equivalent, capped
at the configured 142.857 ticks/s when reporting a scheduled host rate.

Full process lists with command lines are retained at every hold boundary
outside Git. Public selected results contain their hashes, other-process CPU
deltas, system CPU deltas and interference assessment. Private machine and
account details are omitted. Hashes identify artifacts without making raw
data publicly retrievable.

## Missing real-socket cost: historical extrapolation

Split (b) does **not** contain the send/receive/framing cost of 2,281 real
connections. A retained [500-socket recheck](../2026-10-27-c7a-main-recheck/REPORT.md)
at runtime `4532a76f22958ae3abcca801e90edd30ed9f466f` reports socket-thread CPU
of 34.9% of one core at 142.851 ticks/s. Dividing by 500 connections gives
about **4.89 microseconds per connection per executed tick** on that older
platform. Linear scaling to the **2,280 additional** connections absent here
would add about **11.1 CPU ms/tick**, or **159% of a core** at the 7 ms target.

This is an **extrapolation**, not a measured cost on the current guest and not
a bound or confidence interval. The retained run used different EPYC hardware,
a portable build, a short scripted nonboosting workload, 100 ms controls and a
different packet mix. It does not calibrate the current mature A4 world,
official delivery, loopback/physical networking or 2,281 real sockets. It
illustrates a missing budget; it is not silently added to measured CPU or used
to claim a 2,281-player capacity result. The inputs and arithmetic are retained
in the selected JSON. Real-socket tail latency remains unqualified.

## Instruction attribution

The target's separate post-hold profile counts **500 complete simulation ticks**
and **649,549 bot policy calls**. All current-thread brackets and whole-thread
counter windows pass the >=99% PMU scheduling checks; the whole-thread window
has 100% running/enabled coverage. These are user instructions, not timings.

| Target phase | Million user instructions per counted physics tick |
| --- | ---: |
| Movement/preparation, including boost food | 2.315 |
| Collision index/bounds build | 2.055 |
| Collision query/exact tests | 7.484 |
| Death processing/food | 0.0107 |
| Pickup/food cache | 7.331 |
| Prey | 0.742 |
| Ambient food | 0.0199 |
| Admission search, nested in complete tick | 0.823 |
| Broadcast/encoding, nested in complete tick | 74.813 |
| **Complete simulation tick** | **98.866** |

The seven world phases total 19.957M/tick. Broadcast is 75.7% of the complete
simulation-thread user-instruction budget; collision build/query together are
9.65%, pickup/cache 7.42%, admission 0.83%. The small remainder includes
unbracketed command/lifecycle/handover work and the stated world-step tail.
These fractions are not CPU-time fractions and are not predicted savings.

| In-process bot stage | Thousand user instructions per profiled policy call |
| --- | ---: |
| Delivered-frame decode/application, amortized | 9.490 |
| Owned client-frame materialization | 44.134 |
| Unchanged A4 policy | 162.165 |
| Three bracketed stages | 215.790 |

Policy is 75.1% of these three bracketed bot stages, materialization 20.5%
and decoding 4.4%. Frame destruction, scheduling, channels, pings and other
worker work remain outside them. Their CPU time is not isolated. In the
independently edged 45-second whole-thread window, bot workers retire
114.219 billion user instructions, simulation 47.555 billion, sockets
0.0737 billion and CLI statistics 0.000230 billion. Those totals must not be
subtracted from differently edged phase totals to invent an exact residual.

The supplemental IP sample places 10.7% of simulation-thread user-instruction
weight in inline `snakes::covered`, and 4.82% in inline food-vector rotation.
They overlap their children and must not be added to self counts. Another
45.1% lands in `libm`, whose callers are not recoverable from these samples.
On the two bot workers, inline `gather` is 17.5–17.9%, `Scene::new`
16.4–16.9%, with overlap; 36.4–37.0% lands in `libm` and 20.7–22.2% in
`libc`. Library weights are not assigned to a particular policy subroutine,
allocator or phase. Non-user overflow locations remain unattributed
(about 0.55% of simulation weight and 1.3% per bot worker); socket sampling
is sparse and supplies no reliable send-path ranking.

The population-scaling study's current-thread user PMU counter is reused.
Coarse world phases cover movement/preparation, collision index build,
collision query/tests, death processing/food, pickup/food cache, prey and
ambient food in order. Final score finishing, buffer restoration/destruction
and call overhead remain outside these seven phase brackets, inside the
complete simulation tick. Admission search and world-wide broadcast are nested subsets
of the complete simulation tick, not additional work to add to it. Bot decode,
owned `ClientFrame` materialization and unchanged A4 policy are disjoint
brackets on each bot worker; frame destruction and scheduling/channel work
remain outside those three brackets. Bracket/compiler overhead remains.

After the holds, a marker activates a 60-second profile without rebuilding or
resetting the world. Simulation counters flush every 100 ticks. Bot counters
flush at the next complete worker-loop boundary after the integer wall second
changes, so a late worker can skip nominal report seconds. A separate
45-second counter window counts every server
thread, including sockets. These intervals have different edges and must not
be summed as a single exact decomposition. Every counted PMU interval requires
at least 99% enabled/running coverage. CPU seconds and instruction counts are
reported in their own units; neither is converted to the other using an assumed
instructions-per-cycle constant.

The reference's post-hold profile counts 3,700 complete ticks and 700,825
decisions, also passing the PMU coverage checks. Its complete simulation tick
is 28.281M user instructions: broadcast 22.732M, collision build/query 3.284M,
pickup/cache 0.797M, movement 0.510M, prey 0.214M, admission 0.0848M,
death food 0.00247M and ambient 0.00480M. Decode/application, materialization
and policy cost 9.680k, 52.932k and 131.692k instructions per profiled
decision. These values describe its different post-hold world; they are not
a controlled population scaling curve. Complete counters for both cases are
retained in the selected results.

A separate 15-second retired-user-instruction IP diagnostic within the profile
window supplements the brackets. Its executable locations use ELF PT_LOAD
virtual addresses, independently checked with a synthetic regression. Shared
libraries remain module-only: nearest exported symbols in stripped libraries
are insufficient to identify a function or caller. Non-user overflow locations
remain unattributed and are not kernel instruction counts. Compiler inline
locations overlap and are not full dynamic call chains. These sample weights
can suggest a source owner; they are neither exact phase counts nor measured
optimization savings. All initial failed/superseded warmup pilots are retained
in the attempt ledger and excluded from final profiles.

## Ranked exact-behavior optimization work

The owner's capacity target is real players on one core, so the deciding
split is **(b), world work with clients elsewhere**. Bot-policy
or client-view savings do not fix that budget. The rankings below identify
concrete byte-preserving strategies, with measured **parent-path budgets**;
they are not implemented candidates, validated speedups or promises that three
changes achieve 7 ms. Subcomponent instruction counts and CPU-time savings are
unavailable where stated. Do not replace these unknowns with a guessed saving.

### (b) World work, including encoding every recipient

| Priority | Exact transformation to investigate | Measured budget and evidence | Required preservation |
| --- | --- | --- | --- |
| 1 | Refine recipient body visibility by indexing body segments, instead of testing every edge of each whole-body-box candidate in `snakes::covered`. | Broadcast is 74.813M user instructions/tick (75.7% of simulation); inline `covered` has 10.7% of sampled simulation instruction weight. `libm` is another 45.1%, with unknown callers. The exact visibility-only budget is not isolated. | Conservative segment bounds, head/first-edge coverage, original exact distance predicate, original ordered snake IDs and lifecycle/removal behavior. Fall back for unindexable/extreme cases. Every snake still runs full physics. |
| 2 | Refine collision candidates with conservative segment bounds before the existing exact `strikes_scaled` tests; reuse immutable per-step segment geometry where possible. | Collision query/tests are 7.484M/tick; index construction another 2.055M. Construction overhead can erase the win, so the combined 9.539M is a budget ceiling, not a saving. | Original victim/killer order, widths, swept-tip geometry, exact predicates, tangency and tie results. No reduced collision rate or authoritative shortcut. |
| 3 | Use `Food: Copy` to move retained ranges with overlap-safe bulk copying in `compact_food`, replacing range rotation. | Pickup/cache is 7.331M/tick; inline food rotation has 4.82% of sampled simulation instruction weight. The rotation and its children overlap. | Same survivor bytes/order, descending hit application, cache mirror/removal sequence, RNG and truncation. Surviving food still has to be copied; the rotation budget cannot all disappear. |

The prior spawn optimization is already present. At 0.823M/tick, admission
search is only 0.83% of current simulation user instructions; repeating the
old 162M/tick diagnosis from #242 would choose the wrong priority here.
The old synthetic wander world and this hosted A4 world also have different
length and food distributions; they are not a matched runtime comparison.

### (a) Everything, including in-process clients

| Priority | Exact transformation to investigate | Measured budget and evidence | Required preservation |
| --- | --- | --- | --- |
| 1 | Cache each opponent's exact forecast terms/points within one A4 decision and reuse them across cruise/boost scene gathers. Preserve the original scalar operations rather than replacing trigonometry with an approximation. | Policy is 162.165k instructions/call and 75.1% of the three bot-stage brackets. Inline `gather` is 17.5–17.9% of worker sample weight; library callers and the repeated-forecast subset are not isolated. | Same input frame, headings, ages, uncertainty, candidate order, sums, thresholds and action bits. No cross-frame stale cache, parameter change, weaker policy or reduced decision rate. |
| 2 | Apply the recipient-visibility refinement from split (b). | Its 74.813M/tick broadcast parent path is the largest simulation path; bot workers alone consume about 65.7% of the measured core, so client-only optimization cannot remove the remaining world bottleneck. | The same full wire/state trace obligations as (b). |
| 3 | Materialize the complete owned client frame into reusable worker scratch storage, retaining capacities for food, bodies, metadata and HUD rather than allocating fresh vectors/strings every decision. | Materialization is 44.134k/call, versus 9.490k/call for decode/application. Destruction is outside these brackets and is not assigned a guessed budget. | Every current frame field and receipt age must be overwritten exactly; retained clones remain independently owned, and reconnect/death/absent fields cannot leak prior values. Keep normal decoding of every delivered byte. |

Those orders use parent budgets and sampled source owners, not measured
relative savings of the proposed changes. The two profile cohorts have different
edges, and instruction fractions do not allocate the threads' CPU time.
In particular, `libc` samples are not all assigned to allocation, `libm`
samples are not all assigned to visibility or forecasting, and missing
2,281-socket kernel work has no current per-operation profile.

Any implementation must first preserve complete deterministic world/client/wire
traces and action bits, including births, deaths, ID reuse, signed-zero/boundary
cases and all supported packet profiles. Then measure fixed-work instructions
and fresh end-to-end holds. Source inspection and broad-path budgets alone do
not establish enough recovery for either observed gap. This PR delivers no
runtime implementation or behavior-preserving speedup claim.

## Reproduction

Use Linux x86-64 with cached locked dependencies, Rust 1.99 and user-PMU access.
Replace path placeholders with fresh task-owned storage; keep full raw artifacts
outside the repository. Finish all builds before starting the two serial cases.

```sh
STUDY=research/performance/2026-10-28-official-density-capacity
OUT="<bench-root>/official-density"
git archive --format=tar -o runtime.tar d127acd1f0e9db2100c8a1dd5c2fa43e2deff506
python3 "$STUDY/prepare.py" runtime.tar "$OUT" \
  --target /tmp/official-density-target --cpu 0
python3 "$STUDY/environment.py" "$OUT/environment.json"
```

Before starting either case, run
`python3 "$STUDY/thread_instructions.py" "$OUT/target-2281" "$OUT/reference-500"`
in a separate shell while the following commands run sequentially. It waits
for each profile marker and does not count timed holds.
For collection over SSH, redirect coordinator/sampler stdout and stderr to
task-owned files and detach stdin, so a disconnected reporting pipe cannot
terminate collection. Preserve each output directory on every failed attempt.

```sh
python3 "$STUDY/run.py" "$OUT/bin" "$OUT/target-2281" --bots 2280 \
  --warm-cpus 0,2,3 --max-warm-seconds 10800
python3 "$STUDY/run.py" "$OUT/bin" "$OUT/reference-500" --bots 500 \
  --warm-cpus 0,2,3 --max-warm-seconds 10800
python3 "$STUDY/plain_control.py" "$OUT"
python3 "$STUDY/select_results.py" "$OUT/selected.json" \
  "$OUT/target-2281" "$OUT/reference-500" --plain-control "$OUT/plain-control"
```

For the optional source-location diagnostic, start
`python3 "$STUDY/instruction_samples.py" watch "$OUT/target-2281" "$OUT/reference-500"`
in another shell before collection; it waits for the post-hold markers.
The pristine build command includes `--bin slither-sim --example portable_bot`;
the observe/profile arms build only `--bin slither-sim` and copy that same
pristine client. The receipt's common release command describes the pristine
build, with the per-arm distinction implemented in `prepare.py`.

The source archive, source and binary SHA-256 receipts, build logs, complete
host/client logs, telemetry, process lists and interrupted/exploratory attempts
are local-only. Selected bounded results accompany this report; historical
studies are not relabelled as current measurements.

## Verification and limits

The pristine CLI/client and both observation/profile copies build successfully
with the pinned locked/offline release recipe. An eight-bot loopback smoke
checks user PMU counting and affinity before primary collection. All eight
final profile-overlay Rust source hashes match the reconstructed archived-copy
recipe. After collection, all four research arithmetic/ELF tests and all seven
documentation tests pass on the guest. No fresh full Rust regression suite or
independent review was performed for this research-only change.

The target collector retained an older, unreached 180,000-tick warmup guard;
it passed the unchanged screen at 105,000 ticks. The reference and published
collector use the amended three-hour wall ceiling without that guard. Neither
case reaches a ceiling, and no hold is omitted or repeated. The collector's
`screen.wallSeconds` is its final pre-hold screen check, which also includes
the discarded post-pin minute; the re-pin receipt identifies the transition.
The `warmBlocks` array keeps 5,000-tick checkpoints through holds/profile too,
so use `screen.lastTick` to delimit the screening trajectory.

Final cleanup verifies no study host, client, collector or profiler remains;
the one bounded build target is absent. Raw attempts and process lists remain
outside Git. Public privacy and whitespace scans pass. The capacity result is
limited to this one seed, evolving trajectories and guest; neither stationarity
nor a 2,281-real-socket service is qualified.
