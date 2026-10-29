# One-core Battledome experience, 2026-10-29

**80 seats are comfortable in the instrumented windows; 480 already has a
delivery tail, and 640 fails badly because admission searches join an already
busy socket/broadcast workload.** At 80, median/p99/max world CPU is
1.118/1.963/3.270 ms per tick, with frame-excess p99 0.981 ms and no deferrals.
The mean uses about 16.7% of one c7a core. At 480, world CPU p99 is 8.774 ms and
frame-excess p99 10.268 ms; at 640, 5,470 deferred ticks and 38.2 seconds of
frame excess make play clearly degraded. No maximum between 480 and 640 is
identified. Production code is unchanged.

**A4 boosts for only 3.0–4.7% of living time in the ascending windows.** It
really boosts and makes boost food, but this does not qualify constant-boost
human play. All eight completed windows pass generator, queue and CPU-accounting
checks. The final human hold was skipped on the maintaining agent's instruction.

[Selected results](selected-results.json), [table calculation](tables.py),
[cost](cost.json), [cleanup](cleanup.json), [verification](verification.json).

## Results

One evolving world, one window per row; no repeat-based uncertainty interval.
CPU is aligned simulation + socket task user/kernel CPU. Header excess is a
delivery diagnostic relative to the window's baseline, not input-to-effect
latency. Gap residual subtracts the protocol elapsed-header increment. Stalls
count gaps above the indicated residual threshold; raw gaps are separate below.
Alive counts are sampled every 100 ticks. Boost share uses actual living ticks.
Boost births count admitted physical boost pellets during those ticks.

| Bots | World CPU p50 / p99 / max ms | Deferred | Header excess p50 / p99 / max ms | Gap residual p99 ms | Residual stalls >100 / >250 ms | Alive mean / min | Boost % | Boost births |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| up-80 | 1.118 / 1.963 / 3.270 | 0 | 0.299 / 0.981 / 5.504 | 0.673 | 0 / 0 | 79.91 / 79 | 4.15 | 3803 |
| up-120 | 1.598 / 2.543 / 3.489 | 0 | 0.406 / 1.264 / 5.803 | 0.848 | 0 / 0 | 119.94 / 119 | 4.68 | 6867 |
| up-160 | 2.141 / 3.402 / 4.442 | 0 | 0.443 / 1.483 / 8.026 | 1.020 | 0 / 0 | 159.84 / 159 | 4.38 | 8613 |
| up-240 | 3.184 / 4.520 / 6.052 | 0 | 0.674 / 2.267 / 10.335 | 1.582 | 0 / 0 | 239.82 / 238 | 4.16 | 12011 |
| up-320 | 4.058 / 5.801 / 7.450 | 0 | 0.741 / 3.144 / 12.913 | 2.345 | 0 / 0 | 319.84 / 317 | 3.46 | 13790 |
| up-480 | 6.327 / 8.774 / 12.266 | 0 | 1.604 / 10.268 / 30.453 | 5.259 | 0 / 0 | 479.68 / 478 | 3.04 | 18152 |
| up-640 | 8.138 / 31.299 / 38.249 | 5470 | 22901.711 / 38201.065 / 38373.996 | 75.444 | 15574 / 0 | 633.42 / 610 | 4.52 | 17650 |
| down-80 | 1.179 / 2.503 / 3.352 | 0 | 0.354 / 1.495 / 4.958 | 1.016 | 0 / 0 | 79.98 / 79 | 2.44 | 2325 |

| Bots | Raw frame gap p50 / p99 / max ms | Raw gaps >100 / >250 ms (not automatically stalls) |
| --- | ---: | ---: |
| up-80 | 7.004 / 28.446 / 231.060 | 1215 / 0 |
| up-120 | 7.001 / 28.158 / 231.568 | 2780 / 0 |
| up-160 | 7.003 / 35.346 / 231.413 | 4488 / 0 |
| up-240 | 7.011 / 48.931 / 232.379 | 8446 / 0 |
| up-320 | 7.016 / 62.795 / 238.242 | 12647 / 0 |
| up-480 | 7.002 / 69.362 / 247.705 | 23094 / 0 |
| up-640 | 8.959 / 140.407 / 375.185 | 61422 / 3971 |
| down-80 | 7.000 / 8.016 / 11.671 | 0 / 0 |

| Bots | Added-cohort initial p50 / p95 / max ms (n) | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Deaths <3 s / <10 s / all completed |
| --- | ---: | ---: | ---: | ---: |
| up-80 | 3118.001 / 8144.701 / 8881.362 (80) | 6.222 / 9.158 / 12.066 (148) | 0 / 3.174 | 0 / 0 / 148 |
| up-120 | 3063.755 / 3130.735 / 3134.986 (40) | 5.970 / 9.657 / 9.999 (137) | 0 / 0.000 | 0 / 0 / 137 |
| up-160 | 3063.591 / 3116.688 / 3122.581 (40) | 6.415 / 10.220 / 12.355 (180) | 0 / 0.000 | 0 / 0 / 180 |
| up-240 | 3110.192 / 3312.415 / 3339.025 (80) | 8.394 / 11.677 / 19.784 (270) | 0 / 13.362 | 0 / 0 / 270 |
| up-320 | 3114.528 / 3314.255 / 3356.196 (80) | 10.124 / 13.153 / 16.455 (358) | 0 / 6.867 | 0 / 0 / 358 |
| up-480 | 3234.755 / 4159.281 / 4286.842 (160) | 12.960 / 16.744 / 32.550 (427) | 0 / 18.275 | 0 / 0 / 428 |
| up-640 | 3260.439 / 4325.850 / 4541.797 (160) | 1467.883 / 4975.213 / 5394.808 (441) | 0 / 5022.906 | 0 / 0 / 441 |
| down-80 | — / — / — (0) | 5.910 / 9.067 / 9.067 (14) | 0 / 0.000 | 0 / 0 / 14 |

| Bots | Slow 1% CPU mean ms | Admission % | Broadcast % | Socket % | Food epoch current/prior ticks | Searches / strict / fallback / refused (all ticks) |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| up-80 | 2.232 | 1.6 | 39.2 | 45.2 | 25 / 128 | 148 / 148 / 0 / 0 |
| up-120 | 2.700 | 0.0 | 36.2 | 51.5 | 9 / 128 | 142 / 142 / 0 / 0 |
| up-160 | 3.548 | 0.3 | 36.9 | 50.8 | 14 / 128 | 183 / 183 / 0 / 0 |
| up-240 | 4.834 | 1.0 | 39.9 | 45.5 | 42 / 127 | 273 / 273 / 0 / 0 |
| up-320 | 6.100 | 1.7 | 42.3 | 40.1 | 67 / 128 | 359 / 359 / 0 / 0 |
| up-480 | 9.279 | 4.3 | 43.4 | 34.1 | 71 / 128 | 430 / 430 / 0 / 0 |
| up-640 | 32.754 | 60.6 | 13.3 | 20.9 | 6 / 73 | 2079 / 428 / 23 / 1628 |
| down-80 | 2.727 | 0.0 | 53.8 | 35.3 | 10 / 128 | 14 / 14 / 0 / 0 |


Initial waits describe each newly added cohort, including its preceding settling
period. Thus the first 80 and later 40/80/160 additions have different burst
sizes. Respawn waits use complete worker reports inside each counted interval.
Zero pending in the final reports does not erase requests that waited earlier.
The worst sampled pending age at 640 is 5.023 s. Whole-run completed lifetimes:
4,041, with zero under three seconds and zero under ten seconds. These bots
usually travel toward the pit; this is not a guarantee against unsafe human
turns, boosting trajectories or visual spawn defects.

The nominal 90-second windows retain 12,700–12,800 complete ticks except the
overloaded 640 row, which completes only 7,300. Its 5,470 deferrals shift world
time by 38.29 s, consistent with its rising header excess. **The whole run has
7,805 deferred ticks, including 2,335 outside counted windows.** Transitions
and buffered logging flushes compete on the same vCPU; those outside deferrals
are not assigned entirely to gameplay or hidden by window selection. This is
not an uninterrupted-session smoothness qualification. No new deferrals occur
in the descending 80 window; resetting its excess baseline removes the prior
world-clock offset, not the historical deferral count.

All human-playing samples in the accepted final run are zero. No human window
is relabeled as bot-only. The earlier failed pairs' human effects are not part
of the accepted results.

## Ranked fix candidates and decisions

Ranks prioritize the observed experience failure at 640, then the lower-count
tail. Shares below are conditional CPU shares of the slowest 1%, not predicted
savings. Nested phases must not be added to their parents. Every exact candidate
still needs byte-identical ordered wire/state/RNG controls and matched timing
before adoption; this research implements none of them.

1. **Circular admission search — exact candidate first; rule changes separate.**
   At 640 it takes 19.860 ms of the 32.754 ms slow-tail mean (**60.6%**).
   Every one of those 73 ticks searches; 69 exhaust the search and four admit
   through fallback. Across the full window: 2,079 searches, 428 strict accepts,
   23 fallback accepts, 1,628 exhausted (**78.3%**). Admission averages 3.782 ms
   per tick over the entire 640 window, versus 0.026 ms at 480. Respawn p95 rises
   from 16.7 ms to 4.975 s. Source ownership is
   [spawn_pose_waited](../../../src/client/simulation.rs) and
   [Safety](../../../src/spawn/safety.rs). Profile the failed 4,096-pose path,
   then conservatively index cached obstacles to avoid scanning irrelevant
   edges; retain existing narrow-phase arithmetic, candidate/RNG order,
   strict/fallback choices and refusal outcomes. Existing per-edge bounds are
   already present; merely adding them again is not a fix. Earlier head/body
   construction rejection is another exact candidate only where it preserves
   validation/error behavior. Reducing attempts, retrying less often, changing
   the 14-ms world-time retry, clearances, fallback wait, density or growth hold
   changes admission/feel and requires approval on a local play server.
2. **Socket delivery and recipient work — exact candidates, with a kernel limit.**
   Sockets use **34.1–51.5%** of the ascending 80–480 slow-tail CPU and 20.9% at
   640. At 480 the socket task averages 0.422 ms user + 1.954 ms kernel per tick;
   removing userspace bookkeeping cannot remove its entire cost. The 480
   broadcast is 43.4% of the slow tail, including 38.8% recipient work; the
   minimap is nested in that recipient value. Investigate exact queue/framing,
   payload-copy and per-recipient update reuse in
   [server delivery](../../../src/client/server.rs) and
   [broadcast](../../../src/client/simulation/broadcast.rs), retaining message
   boundaries, bytes, ordering and due-time rules. No isolated syscall/write
   split was collected here, so a specific batching gain is unmeasured. Delaying
   frames, lowering report cadence or changing delivery is a feel change.
   [#260](../2026-10-29-food-diffs/REPORT.md) already reuses food diffs; it is
   outside this runtime pin and must be measured before proposing duplicate work.
3. **Pit minimap rasterization — plausible exact cache.** Its nominal 2.504-s
   clock accounts for **8.7–16.3%** of ascending 80–480 slow-tail CPU. At 80 all
   36 minimap opportunities land in the slowest 128 ticks; map-due world CPU
   median is 2.746 ms versus 1.118 ms overall. The
   [rasterizer](../../../src/client/minimap.rs) recomputes pit mask/outline
   geometry across 256² cells. Cache that mask keyed by the exact effective
   projection inputs, preserving the original comparisons and per-connection
   map bases. Do not duplicate the already-shared packet/delta cache. Changing
   resolution or period is a feel/presentation change requiring local approval.
4. **Shrinking-pit marker/listing refresh — exact reuse candidate for population
   drops.** In descending 80, 90/128 (**70.3%**) slow ticks coincide with marker
   changes. Shared food-sector preparation takes **28.6%** of tail CPU, including
   **12.0%** in marker update; broadcast totals 53.8%. The arena still shrinks
   from radius 15,282 to 13,182, far larger than the initial 80 arena. Investigate
   exact reuse of unaffected marker/sector overlays in
   [markers](../../../src/client/simulation/markers.rs) and
   [sector synchronization](../../../src/client/simulation/food.rs). Preserve
   quantization, ring overlap, ordering and 0.5-s fade lifetime. Slower marker
   refresh or less overlap changes presentation and needs local play approval.
5. **504-ms food maintenance — secondary, not the universal Battledome limiter.**
   Food epochs coincide with 25/128 slow ticks at 80, 71/128 at 480, but only
   5/73 at 640; including the following tick gives 6/73 there. This differs from
   the official-density study's 88–98% concentration. At 480 shared food-sector
   work is only 2.4% of tail CPU, whereas recipient work is 38.8%, so this coarse
   profile cannot attribute the whole food-epoch effect to shared maintenance.
   Collision queries are about 4–5.5% at 80–480; kill-board construction is at
   most 0.13%. Do not prioritize kill-board or ambient-refill changes over the
   measured admission/socket/recipient paths. Food supply, boost cost or floor
   changes are rules, with no performance adoption justified by this study.

**Burst joins have a separate policy wait.** Initial 80 waits are
3.118/8.145/8.881 s p50/p95/max; later cohort medians stay near 3.1–3.3 s even
while steady respawns are milliseconds. The retained startup census holds at
ten living seats from ticks 400–700, then expands and reaches 80 by tick 1700.
The [host admission controller](../../../src/client/server.rs) explicitly
protects a newborn's three-second departure before growth. This supports a
policy/geometry explanation for burst waits rather than CPU saturation at 80,
but the startup hooks do not time the exact split between growth holds and
failed safety searches. Reducing that hold or widening the band is a proposal
for an approved local playtest, not an exact optimization. No geometry change
is justified merely by zero observed short lives.

## World history and accounting

| Bots | Mean length first → last | p90 length first → last | Food first → last | Mean boost food alive |
| --- | ---: | ---: | ---: | ---: |
| up-80 | 76.95 → 76.66 | 179 → 174 | 1658 → 2268 | 417.3 |
| up-120 | 59.90 → 66.78 | 143 → 147 | 3262 → 3164 | 626.1 |
| up-160 | 60.82 → 69.94 | 144 → 150 | 3612 → 3981 | 856.2 |
| up-240 | 60.61 → 67.31 | 148 → 151 | 4518 → 5578 | 1381.8 |
| up-320 | 63.32 → 66.10 | 160 → 155 | 5315 → 8028 | 1475.2 |
| up-480 | 57.30 → 63.62 | 135 → 146 | 8297 → 10880 | 1798.2 |
| up-640 | 57.14 → 58.70 | 148 → 151 | 11297 → 15034 | 4708.6 |
| down-80 | 89.20 → 103.25 | 204 → 210 | 13427 → 8751 | 1620.3 |

All canonical clock/task means reconcile within 0.0021 ms/tick. The observer's
separate process CPU is 0.048–0.054 ms/tick below overload and 0.097 ms at 640;
it competes on the same core and is not added to world task CPU. CPU-clock and
nested phase overhead stays in the measured host tasks. Generator worker p99
CPU peaks at 28.7%, worst one-second input-lateness p99 at 39.02 ms, both in
the overloaded row and within the frozen 80%/42-ms gates. No sampled write
backpressure, buffer-full or rejection counter increases; queues pass their
bounded-growth gates. Task affinities, byte identity of the copied executable,
frame decoding/progress and private boundary process inventories are verified.
These controls qualify the workload and arithmetic; they do not remove
instrumentation overhead or make this an uninstrumented production benchmark.

## Cost, teardown and verification

Estimated whole-task cost is **$1.72**, including all four
pairs and failed attempts, below $3. This is an estimate, not an invoice.
Launch-quote compute is $0.295; compute at bid caps is
$0.492. Storage/public-IPv4 allowance is
$0.021; the deliberately conservative
control/artifact-transfer and miscellaneous allowance is $1.401.
Even using bid caps the resulting total is $1.91. The
transfer bound overcounts every poll using the terminal payload plus 1 MiB,
adds downloaded archives and overhead, and ignores free credits. See
[cost.json](cost.json) for rates, upper lifetimes and arithmetic. No extra
instance pair or paid persistent service remains.

All four pairs had verified in-guest termination timers and delete-on-termination
root disks. First three pairs used 130-minute timers; the authorized final pair
used a 60-minute plan plus 30-minute margin. At **2026-10-29T04:20:11.572325+00:00**,
all **17 enabled regions** had zero nonterminated instances, open/active Spot
requests, volumes and Elastic IPs. All eight task instances, four created keys
and four created groups are gone, task volumes absent and endpoint removed.
Seventeen unavailable regions remain labeled unavailable. The protected
pre-existing key/group were never targeted. The existing AWS session stayed
valid; no refresh or workaround was used. [Cleanup receipt](cleanup.json).

Checks: native Rust 1.99 portable release build and eight real network windows;
independent offline canonical re-reduction and CPU reconciliation; synthetic
aligned-sum/cadence, generator rejection, normal-gap, report-boundary,
unterminated-tail, delayed-stream and archive-ready/failure/cleanup controls;
documentation links, whitespace and private-identifier scan. Full Rust/tool
groups were not run locally for this research-only change. No independent
review or human-browser acceptance is claimed. Raw archive SHA-256:
`d89344ba863f513abdb343b0c04dc92b7f42227ea4907742444f694f4855d7bc`.

This study measures the shipped Battledome preset with real received-frame A4
WebSocket bots on a second machine. It changes no production code or game rule.
The [plan](PLAN.md), [attempts](ATTEMPTS.md) and
[reproduction instructions](REPRODUCE.md) retain corrections and failed work.
The [first excluded attempt](failed-attempt-results.json) retains its measured
logging artifact and CPU-counter reconciliation separately from game results.

## Method and scope

Runtime `165a5bb764aba6cbf39f42503e090f9b5e3c4d22`, Rust 1.99.0, locked release
dependencies and portable `-C target-cpu=x86-64`. The c7a.medium reports
AMD EPYC 9R14 and has one vCPU; all simulation, socket and CLI tasks share it. Six real client workers
use separate cores on an eight-vCPU generator in the same Frankfurt AZ, with
control on core 7. Private TCP WebSockets, no TLS or artificial latency. The
separate shared SSH guest and local heavy builds are not used.

The host has 641 reserved socket slots, zero in-process bots, seed 7 and
`simulation: {"world":{"preset":"battledome"}}`. Delivery is its shipped
responsive default. Reserved disconnected slots do not enlarge the arena, but
their bookkeeping remains in measured CPU. Pit, floor, free boost, food,
markers, minimap, kill board and circular admission stay at their defaults.
Every bot decodes all frames, opts into physical food/live rules, materializes
its received view and calls unchanged shared A4 every 42 ms with normal pings.
No authoritative world observations drive decisions. No thin mode is used.

Grow naturally from tick zero for five minutes at 80 bots. Then ascend the
frozen seat ladder, settling 60 s and counting 90 s at each count, until a
predeclared degradation/validity boundary or 640 bots. Descend once to 80 with
30 s settling and a 90 s confirmation. Report actual populations, food and
lengths; this is not the official world's mature-class screen. New upper-count
joiners start at the floor and the world evolves throughout.

CPU is simulation plus socket task user/kernel work at aligned consecutive
tick edges. Sum components before quantiles; independent p99s cannot be added.
Keep complete 100-tick interior blocks and reconcile their mean against task
counters within max(0.03 ms/tick, 2%). Phase clocks are read-only measurement
overlays on archived sources. Their cost stays included. Expanded records are
buffered and serialized outside counting; all deferrals remain in raw evidence.
Server and generator command inventories bracket completed primary windows.

## Receipt and spawn interpretation

Raw frame gaps include legitimate emission intervals: straight unchanged snakes
can wait roughly 200 ms for an anchor. They are not all network stalls. Record
receipt-gap minus elapsed-header increment as a signed jitter diagnostic, and
receipt time minus cumulative server time relative to the running minimum as
frame excess. Reset that minimum before each counted window. The latter
includes server slowdown, delivery and client scheduling; it is not absolute
one-way or input-to-effect latency. No input-to-effect matcher was added: the
earlier sampled enqueue/ingest instrumentation is a different harness and does
not cheaply identify received effects for these A4 actions.

Pool microsecond-floor receipt histograms from whole worker reports whose
preceding and ending timestamps lie inside the CPU interval. Client edges
therefore sit up to about one second inside it. Preserve the original online
end-timestamp view separately. Report raw gaps and
header-relative excess separately; count residual gaps above 100/250 ms as
delivery stalls. Generator CPU, input lateness, decode progress and socket
queues are checked independently, so failed generator rows cannot qualify host
capacity. Quantiles use index `ceil((n-1)*p)`, matching the linked harness.

Join waits begin at login enqueue and end at the first observed Playing frame.
Respawn waits exclude the 28 ms reconnect delay and that connection's TCP/upgrade
handshake. Worker polling and other connections' serialized handshakes can delay
the first observed frame and remain included. Completed quantiles exclude pending
requests, whose counts/ages are separate. The retained data include cumulative
initial completions; the table selects the newly added logical cohort. Handshake serialization
differs from an exactly simultaneous 80-player wave. Early deaths are observed
life durations, without enough geometry to identify an unfair admission.
The socket driver reconnects after death, as the portable network backend does;
new connections clear the reused seat's rolling kills. This does not stress a
full 15-minute same-connection kill history or the stock browser's death fade.

## Evidence limits

This is an exploratory single evolving-world ladder, not a paired optimization,
held-out mechanics fit, long-uptime qualification or human-browser frame-rate
test. The physical-food bot opt-in omits decorative pellets from the wire;
shared marker work remains, and human stock-client windows add their decoration
delivery. No claim that these bots constantly boost like human players follows
from calling A4; actual boosting and boost-food births are measured separately.
At the permanent floor boosting is free and produces no pellets.

The older [c7a nonboosting study](../2026-10-27-epyc-index-recheck/REPORT.md)
and [Zen 5 study](../2026-10-27-native-zen5-bench/REPORT.md) used 100 ms scripted
steering and different runtime/workload histories. They are historical context,
not matched controls. The competing Broadwell server has no captured paired
workload here; its reported core percentage cannot establish a hardware or
implementation speed ratio. No exact optimization is validated by phase shares
alone. Feel/admission/rule proposals require the maintaining agent's approval
on a local play server before adoption.

Main advanced during collection with
[#260's food-diff reuse](../2026-10-29-food-diffs/REPORT.md). This study's runtime
remains the requested pre-#260 pin. Its results do not qualify post-#260 Battledome
performance, and already-landed diff reuse is not proposed here as a new fix.

Full raw logs, source copies, executables, cloud metadata and process commands
are local-only and not publicly downloadable. Selected data retain cited values
and the code that recomputes the tables; archive digests establish identity,
not scientific or product acceptance.
