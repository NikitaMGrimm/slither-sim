# Skip unchanged snake delta work, 2026-10-27

The recipient-snake phase remains the largest recipient-side aggregate after
#223. Its existing cache already shares peer packets by previous sent state,
but every known-snake call quantizes wanted heading before consulting it, and
empty changes still walk/copy history. The candidate returns early when front,
length and kills match and no open motion/mass gate needs a report. Nonempty
misses evaluate wanted heading lazily; hits avoid that calculation too.
The caller still observes every successful history for stale-report detection.
Signed-zero front bits retain the uncached function's assignment.

Measured runtime: `ef866d73259e35fe8659a1996d7b97c36d511e0b` ->
`be888d5e0accd0e07fc3d875e4e031b273265656`. [Selected results](results.json).
WSL2 Linux x86-64, locked Rust 1.99 release/debug=1, thin LTO/one codegen unit,
default target. Jobs/Rayon/tests one, at most two pinned CPUs; offline
official-500, seed 7, no bots. No physics, food, prey, socket or protocol changes.

## Profile and choice

Coarse user-instruction brackets include probe overhead. Same 853 steady ticks:
tick>=120, all 500 alive, forced reconnect 720 excluded, PMU>=99% scheduled.
Diagnostic traces match unprofiled controls. Million instructions/tick:

| Phase | Stock base | Half13 base | All13 base | All13 candidate |
| --- | ---: | ---: | ---: | ---: |
| World step | 2.346 | 2.351 | 2.362 | 2.365 |
| Shared food | 0.857 | 0.857 | 0.858 | 0.858 |
| Reports | 0.220 | 0.220 | 0.220 | 0.220 |
| Coverage bounds/grid | 0.135 | 0.135 | 0.135 | 0.135 |
| Candidate merge | 0.414 | 0.414 | 0.414 | 0.414 |
| Snake updates | 1.899 | 1.894 | 1.892 | 1.571 |
| Recipient food | 0.618 | 0.617 | 0.617 | 0.617 |
| Recipient prey | 0.677 | 0.677 | 0.677 | 0.677 |
| Emission, including conversion | 0.467 | 0.697 | 0.911 | 0.911 |
| Of which legacy conversion | 0.000 | 0.184 | 0.356 | 0.357 |
| Other prep/bookkeeping | 1.365 | 1.391 | 1.402 | 1.396 |
| Total | 8.998 | 9.253 | 9.488 | 9.165 |

The finer baseline census sees 2,665.6 known-snake delta calls/tick; 1,932.9
(72.5%) are exact no-ops, including 363.0 own-snake calls. There are 1,660.6
peer-cache hits, 1,005.0 uncached calls and 2,667.6 eager wanted calculations
(including appearances). Wanted scopes minus matching empty probes suggest
about 0.203M/tick in eager quantization; observe bookkeeping is only about
0.045M/tick after the same correction. Fine probes materially perturb the
aggregate; these are diagnostic bounds, not measured unprofiled savings.
Skipping safe no-ops and unnecessary wanted work has no persistent new state.

## Alternating pairs

One untallied warmup then five alternating whole-process instructions:u pairs,
PMU>=99%, no exclusions. Includes setup, all 1080 ticks and client drains;
excludes sockets/kernel. Raw fixtures use controls every six ticks and reconnect
at 720; minimum 499 alive, terminal 500, three rejoins, zero refusals.

| Official500 | Base/candidate billion | Reduction (range) |
| --- | ---: | ---: |
| stock | 10.379 / 10.031 | 3.353% (3.351-3.354%) |
| half13 | 10.732 / 10.384 | 3.242% (3.241-3.244%) |
| all13 | 11.068 / 10.720 | 3.144% (3.144-3.146%) |

Supplementary U5/U250/U500 paired reductions are 3.144%/3.144%/3.144%; their ranges are retained in selected JSON. Phase and whole-process denominators differ.

Descriptive loopback holds use one warmup then five alternating pairs per mode,
10s warmup/12s hold, host CPU0 and load/sampler CPU1. Whole-host user
instructions/tick show median reductions of 3.44% stock (range 2.62-6.67%),
3.57% half13 (3.24-4.72%) and 3.95% all13 (0.67% increase to 6.44% reduction).
The all13 live range crosses parity. Measured playing means span
499.031-499.424; generator busy is at most 67.34% of its core. All 36 selected
holds pass error/PMU gates. One earlier all13 pair-0 base hold failed with two
TCP connect timeouts; it is retained locally and summarized in selected JSON.
The unchanged harness resumed that pair. Evolving worlds and shared scheduling
make these supplementary, not a capacity or consistent live-speed claim.

## Correctness, reproduction and limits

Fifteen complete traces match, including frame bytes, boundaries and elapsed
allocation, checkpoints, RNG endpoints and terminal summaries. The full Linux
Rust group and eight snake module tests pass. A 4,608-case uncached oracle
covers fronts, lengths, motion masks, mass, kills, gates and own/peer/cache-hit
paths, checking packet prefixes and post-state including front bits. Additional
tests cover signed zero, lazy evaluation, errors and partial state. Removing
the mass gate, front assignment or length guard fails mutation controls; the
restored tests pass. Both production files match the tested archived candidate.
A transient WSL command-launch timeout occurred before controls started; its
retry completed. All five native pairs are retained.

Export base.tar/candidate.tar at the full measured pins. Run `reproduce.py MODE
ARCHIVES OUT`: build, trace, count (--perf PERF), sockets, pairs. The frozen
six-case fixture includes stock/half13/all13 and U5/U250/U500, plus nine
historical catalog cases. `profile.py ARCHIVE OUT --cpu 1 --control BINARY`
reproduces coarse brackets; add `--fine` for the baseline-only census.
Python 3.12+, Rust 1.99, patch and Linux user PMU are required. Scripts refuse
-O; separate arm targets/copied binaries are rehashed around measurement.
Raw traces, failures, logs and preliminary screen remain local-only outside /tmp.

Arena-vision's overlap is limited to the known-snake delta call in broadcast.rs
and the cache method in snakes.rs. CoverageGrid, covered/near and arena handling
are unchanged.
Shared-machine timing is contaminated; perf_event_paranoid remains 2, kernel
instructions unavailable. No CPU-time, latency or native capacity claim.
