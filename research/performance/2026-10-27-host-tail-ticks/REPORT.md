# Legacy minimaps and tail ticks, 2026-10-27

Keep: share checked M/V conversion and U encoding within each broadcast.
Main `fa92c47` → measured runtime `20bd42d`. [Selected results](results.json).
Recipient baselines, anchors, release gates, frame order and budgets stay local.

WSL2 Linux x86-64, Rust 1.99 locked release/debug=1; jobs/Rayon/tests=1.
Host threads CPU 0, drain-only scripted socket load CPU 2, no bots. Census:
15s warmup/20s hold per arm/case, user instructions through handoff, separate
socket counter, >=99% scheduled. Nested phases use exclusive remainders.
Median is mathematical; tails use nearest ranks. These finite, instrumented,
different live trajectories support no CPU, latency or capacity claim.

Main census, millions of simulation-thread instructions/tick:

| Workload | n | p50 | p99 | p99.9 | max | >2×median | Socket mean |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| official500 stock | 2609 | 12.51 | 23.28 | 26.70 | 28.31 | 0.575% | 1.786 |
| official500 half legacy | 2037 | 12.93 | 25.21 | 59.63 | 62.95 | 0.982% | 2.042 |
| dome64 stock | 2858 | 1.40 | 83.10 | 87.38 | 88.31 | 35.34% | 0.229 |
| dome80 stock | 2858 | 1.70 | 96.27 | 100.10 | 101.50 | 30.62% | 0.289 |
| dome80 legacy | 2820 | 1.83 | 93.40 | 100.72 | 138.21 | 31.81% | 0.336 |

Admission dominates 979/835/864 heavy Battledome ticks, outside this change's
ownership. Mean playing populations 41.70/50.15/50.40, waiting 21.08/28.72/28.25;
completed wait p99 including warmup 6.86/8.72/7.40s. These socket workloads have
partial populations. Shrink culling averages 28/33/32K instructions,
max 39.4K. Food maintenance every 72 ticks dominates official stock's 15 heavy
ticks. Legacy U dominates 6 mixed/6 dome heavy ticks. On main, boards/death each stay
below 0.41M; rules emissions zero; food-vector capacity changes 0–2 ticks. Other allocation
remains inside its phase. JSON assigns every heavy tick its largest phase.

Map-due U+decode means fall 44.06→0.174M (mixed) and 33.81→0.767M (dome legacy),
with one U call per raster. Their observed maxima become 27.79/94.55M;
Battledome's admission spikes remain. Socket work stays separate; differing
populations prevent interpreting its live differences as marginal legacy cost.

Uninstrumented whole-process instructions:u: one warmup then five alternating
pairs, construction/admission/1080 ticks/control/drain included, sockets/kernel
excluded. Official uses 500-seat ecology; synthetic dome starts at 2×seat radius
then shrinks normally. All seats admit; forced reconnect at tick 720; three map epochs.

| Fixture | Paired reduction | Range |
| --- | ---: | ---: |
| official500 stock | 0.026% | 0.011–0.035% |
| official500 half legacy | 0.709% | 0.694–0.731% |
| dome64 stock | 0.028% | 0.005–0.032% |
| dome80 stock | 0.028% | 0.004–0.041% |
| dome80 legacy | 3.516% | 3.500–3.551% |

Stock shifts are negligible. Amortized extra instructions per replaced recipient
per tick: mixed 2494→1992, dome 3070→1312, including setup/framing. Their full
world checkpoints match across protocols. Remaining conversion/framing means
and waits/censoring are selected in JSON; censored ages are lower bounds.

Five complete raw-protocol traces plus nine retained stock traces match bytes;
frames/times, initial/final checkpoints, terminal records and lifecycle gates
checked. Rust group/docs pass; wrong-baseline-key negative control fails.
Windows/macOS unrun locally. Rejected tiny/obsolete admission fixtures and
outdated challenge-solver hold remain local; no gate relaxed or pairs discarded.

From a full Linux checkout, run [reproduce.py](reproduce.py) modes build, trace, pairs with REPO,
NEW_OUTPUT and --perf PERF. Nine retained cases use the
[prior runner](../2026-10-27-host-sector-union/reproduce.py) with the above pins.
Raw rows, diagnostic overlays, traces, binaries and all attempts are local-only.
