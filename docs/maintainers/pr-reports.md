# Automatic PR comparisons

For contributors: a measured change gets one updated advisory comment. Correctness
checks still gate merging; timing shifts do not. Use the
[local comparison command](../performance.md#compare-a-change) to investigate a result.

## When a comparison runs

A PR that changes a measured input runs a performance and regression comparison
when it is opened, updated or reopened, including draft PRs and forks. The report
workflow is separate from `Required checks`.

Measured inputs are the simulator sources, Cargo manifests and build
configuration, the two measurement examples, the selected regression tests and
fixtures, and the collector scripts and workflow. The `paths` list in
`pr-report.yml` is authoritative; `tools/test_pr_report.py` checks it against the
collector's inputs, including every `tools/` module the collector imports. A PR that changes none of them, such as documentation,
research records, other tests or other tools, starts no comparison run and gets
no comment; its correctness checks still run. With no run, the publisher has
nothing to report, so a skipped PR is not confused with a failed or missing
artifact. Keep `Measurements` out of required checks: GitHub would leave a
required but filtered workflow waiting on every skipped PR.

GitHub matches the filter against the whole PR diff. Once a PR changes a measured
input, every later push is measured, including documentation-only pushes, so the
comment follows the current head and target. If a later push reverts every
measured change, no new run starts and the existing comment keeps its recorded
PR head. GitHub's [path filters](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#git-diff-comparisons)
consider only the first 3,000 files of a diff, so a larger PR can go unmeasured.
Drafts are measured on every push, so marking one ready does not repeat the run.

## What is measured

Each PR update measures the merge commit against its first parent (the exact
target revision used for that merge). The report records that pair and the PR
head. A Windows runner builds release executables for both revisions with the
same toolchain, after copying the PR's benchmark and fidelity examples, selected
test sources and fixtures into a fresh, detached baseline worktree. Both runtimes
therefore see identical measurement code and evidence. Cargo lockfiles can differ
and their hashes are retained. Compilation failures make results unavailable.
Snapshots, the input overlay, builds and identity records come from
`tools/perf_build.py`, which [local comparisons](../performance.md#compare-a-change)
also use. Both revisions build in the checkout's Cargo target directory, which
starts from a [dependency cache](#fixed-costs-and-the-dependency-cache). Before
each revision builds, and after the previous revision's executables are copied
and hashed, the simulator package's release and dev outputs are cleaned, so each
revision compiles its own package without recompiling compatible dependencies.
A failed cleanup leaves that revision's results unavailable.

The performance table and state traces cover the `ci` suite of the PR harness's
[workload catalog](../performance.md#workload-selection): the six legacy workloads plus
selected steady-state scenarios, at most ten rows. Both builds must list the same
plan; otherwise performance and trace results are unavailable. Adding a `ci`
workload to the harness adds it here without a collector change, up to the ten
rows the error budget covers. Every row is timed by
[operation wall time](#how-results-are-decided).

The fidelity table exposes the existing duration-weighted turn residuals and
speed-ramp residuals by source, with denominators, using the same recorded
fixtures on both runtimes. It also compares named outcomes from `movement_turns`
and `physics` and one canonical trace per `ci` workload over its catalog trace
window, listed under Details. A scenario trace without its terminal record is
unavailable rather than equal. Test source changes,
new tests and removals are disclosed; removed tests are never counted as fixes.
Ignored tests stay distinct from passes. A changed trace means changed behavior,
which may be appropriate for a fidelity correction. No overall accuracy score
or empirical claim about unmeasured server mechanics is produced.
The denominator includes every declared test in these two selected source files,
including cases absent from both execution logs after an abort. The table marks
those cases unavailable and labels the report incomplete.

## How results are decided

New runs implement [statistics protocol v2](performance-statistics-v2.md);
the frozen v1 contract and its evidence retain their historical allocation.
`tools/perf_stats.py` cites its sections, and the sampler is
`perf_compare.sequential`.

**Metric.** Every row uses operation wall time: the sum of the harness's
monotonic operation intervals, excluding setup, placement, warmup, resets,
output destruction and serialization. It is instrumented operation cost, not
process or end-to-end latency. Process CPU stays in the raw results as a
diagnostic only; Windows reports it in 15.625 ms steps.

**Sampling.** One observation is an adjacent pair of fresh baseline and PR
processes for one workload. Both sides follow one plan frozen before sampling
(`performance/plan.json`): workload definition hash, seed, operation count,
executable hashes, the order sequence and the deadline. A row whose workload
definitions differ between the revisions is unavailable. Each workload first
runs one discarded baseline-then-PR warmup pair. Rounds then visit the active
rows in catalog order, one pair each, following a fixed sequence in which every
two consecutive pairs contain one baseline-first and one PR-first pair.

**Analysis.** Covered looks are 8, 12, 26, then doubling through 53,248 pairs. With
`d = log(PR time / base time)` per pair, a regression `d = mu + gamma * order`
removes a constant first-run effect, and `mu ± t * s / sqrt(n)` is the interval,
where `s` is the residual standard deviation and `t` the Student-t quantile at
1 − eta/2 with `n − 2` degrees of freedom. Change shows `exp(mu) − 1` with the
interval's transformed ends, so the bracket is asymmetric; Base and PR are
geometric means over the same pairs. The allocation is 1/750 at the first three
looks, then `1/(1000 * 2^k)` at later doubling look k. These are not per-row 95% intervals.

| Result | Meaning |
| --- | --- |
| 🟢 Faster | The whole interval is below −5% time. |
| 🔴 Slower | The whole interval is above +5% time. |
| ⚪ Within ±5% | The whole interval lies in the −5% to +5% band, ends included. |
| 🟡 Undecided | Anything else, including numerically zero residual spread, which shows `[interval unavailable]`. Neither equality nor the absence of a regression is claimed. |
| ⚠️ Unavailable | A failed or invalid measurement, mismatched plans, or no completed look. |

The band is about time, not throughput: 10% less time is not exactly 10% more
operations per second. Labels use unrounded interval ends, so a displayed end of
exactly ±5.00% can coexist with a strict label.

**Error bound.** Ten slots and fourteen pre-budgeted looks have total
noncoverage allocation 0.0499951171875. By the union bound, all of them
cover their true values with probability at least 95%, and any wrong Faster,
Slower or Within claim in a report requires a miss. So the chance that a report
contains any wrong claim is at most 5%, whatever the correlation between
workloads. Stopping a resolved row and keeping the last look at a deadline only
choose among these planned intervals. The bound is model-based: within each
workload it assumes independent, normally distributed log-ratio errors with
constant variance, and constant change and order effects over the run. Heavy
tails, drift, serial dependence or changing effects can break it. It covers
these two executables on this runner, not other machines, builds or later
commits. Hosted model calibration is not established; labels remain conditional
and advisory. This extension selects no new OS, block count or power claim.

**Stopping and budget.** A Faster, Slower or Within row stops at once and keeps
that look. An Undecided row continues until the cap or the explicit 53,248-pair
safety guard; that guard is not a confidence claim. A
crash, hang, invalid timing record, changed executable or numeric failure makes
the row unavailable for the rest of the run; earlier looks stay in its raw
history but are never displayed. An unexpected sampler exception also invalidates
final claims after an interim snapshot. The job has a provisional 30-minute timeout.
Its first step records a monotonic start time, and sampling stops at the earlier of 1,500 seconds
after the stage starts (warmups and upload pauses included) or 28.5 minutes after job start, leaving
90 seconds to write the report and upload artifacts. No child starts after the
deadline; a child still running is killed and its pair is kept as an incomplete
attempt. Each unfinished row then keeps its last completed look: 10 or 11 pairs
report the 8-pair result, 45 pairs use 26, and fewer than 8 pairs are unavailable. A time-capped
Undecided result is not a failure; the job stays advisory and outside required
checks.

The `performance/` artifact directory holds the frozen plan, every child attempt
including warmups, failures and process CPU (`results.jsonl`), the complete
pairs (`pairs.jsonl`) and each row's evaluated looks (`sequential.json`).
`report.json` repeats the looks with phase times measured from job start.

### Fixed costs and the dependency cache

Builds, tests and traces take most of the job, so the workflow trims them
without touching the measured binaries. A `push` to `main` runs only the
`Dependency cache` job, which builds the release and test dependencies into a
cache keyed by the lockfile, toolchain and Cargo environment, and skips the
build when that cache already exists. PR runs restore it read-only before the
collector starts and never save it, so one PR's build cannot reach another PR's
measurement. The cache omits workspace packages, and Cargo rebuilds any
dependency whose inputs changed. Dependencies compile with all four runner
cores, and test builds keep only line tables for debug information. Cache
restoration and every build, test and trace finish before the first warmup.

### Calibration

The `PR comparison calibration` workflow runs the same-binary (A/A) pilot from
the [calibration plan](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-pr-comparison-statistics/CALIBRATION-PLAN.md).
It is dispatched manually from the Actions tab with a runner image
(`windows-latest` or `ubuntu-latest`) and workload group (A or B). Each job
builds the harness once, copies and hashes it, and runs both roles from that one
file with identical arguments through `tools/perf_calibration.py`. For every
workload in the group it times processes with 1, 2 and 4 reset-identical blocks:
one discarded warmup pair per block count, then six balanced pairs each, with no
early stopping. This frozen v1 pilot retains its 620-second stage/15-minute job
budget rather than adopting the v2 comparison deadline. The artifact keeps every child
attempt with its block times, the manifest (executable, catalog, toolchain,
runner image, CPU) and `summary.json`. The summary holds per-job estimates for
each block count (order-adjusted paired residual variance, within- and
between-process components, cost per pair and `Q`) and a linear cost fit. These
feed the plan's offline analysis; they are not a decision. The plan's pilot is
two separately dispatched jobs per runner image and group, eight in all. A cell
the deadline cuts short is reported as incomplete.
If a warmup or counted attempt fails, that workload/block-count cell is marked
failed and receives no further attempts. Its completed pairs and failed attempt
remain diagnostic evidence; variance, `Q` and the cost fit use only successfully
warmed, complete cells. Other cells continue within the deadline.

The comparison's runner is set once, as the `&runner` anchor in `pr-report.yml`.
Change it only after a calibration supports the new runner. The cache key
includes the OS, so the first runs on a new runner miss the cache until a
`main` push saves one.

## Publishing and permissions

The comment is updated in place, keyed by a hidden marker and bot author.
It is generated automatically from the measurements, with no model call or
agent byline. Colored status dots mark the results described above;
methodology and diagnostics are collapsed under Details.
Before posting, the publisher checks the artifact's PR/repository/head/merge
identity, final PR head and target, and prior published run/attempt/sequence.
The current target is resolved through the branch API; the PR's `base.sha` can
retain an older snapshot even when the merge checkout uses a newer target.
If concurrent work advances the target along the same history, the pinned
comparison is published and Details records the newer target. Older PR heads
and rewritten or diverged target histories are still skipped.
The `PR comparison` workflow has read-only repository access. Its separate
`workflow_run` publisher loads code only from the default branch, verifies the
originating workflow, run attempt, PR head/repository/branch and merge parents,
and reads a bounded comment artifact as data. It never runs PR scripts with its
comment-write token. Artifact identity, bot authorship and published run metadata
prevent unrelated or older runs from replacing a newer report. GitHub mentions
in PR-provided diagnostics are suppressed.

The unprivileged collector writes immutable snapshots at at most three covered
complete-round checkpoints. Each is bound to the exact run, attempt, PR head,
base, two-parent preview, report/snapshot schema and frozen plan hash. The workflow
pauses timing while pinned `upload-artifact` steps upload them, then acknowledges
each upload before the next child starts. These pauses are retained and count
against the deadline; their measurement-process effect is not calibrated.
The trusted `workflow_run: in_progress` observer polls only for this source run,
every 30 seconds for at most 31 minutes. It edits the same comment using monotone
sequences. A separate `completed` event reconciles terminal outcomes, including
failure/cancellation/timeout. Any unsuccessful source run withdraws all interim
claims using saved validated bot provenance even if its artifacts were deleted,
expired or malformed. ZIP paths, symlinks, duplicate entries and oversized data
are rejected; downloaded data is never executed.

Withdrawal revalidates the saved bot-owned provisional identity and source
provenance rather than requiring the old PR head to remain current. It can remove
a cancelled run's claim after a head update, unmerged closure or target rewrite,
including an older provisional attempt of the same rerun/head. It re-reads comment
ownership immediately before editing and preserves newer or completed results.
A successful run's progress artifact never consumes terminal precedence. Terminal
artifact download/validation gets at most three attempts within the observer's
existing deadline; failure/cancellation withdrawal is immediate. Exhausted artifact
failures become incomplete, and a later valid final artifact can still replace
that incomplete state for the same measured identity.

Late results can update a merged PR when the source run, final PR head, original
preview parents and actual merge identity still verify, with the measured base
an ancestor of the actual merge. The publisher compares preview and actual trees
and explicitly labels a different tree: the original preview remains evidence,
but it does not measure the shipped tree. Closed-unmerged PRs and stale heads
cannot receive current-looking updates. No merge method is assumed equivalent.
If GitHub omits the run's PR association for a fork, the source branch must
identify exactly one PR in this repository, including closed PRs. Ambiguous
branches cannot publish a comment; their raw measurement artifacts remain
available. Using a separate branch for each PR avoids this ambiguity.

Both workflows use pinned actions and leave checkout credentials unpersisted.
No personal token or external benchmarking service is needed. Full measurements,
input/binary hashes, traces and logs remain in a separate artifact for 14 days,
including partial output on failure. The publisher downloads only the small
comment artifacts. Launcher logs/PID and partial diagnostics are included even
when an upload step fails before the collector's finish step. Broken report generation remains visible as a failed job.

The Windows measurement job keeps a supervisor owning the collector and all its
descendants through a kill-on-close process Job. Before terminal/full uploads,
an `always()` cleanup step requests stop and waits for explicit quiescence. The
supervisor terminates and reaps descendants even after a normal collector exit,
closes its evidence output, then acknowledges. Strict Job query, termination or
timeout failure withholds that acknowledgment; upload is skipped if cleanup is
unverified or the supervisor was killed. Successful cleanup does not turn a failed
or interrupted comparison into success. This precedes the runner's later orphan
cleanup: [cancellation signals target the active step](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation),
and [`always()` can still run uploads after cancellation](https://docs.github.com/en/actions/reference/workflows-and-actions/expressions#always).
Real synthetic process-tree tests cover local cleanup; hosted cancellation has
not been exercised.
Full-state trace archives use gzip level 1 to reduce untimed compression work;
parity still hashes every uncompressed byte. Archive sizes can therefore grow.

GitHub's usual approval policy still applies to first-time fork contributors.
Conflicting PRs need their conflicts resolved before a merge checkout can run.
The publisher workflow must exist on the default branch before GitHub can
trigger it; see the [workflow_run documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run).

## Rolling release evidence

A separate bot-owned comment on the rolling `release-plz-*` PR consolidates merged
PR reports since the latest published release tag, pinned to that tag's commit
and the current main candidate. It preserves all outcome-label counts and exposes
two preselected headline workloads (`step-default`, `official-100-bot`) with their
own uncertainty and source links. Missing, historical, failed and incomplete
reports stay explicit. Preview measurements are not relabeled as shipped-tree
measurements, and PR percentages are never summed.

Cheap refreshes follow terminal PR reporting, merges, and completion of Release PR
and Release workflows. This includes release-plz updates made with `GITHUB_TOKEN`
that do not produce ordinary PR events. All summary writers share one concurrency
group, recheck candidate/release-head freshness and skip identical content. They
edit no release-plz body or branch files and change no release authorization,
metadata-only diff, publication, merge or cooldown guard. The range is bounded
to 1,000 commits; a larger or diverged range is explicitly unavailable.

A direct prior-published-tag versus pinned release-candidate comparison is a
separate on-demand layer. Consolidation starts no measurements on main pushes
or release metadata churn. Comparing a release PR merely to its target would
miss accumulated changes; historical harness incompatibility must be unavailable
with a reason rather than a fabricated comparison.

## Local checks and diagnosis

The selected tests and fixture measurements also run locally, without GitHub
credentials or Actions:

```sh
cargo test --locked --test movement_turns --test physics
cargo run --locked --example fidelity_metrics -- tests/fixtures
```

The second command prints JSON with the turn and speed-ramp errors and their
sample counts. To compare performance against the target branch, use a fresh
output directory; the [performance guide](../performance.md#compare-a-change)
describes the snapshots, builds and results:

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

This saves parity readings, profiles, individual paired timings and a summary.
Windows provides CPU and wall timings; other systems provide wall timings. The combined report
collector currently expects a PR merge checkout and GitHub environment
variables; these individual measurement commands do not.

To check the reporting tools themselves:

```sh
uv run --locked --only-group tools python -m unittest discover -s tools -p test_pr_report.py
uv run --locked --only-group tools python -m unittest discover -s tools -p test_perf_stats.py
cargo test --locked --example fidelity_metrics --test movement_turns --test physics
```

Performance comparisons need a quiet machine with matched builds and inputs.
Full local timing is not required to edit the reporting workflow. After a push,
the existing PR comment should retain its ID and identify the new head and run.
Repeated runs rebuild both revisions and retain full-state parity traces; the
wall-clock duration therefore includes compilation and serialization as well as
the timed workloads. Simulator speed alone does not predict report duration.
