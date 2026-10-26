# Shared prey visibility candidates, 2026-10-26

Live CPU is inconclusive: every workload's paired range crosses 1. The fixed
official-500 fixture retires 4.60% fewer user instructions. Neither result
establishes a capacity increase.

Prepare one world-ordered array of conservative prey axis bounds per broadcast.
Recipients reject distant bounds, then retain the exact `distance <= vision`
comparison. Half-width `max(1, 2*vision)` leaves rounding slack. Sorted ID output
and first-visible duplicate selection stay unchanged. No persistent cache,
mechanics, radius, admission, socket-service or thread-ownership changes.
Frozen profiles put all-caller `hypot` self share at 11.57 → 5.64%.

## Method and parity

Frozen main baseline `249cdb217d83a234895bc7eee4d98c3636c1cd76`; measured runtime
`fde812bb4c5c6dea52ebb32a8341d98fe45ea76b`. [Plan](PLAN.md) predates timing;
[pins](source-pins.json) identify archives, copied binaries and shared harness.
The PR integrates main `c012a723`, including #168. Only supplemental profiles
cover that combination; no paired CPU gain is claimed. Prey source is unchanged.

Shared WSL2 Linux x86-64; Rust 1.99.0, locked release, thin LTO, one codegen unit,
debug level 1, independent targets and copied executables. At most four logical
CPUs: every host thread on CPU 0, three generators on CPUs 2/4/6. Build jobs,
test and Rayon threads limited to two. Simulation/broadcast retain their single
thread. Synthetic loopback only; no cloud or outside game server. Unrelated
desktop work was not stopped. Full samples remain local-only in scratch.

Use #168's pinned probe, adaptations and runners at `489a6b0`. Each seed-7 host
has no bots and uses the default official or Battledome preset. Battledome
clients steer and boost in staggered 3-second bursts every 8 seconds, keeping
pending admission connected. Both arms use the same baseline-built generator.

All nine full decompressed comparisons passed: portable/protocol (32
operations), official-500 (1 decision after warmup), Battledome-40/64/80 (4),
retained step_all/next_frame (384); 64 repeats in the extended catalog.
[Identity summaries](identity.json) supplement byte comparison. Retained replay
includes reconnect and independent decoder replay. These finite traces exclude
some private queues/RNG and live receipt timing. The in-process Battledome
fixture uses synthetic initialization and responsive delivery, separately from
socket admission/default delivery.

Regressions check every eligible index, exact/adjacent floating-point edges,
extreme coordinates/vision, duplicate IDs, world order and rebuilding after
motion. Full Linux Rust checks passed at the measured pin; Windows passed both
visibility tests and all 15 socket-host tests. Independent source-only review
found no actionable defects; it did not rerun checks. macOS was not run locally.
The PR lists subsequent integration checks separately.

## Selected results

Five adjacent alternating pairs follow one discarded pair. Each hold has
15 seconds of warmup, then 45 measured seconds (discarded holds: 15 seconds).
Process user+system CPU/executed tick includes host/probe work and excludes
generators. `/proc` CPU has 10 ms quantization; probe/resource boundaries differ
by a poll. Tables use arm medians of hold summaries; paired ratios are distinct
summaries. Alive/waiting are client time-weighted states. All paired ratios,
arm ranges and population ranges remain in [CPU summaries](cpu-results.json).

| Workload | CPU ms/tick main → candidate | Paired median (range) | Alive main → candidate | Waiting main → candidate | Deferred total main/candidate |
| --- | ---: | ---: | ---: | ---: | ---: |
| official-500 | 6.528 → 6.499 | 1.0008 (0.9693–1.0667) | 499.99 → 499.99 | 0.00 → 0.00 | 50/55 |
| dome-40 | 2.846 → 2.890 | 1.1050 (0.9628–1.3074) | 35.18 → 35.16 | 4.49 → 4.52 | 0/0 |
| dome-64 | 2.969 → 2.934 | 0.9852 (0.9609–1.1634) | 33.72 → 34.35 | 29.94 → 29.30 | 0/0 |
| dome-80 | 3.392 → 3.167 | 0.8585 (0.7528–1.0555) | 33.49 → 33.91 | 46.22 → 45.78 | 34/0 |

Frame excess is receipt minus cumulative elapsed header, normalized by each
connection's hold minimum and pooled over admitted receipts. It includes
generator polling/decoding and the 1 ms header grid, not absolute latency or
admission delay. Values are medians of hold-level quantiles. Waits run from
login-send to setup-receipt; unfinished intervals are censored at hold end,
including prior warmup exposure. Empty wait holds are omitted from wait medians;
counts, p50/p99/max and censoring remain separate in JSON. Censored maxima are
lower bounds on eventual admission, not completed waits.

| Workload | Excess p50/p99 ms main → candidate | Completed wait p50/p99 ms main → candidate | Max censored wait seconds main/candidate |
| --- | ---: | ---: | ---: |
| official-500 | 9.69/249.90 → 9.28/201.05 | — | — |
| dome-40 | 3.82/13.99 → 3.79/12.99 | 3738.38/19140.14 → 3367.21/16726.18 | 4.80/6.89 |
| dome-64 | 3.89/14.08 → 3.80/11.31 | 23918.45/37931.45 → 23410.40/37104.38 | 28.57/27.07 |
| dome-80 | 4.47/31.17 → 4.13/15.33 | 34048.96/48233.98 → 34577.98/46745.27 | 37.38/44.56 |

Separate profiles: `cpu-clock:u`, 199 Hz, DWARF/16384-byte stacks. Inherited
call-chain categories exclude kernel work; inlining/unwinding limit attribution.
Prey visibility is included in encoding. Official main → candidate shares:
encoding 42.82 → 39.98%, visibility/food 26.61 → 29.27%, physics 13.66 → 14.23%,
framing/flush 6.96 → 6.67%, socket service 4.55 → 4.97%, allocation 4.12 → 3.83%,
other/unknown 1.26 → 1.05%. [Profiles](profiles.json) retain all workloads and
profile-hold populations. Shares are not speed measurements; physics includes
Battledome admission.

A separate integrated-runtime profile (`bf5e29c`) has encoding 36.72%,
visibility/food 30.76%, physics 14.95%; Battledome physics/admission is
88.93–90.22%. This unpaired snapshot updates remaining costs, not speed.

Instruction diagnostic: one discarded pair, then three alternating pairs of
100 deterministic decisions, including construction/internal warmup; at least
99% counter scheduling. No sockets or kernel work. Paired median ratios:
official-500 0.95404; Battledome-40/64/80 0.99952/0.99969/0.99971.
[Instruction summaries](instruction-results.json) retain arm medians/ranges and
all paired ratios. These fixtures are not a player distribution.

## Limits, failed attempt and reproduction

All live CPU ranges overlap no change; there is no confidence interval or
native capacity grade. Shared-desktop frequency/cache/WSL effects remain after
using process CPU. The v0.6 host starts at the bots-only population (zero here);
prey targets grow with occupied seats and refill at one per second. Fixed
fixtures start at their full population. Cold-start holds do not match that
ecology; no live prey census was recorded. Inputs, admission and deaths differ
between holds. Pending admission means these rows do not expose 80 living
players. Admission belongs to the separate workstream. No generator errors
were recorded; busy-wall maxima and observed peer density remain in JSON.
Long snakes, slow readers, TLS, Internet paths and other configurations were
not timed. No one-core real-time or capacity improvement claim follows.

The first grid (`3f54b24`) passed checks/parity but was rejected for extra
hashing/memberships/allocation without an established live benefit. Its five
official CPU ratios were 0.85345, 1.03448, 1.07758, 1.14174, 0.93487; median
1.03448. Its Battledome series stopped in an untallied warmup, not a completed
result. The [patch](rejected-grid.patch) is retained; raw data/builds stay in
scratch. Bounds started fresh builds/checks/parity and all four workloads.
No completed pair was discarded to improve a result.

[Reproduce](REPRODUCE.md) with pinned shared scripts/patches and fresh outputs.
Full traces, stacks, samples, binaries, logs and screens remain local-only.
The first focused compile failed on a closure lifetime annotation; the corrected
index-based closure passed before builds/parity/timing.
