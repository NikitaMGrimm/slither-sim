# Internal bots on the normal map, 2026-10-30

The measured 10k A4 world is far from a 7-ms tick: simulation-only replay
achieves **7.17 ticks/s**, serial A4 **3.59**, and three-worker A4 **5.14**,
against 142.86 required. Its ecology is **unsettled** after the frozen warmup
guard. The small anchor achieves 166.61/237.77 ticks/s with serial/parallel A4;
2.5k already misses real time with either decision mode. The center-steering
12k comparison achieves 91.06 ticks/s and misses too.

The main 10k cliff is pickup's fixed dense-grid cap, not its food-relative
guard. A separate, requested [#331](https://github.com/NikitaMGrimm/slither-sim/pull/331)
replay on the same snapshot reduces world CPU p50 from **140.19 to 17.54 ms**
with identical final state. It does not establish 10k capacity: pickup,
collisions and observation still exceed the available budget.

This is measurement only. No runtime optimization, policy change or capacity
commitment is adopted. The anonymous comparison is a **12k-snake single-core
reference world (64,800 across, center-steering bots)**. Its population and
geometry are reported inputs; its hardware and older tick rate are unknown.
We do not compare our measured speed directly with that unverified report.

[Plan](PLAN.md), [botstorm plan](BOTSTORM_PLAN.md),
[maintainer amendment](AMENDMENT.md), [attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md), [selected A4 data](selected-results.json),
[selected botstorm data](botstorm-selected.json),
[sampled diagnostic](diagnostic-selected.json), and [table generator](tables.py).

## What was run

Runtime `d1bab317d2d1471598971d2c7cd20210e83677af`, portable repository
release profile (thin LTO, one codegen unit, debug=0, no native Rust flags),
Rust 1.99. A **4-vCPU Zen 5 EPYC KVM guest**: the world uses one core; the
parallel variant uses the other three for observations and policy. No sockets,
wire encoding, frame decoding, ForagerBot adapter, minimap or leaderboard.
Those phases are structurally absent, not optimized to zero. This is not a
socket-host or wall-paced sustained-capacity qualification.

A4 receives core world observations and calls `bot::Forager::decide` directly,
with `Timing::default()` (zero lag) and no intents. Every six ticks, bot i
updates where `(tick+i)%6==0`. Actions persist between decisions. Serial work
runs on the simulation core. The parallel path uses the existing selected-batch
observation implementation and a three-worker pool for A4, with a barrier before
the next step. There is no stale-state overlap or speculative decision work.

Normal A4 worlds use unchanged `Config::official(N)`, including adaptive radius
`670*sqrt(N)` and official food/prey density. Core construction applies no stock
wire radius cap. The 10k radius is 67,000. Botstorm uses fixed 32,400 at 12k,
center heading from its own head only, the same staggered cadence, and no boost,
observations or A4 calls. Its optional populations keep the same density by
scaling radius by `sqrt(N/12000)`; this is about 5.13 times official snake density.
The center-policy CPU is separated because the reported bots were external.

All rows have normal physics, food, prey, deaths and respawns. Admission uses
public core methods: 64 sampled poses, preferred clearance 600, first best
positive-margin fallback, ascending slot order and a 143-tick refusal retry.
The public `spawn_margin` computes a complete margin, while the socket host's
private bounded helper may return early. Its extra cost is included and labelled
admission here; do not call these socket-host admission timings.

## Census and throughput

Warmup records a census every 1,000 ticks. A4's four-bin screen checks mean
length/food within 5%, p90 length within 10% and at least 99% alive over four
complete 30-virtual-second bins, after at least 300 virtual seconds. This finite
screen does not prove stationary ecology. Botstorm checks population, mean
length and positive death/admission rates with its separately frozen tolerances.
Full trajectories and censored attempts are retained.

The amendment keeps a bounded warmup: if its guard fires, the final checkpoint
is measured and explicitly marked unsettled. No extra warmup attempt is made
to improve performance. The 10k guard stopped at 34,000 ticks (238 virtual seconds), after 7,378.16 wall seconds including its census-boundary overshoot and checkpoint work. All 10,000 slots were alive; mean length was 51.3174, p90 73 and p99 87, with 515,400 food items. It failed the original screen, and no warmup extension ran.

<!-- census:start -->
| Bots | Radius | Alive | Mean / p90 / p99 length | Food / prey | Warm virtual s |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 21,187 | 1,000 | 95.37 / 142 / 183 | 51,962 / 24 | 609.0 |
| 2,500 | 33,500 | 2,500 | 94.40 / 144 / 189 | 127,219 / 14 | 609.0 |
| 5,000 | 47,376 | 5,000 | 96.76 / 149 / 192 | 254,104 / 16 | 637.0 |
| 10,000 | 67,000 | 10,000 | 51.32 / 73 / 87 | 515,400 / 18 | 238.0 |
<!-- census:end -->

<!-- throughput:start -->
| Bots | Ecology screen | Simulation-only ticks/s | Serial ticks/s | Three-worker ticks/s |
| ---: | --- | ---: | ---: | ---: |
| 1,000 | pass | 1117.8 (1082.7–1156.8) | 166.6 (163.0–170.3) | 237.8 (222.3–252.1) |
| 2,500 | pass | 299.4 (266.6–323.6) | 48.4 (47.1–49.2) | 76.4 (69.6–84.6) |
| 5,000 | pass | 110.4 (105.8–114.0) | 17.1 (16.9–17.4) | 30.2 (29.4–30.7) |
| 10,000 | unsettled / guarded | 7.2 (6.6–7.2) | 3.6 (3.2–3.6) | 5.1 (4.7–5.2) |
<!-- throughput:end -->

<!-- costs:start -->
| Bots | Mode | World CPU p50 / p99 / max ms/tick | Admission mean ms/tick | Observe CPU µs/bot | A4 loop CPU µs/bot | RSS / peak RSS MiB |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1,000 | simulation | 0.876 / 1.050 / 5.796 | 0.002 | — | — | 124.4 / 137.2 |
| 1,000 | serial | 1.245 / 1.866 / 5.443 | 0.002 | 16.27 | 11.96 | 124.4 / 137.2 |
| 1,000 | parallel | 1.735 / 2.394 / 7.892 | 0.002 | 19.21 | 12.44 | 172.9 / 172.9 |
| 2,500 | simulation | 3.211 / 6.068 / 17.665 | 0.010 | — | — | 301.4 / 333.5 |
| 2,500 | serial | 4.717 / 6.573 / 16.437 | 0.012 | 24.88 | 13.35 | 301.4 / 333.5 |
| 2,500 | parallel | 5.441 / 7.521 / 20.683 | 0.011 | 26.90 | 13.32 | 421.2 / 421.2 |
| 5,000 | simulation | 8.773 / 11.627 / 42.375 | 0.093 | — | — | 607.6 / 673.0 |
| 5,000 | serial | 10.972 / 13.540 / 37.884 | 0.117 | 43.47 | 13.19 | 607.6 / 672.8 |
| 5,000 | parallel | 12.419 / 14.773 / 40.381 | 0.104 | 42.27 | 13.43 | 845.6 / 845.6 |
| 10,000 | simulation | 140.190 / 155.468 / 184.471 | 0.056 | — | — | 879.6 / 990.6 |
| 10,000 | serial | 139.143 / 157.734 / 174.364 | 0.073 | 76.09 | 10.43 | 879.5 / 990.7 |
| 10,000 | parallel | 140.082 / 155.957 / 193.648 | 0.059 | 71.91 | 10.48 | 1236.6 / 1236.6 |
<!-- costs:end -->

Ticks/s entries are medians and min-max ranges of five identical 512-tick
continuations (3.584 virtual seconds each), not independent-world confidence
intervals. Serial/parallel pairs alternate; simulation replay rotates around
pairs. One complete triplet is discarded before counting. All modes reload
the same checkpoint, and both decision modes prime six retained observation
buffers without changing world state or actions. Replay consumes the complete
held-action tape with the same lifecycle and physics, without observations/A4.
All serial/parallel action tapes and counted final states must match.

World CPU is the same-thread clock around step; admission is separate. Per-bot
observation CPU includes shared-index construction amortized over actual
observations. Policy-loop CPU sums decision-worker clocks and excludes enclosing
pool scheduling; that envelope is retained separately in selected data. Whole
window throughput includes loop, scheduling and replay decoding overhead. It
excludes checkpoint I/O, priming and final-state serialization.

Do not add isolated replay CPU to bot costs to predict combined throughput:
world CPU differs by execution mode despite identical output. Cache/scheduling
or frequency mechanisms behind that difference were not isolated. The measured
whole-mode result is the relevant combined cost.

Restoring a checkpoint rebuilds derived world-step caches. Tables include that
first tick (1/512 per window); selected data also retain its distribution and
the distribution excluding it. Maxima are not automatically steady live stalls.
Memory is process RSS/high-water RSS, sampled before final serialization; it can
retain checkpoint parser and replay-tape allocations. It is not an exact allocator
breakdown for world versus observation buffers.

## Phase costs and scaling

<!-- phases:start -->
| Bots | Profile mode | Motion CPU bounds ms/tick | Collision | Death | Food pickup | Prey | Ambient | Profile / ordinary CPU ratio range |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | serial | 0.167–0.168 | 0.635–0.636 | 0.000–0.000 | 0.418–0.419 | 0.028–0.028 | 0.013–0.014 | 0.945–1.030 |
| 1,000 | parallel | 0.217–0.217 | 0.578–0.578 | 0.000–0.000 | 0.727–0.727 | 0.026–0.026 | 0.011–0.012 | 0.850–0.979 |
| 2,500 | serial | 0.550–0.553 | 2.168–2.172 | 0.001–0.001 | 1.753–1.757 | 0.068–0.072 | 0.036–0.039 | 0.926–1.008 |
| 2,500 | parallel | 0.507–0.509 | 1.942–1.944 | 0.001–0.001 | 2.158–2.160 | 0.064–0.066 | 0.034–0.036 | 0.749–0.977 |
| 5,000 | serial | 1.628–1.632 | 5.058–5.062 | 0.004–0.005 | 4.006–4.010 | 0.157–0.161 | 0.076–0.080 | 0.989–1.003 |
| 5,000 | parallel | 1.336–1.342 | 4.818–4.823 | 0.004–0.004 | 5.159–5.164 | 0.147–0.152 | 0.073–0.077 | 0.909–0.976 |
| 10,000 | serial | 3.414–3.474 | 6.757–6.817 | 0.000–0.002 | 159.338–159.398 | 0.295–0.351 | 0.073–0.120 | 1.124–1.236 |
| 10,000 | parallel | 3.156–3.282 | 6.522–6.649 | 0.000–0.002 | 161.480–161.608 | 0.290–0.378 | 0.060–0.120 | 1.147–1.239 |
<!-- phases:end -->

At 5k, the serial diagnostic spends about 5.06 ms in collision work, 4.01 ms
in pickup and 1.63 ms in motion. At 10k the counted world median jumps to
139–140 ms. The operation-counting StepProfile variant costs **12–24% more**
than ordinary 10k world work: its 159–162-ms pickup bounds describe that
instrumented path, not a component to add inside the 139-ms ordinary tick.
Use the separate sampled normal-step attribution below for ordinary shares.

StepProfile phases are wall brackets. The report retains conservative individual
CPU bounds using per-tick wall minus same-thread CPU; these bounds are not
additive. Unbracketed work and profiled/ordinary ratios remain explicit. A ratio
below one is variation/codegen evidence, not negative instrumentation overhead.
`segmentTests` sums candidate body lengths before the narrow collision predicate;
it does not count executed distance tests, which can return early.

The 1k-to-5k replay world median grows 10.0-fold for five times the population;
10k then grows another 16.0-fold for twice the population. This last comparison
also has a younger, unsettled ecology, so it is not a pure population exponent.
The exact per-tick guard audit identifies the discontinuity. Candidate food
pairs rise from 4.13M to 18.74B per 512 profiled ticks between 5k and 10k;
#331 brings the identical 10k continuation back to 7.54M.

Observation cost per decision grows from 16.27 to 76.09 µs; A4 itself stays
about 10–13 µs. At the six-tick cadence, 10k serial observations alone use
about **126.8 ms CPU per tick**, A4 another **17.4 ms**; the three-worker
measurements give 119.8 and 17.5 ms of aggregate CPU respectively. Even perfect
balance across three workers would require about 45.8 ms/tick for those two
budgets, before pool overhead. More cores alone would need far more than three.
The sampled 10k observation bucket attributes 44.0% to the snake candidate
scan, 5.9% to food-grid construction and 2.6% to food-order restoration, with
41.4% in copying/other/unresolved source. These are inclusive diagnostic leads,
not removable percentages; measured per-decision clocks own the total budget.

Collision candidate-body-length counts grow 5.13-fold from 1k to 5k, while
serial collision CPU grows about eightfold. That establishes superlinear CPU
cost in these rows, not a demonstrated quadratic narrow-phase loop or a proven
cache mechanism. Admission is small in the A4 rows (mean 0.002–0.117 ms/tick)
and is not the 10k cliff observed here.

## Center-steering comparison

<!-- storm-census:start -->
| Bots | Ecology screen | Fixed radius | Alive | Mean / p90 / p99 length | Deaths / admissions per virtual s |
| ---: | --- | ---: | ---: | ---: | ---: |
| 12,000 | stabilized | 32,400.00 | 12,000 | 6.15 / 11 / 54 | 64.73 / 64.45 |
| 6,000 | stabilized | 22,910.26 | 6,000 | 4.72 / 8 / 25 | 46.87 / 46.87 |
| 9,000 | stabilized | 28,059.22 | 9,000 | 5.51 / 10 / 34 | 70.87 / 70.87 |
<!-- storm-census:end -->

<!-- storm-cpu:start -->
| Bots | Physics CPU p50 / p99 / max ms | Admission mean / max ms | Physics+admission CPU p50 / p99 / max ms | CPU ticks >7 ms | Physics+admission wall p99 / max ms | Wall ticks >7 ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12,000 | 9.647 / 12.433 / 23.328 | 1.213 / 9.079 | 10.718 / 18.144 / 23.343 | 100.00% | 18.148 / 23.346 | 100.00% |
| 6,000 | 3.296 / 4.100 / 11.203 | 0.415 / 4.220 | 3.381 / 7.032 / 11.210 | 1.05% | 7.032 / 11.211 | 1.05% |
| 9,000 | 6.362 / 9.139 / 17.911 | 1.068 / 8.087 | 7.394 / 13.768 / 17.926 | 57.54% | 13.768 / 17.927 | 57.70% |
<!-- storm-cpu:end -->

<!-- storm-phases:start -->
| Bots | Motion CPU bounds ms/tick | Collisions | Death processing | Food pickup | Prey | Ambient | Center steering mean ms/tick |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 12,000 | 1.966–1.971 | 4.439–4.444 | 0.022–0.024 | 3.692–3.698 | 0.399–0.404 | 0.029–0.034 | 0.067 |
| 6,000 | 0.653–0.655 | 1.681–1.683 | 0.005–0.005 | 0.673–0.675 | 0.137–0.138 | 0.014–0.016 | 0.031 |
| 9,000 | 1.241–1.244 | 3.019–3.022 | 0.017–0.018 | 2.240–2.243 | 0.279–0.282 | 0.022–0.025 | 0.048 |
<!-- storm-phases:end -->

Whole-loop ticks/s medians (five-window ranges): 12k **91.06 (84.29–98.53)**,
6k **262.98 (254.20–273.89)**, 9k **127.57 (119.72–141.35)**. The 12k miss
triggered both optional rows. Only 6k has mean headroom, and 1.05% of its
world+admission ticks still exceed 7 ms. No wall-paced capacity test ran.

The 12k warmup saved at 87k ticks / 609 virtual seconds. Its four accepted
bins have roughly 71–88 deaths/admissions per virtual second; the later short
counted continuation has about 65/s. The finite screen is not proof of long-run
stationarity. Length p50/max at 6k, 9k and 12k are 3/55, 4/77 and 4/125;
these are much shorter than A4. Maximum final/peak RSS is respectively
143.6/162.8, 216.8/245.8 and 295.0/334.4 MiB. Steering itself costs only
0.031/0.048/0.067 ms per tick; physics and lifecycle own the result.

## Slow ticks and pickup-grid limits

<!-- slow-attribution:start -->
| Workload | Sample group | Samples | Movement | Collisions | Food pickup/cache | Grid build | Death | Admission | Prey | Ambient | Unresolved |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 10000 | all world ticks | 39350 | 2.1% | 4.3% | 83.8% | 0.2% | 0.0% | 0.0% | 0.2% | 0.1% | 9.2% |
| 10000 | slowest 1% excluding first | 494 | 2.0% | 4.3% | 82.0% | 0.0% | 0.0% | 0.6% | 0.2% | 0.6% | 10.3% |
| botstorm-12000 | all world ticks | 3198 | 18.3% | 35.9% | 28.1% | 2.8% | 0.2% | 10.6% | 2.8% | 0.3% | 1.0% |
| botstorm-12000 | slowest 1% excluding first | 57 | 14.0% | 22.8% | 19.3% | 1.8% | 0.0% | 38.6% | 1.8% | 0.0% | 1.8% |
<!-- slow-attribution:end -->

<!-- diagnostic-overhead:start -->
| Workload | Diagnostic / primary world CPU range | Sampled / ordinary CPU | Phase-profile / ordinary world CPU | Lost samples | Unparsed records |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1000 | 1.011–1.101 | 1.093 | 1.053 | 0 | 0 |
| 10000 | 0.972–1.070 | 1.041 | 1.148 | 0 | 0 |
| botstorm-12000 | 0.993–1.176 | 1.063 | 0.921 | 0 | 0 |
<!-- diagnostic-overhead:end -->

<!-- pickup-guards:start -->
| Workload | Ticks | Box cells min–max | Food at pickup min–max | Cover area min–max | Declines a / b / c ticks | Dense / hashed / linear ticks | Sampled pickup share of slow world CPU | Pickup share of profiled world CPU on matched slow ticks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 10000 | 512 | 1,094,116–1,096,209 | 514,717–515,485 | 28,708–29,339 | 512 / 0 / 0 | 0 / 512 / 0 | 82% | 93.2–93.2% |
| botstorm-12000 | 512 | 247,504–250,500 | 131,496–133,726 | 29,485–30,004 | 0 / 0 / 0 | 512 / 0 / 0 | 34% | 34.8–34.8% |
<!-- pickup-guards:end -->

Main 10k's slowest 1% has 494 user-CPU samples: pickup/cache owns 82.0%,
collision 4.3%, motion 2.0%, and 10.3% remains unresolved. Including all world
and admission ticks gives 83.8% pickup plus 0.2% grid construction. In botstorm,
admission leads the slowest ticks at 38.6%, then collision at 22.8%, pickup at
19.3% and movement at 14.0%. That slow group has only 57 samples, so its shares
are coarse. All-tick botstorm instead leads with collision at 35.9% and pickup
at 28.1%, plus 2.8% grid construction. Do not substitute its tail composition
for its ordinary budget.

After the callsite correction, samples attributed to hash lookups remain in
pickup/cache; they are not incorrectly counted as grid construction. Earlier
classification output and its exact source remain in the private archive.

The separate diagnostic uses portable codegen with debug=2 and study-only
monotonic world/admission timestamps, preserving counted binaries. Perf samples
user CPU at 499 Hz with 4,096-byte DWARF stacks on the same clock. State equality,
diagnostic/primary ratios and sampled/ordinary ratios are retained. It is a
separate diagnostic, not a replacement primary measurement.

Samples are matched to world/admission brackets, excluding loading, observations
and A4. The six slowest ticks are ranked by sampled-run world+admission CPU;
results are retained both including and excluding the first restored tick.
Source-context labels split movement/pre-collision setup, collisions, death,
food pickup/cache, pickup-grid construction, prey and ambient maintenance.
Unresolved stacks, unparsed records, sample counts and unknown lost-sample
counts are explicit; sampled shares are approximate user-CPU attribution.

The read-only checkpoint census remains a separate geometry diagnostic. The diagnostic instead audits every measured tick exactly: public pickup and
food-birth events reconstruct food at pickup entry; post-step head, heading,
speed, integer segments and alive fields still equal the pickup-circle fields.
The fold runs after the world clock. Independent pre/post food conservation
checks and equality across ordinary, sampled and profiled guard traces pass.

At radius 67,000, every box is 1,094,116–1,096,209 cells, exceeding
**1,048,576**. Food at pickup is 514,717–515,485, whose fourfold guard is
2,058,868–2,061,940; it never declines. Summed cover area is only
28,708–29,339. Thus **(a) alone declines on all 512 ticks**, selecting hashed
`indexed` pickup rather than dense pickup. On slow world ticks, pickup plus
grid is about **82% sampled user CPU** (admission excluded); the separately
counter-profiled matched ticks give **93.2%**, with their overhead disclosed.

At radius 32,400, botstorm's box is 247,504–250,500 cells; food is
131,496–133,726 and cover area 29,485–30,004. None of (a), (b), (c) declines
and **dense pickup runs all 512 ticks**. Its sampled slow-world pickup share
is 34% after excluding admission, versus 34.8% in matched phase diagnostics.
The guard is actually `max(food.len()*4,4096)`; the minimum is immaterial here.

Core food pickup runs every tick. There is no 504-ms wire food-update cadence in
this workload. Ambient credit is advanced each tick; at these density rates its
attempt interval is shorter than 7 ms. The per-six-tick phase summaries and
slow-tick work counts are retained rather than assigning a packet cadence to
core maintenance.

## Separately labelled #331 snapshot replay

[Supplement plan](POSTFIX_PLAN.md), [selected results](postfix-selected.json).
Pinned external head `a6511aadf029adfc1f34b9cdb360a326e2ba5204`. Its runtime
source change is only the dense-grid cap 2²⁰ → 2²². Selected simulator dependency
versions, toolchain and portable profile match, despite its client workspace
import. Main rows remain the baseline. This is a chronological supplement,
**not alternating paired evidence**: one discarded candidate window, five
counted 512-tick replays, and separate matched main/candidate phase diagnostics.
No observation or A4 decision work is included in this added row.

<!-- postfix-costs:start -->
| Workload | Arm | World CPU p50 / p99 / max ms/tick | Admission mean ms/tick | Whole-loop ticks/s median (range) |
| --- | --- | ---: | ---: | ---: |
| 10000 | main | 140.190 / 155.468 / 184.471 | 0.056 | 7.17 (6.64–7.19) |
| 10000 | #331 | 17.545 / 19.129 / 110.274 | 0.056 | 56.54 (56.35–57.15) |
<!-- postfix-costs:end -->

<!-- postfix-phases:start -->
| Workload | Arm | Movement CPU bounds ms/tick | Collision | Death | Food pickup | Prey | Ambient | Profile / ordinary CPU range |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 10000 | main | 2.654–2.738 | 5.484–5.567 | 0.000–0.001 | 155.751–155.835 | 0.274–0.354 | 0.048–0.114 | 1.094–1.186 |
| 10000 | #331 | 2.650–2.664 | 5.996–6.009 | 0.001–0.002 | 8.860–8.874 | 0.355–0.368 | 0.161–0.173 | 1.031–1.045 |
<!-- postfix-phases:end -->

World CPU p50 is 87.5% lower, but remains 2.5 times the 7-ms budget.
The 110.27-ms candidate maximum includes restored-cache construction; selected
first-tick/excluding-first distributions remain available. Every final world
state matches main's reference
`bf2a4b85b86fc12757961ac004cd86060b805502bb7e60758e69c6ed4899c1a1`.
Botstorm's maximum audited box is 250,500 cells, so its conditional supplement
was correctly skipped. The next bottlenecks are pickup and collision work,
not a remaining cap refusal.

## What to attack first

For the **main-revision 10k world**:

1. **Exact algorithmic speedup — dense pickup eligibility and hashed fallback.**
   Pickup/cache is 82.0% of slow world+admission samples (84.0% including grid
   over all ticks). #331's separate replay demonstrates the dominant cap effect;
   further pickup candidates need their own exactness/performance evidence.
2. **Exact algorithmic speedup — collision candidate/narrow-phase work.**
   4.3% of main's slow samples, but about **33.0%** of the post-fix diagnostic
   world CPU, 6.00 ms/tick. It becomes a primary remaining one-core target.
3. **Exact algorithmic speedup — motion and pre-collision setup.**
   2.0% of main's slow samples, about **14.6%** after the cap change,
   2.65 ms/tick. Prey, ambient and death handling are much smaller; admission
   averages 0.056 ms/tick in the world-only 10k replay.

**After #331, pickup still ranks first:** about **48.7%** of its phase-profiled
18.20-ms world CPU, or 8.86–8.87 ms/tick, then collision and motion as above.
Those post-fix percentages use separately instrumented mean bounds, not the
main perf sample denominator. Phase bounds are not exactly additive.

Outside world work, **exact observation-index/candidate improvements** are
necessary: 76.09 µs per decision is 126.8 ms aggregate CPU/tick at 10k.
**Needs more cores:** parallelize only after measuring the reduced aggregate
budget; the existing three decision workers do not approach 7 ms. **Changes
behavior:** fewer decisions, reduced observation content/range or simpler
policies could cut that budget, but alter the defined A4 ecology. None is
implemented or adopted by this study.

These are measured targets and unimplemented leads. Exact algorithmic candidates
still need independent parity and matched implementation measurements. More-core
and changed-cadence proposals are separate from exact one-core speedups.

## Evidence and limits

All full serial/parallel action tapes match, and every counted/profiled
continuation has the reference final-state SHA-256. The three diagnostic guard
traces match for each row. #331's discarded/counted/profiled worlds also equal
the original main state over the same 512 ticks. This establishes the measured
continuations only, not a universal exactness proof for that external PR.
Seventeen method tests and both raw-data reducers pass; the report's twelve
tables are regenerated/checked from the selected JSON. No production Rust
source changes; no full Rust check group was needed or run for this report.

Qualified A4 warmups first passed at 86k/86k/90k ticks but saved at
87k/87k/91k: asynchronous stop polling added one 1,000-tick interval. This is
a disclosed deviation from the original first-qualifying-checkpoint wording.
Both qualification and saved counters are retained; no later checkpoint was
selected for faster performance.

One seed, finite ecological screens and short frozen continuations limit
extrapolation to arbitrary uptime or other policies. A censored 10k snapshot is
not interchangeable with a mature world. The dense center-steering comparison
is deliberately easier for its policy and can exercise a different pickup path;
it is not evidence that 10k A4 bots should have the same cost.

Full sources, binaries, checkpoints, action tapes, raw rows, profiles and failure
logs are **local-only** outside Git. Their archive digest/verification receipt is
published; a digest identifies material but does not make it publicly retrievable.
See [archive receipt](archive-receipt.json): 727 members, 362,665,997 bytes,
SHA-256 `6207e45561438ed30bf55f688982dcb2b15c33daeeb24e8763cf1012354906fd`.
Every member was checked before the archive-ready receipt was written. The
transferred copy is hash-verified before source cleanup.

Only study tooling, selected evidence and the research index row change. No
CHANGELOG entry, runtime change or deployment. No AWS resources or external game
servers were used. Guest cleanup/release is recorded after the verified transfer.