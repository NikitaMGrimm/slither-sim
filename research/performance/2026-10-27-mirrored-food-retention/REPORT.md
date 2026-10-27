# Mirrored radius food retention, 2026-10-27

Keep mirrored retention. Main `339041fec0ebde58e5da0b544a6ab2cc6c720850` ->
candidate `3e22127756da910488ac3f4dbb998806d87a749d`. This addresses repeated
warm rebuilds identified after the [small-server study](https://github.com/NikitaMGrimm/slither-sim/pull/222).
Cold first-tick pellet-index construction remains present. Production edits
are confined to `simulator.rs` and `food_grid.rs`; socket, broadcast and snake
coverage/update files are untouched. [Selected results and pins](results.json).

## Preserved behavior and cache invariant

Both original radius guards and the strict `distance_lt` predicate remain.
Collect removed original indices in ascending order, mirror them through
`FoodCache::remove`, then apply the existing stable physical-food compaction.
Reusable step scratch avoids an allocation on each shrink tick. No removal
leaves the cache untouched. A closed warm mirror retains `matched`; the next
`begin_mirrored` reuses verification with its existing debug assertion. Cold or
publicly reset caches still verify normally before use. Grids and stable pellet
handles survive, including repaired survivor positions and freed handles.

The regression compares strict/ULP boundaries and survivor bytes to the original
retain operation in official and Battledome configurations. It checks persistent
handle/free-list counts and all three warm admission grids before and after
re-access, then compares ambient/cell counts to recounts. It covers partial,
all and no-op removal, expansion and cold-cache equivalents. Rebuilding would
lose the retained free-list holes. A scratch mutation omitting the mirror
removal makes this regression fail; the selected runtime is unchanged.

## Full traces and fixed process pairs

All fourteen traces are byte-identical: the historical twelve-case set, including
stock/half/all protocol-13 official-500, plus two new shrink cases. The latter
contain an initial checkpoint, a complete checkpoint and ordered deaths/pickups/
births for every tick, and a terminal report: 3,202 records each. Full decompressed
bytes are compared literally, including food order, timestamps and RNG state;
compression changes no fields.

Each process case has one untallied warmup pair and five alternating pairs.
Whole-process `instructions:u` includes initialization, all simulation work and
the terminal checkpoint checksum. Same default x86-64 target, Rust 1.99.0,
release thin LTO/one codegen unit/debuginfo=1/default features. No samples are
excluded or replaced. Negative reductions below mean small increases.

| Case | Base median | Candidate median | Paired reduction [full range] |
| --- | ---: | ---: | ---: |
| Official-500 stock control | 11,814,096,334 | 11,818,953,569 | -0.041% [-0.048, -0.021%] |
| Official-500 half protocol-13 | 12,169,458,374 | 12,174,238,852 | -0.038% [-0.046, -0.022%] |
| Official-500 all protocol-13 | 12,517,545,715 | 12,521,599,253 | -0.032% [-0.058, -0.017%] |
| Official shrinking seats | 39,631,651,557 | 5,775,459,850 | 85.427% [85.426, 85.428%] |
| Battledome shrinking seats | 368,415,775 | 233,131,297 | 36.721% [36.721, 36.721%] |

The tiny normal-fixture increase is retained as a cost, not relabelled as a
saving. The large shrink gains apply to the following synthetic stress cases,
not to 500 continuously active real players or a production distribution.

## Shrink fixture and per-tick distributions

Seed 47, ordinary presets, 3,200 ticks. A time-zero radius sample initializes the
preset's occupied-seat radius before ecology generation. Subsequent shrinking
uses the unmodified controller/rate as seats retire: official 500 ->100 (four
every 12 ticks after tick96), Battledome 80 ->16 (one every 24 ticks). Synthetic
`TestRing` placement bypasses admission; natural deaths are allowed. Official
live population ranges 500 ->9; Battledome has three lives after its first tick
and later one. These are cache stress/control cases, not capacity trials.
Initial food counts are 52,827/1,851. A feasibility-only run checked an eater
survives and at least 25 ticks actually cull food before comparisons were run.

Per-tick diagnostic pairs also use one warmup and five alternating pairs per
case. Each tick's value below is its median over five matched measured runs;
quantiles interpolate sorted tick medians linearly. PMU brackets cover `step`,
including retention, cache work and physics. Controls, retirement, trace output
and cull classification stay outside that bracket. Classification copies the
pre-step points outside the bracket; this can affect allocation/cache state.
These distributions are diagnostic, not unprofiled process totals to subtract.

| Tick subset | N | Median base / candidate, M instructions | p95 base / candidate, M |
| --- | ---: | ---: | ---: |
| Official shrinking | 3,020 | 1.709 / 1.512 | 27.126 / 2.071 |
| Official with culling | 1,271 | 26.719 / 1.794 | 40.649 / 2.152 |
| Official shrinking without culling | 1,749 | 1.386 / 1.384 | 2.341 / 1.757 |
| Battledome shrinking | 3,008 | 0.064348 / 0.064351 | 0.885228 / 0.066609 |
| Battledome with culling | 159 | 0.923137 / 0.073161 | 0.950557 / 0.081767 |
| Battledome shrinking without culling | 2,849 | 0.064248 / 0.064257 | 0.065573 / 0.065580 |

There are 1,647/164 culled pellets. No-cull medians remain approximately equal;
the main benefit removes rebuild-heavy tails. Cold diagnostic tick0 remains
23.935 / 23.948M official and 1.714 / 1.720M Battledome. The complete means,
p90/p99, maxima, paired tick-ratio distributions and schedule checks are in JSON.

## Reproduction and limits

```sh
STUDY=research/performance/2026-10-27-mirrored-food-retention
OUT=/tmp/retention-new
python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" trace "$PWD" "$OUT" --cpu 3 --compression-cpu 2
python3 "$STUDY/reproduce.py" count "$PWD" "$OUT" --cpu 3 --perf /path/to/perf
python3 "$STUDY/reproduce.py" ticks "$PWD" "$OUT" --cpu 3
cargo test --locked --lib retention
python3 tools/check.py rust
```

Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access are required.
Builds/counts use one logical CPU/job; streaming shrink traces use at most two
logical CPUs for simulation/compression. Other workers share the WSL2 machine.
No administrator/security changes or outside game servers. There is no CPU-time,
latency, native EPYC transfer or capacity claim.

Raw arrays, binaries, complete compressed traces and failures are local-only
outside Git. Retained unselected preparation includes a rejected invalid archive
abbreviation and a trace-adapter compile error (event types lack Serialize).
An initial final-check attempt reused a debug mutation executable from a shared
Cargo target despite correct archived source/older archive mtimes. Validation
was restarted in a fresh isolated target; release measurement logs show separate
rebuilds and immutable copies made before that debug mutation. No process or
per-tick measurement pair was discarded or replaced.

Verification: all five focused retention tests, the full Linux Rust group
(15 commands), seven documentation tests and all seven study-entrypoint
`python -O` rejection checks pass. The mutation control fails as expected.
All selected process and per-tick counters are 100% scheduled.

After measurement, main advanced to `ef866d73259e35fe8659a1996d7b97c36d511e0b`
(#222 study and #223 host coverage). Integration revision
`dea716ef1ea01fe97f0e1bcefda5e725c7c858a9` preserves the measured simulator,
food cache, physics, bounds index and Cargo inputs byte for byte. The five
focused tests and full Linux Rust group were rerun on that integration in a
fresh isolated target. Measurement tables retain their original revision pins;
these later host changes are not included in those paired numbers.
