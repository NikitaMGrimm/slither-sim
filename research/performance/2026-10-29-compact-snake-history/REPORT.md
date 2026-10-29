# Compact occupied recipient snake histories

The following study verdict is historical and unchanged. The separate
[post-result maintainer decision](#maintainer-decision) adopts the layout.

**Rejected under the frozen short-world gate.** Compact histories reduce
mature-500 whole-window CPU in all ten protocol-specific pairs, with median
paired ratios **0.8579 / 0.8199**, but three short controls exceed 1.02 in all
five pairs. Production code remains unchanged; the complete implementation and
oracle are retained in [measured-bounded.patch](measured-bounded.patch).

The [memory diagnostic](../2026-10-29-simulation-memory/REPORT.md) identified
sparse recipient histories as the strongest portable layout lead: hundreds of
optional world-slot records per recipient hold only a few known snakes. This
study compares that storage with a sorted vector containing occupied records.

Selected [results](results.json), [pilot observations](pilot-results.json),
[verification](verification.json), [table calculator](tables.py) and
[reproduction](REPRODUCE.md) retain the measurements and checks.
Final [delivery checks](postprocessing-checks.json) cover documentation,
independent table/gate recalculation and source-exact patch round trips.

## Design and exactness

Each entry retains its original slot and complete `KnownSnake` value. Keys stay
ascending and unique; replacement, returned removal values and mutable lookup
retain their meaning. Candidate enumeration merges the same ascending slot
sequence with nearby slots. Each recipient still owns its history, and clone
and reconnect/reset operations retain independent values. Linear lookup is
bounded to eight records, with binary search above that; this is a portable
work bound, not a cache-size or CPU-specific setting. No numeric precision,
protocol, physics, packet release opportunity or dependencies change.

The focused oracle compares against the literal previous optional-slot
implementation over 20,000 deterministic random operations and eight
recipients. It checks every field by bits, including signed zeros and NaN
payloads, high slots, replacement, removal, mutable access, reset, cloning and
ordered enumeration. Existing lifecycle and protocol fixtures cover death,
disconnect, reconnect and slot/life reuse.

All 27 catalog/host traces match. Complete mature warm streams match byte for
byte: **5,876,276,676 bytes per stock arm** and **10,132,210,980 per protocol-13
arm**. Both start at 499 living snakes, mean length 55.3307, p90 176 and p99 247;
each continuation ends at 500 living, mean 56.16 and the same p90/p99. Each of
the ten rows passes two child continuations per arm and both direct-parent
continuations, with terminal records and cumulative frame/byte counts checked.
All three oracle mutations fail at runtime, restored tests pass, the full
offline Rust group passes, radius mutations change host payloads in both
protocols, and a stream-byte flip fails the warm comparator.

## Primary results and decision

CPU numbers below are ms per advance. Ratios are paired candidate/base; their
ranges cover all five reported pairs. No counted window was excluded, replaced
or retried. Every counter interval has running/enabled equal to one.

| Protocol / tick | Base CPU | Compact CPU | Paired CPU ratio [range] | Paired instructions ratio | Paired cycles ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| 19 all | 1.8153 | 1.5500 | 0.8579 [0.7506, 0.8893] | 1.0279 | 0.8551 |
| 19 ordinary | 1.7862 | 1.5218 | 0.8561 [0.7476, 0.8878] | 1.0282 | 0.8534 |
| 19 maintenance | 3.8822 | 3.5189 | 0.9103 [0.8445, 0.9380] | 1.0152 | 0.9096 |
| 13 all | 2.0028 | 1.6022 | 0.8199 [0.7439, 0.8735] | 1.0271 | 0.8216 |
| 13 ordinary | 1.9720 | 1.5742 | 0.8186 [0.7426, 0.8715] | 1.0275 | 0.8202 |
| 13 maintenance | 4.1562 | 3.6110 | 0.8650 [0.7864, 0.9405] | 1.0147 | 0.8712 |

Instruction and miss values are base → compact. M means millions; MPKI is
misses per thousand measured user instructions. Instructions increase despite
lower cycles and CPU. This is consistent with the earlier working-set lead,
without establishing a unique causal mechanism.

| Protocol / tick | M instructions | M cycles | L1d MPKI | dTLB MPKI |
| --- | ---: | ---: | ---: | ---: |
| 19 ordinary | 10.158 → 10.445 | 7.687 → 6.582 | 29.162 → 27.350 | 1.110 → 0.412 |
| 19 maintenance | 19.730 → 20.031 | 16.739 → 15.321 | 19.954 → 19.070 | 1.006 → 0.489 |
| 13 ordinary | 10.476 → 10.764 | 8.541 → 6.857 | 28.484 → 26.969 | 1.162 → 0.480 |
| 13 maintenance | 20.993 → 21.303 | 18.129 → 15.784 | 19.024 → 18.382 | 1.027 → 0.544 |

At the mature barrier, both protocols have 3,681 occupied records across 500
recipients, against 243,903 logical optional slots in the base. Base record
capacity is 20,381,256 bytes plus 63,552 bytes of ascending-slot keys; compact
entry capacity is 444,864 bytes including keys. These are requested vector
capacities, excluding vector headers, allocator overhead and unrelated state.

Whole-window CPU p99 medians fall from **3.8744 to 3.3895 ms** for stock and
**4.1350 to 3.5960 ms** for protocol 13. Ordinary/maintenance p99 medians are
3.5044/4.3453 → 2.7460/4.2107 ms for stock and 3.9389/5.1284 →
3.0866/4.0906 ms for protocol 13. These instrumented, forked no-socket tails
are descriptive; they are not world/socket p99 or a real-player capacity result.

Short CPU values below use µs per advance to expose their absolute size. The
ordinary/maintenance columns give paired median CPU ratios. Every row has five
pairs; all short instruction/cycle/miss splits and ranges are retained in
`results.json` and printed by `tables.py`.

| Protocol / seats | Base → compact CPU µs | Whole-window CPU ratio [range] | Ordinary ratio | Maintenance ratio | All five above 1.02? |
| --- | ---: | ---: | ---: | ---: | --- |
| 19 / young 500 | 1429.805 → 1183.608 | 0.7977 [0.7721, 0.8700] | 0.7969 | 0.8402 | No |
| 19 / 1 | 1.555 → 1.626 | 1.0435 [1.0263, 1.0604] | 1.0465 | 1.0277 | **Yes** |
| 19 / 10 | 8.889 → 9.017 | 1.0121 [0.9957, 1.0467] | 1.0158 | 0.9990 | No |
| 19 / 50 | 49.365 → 50.956 | 1.0370 [0.9857, 1.0457] | 1.0380 | 1.0361 | No |
| 13 / young 500 | 1627.787 → 1323.494 | 0.7934 [0.7503, 0.8748] | 0.7913 | 0.8681 | No |
| 13 / 1 | 1.609 → 1.683 | 1.0448 [1.0226, 1.0807] | 1.0449 | 0.9949 | **Yes** |
| 13 / 10 | 9.591 → 9.746 | 1.0028 [0.9597, 1.0516] | 1.0033 | 0.9937 | No |
| 13 / 50 | 53.335 → 54.945 | 1.0478 [1.0265, 1.0831] | 1.0475 | 1.0552 | **Yes** |

All ten mature whole-window CPU ratios are below one and both mature
maintenance CPU medians are lower. Nevertheless the final frozen condition
fails in three short rows. Their shifts are small in absolute terms (roughly
70 ns at one seat and 1.6 µs at 50 protocol-13 seats), include identical
instrumentation overhead and were not attributed to a specific instruction or
cache mechanism. The predeclared gate still rejects this implementation. There
is no post-result threshold change, retry, layout tuning or runtime adoption.

## Maintainer decision

**Post-result adoption by the maintaining agent; not the study's verdict.**
The maintaining agent overrides the adoption gate for #274 while preserving
the frozen study's rejection, results and tables. The whole-window short-world
costs are about 70 ns per tick at one seat and 1.6 µs at 50 protocol-13 seats,
below 0.03% of the 7 ms tick budget. The target mature-500 world saves about
265–400 µs per tick (14% stock, 18% protocol 13); young-500 saves about 20%.

The maintaining agent judges those absolute short-world costs acceptable,
citing the relative 0.6–1.0% short-world costs accepted for mature wins in
[#252](https://github.com/NikitaMGrimm/slither-sim/pull/252) and
[#255](https://github.com/NikitaMGrimm/slither-sim/pull/255), and the simpler
single-vector structure compared with the previous two-vector layout.

No retry, tuning or threshold change is applied to the study. Its verdict
remains rejection under its own gate. The measured implementation is adopted
on main after #273, with only a short sorted-merge explanation restored in the
runtime source. `measured-bounded.patch` preserves the historical measured
snapshot under its new name; it was previously `rejected-bounded.patch`.
Integrated-head verification is recorded separately, without new timing.

## Method and provenance

The [plan](PLAN.md), [pilot selection amendment](SELECTION.md) and
[baseline refresh](BASELINE-REFRESH.md) precede primary observations. Final
measured runtime pins are base `e860ec237f172de5df34caff86a9eb26082e82d6` and candidate
`d781ff000d2dfee10627ab1b8a4c07bfff3638c0`.
[Refreshed build pins](refreshed-build-pins.json) were committed before timing.
All compilation, runtime/output checks, measurements and numerical reductions
run on a 4-vCPU Zen 5 EPYC KVM guest. Rust 1.99 portable release code generation
uses thin LTO and one codegen unit, without affinity, native CPU targeting or
profiler sampling.

Main's later #273 landed after the counted series. It is preserved in the
research-only delivery; no measurements here are attributed to that runtime.

The [earlier full-backend fixture](../2026-10-28-mature-covered-bounds/fixture.rs)
warms once per runtime/protocol at full speed to tick 193,000, with 500 remote
recipients and no sockets or wall-clock pacing. A physics checkpoint cannot
restore its presentation and recipient state. Every child window forks the
same complete frozen backend. Complete warm streams are compared live with
bounded buffers. Repeated child and direct-parent continuations compare full
state/action/frame streams, wire payloads and terminal summaries.

Each row has one discarded AB and five AB/BA/AB/BA/AB pairs of 1,080 advances.
Actual food-epoch changes separate 1,065 ordinary and 15 maintenance ticks.
Young-500 controls start at tick zero; 1/10/50-seat controls warm for 600 ticks
and omit the fixture's 500-seat-only disconnect recipe. They use the same pair
count and exact-output gates. No result-driven retries or threshold tuning are
allowed. Parent state and lifecycle summaries must agree across arms.

Grouped current-thread user counters measure instructions, cycles, L1d read
misses and dTLB read misses. Values are matched by event ID, and every interval
must schedule at least 99% of its enabled time. CPU brackets `advance_tick_with`;
policy/admission preparation, frame draining and serialization remain outside.
Identical clock/counter overhead is included without correction. A KVM dTLB
event is not equated with a hardware page walk; these selectors do not identify
memory latency, bandwidth or a deeper cache level.

Tables use medians of the five run means and medians/ranges of their paired
candidate/base ratios. Ranges are observed spread, not confidence intervals.
CPU p99 is the median of five per-window p99s, not a pooled distribution or a
live-host tail estimate. This is a synthetic simulation-thread result; it does
not establish socket CPU savings, real-player capacity or performance on other
CPUs.

## Earlier attempts

The disclosed three-pair stock pilot used base `5f88748f` and two compact
prototypes. Separate keys/records with binary lookup had median CPU ratio
0.869622 [0.775949, 0.893694], versus 0.800889 [0.740271, 0.863048] for one
vector with unrestricted linear lookup. Instruction ratios were 1.029076 and
1.021633. Both passed focused oracles, three runtime mutations, restored-source
checks and short stock/13 stream proofs. All three stock warm streams matched
through tick 193,000, followed by repeated child and direct-parent proofs.
Ranges overlap; these observations select a prototype and do not establish its
general superiority. The [separate-vector patch](separate-vectors.patch) and
[unrestricted-linear patch](unrestricted-linear.patch) retain both unlanded
implementations. Their numbers do not measure the bounded final design.

The first primary proof cohort (`5f88748f` / `6ab7b130`) passed the Rust group,
27 traces, mutations, both complete mature warm streams and all short proofs.
Main then landed #272 before any primary timing. Those parents were closed
through direct continuation proofs with `timedPairs: 0`; every source, binary,
trace, receipt and log was archived, verified and retained. Its original
[build pins](build-pins.json) remain superseded provenance. No pilot or
superseded-primary observation enters the final-primary table.

## Reproduction and access

[Reproduction](REPRODUCE.md) documents the exact prerequisite receipt producer,
mutations and source restore, imports from earlier studies, warm-stream and
continuation checks, counter scope, pairs, gate and cleanup. The measurement
overlay is retained as [harness.patch](harness.patch); the unlanded runtime and
its regression oracle are retained as a separate rejected patch. Raw streams,
sources, binaries, counters, inventories and
failed attempts remain local-only in verified ignored-output archives.
[Archive identities and access](raw-access.json) identify that material without
making it publicly downloadable.
