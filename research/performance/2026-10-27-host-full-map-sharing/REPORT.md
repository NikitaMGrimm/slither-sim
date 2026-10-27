# Shared opt-in legacy full maps, 2026-10-27

Keep: share [99,3] U decoding/projection/encoding by exact projection bits,
retained baseline identity and input bytes. Intern snapshots so histories
converge; clear each broadcast. Recipient framing/state/RNG stay exact;
default [99,4] keeps M/V.
Measured runtime `6769d71` -> `71b4df1`; delivery integrates research-only main
`16001f2`. Runtime source equals the measured candidate. [Selected results](results.json).

WSL2 Linux x86-64, locked Rust 1.99 release/debug=1, thin LTO/one codegen unit.
Jobs/Rayon/tests one, at most two pinned CPUs. Offline official-500, seed 7, no
bots, 1080 ticks, controls every 6, reconnect 720. Five alternating pairs after warmup; PMU>=99%, no excluded runs. Whole-process instructions:u
include setup
and drains. Simulation-thread counts cover advance_tick_with; same 853 steady
ticks: tick>=120, all 500 alive, reconnect 720 excluded. Map due: 357/715/1073.

All recipients below are legacy. Million instructions/tick, arm medians:

| U clients | Due mean base -> candidate | Paired change | Due max base -> candidate | Other mean base -> candidate |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 11.827 -> 11.837 | +0.10% | 13.144 -> 13.152 | 10.864 -> 10.874 |
| 5 | 15.215 -> 12.525 | -17.68% | 16.534 -> 13.814 | 10.862 -> 10.869 |
| 250 | 181.527 -> 12.591 | -93.06% | 183.752 -> 13.883 | 10.865 -> 10.868 |
| 500 | 351.188 -> 12.644 | -96.40% | 354.280 -> 13.923 | 10.862 -> 10.869 |

Candidate peak is 24.05M at tick 575, also the maximum over all 960 post-warmup
ticks including reconnects/dead slots. The cross-case baseline U increment is
about 0.679M instructions/recipient/due tick. Copies remain per recipient.

| Whole fixture | Paired instruction change (range) |
| --- | ---: |
| Stock | -0.032% (-0.050 to -0.018%) |
| Half default13 | +0.025% (+0.016 to +0.043%) |
| All default13 / 0 U | +0.064% (+0.050 to +0.099%) |
| All13 / 5 U | +0.014% (-0.027 to +0.020%) |
| All13 / 250 U | -3.702% (-3.735 to -3.690%) |
| All13 / 500 U | -7.211% (-7.218 to -7.189%) |

Retain small default-path increases; five U clients improve the burst without
a resolved whole-fixture gain. The 0-U due change crosses zero across pairs.

Socket pairs (10s warmup/12s hold, host on one CPU, load/sampler on another):
whole-host user instructions change -1.15%, -0.13%, -3.27%, -7.50% at 0/5/250/500 U;
ranges are selected. First two cross zero. All 48 holds pass map-family/error/PMU
gates. Evolving worlds make these descriptive.

Fifteen complete traces match: six protocol/option cases plus nine historical
cases, including bytes/boundaries/times/checkpoints/RNG. Worlds match: minimum
499 alive, terminal 500, three rejoins, no refusals. U message
counts 0/15/750/1500; complementary M/V counts 1500/1485/750/0.
Full Linux Rust group, 14 encoder tests and 5 map tests pass. Removing input
matching, interning or clearing fails regressions; restored tests pass. Source
review found only a test-count expectation error, corrected before measurement.

Export base.tar/candidate.tar at selected pins. `reproduce.py MODE ARCHIVES OUT`:
build, trace, count (--perf PERF), sockets, pairs; then `mapticks.py OUT`. Linux
user PMU, Python 3.12+, Rust 1.99, patch. Separate targets/copied binaries; -O
refused. Raw evidence is local-only outside /tmp; loader type-error logs retained.
One seed/geometry; shared timing contaminated. Kernel instructions unavailable,
perf_event_paranoid stays 2. No CPU-time, latency or capacity claim.
