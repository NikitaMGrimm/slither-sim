# First optimization: collision bounds lookup

This is a historical study, separated from the maintained guides without new
measurements or changed conclusions. Source paths and local output addresses in
the record refer to the original study, not this archive directory. Full raw
outputs are local-only references, not downloadable evidence in this checkout.

The runtime change in `src/simulator.rs` indexes body bounds by stable snake slot,
with empty entries for dead slots. Previously, every candidate pair binary-searched
the living list to locate the same bounds. Pair/join order, bounds arithmetic,
segment values and narrow collision calls are unchanged. No settings, physics,
RNG order, visibility or wire behavior changed.

The [matched comparison](results.json) retains five adjacent
baseline/candidate pairs, alternating label order, after one untallied warmup per
label/case. Both peers held heavy local work around 14:38–14:40 UTC. Builds,
tests and trace serialization finished first. Source/build conditions match the
[initial baseline](../2026-10-23-baseline/REPORT.md); candidate runtime commit is `67db506` and both copied executable
hashes are recorded. The benchmark source is identical in both executables.

Reproduce the comparison with fresh output storage and copied executables:

```sh
python -B tools/perf_compare.py --baseline PATH/TO/BASELINE --candidate PATH/TO/CANDIDATE --output outputs/perf-comparison
```

For an optional shared time limit and retained incomplete results, see
[comparison deadlines](../../../docs/performance-deadlines.md).

The runner fixes longer per-case iteration counts below and preserves raw paired
rows. Ratios are medians of each matched candidate/baseline ratio; lower is less
cost. They are descriptive results from five pairs, without confidence intervals.

| Case | Iterations per sample | Process CPU ratio | Inner wall ratio |
| --- | ---: | ---: | ---: |
| step-default | 100,000 | 1.000 | 0.965 |
| step-food | 10,000 | 0.967 | 1.021 |
| step-body | 50,000 | 0.716 | 0.699 |
| observe | 1,000 | 0.989 | 0.989 |
| portable | 5,000 | 0.964 | 0.959 |
| protocol | 5,000 | 0.967 | 1.002 |

The separated 64-snake/8,192-anchor body fixture used **28.4% less process CPU**
and **30.1% less inner wall time** by median paired ratio. All five body pairs
improved (CPU ratios 0.626–0.725). Median process CPU totals were 2.094 → 1.516 s,
median operation p99 93.1 → 68.1 µs, and maximum working sets 5.71 → 5.64 MiB.
Memory differences are small and do not establish an allocation improvement.
The other cases have mixed small changes and outliers; no improvement or
regression conclusion is drawn for them. There is no demonstrated end-to-end
speedup or general production-world speedup.

Output parity passed for all six 32-iteration canonical full-state traces and a
128-decision portable trace (5.376 virtual seconds, including minimap cadence).
The traces retain internal serialized RNG/motion state, events, observations,
frames and wire bytes. A separate bounded regression exercises a dead middle
slot and original/respawn collision join order. Full `tools/check.py rust` passed
(default/core-only all-target tests, doctests, Clippy, formatting and basic
example), along with three standard-library runner failure/cleanup regressions.
An independent source review found no remaining defects in the runtime, runner
or comparison logic; it did not independently execute these checks.

This comparison uses the published 0.2.0 runtime baseline. If correctness changes
are integrated, establish and name that updated baseline before extending these
performance claims. The remaining shortlist in the initial report is investigative work, not
additional promised speedups.

The [observation follow-up](../../../docs/performance-observations.md) adds immutable geometry
and selection cases with separate output-destruction timing and an explicitly
updated source baseline.
