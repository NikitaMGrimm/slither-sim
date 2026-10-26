# Automatic PR comparisons

Measured changes get one updated advisory comment. Correctness checks gate
merging; timings do not. Investigate results with the
[local comparison command](../performance.md#compare-a-change).

## When a comparison runs

Opened, updated and reopened PRs, including drafts and forks, run a comparison
when they change a measured input. This workflow is separate from `Required checks`.

The authoritative `paths` list in `pr-report.yml` covers simulator sources, Cargo
manifests/build configuration, measurement examples, selected regression tests
and fixtures, collector scripts (including imported `tools/` modules) and the
workflow. `tools/test_pr_report.py` checks that coverage.

Other changes, such as docs, research records and unrelated tests/tools, start no
comparison and get no comment; correctness checks still run. With no run, the
publisher cannot mistake a skipped PR for a missing artifact. Never require
`Measurements`: GitHub leaves filtered required workflows waiting.

GitHub matches the filter against the whole PR diff. Once a PR changes a measured
input, every later push is measured, including documentation-only pushes, so the
comment follows the current head and target. If a later push reverts every
measured change, no new run starts and the existing comment keeps its recorded
PR head. GitHub's [path filters](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#git-diff-comparisons)
consider only the first 3,000 files of a diff, so a larger PR can go unmeasured.
Drafts are measured on every push, so marking one ready does not repeat the run.

## What is measured

Each update compares the merge commit with its first parent: the exact target
revision used for that preview. The report records both commits and the PR head.
An Ubuntu runner builds release executables with one toolchain and identical PR
benchmark/fidelity examples, selected tests and fixtures overlaid on a fresh,
detached baseline. Cargo lockfiles may differ; their hashes are retained.
Compilation failures make results unavailable.

`tools/perf_build.py` owns snapshots, overlays, builds and identities, including
[local comparisons](../performance.md#compare-a-change). Both revisions share the
checkout's Cargo target and [dependency cache](#fixed-costs-and-the-dependency-cache).
Before each build, after copying and hashing the previous executables, the
simulator's release/dev outputs are cleaned. Compatible dependencies survive;
failed cleanup makes that revision unavailable.

The PR timing budget covers six rows: `step-food`, `observe`,
`official-100-step`, `official-100-bot`, `host-official-500` and
`host-battledome-80`. The socket rows reuse `host_load`, with no background bot
workers, a separate load-generator process, one host CPU and a second driver CPU.
Official uses a fixed official-size 500-seat world; Battledome uses its preset.
Each sample warms for one second after opening all sockets, allows up to five
additional untimed seconds for every slot to connect, then measures at least
286 physics ticks, ending at a boundary with every socket connected. Connected/living counts, admissions, deaths, bytes, deferred
ticks and actual tick counts stay in the raw record. This is a short synthetic
load, not sustained capacity qualification or an 80-living-snake guarantee.

The ten existing deterministic trace windows remain independent of timing
selection. Socket scheduling has no deterministic state trace. The legacy
catalog and direct release comparison suite remain available unchanged.
The [row audit](../../research/performance/2026-10-26-pr-report-audit/REPORT.md) records removals, costs and calibration.
Both builds list the same operation counts. The head resolves Config defaults
once into `head-configs.json`, supplied to both runtime processes. Unsupported
head configurations or remaining definition differences never yield a speed
claim: the collector shows a measured head-only value marked **Redefined**, with
its reason, when the candidate runs successfully.

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

New PR runs implement the metric extension of [statistics protocol v2](performance-statistics-v2.md#pr-metric-extension);
the frozen v1 contract and its evidence retain their historical allocation.
`tools/perf_stats.py` cites its sections, and the sampler is
`perf_compare.sequential`.

**Metric.** The four deterministic rows retain operation wall time: the sum of
monotonic operation intervals excluding setup, controls, resets and output work.
Linux process CPU includes the entire case including setup/warmup and is retained
as a diagnostic, not substituted for operation cost. Socket rows retain host
process CPU (simulation plus sockets, excluding the child driver), simulation
busy wall time and elapsed wall time per completed tick. They normalize the
observed tick count to the requested count and record both.

Before comparison sampling, a bounded same-binary pilot takes six balanced A/A
pairs plus one discarded warmup pair per row. For socket rows it selects CPU
when its order-adjusted log residual SD is no greater than busy wall, otherwise
busy wall; missing/incomplete CPU retains busy wall. The frozen comparison plan
pins this choice before any baseline/head pairs. CPU rows are marked in the
comment; their busy-wall values remain in Details. This pilot is diagnostic,
not independent hosted calibration. Instruction counter probes retain availability
and errors for `perf` and Cachegrind; neither is assumed installed or permitted.

**Sampling.** One observation is an adjacent pair of fresh baseline and PR
processes for one workload. Both sides follow one plan frozen before sampling
(`performance/plan.json`): workload definition hash, seed, operation count,
executable hashes, the order sequence and the deadline. A row whose workload
definitions still differ after head configuration pinning is shown head-only as
Redefined when possible. Each workload first
runs one discarded baseline-then-PR warmup pair. Rounds then visit the active
rows in catalog order, one pair each, following a fixed sequence in which every
two consecutive pairs contain one baseline-first and one PR-first pair.

**Analysis.** [Protocol v2](performance-statistics-v2.md#covered-looks-and-allocation)
owns the covered looks, allocation and conditional simultaneous-coverage bound.
For paired `d = log(PR time / base time)`, regression `d = mu + gamma * order`
removes a constant first-run effect. The interval is `mu ± t * s / sqrt(n)`, with
residual standard deviation `s` and Student-t quantile `t` at 1 − eta/2 with
`n − 2` degrees of freedom. Change is `exp(mu) − 1` with transformed interval
ends (an asymmetric bracket); Base and PR are geometric means of the same pairs.
These are not per-row 95% intervals.

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

**Error bound.** Under the [protocol's log-error model](performance-statistics-v2.md#covered-looks-and-allocation),
any wrong Faster, Slower or Within claim requires a miss, so a report's chance
of any wrong claim is at most 5%, regardless of correlation between workloads.
Heavy tails, drift, serial dependence or changing effects can break the model.
Stopping resolved rows or retaining the last covered look selects only planned
intervals. The bound covers these executables on this runner, not other machines,
builds or later commits. Hosted calibration is unestablished; labels remain
conditional and advisory, with no block-count selection or power claim.

**Stopping and budget.** Follow [v2's deadline and failure rules](performance-statistics-v2.md#time-and-publication).
The first job step records the monotonic start; 28.5 minutes leaves the protocol's
90-second reserve. A crash, hang, invalid timing record or numeric failure makes
that row unavailable for the run; earlier looks remain raw history only.
At a normal cap, 10 or 11 completed pairs use the 8-pair look; fewer than 8 are
unavailable. A time-capped Undecided result is advisory, not a job failure or
required check.

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

Manually dispatch `PR comparison calibration` from Actions with `windows-latest`
or `ubuntu-latest` and workload group A or B. It follows the
[same-binary (A/A) pilot plan](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-pr-comparison-statistics/CALIBRATION-PLAN.md):
two independent jobs per runner/group, eight total. Each builds, copies and hashes
one harness; `tools/perf_calibration.py` runs both roles with identical arguments.
For each workload and 1, 2 or 4 reset-identical process blocks, it discards one
warmup pair then times six balanced pairs without early stopping.

The current pilot uses plan ID `slither-perf-calibration-pilot-v2`: group A
replaces `dome-proxy-32-step` with `battledome-32-step`, and group B replaces
`dome-proxy-32-bot` with `client-battledome-64`. It retains v1's sampling schedule
and 620-second stage/15-minute job budget. Historical v1 records keep their
original workload definitions. Artifacts keep every attempt and block time, the executable/catalog/toolchain/runner/CPU
manifest and `summary.json`. Per-job, per-block estimates include order-adjusted
paired residual variance, within/between-process components, pair cost and `Q`,
plus a linear cost fit. They feed offline analysis, not a decision. Time-capped
cells are incomplete.
If a warmup or counted attempt fails, that workload/block-count cell is marked
failed and receives no further attempts. Its completed pairs and failed attempt
remain diagnostic evidence; variance, `Q` and the cost fit use only successfully
warmed, complete cells. Other cells continue within the deadline.

The comparison's runner is set once, as the `&runner` anchor in `pr-report.yml`:
`ubuntu-latest`. In the pilot's analysis, Ubuntu had lower pooled same-binary noise
on nine of ten workloads and needed about 210 instead of 370 sampling seconds, and
its forecast resolved the least-resolved workload far more often than Windows.
That analysis selected no calibrated winner, so labels stay advisory. The
cache key includes the OS, so the first runs on a new runner miss the cache
until a `main` push saves one.

## Publishing and permissions

One bot-owned comment, keyed by a hidden marker and author, is rendered from
measurements without a model call or agent byline. Status dots show outcomes;
Details collapses methodology and diagnostics.

The read-only collector and trusted default-branch `workflow_run` publisher have
separate permissions. The publisher validates the source workflow/run/attempt,
PR repository/head/branch, preview parents, final head/target and prior published
run/attempt/sequence. Artifact identity and bot authorship prevent unrelated or
older runs from replacing newer reports. It reads bounded artifacts as data,
never running PR scripts with its write token. Mentions in PR diagnostics are
suppressed.

The target comes from the branch API because PR `base.sha` can lag the merge
checkout. A target advancing along the same history is recorded in Details;
stale heads and rewritten/diverged targets are skipped.

The unprivileged collector's [protocol checkpoints](performance-statistics-v2.md#time-and-publication)
bind immutable snapshots to the run, attempt, PR head, base, two-parent preview,
report/snapshot schemas and frozen plan hash. Pinned `upload-artifact` steps
upload while timing is paused and acknowledge before the next child starts.
The trusted `workflow_run: in_progress` observer polls only that source run every
30 seconds for at most 31 minutes, updating monotone sequences. The `completed`
event reconciles final outcomes, including failure/cancellation/timeout.
Unsuccessful runs withdraw interim claims using saved validated bot provenance
even when artifacts are deleted, expired or malformed. Readers reject ZIP paths,
symlinks, duplicate entries and oversized data; downloads are never executed.

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

Both workflows pin actions and disable persisted checkout credentials; they
need no personal token or external benchmarking service. Once cleanup is
verified (below), full measurements, input/binary hashes, traces and logs upload
with 14-day retention, including partial failures; otherwise only progress
checkpoints already uploaded remain. Publishers download only small comment
artifacts. Launcher
logs/PID and partial diagnostics survive upload failures before collector finish;
broken report generation fails the job.

A supervisor owns the collector and descendants through a Linux process group
or Windows kill-on-close process Job. Before terminal/full uploads, `always()`
cleanup requests stop and waits for explicit quiescence. Even after normal
collector exit, the supervisor kills/reaps descendants, closes evidence output,
then acknowledges. Failed termination or remaining members at timeout withhold
acknowledgment. Uploads are skipped if cleanup is unverified or the supervisor
was killed. Linux descendants starting their own session escape this guarantee.
Cleanup success cannot turn an interrupted/failed comparison into success.

This precedes runner orphan cleanup:
[cancellation targets the active step](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-cancellation);
[`always()` permits uploads after cancellation](https://docs.github.com/en/actions/reference/workflows-and-actions/expressions#always).
Synthetic process-tree tests cover local cleanup; hosted cancellation is untested.
Traces use gzip level 1 to reduce untimed compression; parity hashes every
uncompressed byte, so larger archives do not weaken comparison.

GitHub's usual approval policy still applies to first-time fork contributors.
Conflicting PRs need their conflicts resolved before a merge checkout can run.
The publisher workflow must exist on the default branch before GitHub can
trigger it; see the [workflow_run documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run).

## Rolling release evidence

A separate bot-owned comment on the rolling `release-plz-*` PR consolidates merged
PR reports since the latest published release tag, pinned to that tag's commit
and the current main candidate. It preserves all outcome-label counts and exposes
two headline columns (`host-official-500`, with `step-default` retained for
historical reports, and `official-100-bot`) with their
own uncertainty and source links. Missing, historical, failed and incomplete
reports stay explicit. Redefined outcomes stay head-only; host workload names
and CPU/wall units remain explicit. Preview measurements are not relabeled as shipped-tree
measurements, and PR percentages are never summed.

Cheap refreshes follow terminal PR reporting, merges, and completion of Release PR
and Release workflows. This includes release-plz updates made with `GITHUB_TOKEN`
that do not produce ordinary PR events. All summary writers share one concurrency
group, recheck candidate/release-head freshness and skip identical content. They
edit no release-plz body or branch files and change no release authorization,
metadata-only diff, publication, merge or cooldown guard. The range is bounded
to 1,000 commits; a larger or diverged range is explicitly unavailable.

[Direct prior-release measurement](release-performance.md) is on demand;
consolidation starts none on main pushes or release metadata churn. Comparing a
release PR only to its target misses accumulated changes. Historical harness
incompatibility must report unavailable with a reason.

## Local checks and diagnosis

The selected tests and fixture measurements also run locally, without GitHub
credentials or Actions:

```sh
cargo test --locked --test movement_turns --test physics
cargo run --locked --example fidelity_metrics -- tests/fixtures
```

The second command prints JSON turn/speed-ramp errors and sample counts.
For target-branch performance, follow the [local performance guide](../performance.md#compare-a-change):

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

That guide owns output, metric and quiet-window requirements. These commands
need no GitHub environment; the combined collector requires a PR merge checkout
and GitHub variables.

To check the reporting tools themselves:

```sh
uv run --locked --only-group tools python -m unittest discover -s tools -p test_pr_report.py
uv run --locked --only-group tools python -m unittest discover -s tools -p test_perf_stats.py
cargo test --locked --example fidelity_metrics --test movement_turns --test physics
```

Full local timing is unnecessary for reporting-workflow edits. After a push,
the comment should retain its ID and identify the new head/run. Report duration
includes rebuilding both revisions and serializing full-state traces; simulator
speed alone does not predict it.
