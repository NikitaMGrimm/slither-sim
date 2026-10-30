# PR comparison statistics, October 24, 2026

Status: v1, adopted for the advisory PR comparison. For later runs, the v1
procedure is superseded by [protocol v2](../../../docs/maintainers/performance-statistics-v2.md)
since 2026-10-25; its frozen contract and reference vectors remain historical
evidence. Design and synthetic reference vectors only; no new simulator
measurements. Hosted-runner calibration pending.

## Question

The PR comparison labeled a row Faster or Slower only for a median shift of at
least 5% with all three pairs agreeing, and Inconclusive otherwise; 89% of rows
in one sampled run were Inconclusive. Six rows used Windows process CPU, which
ticks in 15.625 ms steps. Which bounded, error-controlled comparison fits a
15-minute job?

## Decision

[`STATISTICS-CONTRACT.md`](STATISTICS-CONTRACT.md) is the frozen v1 contract a
statistician (GPT 6.1 Sol) wrote for the implementation: operation wall time on
every row, paired log ratios with a fitted order term, a fixed balanced order
sequence, looks at 8, 12 and 26 pairs, 1/600 error per look and workload, a
±5% band, and a 620-second sampling stage inside a 15-minute job with a
60-second reserve. [`COMMENT-WORDING.md`](COMMENT-WORDING.md) fixes the comment
text. [`CALIBRATION-PLAN.md`](CALIBRATION-PLAN.md) is the proposed hosted A/A
pilot and confirmation. All three are copied unchanged from the handoff; their
references to sibling files and scratch paths are original provenance.

The current explanation is in the
[PR comparison guide](../../../docs/maintainers/pr-reports.md#how-results-are-decided).
The code is `tools/perf_stats.py` (statistics) and `perf_compare.sequential`
(sampling). The SciPy-generated reference vectors, their generator and its uv
script lock are in
[`tools/fixtures/perf-statistics/`](../../../tools/fixtures/perf-statistics/README.md).

## Evidence and limits

The contract's cost projections come from one retained Windows run (Actions run
40104570739): builds and tests took about 179 s, traces 66 s and sampling 88 s of
a 366 s job. Projected full 26-pair sampling with the old operation counts is
about 602 s, which needed at least 40.5 s of fixed-cost savings to keep the
reserve. These are projections, not latency guarantees.

The error bound is model-based (independent normal log errors with constant
variance and effects within each workload). A same-binary A/A calibration on
hosted runners has not been run; until it is reviewed, the comment carries a
pending-calibration note.
