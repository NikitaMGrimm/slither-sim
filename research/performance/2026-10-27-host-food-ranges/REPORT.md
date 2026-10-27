# Recipient food hint ranges, 2026-10-27

Keep: bound sorted hints to each recipient's held range. Snapshot guards retain
full recovery; packet order is unchanged.
Measured runtime: `65ff713` -> `8a14367`; delivery head adds docs.
[Pins, medians, paired ranges and twelve trace hashes](results.json).

WSL2 Linux x86-64, Rust1.99 locked release/debug=1, thin LTO, one codegen unit.
Jobs/Rayon/tests one, <=2 pinned CPUs. Offline synthetic official-500.

## Fresh profile

Mean million user instructions/tick, including diagnostic overhead. Same853
ticks: tick>=120, all500 alive, reconnect720 excluded; PMU>=99% scheduled.
Both diagnostic arms match unprofiled control traces.

| Phase | Stock | Half13 | All13 |
| --- | ---: | ---: | ---: |
| Physics | 3.998 | 4.004 | 4.014 |
| Snake updates | 1.898 | 1.881 | 1.875 |
| Shared coverage | 1.388 | 1.389 | 1.398 |
| Recipient food | 1.594 | 1.592 | 1.590 |
| Shared food | 0.857 | 0.857 | 0.857 |
| Recipient prey | 0.922 | 0.921 | 0.921 |
| Framing, including conversion | 0.454 | 0.845 | 1.207 |
| Other | 2.059 | 2.088 | 2.109 |
| Total | 13.170 | 13.578 | 13.971 |

Recipient snake/food/prey ceilings1.90M/1.59M/0.92M are not predicted savings.
Snake deltas, food listings, coverage and default13 M/V are already shared.
Food's unrelated searches were the largest safe redundant loop found;
recipient food falls1.594M->0.618M. Conversion alone is0.166M/0.312M.

## Comparisons

One warmup pair, five alternating instructions:u pairs, >=99% scheduled,
no exclusions. Whole process includes setup,1080 ticks/drains, excludes sockets.

| Official500 | Base/candidate billion | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 14.857/13.842 | 6.832% (6.814–6.846%) |
| Half13 | 15.386/14.377 | 6.566% (6.516–6.606%) |
| All13 | 15.905/14.892 | 6.369% (6.364–6.391%) |

Live: same pair plan, 10s warmup/12s hold, host one CPU, load/sampler another.
Main+sim+socket user instructions/tick medians fall1.98%/1.60%/1.26%; legacy
ranges cross zero. Mean alive498.951–499.515; no load errors, PMU>=99%.
Evolving worlds make these descriptive. Phase/fixture/live denominators differ.

Twelve complete traces match order, times, checkpoints and RNG endpoints.
Raw cases: 500 final/499 minimum alive, three rejoins, no refusals. Rust group,
16,384-subset oracle and endpoint negative control pass, plus
pickup/decorated/recovery regressions. Independent source-only review clean.

## Reproduce / limits

Export base.tar/candidate.tar at the full pins. Linux PMU, Python3.12+, Rust1.99,
patch required. Run `reproduce.py MODE ARCHIVES OUT` serially: build, trace, count
(--perf PERF), sockets, pairs; separate arm targets/copied binaries. Diagnostics:
`profile.py ARCHIVE OUT --control BINARY`. Python-O refused.
Raw evidence is local-only. Timing contaminated; kernel instructions unavailable
at perf_event_paranoid2. No CPU-time, latency or capacity claim; no security changes.
Arena vision may need food.rs/test reconciliation; broadcast/snakes and pickup
are untouched.
