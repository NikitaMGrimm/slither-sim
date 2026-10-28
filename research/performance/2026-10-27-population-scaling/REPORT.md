# One-core population scaling, 2026-10-27

The frozen implementation does not justify a 7 ms / 10k commitment. Full-population
physics remains far cheaper than its ongoing original admission search in the
dense capped worlds; today's sampled full-client bot path adds substantial
work of its own. The uncapped 10k arm is **unsettled**, with a verified pickup
fallback cliff. These are synthetic instruction budgets, not a native capacity
qualification. No production runtime source changes are part of this study.

[Selected results](results.json), [protocol and amendments](PLAN.md),
[attempt ledger](ATTEMPTS.md), and [table generator](report_tables.py).

## Length-screened scaling

Million current-thread user instructions per 7 ms tick. World-step medians use
five physics repetitions. Original admission and combined budgets use the
complete original-search references; their run count is shown in brackets.
The combined column is measured directly, rather than adding rounded columns.
The three smaller populations share one common capped/heuristic case.

| Requested / arena | Radius | Density x | Step M/tick, n=5 | Original admission M/tick [n] | Original combined M/tick | Step k/snake |
| --- | --- | --- | --- | --- | --- | --- |
| 500 | 14,981.66 | 1.000 | 7.754 | 5.893 [5] | 13.647 | 15.508 |
| 1,000 | 21,187.26 | 1.000 | 18.652 | 31.476 [1] | 50.128 | 18.652 |
| 2,000 | 29,963.31 | 1.000 | 42.686 | 162.287 [1] | 204.973 | 21.343 |
| 5,000 cap | 32,000.00 | 2.192 | 81.908 | 7,719.190 [1] | 7,801.097 | 16.382 |
| 5,000 heuristic | 47,376.15 | 1.000 | 113.110 | 1,207.883 [5] | 1,320.993 | 22.622 |
| 10,000 cap | 32,000.00 | 4.384 | 133.788 | 35,173.612 [1] | 35,307.400 | 13.379 |
| 10,000 heuristic | 67,000.00 | 1.000 | Unsettled | Unsettled | Unsettled | Unsettled |

### Population and lengths

All requested snakes were alive at step entry in every counted tick. End-tick
living counts expose normal deaths; every measured window had zero refusals.
Lengths and food/prey counts are endpoint values; living means/minima and
deaths cover the 1,000 counted ticks. Lengths are segment counts.

| Case | Step entry mean (min) | End tick mean (min) | Deaths/s | Refusals | Mean segments | p50/p90/p99/max | Food / prey |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 500 | 500.000 (500) | 499.799 (498) | 28.714 | 0 | 22.96 | 14/59/118/164 | 170,185 / 71 |
| 1,000 | 1,000.000 (1000) | 999.597 (997) | 57.571 | 0 | 26.14 | 16/66/125/199 | 367,745 / 139 |
| 2,000 | 2,000.000 (2000) | 1,999.130 (1995) | 124.286 | 0 | 28.07 | 18/69/143/213 | 809,222 / 193 |
| 5,000 cap | 5,000.000 (5000) | 4,995.991 (4986) | 572.714 | 0 | 22.66 | 15/54/108/221 | 1,234,596 / 61 |
| 5,000 heuristic | 5,000.000 (5000) | 4,997.788 (4991) | 316.000 | 0 | 29.11 | 19/72/137/189 | 2,119,326 / 191 |
| 10,000 cap | 10,000.000 (10000) | 9,986.596 (9972) | 1,914.857 | 0 | 17.32 | 12/40/82/162 | 1,478,233 / 21 |

### Warmup

Ratios are maximum/minimum over the last three 10,000-tick blocks. Mean and
p90 must each be at most 1.05. Food is reported separately: the observed drift
does not prove a long-uptime cost plateau, even when length passes the screen.

| Case | Ticks | Length screen | Mean ratio | p90 ratio | Food-stock ratio |
| --- | --- | --- | --- | --- | --- |
| 500 | 130,000 | pass | 1.020 | 1.022 | 1.062 |
| 1,000 | 140,000 | pass | 1.047 | 1.030 | 1.054 |
| 2,000 | 160,000 | pass | 1.025 | 1.015 | 1.041 |
| 5,000 cap | 110,000 | pass | 1.047 | 1.032 | 1.068 |
| 5,000 heuristic | 160,000 | pass | 1.036 | 1.038 | 1.043 |
| 10,000 cap | 70,000 | pass | 1.028 | 1.027 | 1.029 |

## Scope and method

Synthetic, offline SlitherSim worlds at runtime
`ef866d73259e35fe8659a1996d7b97c36d511e0b`. Rust 1.99.0, portable x86-64 release,
thin LTO, one codegen unit, debuginfo=1, default features. Physics and each bot
sample use one logical CPU. Initially two collectors used CPUs 2 and 3, with
Cargo jobs=2/Rayon=2 for builds; the simulator's parallel observations are off.
From 2026-10-28, the maintaining agent's one-CPU allocation under the owner's
machine budget restricts all remaining work to CPU 2,
jobs=1/Rayon=1 and one active world at a time. In-flight single-threaded warmups
were paused and resumed sequentially, preserving their state; they use no
Rayon work. Both finished and released their memory before counted checkpoint
loads. The retained worlds totalled about 650 MB at the transition. VM
compiler targets were deleted after verifying copied binaries. Selected runs
use a 4-vCPU Zen 5 EPYC KVM guest; task affinity is
restricted as above.
Collection uses Linux 6.18.54, glibc 2.41 and Python 3.13.5.
Preparatory WSL2 attempts are retained separately. After the maintaining agent
prohibited workloads on the owner's Windows workstation, all builds,
measurements and checks moved to the VM. Virtual machine wall time is
descriptive; no target-hardware capacity qualification.
`perf_event_paranoid=2` remains unchanged.

These numbers describe the frozen runtime, not a moving main branch. During
collection main gained #224 (shrink-cache reuse), #228 (shared geometry) and
#230 (pellet live-bit ranks and amortized migration). Their mature high-N
effect is unmeasured here. In particular, pickup/cache optimization budgets
below must be reprofiled on current main before choosing an implementation;
the report does not count those later improvements as remaining savings.

The [protocol](PLAN.md) freezes populations, seed, controller, convergence screen
and sample selection. The [attempt ledger](ATTEMPTS.md) retains preparatory
failures and the exploratory warmup. Full raw worlds, rows, binaries, archived
sources and logs are local-only outside Git; hashes establish identity, not
public access or scientific validity.

Every living snake receives a public in-process Action on every 7 ms tick.
Headings change every six ticks by seeded slow wander; one of eight staggered
300-tick intervals requests boosting (12.5%; normal mass limits still apply).
Physics always executes the normal complete step:
movement, collisions, pickups, ecology and lifecycle. There are no clients,
observations, distant-snake exemptions, skipped ticks or changed collision rules
in the physics rows. Controller calculation is outside the step PMU bracket.
Per-snake step costs divide by mean living population at step entry, recovered
as end-of-tick living plus that tick's deaths. End-of-tick living is reported
separately so normal death/respawn churn remains visible.

Fresh lives use the host's complete 64-candidate official search, extracted
without changing predicates, RNG draws, fallback scoring or tie order. The
driver uses public respawn_body; refusals retry after two ticks in stable slot
order. Original-search references count search plus the dead-slot scan
separately. This reproduces
pose search and newborn geometry; it does not reproduce socket queue order,
client death-report latency or gateway scheduling. Initial admission and warmup
are excluded from counted windows; ongoing death food and admissions remain.
Pose draws use the host's xorshift transition with the study initializer
`7 ^ 0x7370_6177_6e01`; steering has separate per-slot streams. This is a
deterministic harness stream, not a replay of a hosted transport session.

Warmup runs in 10,000-tick blocks until three blocks' mean length and p90 differ
by at most 5%, with a ceiling of 240,000 ticks. The original 120,000 ceiling
failed the exploratory 500-slot pilot and was extended before selected runs.
This screen checks length stability, not mathematical stationarity of the full
ecology. Warmup trajectories and food counts remain in selected data.
In particular, a length-screen pass does not establish a long-run food-stock
or cost plateau. The food ratio across the last three blocks is reported
separately; capacity planning must not silently extrapolate a seven-second
window to arbitrarily long server uptime.

Each physics repeat restores the same warmed world and driver, executes 100
untallied cache-settling ticks, then counts 1,000 complete world steps (7 virtual
seconds). One untallied whole run precedes five selected repeats. Medians and
ranges describe process/counter variation for that deterministic window; they
are not independent-world confidence intervals. These repeats use the exact
admission preparer before the world-step bracket, preserving every pose,
candidate, RNG draw and refusal. All snakes still execute normal physics.

Baseline admission and combined step/admission costs come from complete
original-search references on that same 1,100-tick continuation. There are five
retained references at 500 and uncapped 5k, and one at each other primary case.
All original references use one retained plain-binary cohort.
The capped-10k reference was the former untallied run; the resource amendment
was recorded before inspecting its instruction totals. Every repeated physics
and diagnostic endpoint must equal its original reference's complete world and
driver bytes. The per-tick population, death, birth, attempt and refusal census
must also agree. Reference run counts are reported beside their medians/ranges;
one reference supplies no run-to-run range. Private preparation admission rows
remain raw evidence and are never reported as original server cost or savings.

Step and original-admission brackets count current-thread user instructions
and enforce at least 99% PMU scheduling. Setup, checkpoint I/O, controller
calculation and statistics are excluded. No measured attempt is silently
replaced. This amendment avoids repeating a 41.65-minute original-admission
window seven times solely to resample the same physics trajectory.

Admission preparation accelerates only the copied predicates with fresh
body bounds. Candidates provably beyond clearance, or unable to lower the
current margin, avoid geometry calls; all other operations and ordering remain.
Every normal physics step still runs. The 500-slot checkpoint, driver and all
warmup statistics match original search byte for byte after both 120,000 and
130,000 ticks. Fifty complete pose searches and RNG endpoints are also checked
against original search in every warmed N/arena. Original-admission references
always call the original host search; preparation acceleration is not counted
as a server saving. Full reference/physics continuation equality is required
at every primary N, in addition to the long 500 warmup oracles.
The retained RNG endpoint also recovers the number of candidate iterations
in each 50-pose oracle: official candidates consume exactly three xorshift
draws (distance, angle, heading). This is a read-only conformance sample,
not a candidate census of the counted lifecycle window. Separately, retained
warm/terminal driver RNG and death records recover the full 1,100-tick
continuation's candidates and admissions, including the 100-tick cache prefix.
That denominator is labelled explicitly rather than attributed to the narrower
1,000-tick instruction window.

Checkpoint loading is uncounted preparation. The original public importer
temporarily retains a generic JSON tree alongside typed state and exceeded the
memory guard on a mature 5k fixture. A helper appended only to archived build
copies validates record shapes one at a time, then runs the original typed,
configuration and state validation. The production importer is unchanged.
Every restore checks a byte-identical canonical checkpoint round trip before
advancing. Both arms pass a valid round trip and eight malformed-shape controls;
the complete 500 continuation also matches the original public importer.
All six primary physics rows use the same rebuilt pair. Completed warmups are
reused with checked hashes; original admission, bot and cliff components retain
their actual separately recorded binary pins. The failed loads and superseded
preparation prototype remain in
the ledger. Native children have a 2.5 GiB address-space ceiling and no core
dumps; final collection peak RSS is recorded in selected data.

Cap events are halfway before their intended tick, avoiding cumulative clock
drift past the replay API's 1 ns tolerance in long warmups. A negative control
reproduces the old exact-multiple schedule's first late event at tick 98,477.
The completed capped-10k warmup is earlier: reuse checks identical old/new
event indices at every tick through its counted endpoint (71,100), replaces
only the radius-event array and streams all remaining checkpoint bytes
unchanged. The generated new plan is validated through the maximum 241,100-tick
warmup/cache/count extent. All primary counts use the corrected, rebuilt pair;
the earlier interrupted untallied run remains in the ledger.

Diagnostic hooks exist only in a separately archived build. Coarse phases are
additive, apart from the explicitly nested rotating 1/64 per-head query/test and
boost-emission estimates. Those estimates are multiplied by 64 and include
bracket overhead; they are not exact additive decompositions or measured
optimization savings. Plain and diagnostic terminal checkpoint and driver
bytes must agree. This endpoint check does not assert an all-tick trace oracle.
Boost-food work belongs to movement; death and ambient phases include their
own food-cache admission work. The candidate-test bracket includes AABB
rejection and exact geometry, rather than pretending to isolate pure exact
tests without their calling loop.

## Phase profile

Coarse world-step phases are additive within each separate diagnostic run.
The original admission row comes from its matched reference and is outside
the world-step total. Bracket and compiler effects raise diagnostic step
counts by about 0.48–0.90%; these phase budgets are not measured savings.

| Phase | 500 | 1,000 | 2,000 | 5,000 cap | 5,000 heuristic | 10,000 cap |
| --- | --- | --- | --- | --- | --- | --- |
| Movement, preparation, boost | 0.534 | 1.072 | 2.145 | 5.314 | 5.366 | 10.547 |
| Collision broad-phase build | 0.582 | 1.225 | 2.602 | 5.966 | 6.762 | 10.888 |
| Collision queries / bounds / exact loop | 0.669 | 1.426 | 3.282 | 11.219 | 8.127 | 27.505 |
| Death resolution / food | 0.008 | 0.017 | 0.044 | 0.176 | 0.121 | 0.448 |
| Pickup / food cache | 5.583 | 14.137 | 33.195 | 56.579 | 89.664 | 79.453 |
| Prey | 0.394 | 0.808 | 1.484 | 2.872 | 3.236 | 5.458 |
| Ambient refill / pellet admission | 0.005 | 0.009 | 0.018 | 0.019 | 0.044 | 0.018 |
| Rest of step | 0.034 | 0.068 | 0.135 | 0.336 | 0.336 | 0.671 |
| World step, diagnostic total | 7.808 | 18.761 | 42.905 | 82.481 | 113.656 | 134.988 |
| Original admission, separate reference | 5.893 | 31.476 | 162.287 | 7,719.190 | 1,207.883 | 35,173.612 |

### Nested estimates

Rotating 1/64 samples, scaled by 64, include hook overhead. They overlap the
coarse rows and must not be added. The candidate row includes bounds and the
calling loop as well as exact tests; boost includes accounting and emission.

| Estimate | 500 | 1,000 | 2,000 | 5,000 cap | 5,000 heuristic | 10,000 cap |
| --- | --- | --- | --- | --- | --- | --- |
| Collision query | 0.288 | 0.616 | 1.299 | 4.029 | 3.182 | 9.539 |
| Candidate / bounds / exact loop | 0.403 | 0.861 | 2.076 | 7.402 | 5.172 | 18.411 |
| Boost accounting / emission | 0.012 | 0.024 | 0.048 | 0.112 | 0.118 | 0.244 |

### Original admission search

The continuation census includes the 100 cache ticks as well as the 1,000
counted ticks. Its candidate mean therefore has a different denominator from
the 50-pose oracle. Amortized phase cost per accepted life uses only the
1,000-tick original-reference window and includes the dead-slot scan.

| Case | 50-pose candidates/pose | 1,100-tick candidates/life | 1,100-tick admissions | Original phase M/life in 1,000 ticks |
| --- | --- | --- | --- | --- |
| 500 | 6.220 | 6.398 | 226 | 29.171 |
| 1,000 | 6.300 | 7.529 | 448 | 78.105 |
| 2,000 | 9.100 | 8.229 | 955 | 186.752 |
| 5,000 cap | 59.940 | 61.250 | 4422 | 1,925.946 |
| 5,000 heuristic | 7.600 | 9.348 | 2448 | 546.553 |
| 10,000 cap | 64.000 | 63.995 | 14735 | 2,624.113 |

## What makes the curve grow

These are coupled ecological workloads, not copies of one fixed-length snake.
Population, body lengths, food stock, mortality and candidate-search difficulty
all affect cost. The length screen makes the requested comparison practical;
it does not identify a universal power law in N.

Before the pickup cliff, the constant-density 500-to-5k original-reference
world step grows about 14.6 times for ten times the snakes. Pickup/cache work
grows about 16 times and dominates that step. Its food stock rises from about
170k to 2.12M and mean body length from about 23 to 29 segments. The pinned
cache maintains ordered pellet positions and compacts survivors after hits;
work therefore follows food stock and churn, as well as the number of eaters.
Collision construction and query/test budgets grow roughly 11.6/12.2 times:
construction visits body anchors and builds an ordered index, while query cost
depends on overlapping bounds and exact candidates. Movement/preparation is
approximately linear here; prey grows less than N in these observed worlds.
Prey's threat decisions still contain a head scan, so an N-by-prey component
can grow faster in a different ecology. Death/boost food follows deaths and
mass turnover; ambient refill follows the radius-dependent ecology.

The strongest superlinear term is **original admission**, not the world-step
collision loop. At constant density, its 500-to-5k cost grows about 205 times.
Each candidate can scan existing body geometry, and there are more births per
tick as N grows: body work per search times candidate count times births gives
a roughly quadratic term when length, candidate count and per-snake mortality
are stable. Here those factors also change. The retained candidate census
separates extra search attempts from increased geometry work per attempt.
Preferred 600-unit clearance gets harder in the capped worlds; when it fails,
the host still evaluates its original bounded best-margin fallback.

At the same 5k N, capping lowers the observed step from about 113.1M to 81.9M,
but raises admission from 1.208B to 7.719B instructions/tick. Shorter bodies
and less accumulated food make the capped step cheaper; higher mortality and
harder spawn search make lifecycle cost much larger. At capped 10k, admission
is 35.174B/tick versus 133.8M for the step, about 99.6% of the original combined
budget in this wander workload. A policy that changes survival and growth can
change both sides of that balance. The separate 500-seat Forager sample does
not establish a mature high-N body or mortality distribution.

## Arena and delivery limits

The capped arm uses the largest **currently supported** wire-compatible host
radius, `encode::MAX_RADIUS=32000`. The encoder actually offsets coordinates by
32550, giving the asymmetric raw range -32550..32985; its 550-unit margin is an
explicit encoding policy, not a derived maximum. Thus 32767 is only the
approximate centered-u16 reference, not this backend's supported cap. The
32000 policy binds above about 2281 seats (the idealized 32767 reference near
2392). The 500/1000/2000 variants coincide and are measured once. At 5000/10000,
the cap makes requested-snake density 2.192/4.384 times the `670*sqrt(N)`
heuristic. Uncapped controls keep that heuristic's density.

Capped physics rows scale initial official food/refill/prey by area, disable
startup enlargement and keep dynamic seat-following enabled. Existing public
recorded-radius updates apply 32000 after each controller update. All N seats
remain occupied across death/respawn. Thus the cap is a research configuration,
with a tiny radius-update traversal cost; it is not a new host feature.
Today's SimulationConfig validation also checks the uncapped seat target and
would reject these large dynamic hosted worlds. A supported ceiling on dynamic
seat-following would need a future runtime change. Fixed-radius hosted
configurations can already stay at 32000; their full high-N host path is not
qualified here. Uncapped rows are physics density controls, not wire-compatible
server configurations.

Snake IDs are u16; the host permits at most 65,535 agents because rank/living
counts must also fit. Ten thousand is below that limit. Stock-client rendering
of dense views is untested. This study does not qualify network delivery,
socket scheduling, memory capacity or browser smoothness.

The earlier [host coverage study](../2026-10-27-host-coverage-cells/REPORT.md)
reports about 2.36M world-step instructions in its short official-500 fixture.
The mature 500 row here costs about 7.754M for the step plus 5.893M for ongoing
admission, with roughly 170k food and 23 mean segments at the counted endpoint.
The old fixture runs only 1080 ticks and has a different controller/lifecycle;
the platforms also differ. This is not a matched runtime regression. Neither
`2.36M*20` nor a newborn-10k step is an honest mature 10k budget.

## Uncapped 10k: censored, with a pickup algorithm cliff

**Unsettled.** The maintaining agent limited this arm to one aggregate CPU-hour.
Its
terminated WSL warmup consumed 3343.74 process CPU seconds before flushing its
first 10,000-tick block; the exact tick reached is unknown. A conservative
60-second diagnostic allowance plus 118.052659 remote diagnostic CPU seconds
brings the total charge to **3521.792659 seconds (58.70 minutes)**. This includes
remote setup, all reached ticks and cold probes. Compilation is infrastructure
outside that simulation budget. No further uncapped-10k simulation is run.

The remote bounded run reached tick 362 in 98.22 process CPU seconds. It used
the same seed, host admission and controller as the primary physics harness;
every tick is flushed into selected evidence. Mean living population was
9999.901 (minimum 9997, endpoint 10,000), with 36 deaths (14.207 per virtual
second), 36 respawns after initial admission, and zero refusals. Mean segments
only grew from 2 to 2.036
(last p50/p90/p99/max: 2/2/3/9). This is 2.534 virtual seconds, far below the
30,000-tick minimum length screen. It supplies **no steady-state estimate**.

Million user instructions at particular reached ticks, separate diagnostic
build; initial 10k admission is setup rather than ongoing churn:

| Tick | World step | Pickup/cache subset | Admission | Rectangle cells | Food-relative cell limit |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 4725.105 | 4526.849 | 96848.628 | 1,073,296 | 4,230,208 |
| 100 | 4556.913 | 4524.382 | 0.140 | 1,076,406 | 4,234,684 |
| 200 | 4601.856 | 4567.120 | 0.140 | 1,078,482 | 4,247,904 |
| 362 | 4590.080 | 4557.577 | 90.250 | 1,082,640 | 4,276,104 |

The slowdown is `EaterGrid::build` in `src/food_grid.rs`. Its 128-unit cells
cover the **whole axis-aligned eater rectangle**, including empty space. It
rejects rectangles larger than `min(1<<20, max_cells)`, where the caller passes
`food.len().saturating_mul(4).max(4096)`. It also rejects aggregate cover area
over `1<<20`, invalid/extreme circles and an overflowing eater count. Here the
fixed **1,048,576-cell rectangle ceiling** binds; the food-relative allowance
is much larger. The initial 10k rectangle exceeds the fixed ceiling by only
2.36%. Increasing the food count does not rescue it.

With dense indexing rejected and `eaters.len()*food.len() >= 32000`,
`src/simulator.rs` selects the **256-bucket hashed eater-bitset pickup
fallback**. For each pellet it scans `ceil(eaters/64)` bitset groups (157 at
10k), expands hash aliases, then performs exact candidate checks. Both food
and eater counts grow with N; the scans and aliases can therefore grow roughly
quadratically despite constant arena density. All collisions and pickup rules
still run. At tick 362 pickup is 99.29% of the world step.

Separate newborn cold probes (two cache ticks, then two counted ticks, no
respawn) bracket onset for seed 7 and their admitted placement. They are
diagnostics, not steady rows or replications of the main driver trajectory:

| N | Radius | Eater rectangle cells | Dense accepted ticks | Step M/tick | Pickup M/tick |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 5,000 | 47,376.15 | 538,022 | 2/2 | 38.725 | 24.417 |
| 9,700 | 65,987.35 | 1,040,400 | 2/2 | 75.428 | 47.391 |
| 9,800 | 66,326.62 | 1,052,676 | 0/2 | 4350.350 | 4322.064 |

The cliff appears between 9700 and 9800 in these probes: about **57.7x** step
cost for 1.03% more snakes. It is geometry-dependent, not a universal exact N.
A fully spanned arena approaches 1024 cells per axis around radius 65,536,
or heuristic N about 9568; the newborn spawn band lies inside the rim and
shifts this instance's onset. The primary 10k driver rejects dense indexing
on all 362 reached ticks. A 32,000 cap avoids this rectangle-size cliff.
Consequently the uncapped arm is not a clean density-only control at this
scale: changing radius also changes the pickup algorithm.

For completeness, its **unsettled early prefix**, ticks 2–362, has this full
coarse diagnostic budget (M user instructions/tick). It is not substituted
into the steady-state scaling table:

| Phase | M/tick |
| --- | ---: |
| Movement/preparation and boost food | 10.254 |
| Collision broad-phase build | 6.220 |
| Collision queries, bounds and exact-test loop | 6.467 |
| Death food | 0.00045 |
| Pickup / food cache | 4543.021 |
| Prey | 10.780 |
| Ambient refill / admission | 0.088 |
| Rest of world step | 0.671 |
| World step total | 4577.501 |
| Original host spawn search / admission, outside step | 4.182 |

Nested rotating-sample estimates in that prefix are 4.323M query,
2.633M candidate/bounds/exact-test and 0.061M boost-accounting/emission
instructions per tick. They include scaled hook overhead and must not be
added to the coarse budget. At a fully living 10k tick, the observed 4.59B
world step is about 459k instructions per living snake, already with newborn
lengths; this remains a censored cost, not a mature per-snake estimate.

## Full-client bot path at the frozen revision

Actual `SimulatorBackend::next_frame` calls `drive_background` for 499 Foragers,
then advances six physics ticks in a 500-seat official world. One scripted
controlled seat completes the population. Zero modeled latency/processing and
official delivery are explicit; there are no sockets, worker queues or real
compute-time feedback. The kernel and adapter are today's shared A4 Forager.
`frame_for` itself materializes already-decoded state; wire encoding and
decoding occur in broadcast/delivery, not inside that polling method.

After 1,000 warmup decisions (42 simulated seconds), each run measures 200
decisions (1,200 physics ticks). One plain endpoint control and one untallied
diagnostic run precede five selected diagnostic repeats. Every run ends with
the same complete core checkpoint. All counted PMU intervals are 100% scheduled.
Each selected run has 99,793 live background decisions, mean population 499.97,
seven background reconnect attempts and no refusals. At the endpoint all 500
are alive, with mean 10.988 segments. Mean policy inputs contain 4.91 opponents,
419.41 food and 0.53 prey. This is a sparse 500-seat sample, not a mature dense
10k view or a playing-strength test.

Thousands of user instructions per live background decision, five-run medians.
Every stage's repeat range is less than two instructions per decision; full
precision is retained in selected JSON.

| Stage | k instructions / decision |
| --- | ---: |
| World-wide packet building/broadcast, including encoding, amortized | 83.635 |
| Final wire framing, **subset of broadcast above** | 2.496 |
| Delivery decoding, amortized | 13.685 |
| Received `ClientFrame` materialization | 42.644 |
| `ForagerBot::decide`, including adapter | 59.417 |
| Sum of four disjoint stages (excluding nested framing row) | about 199.4 |

Full precision/ranges remain in selected JSON. Broadcast/decoding are shared
world costs divided by live decisions, not measured marginal costs of adding
one bot. The four-stage sum excludes command submission, frame destruction and
other unbracketed work. The outer `next_frame` operation, including physics, is
21.255M/tick in the single plain control and about 21.429M in the diagnostic
runs: hooks/compilation add about 0.82% here. Diagnostic budgets are not measured
optimization savings.
Controlled-seat polling/action submission and destruction of its returned
frame are outside that outer bracket; this is not a complete hosted-server cost.

About 70.2% of the four-stage sum lies outside policy. Plan for a direct
server-bot input path to the same shared kernel: there is a substantial budget
in packet generation and frame materialization, not just decoding. An idealized
removal of those three stages gives at most 3.36x for this four-stage sum before
replacement visibility/input work, with today's policy-stage cost unchanged.
Human clients still require delivery; shared broadcast work cannot simply be
removed per bot. Equivalent visibility and timing/intent semantics also need
qualification; a direct authoritative input is not proven to choose the same
actions as delayed/quantized received state.

Flat-cost arithmetic illustrates the size of the remaining task. At one
decision per six ticks, today's measured four stages contribute about
66.5/166.2/332.3M instructions per tick at 2k/5k/10k; the policy stage alone
contributes 19.8/49.5/99.0M if its measured cost stays unchanged. These are
scenario calculations, not measured high-N bot costs. Denser capped views and
older food/body populations can raise per-bot costs; new view construction and
all physics/host work remain additional.

## Optimization priorities and feasibility

All savings below are planning estimates against this study's frozen runtime,
bounded by measured phase costs. No production optimization is part of the
change; preparation timings are not claimed as server savings.
Keeping every snake active every tick is a requirement throughout. Spatial
rejection must conservatively preserve every exact test that can affect an
outcome, along with candidate order, tie rules, RNG and lifecycle semantics.

1. **Index official admission geometry.** Reuse conservative body bounds or a
   spatial index for both preferred-clearance and fallback-margin searches.
   Preserve all 64 possible candidates and the original fallback result.
   At 2k, an assumed 80–95% reduction of the 162.287M admission phase saves
   129.8–154.2M/tick. At capped 5k, the same assumption saves 6.175–7.333B
   of 7.719B/tick; its remaining cost would still be 1.544B–386M. These are
   planning scenarios, with the measured phase as their ceiling. At capped 10k the original
   phase is 35.174B/tick: 80–95% saves 28.139–33.415B, but still leaves
   7.035–1.759B. Even a 99% reduction leaves 351.7M before world physics or
   bots. This needs queries that avoid scanning every body, rather than a small
   instruction-level improvement. Full reference equality qualifies the
   preparer's state in these windows; a production index still needs dynamic
   invalidation and burst tests. Actual Foragers can have different mortality
   and longer bodies; their mature high-N physics distribution is unmeasured.
2. **Give server bots a direct input path to the shared policy.** The sampled
   full-client pipeline spends 139.96k of 199.38k instructions/decision outside
   policy. Eliminating 50–80% of that removable budget, after replacement view
   work, would save 70–112k/decision: about 23–37M, 58–93M and 117–187M/tick
   in flat-cost 2k/5k/10k scenarios. These are not measured dense-bot savings.
   Policy alone remains about 59.42k/decision in the 500-seat sample. Keep the
   shared kernel and qualify visibility, quantization and scheduling semantics;
   removing network emulation must not grant cheaper physics or skipped ticks.
3. **Reduce pickup-cache maintenance and query work.** An assumed 25–50% cut
   saves about 8–17M/tick at 2k, 14–28M at capped 5k and 20–40M at capped 10k.
   The uncapped-5k phase is 89.664M/tick, a corresponding 22–45M budget.
   This bracket combines indexing, exact pickup tests, hit sorting and cache/
   food compaction, so it does not identify a single removable operation.
   Main's later #230 already changes position/rank refresh and periodic rebuilds;
   reprofile that version before proposing more work. No duplicate saving is
   credited here. Preserve reverse food order, nearest-head/ID ties and all
   admission/cache invalidation rules.
4. **Remove the large-radius pickup cliff for uncapped worlds.** Use an exact
   sparse/chunked eater index or another bounded representation instead of
   scanning 157 hashed bitset groups per pellet. At uncapped 10k, the early-prefix
   pickup budget is 4543M/tick. Removing 95% would save about 4316M/tick; no
   saving has been measured, and this is an optimistic planning scenario. The
   nearby 9700/9800 cold probes motivate it without proving mature costs.
   A bounded ceiling increase is a simpler candidate for this particular
   geometry (about 4.3 MB of cell heads at 1.08M cells), needing parity and
   memory checks; it leaves a later cliff. The 32k server goal avoids this
   rectangle limit, so admission and bots rank ahead of this control-specific
   fix. Expanded-coordinate worlds would need it before long qualification.
5. **Then optimize collision, movement and prey with exact semantics.** At
   capped 10k, construction plus query/tests costs 38.393M/tick; an assumed
   20% reduction saves about 7.7M. Movement/preparation/boost plus prey costs
   another 16.005M: 20% would save about 3.2M. These budgets
   are much smaller than admission there. Main's #228 already reuses geometry;
   do not assume these estimates survive that change. Prey's threat decisions
   still scan living heads, so a conservative query can matter when prey and
   snake counts both grow. Ambient refill and death-food phases are separately
   visible and small in the selected windows, without proving cheap burst tails.

Nested estimates overlap the coarse diagnostic phases. The grid-cliff and
pickup-cache proposals also share the pickup budget, and estimates drawn from
different populations or the separate bot sample cannot be summed into a
measured server speedup. Changed workloads, replacement structures and
instruction-cache effects can change every phase. This study measures averages
over seven virtual seconds,
not a 7 ms deadline-tail qualification. Spawn bursts, food-cache rebuilds,
network clients and browser rendering need separate acceptance.

## The 7 ms goal

For a measured budget I, I/0.007 is the necessary average instruction retirement
rate. The table excludes steering, policy, visibility replacement, sockets and
host scheduling. Instruction mix, memory/cache behavior and bursts also matter;
these arithmetic rates are not measured CPU capacity or deadline qualification.
| Case | World step G instructions/s | Original combined G instructions/s |
| --- | --- | --- |
| 2,000 | 6.098 | 29.282 |
| 5,000 cap | 11.701 | 1,114.442 |
| 5,000 heuristic | 16.159 | 188.713 |
| 10,000 cap | 19.113 | 5,043.914 |

- **2k is the most credible first optimization target, still unqualified.**
  Physics alone is 42.686M/tick; original admission adds 162.287M. Indexed
  admission can remove a large budget without changing simulation rules. At
  unchanged sampled per-bot cost, the full-client stages would add 66.5M/tick;
  their policy component alone would be 19.8M before replacement view work.
  Those are scenarios, not a measured combined server. Qualify current main
  and the proposed bot path on the intended core before promising 7 ms.
- **5k needs major lifecycle and bot-path work.** The wire-compatible capped
  step is 81.908M, but original admission is 7.719B/tick. Even a 95% admission
  cut leaves about 386M before physics or bots. The heuristic-radius control
  costs 113.110M for physics and 1.208B for admission, but cannot be hosted on
  the stock coordinate path. A direct policy path and current-main pickup
  improvements are promising budgets, not a demonstrated 7 ms result.
- **10k is a research target, not a defensible current capacity promise.**
  The capped step alone needs 19.113G instructions/s. Original lifecycle work
  raises the necessary rate to 5.044 trillion instructions/s in this wander
  workload. Even removing 99% of admission leaves 351.7M/tick before physics,
  policy or delivery. The flat-cost policy scenario alone adds 99.0M/tick
  before new view construction. This calls for structural admission indexing,
  a direct bot path and further measured physics/policy reductions. The
  uncapped arm additionally has the catastrophic pickup fallback and no
  settled length distribution. Its early-prefix cost cannot replace a mature
  10k estimate.

All three estimates depend on this controller's mortality and body/food
population. Mature high-N Foragers, current-main costs and a target CPU are
unmeasured. The one-core goal must keep every snake and collision active every
tick; no estimate here assumes distance-based freezing or reduced physics
rates. Fixed 32k hosting is configurable today; an automatic seat-following
ceiling needs runtime support. Dense stock-client rendering remains untested.
## Evidence checks

All six primary original/reference continuations agree on complete world and
driver bytes and every tick's lifecycle/population census. All counter windows
pass the 99% PMU scheduling gate. The prepared/original step medians differ
by less than 0.001%; this is a compilation/preparation check, not an optimization
speedup. Repetitions describe a fixed seeded window, not independent-world
uncertainty. A single reference supplies no run-to-run range.

| Case | Physics step min-max M/tick | Original admission min-max M/tick [n] | Prepared/original step ratio | Diagnostic/plain step ratio |
| --- | --- | --- | --- | --- |
| 500 | 7.754068–7.754137 | 5.892529–5.892529 [5] | 0.999995 | 1.007009 |
| 1,000 | 18.651998–18.652077 | 31.476328–31.476328 [1] | 0.999996 | 1.005867 |
| 2,000 | 42.685712–42.685952 | 162.287311–162.287311 [1] | 1.000003 | 1.005143 |
| 5,000 cap | 81.907812–81.907884 | 7,719.189584–7,719.189584 [1] | 1.000002 | 1.007001 |
| 5,000 heuristic | 113.109789–113.109992 | 1,207.883016–1,207.883159 [5] | 1.000002 | 1.004826 |
| 10,000 cap | 133.788165–133.788370 | 35,173.612006–35,173.612006 [1] | 1.000000 | 1.008968 |

The final sequential invocation peaked at 1,119,656 KiB child RSS (1.07 GiB),
with a 2.5 GiB address-space guard. VM compiler targets were removed after
verifying copied executable hashes; unique raw evidence remains outside Git.

## Reproduction and validation

Both archived release builds and their valid round trip / eight malformed-shape
checks pass. Thirteen Python evidence/publication guards pass, including the cap-clock
regression, censored-budget enforcement and per-tick lifecycle comparison.
The full selector passes on all six primary cases, the retained bot component
and every reached bounded-control tick. Long 500 warmup oracles at 120k/130k,
the original public-importer continuation and 50 pose/RNG checks per arena
also pass. No independent review or full production Rust check group was run
for this research-only diff; production runtime files are unchanged.

From Linux x86-64 with the pinned runtime in Git, cached locked dependencies,
Rust 1.99.0, Python 3.12+ and current-thread user PMU access:

```sh
STUDY=research/performance/2026-10-27-population-scaling
OUT=/path/outside/repository/new-population-scaling
export RAYON_NUM_THREADS=1 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1
taskset -c 2 python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --cpu 2
taskset -c 2 python3 "$STUDY/environment.py" "$OUT/environment.json" --cpu 2
taskset -c 2 python3 "$STUDY/cliff_probe.py" "$OUT" --cpu 2 \
  --populations 5000 9700 9800 --ticks 2 --bounded-seconds 100 \
  --cpu-budget-seconds 180 --prior-cpu-seconds 0
taskset -c 2 python3 "$STUDY/collect.py" "$PWD" "$OUT" --cpu 2
taskset -c 2 python3 "$STUDY/select_results.py" "$OUT" /path/to/new-results.json
taskset -c 2 python3 "$STUDY/report_tables.py" /path/to/new-results.json > "$OUT/tables.md"
taskset -c 2 python3 "$STUDY/reproduce.py" cleanup "$PWD" "$OUT" --cpu 2
taskset -c 2 python3 -B -m unittest discover -s "$STUDY" -p test_select.py
```

Windows worktree gitdir files require a Linux-readable common repository for
the repository argument; the runtime remains pinned. Optional
`SCALING_TARGET_ROOT` reuses task-owned arm targets, while copied executable
hashes remain bound to each fresh output. Output creation refuses overwrite.
Build pins record build-time file hashes, including the then-current
plan/report drafts; generated Rust sources and executables are retained in the
raw archive. Final analysis scripts are delivered here and bound separately
by their hashes. The selector rechecks the four core Rust templates,
checkpoint-shape helper and copied binaries; `analysisSources` binds the final
Python analysis.
Published paths use `<repo>`, `<wsl-artifacts>` and `<vm-root>` labels while
preserving relative structure and hashes. Kernel metadata omits private build
suffixes. These transformations occur after validation of the retained raw data.
The bounded-probe attribution-only edit is checked against the historical source
pin by reconstructing that exact edit; its published source hash is separate.
Owner resource and maintaining-agent execution/censoring amendments are recorded
in the ledger without rewriting historical build pins.
The fresh bounded reproduction charges no historical CPU; the published run
used `--prior-cpu-seconds 3403.74` to include its terminated WSL attempt and
allowance. Do not rerun a full uncapped warmup or restart the bounded probe in
the published output. The CPU-timed diagnostic can reach a different endpoint
on another machine; compare the retained common tick prefix. Primary windows
use fixed tick counts. Keep all commands and their children inside the chosen
one-CPU affinity; choose a valid CPU on other hosts. Fresh reproduction uses
the final one-CPU settings throughout; historical build/resource metadata is
retained rather than retroactively relabelled.
