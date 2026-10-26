# Known-snake enumeration, 2026-10-26

`KnownSnakes` now keeps ascending occupied slots alongside direct lookup.
Insert/remove maintain membership; replacement and field updates preserve it.
This removes the per-recipient scan of every world slot, retaining packet order.
The old iterator accounted for 11.43% of sampled official-500 user CPU.
Mechanics, radius, admission and thread ownership are unchanged.

## Method

Baseline main: `ce54c607ad5d98a633f2411ff5f0ea1e6bd2a874`. Candidate runtime:
`fd95522cbb082f815dbd9ffdefaeafe35b5f99de`; subsequent commits are evidence/docs.
[Source pins](source-pins.json) bind archives, patches, toolchain and copied
binaries; the [plan](PLAN.md) and its amendments precede their measurements.

Shared WSL2 Linux x86-64, Rust 1.99.0, independent locked release targets,
thin LTO, one codegen unit, debug level 1. All host threads shared CPU 0;
three load shards used CPUs 2/4/6. Builds used two jobs, tests two threads.
Simulation/broadcast stayed on their existing single thread. Only synthetic
loopback traffic was used; no outside game server or cloud compute.

Each fresh seed-7 host had no bots and a 7 ms tick. Official used 500 seats
and the unchanged capacity load. Battledome used its default fixed radius
3,584, 40/64/80 seats, original avoidance/food steering and staggered boost
bursts (3 seconds every 8). Both arms shared each baseline-built load binary.
Waiting logins remained connected until admission/disconnection/hold end;
generator handshake limits/setup timeouts did not suppress them. Server
admission was untouched.

After an untallied pair, five adjacent alternating pairs used 15 s warmup and
45 s measured holds per arm. Host process user+system CPU from `/proc` was
divided by executed ticks. CPU is quantized to 10 ms; sampler and probe
boundaries differ by a polling interval. Builds/tests/traces preceded timing.
Arm medians and medians of paired ratios are different summaries.

## Selected results

CPU, alive/waiting counts and excess quantiles are medians of five holds per
arm; deferred ticks are totals across those holds. Excess is receipt minus
cumulative server header, normalized by each connection's minimum in the hold.
It includes generator polling and the 1 ms header grid, and measures admitted
streams, not admission wait or absolute latency.

| Sockets | CPU ms/tick main → candidate | Paired ratio median (range) | Alive main → candidate | Waiting main → candidate | Excess p50/p99 ms main → candidate | Deferred main/candidate |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Official 500 | 7.029 → 6.556 | 0.9390 (0.8885–1.1486) | 499.94 → 499.99 | — | 586.03/1207.34 → 682.77/855.98 | 900/993 |
| Battledome 40 | 3.203 → 2.684 | 0.8621 (0.7746–1.0784) | 32.71 → 36.09 | 6.95 → 3.60 | 4.25/26.44 → 3.22/12.23 | 0/0 |
| Battledome 64 | 2.859 → 2.836 | 1.0653 (0.8406–1.2123) | 34.10 → 35.04 | 29.54 → 28.63 | 3.88/12.93 → 3.69/10.74 | 0/0 |
| Battledome 80 | 3.132 → 3.047 | 0.9659 (0.9336–1.0232) | 35.51 → 35.96 | 44.13 → 43.71 | 4.04/12.95 → 3.99/18.85 | 0/0 |

Spawn waits are client-observed login-send to setup-receipt, including service
delay. Completed quantiles select admissions inside the hold. Unfinished waits
are right-censored; their durations include prior warmup and are lower bounds
on eventual admission time.

| Battledome sockets | Completed wait p50/p99 seconds main → candidate | Median waiting at end main/candidate | Longest censored wait seconds main/candidate |
| ---: | ---: | ---: | ---: |
| 40 | 4.76/16.58 → 2.95/16.61 | 6/6 | 8.36/7.84 |
| 64 | 21.65/35.86 → 22.16/36.31 | 32/32 | 23.50/24.94 |
| 80 | 33.74/48.64 → 33.95/50.30 | 48/47 | 37.26/35.83 |

[CPU](cpu-results.json) and [instruction](instruction-results.json) summaries
retain arm medians, paired ratios and ranges. [Battledome summaries](dome-results.json)
retain population and excess/wait p50/p99/max. No generator error was recorded.
Complete holds, wait intervals, profiles/stacks, binaries and failures remain
**local-only in scratch**, including the pre-trim record. Hashes identify them
without making them independently retrievable.

Separate profiles used `cpu-clock:u`, 199 Hz, DWARF/16384-byte stacks, the
capacity study's category rules, Rust demangling and crate-hash normalization.
Official-500 main → candidate shares: encoding 43.35 → 38.09%, visibility/food
24.99 → 28.04%, physics 18.12 → 19.57%, framing 5.86 → 6.01%, socket service
3.92 → 4.02%, allocation 2.49 → 3.03%, other 1.28 → 1.25% (4,542/3,930 samples).
These are user CPU only; kernel work is excluded. Inlining/unwinding limit
attribution. Sample-share changes are not speed measurements.

Battledome physics includes admission: shares were 89.10 → 89.14% (40),
88.06 → 88.42% (64), 88.82 → 87.63% (80). Inclusive `admit_waiting` shares
were 90.39 → 90.10%, 89.36 → 89.56%, 90.53 → 88.72%, overlapping categories.
[Official](profiles.json) and [per-seat](dome-profiles.json) summaries retain
all phase shares and sample counts.

The exploratory instruction diagnostic used one untallied pair and three
alternating pairs of 100 deterministic decisions, counters at least 99%
scheduled. Whole-process user instructions include construction/internal
warmup and exclude real sockets/kernel work. Median ratios were 0.96267
(official-500), 0.99870 (40), 0.99897 (64), 0.99888 (80). The in-process
Battledome fixtures use synthetic initialization and responsive delivery,
separately from the live host's default delivery.

## Checks, limits and reproduction

All six host-efficiency traces and three count-specific Battledome comparisons
matched every decompressed byte; 64 is repeated ([identity](identity.json),
[Battledome identity](dome-identity.json)). Retained traces include reconnect
at decision 192 and independent decoder replay. These finite windows do not
expose all private queues/RNG or assert equal live receipt times.

The membership regression covers insertion/replacement/removal, high-slot
reuse, mutation and cloning. Full CONTRIBUTING Rust checks passed on Linux;
Windows passed the regression, all 15 socket-host tests and formatting.
Documentation checks passed. macOS was not run locally.

Four official-500 CPU pairs favored the candidate; one regressed 14.86%.
Every workload range crosses 1; the 64-socket paired median favors main.
These descriptive results have no confidence interval. Frequency/cache/WSL
effects remain; live inputs, admissions and deaths differ between holds.
Median visible peers were only about 5.5–6, with roughly 36 alive at 80
requested sockets: the intended dense 80-living-player regime was not exposed.
Admission dominates those holds; tiny fixed-fixture instruction savings cannot
explain their live CPU swings. No native one-core, dense-capacity or 500-client
real-time claim follows. Long snakes, slow readers, TLS and Internet paths
were not timed. Remaining official costs are encoding, visibility/food and
physics; kernel work needs native profiling.

Build pinned roots with two jobs and capacity probe/load examples copied into
`examples/`. Copy this study into each root and apply [host-players.patch](host-players.patch)
to the capacity load, producing `host_players.rs`; apply the [catalog patch](battledome-catalog.patch)
to `perf_baseline.rs`, producing `perf_battledome.rs` (`patch -o OUTPUT ORIGINAL PATCH`).
Build locked release binaries with debug level 1, then copy outside targets.
Use `trace_compare.py` (also `--battledome`), `paired.py` (also
`--load BASE_BIN/host_players --count N --preset battledome` for N=40/64/80),
`loopback.py --profile PERF_BINARY` and `instructions.py --perf PERF_BINARY`.
Their `--help` gives positional arguments; always use fresh output directories,
`CARGO_BUILD_JOBS=2`, `RAYON_NUM_THREADS=2` and the stated affinity budget.

Setup failures (path conversion, task-local perf libraries, missing Rust tools
and study-example Clippy warnings) remain in scratch. No expectations or
safeguards were weakened.
