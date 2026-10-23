# Automatic PR comparisons

A PR that changes a measured input runs a performance and regression comparison
when it is opened, updated or reopened, including draft PRs and forks. One
Actions comment is updated in place. These measurements are advisory; the report
workflow is separate from `Required checks`, and numerical regressions do not
block merging.

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

Each PR update measures the merge commit against its first parent (the exact
target revision used for that merge). The report records that pair and the PR
head. A Windows runner builds release executables for both revisions with the
same toolchain, after copying the PR's benchmark and fidelity examples, selected
test sources and fixtures into a fresh, detached baseline worktree. Both runtimes
therefore see identical measurement code and evidence. Cargo lockfiles can differ
and their hashes are retained. Compilation failures make results unavailable.
Snapshots, the input overlay, builds and identity records come from
`tools/perf_build.py`, which [local comparisons](performance.md#compare-a-change)
also use. Both revisions reuse one run-owned Cargo target directory for dependencies.
After baseline executables are copied and hashed, only the simulator package's
release and dev build outputs are explicitly cleaned, forcing a fresh candidate
build without recompiling compatible dependencies. A failed cleanup leaves
candidate results unavailable.

The performance table and state traces cover the `ci` suite of the PR harness's
[workload catalog](performance.md#workloads): the six legacy workloads plus
selected steady-state scenarios, at most ten rows. Both builds must list the same
plan; otherwise performance and trace results are unavailable. Adding a `ci`
workload to the harness adds it here without a collector change.
Three adjacent alternating pairs follow a warmup; raw results are retained.
The displayed target/PR costs come from the pair with the median ratio, so they
agree with the paired percentage; they are not separate medians of the costs.
CPU is process user plus kernel time, including setup/teardown; operation wall
time excludes them. Values are descriptive hosted-runner measurements. The 5%
display band and consistent-direction rule are presentation heuristics, not a
significance test or a production speed guarantee. Small or mixed shifts are
inconclusive. CPU counters are Windows-only; the runner can fall back to wall.

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

The comment is updated in place, keyed by a hidden marker and bot author.
It is generated automatically from the measurements, with no model call or
agent byline. Colored status dots distinguish improvements, regressions and
inconclusive changes; methodology and diagnostics are collapsed under Details.
Before posting, the publisher checks the artifact's PR/repository/head/merge
identity, current open PR head and target, and prior published run/attempt.
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
If GitHub omits the run's PR association for a fork, the source branch must
identify exactly one PR in this repository, including closed PRs. Ambiguous
branches cannot publish a comment; their raw measurement artifacts remain
available. Using a separate branch for each PR avoids this ambiguity.

Both workflows use pinned actions and leave checkout credentials unpersisted.
No personal token or external benchmarking service is needed. Full measurements,
input/binary hashes, traces and logs remain in a separate artifact for 14 days,
including partial output on failure. The publisher downloads only the small
comment artifact. Broken report generation remains visible as a failed job.
Full-state trace archives use gzip level 1 to reduce untimed compression work;
parity still hashes every uncompressed byte. Archive sizes can therefore grow.

GitHub's usual approval policy still applies to first-time fork contributors.
Conflicting PRs need their conflicts resolved before a merge checkout can run.
The publisher workflow must exist on the default branch before GitHub can
trigger it; see the [workflow_run documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_run).

The selected tests and fixture measurements also run locally, without GitHub
credentials or Actions:

```sh
cargo test --locked --test movement_turns --test physics
cargo run --locked --example fidelity_metrics -- tests/fixtures
```

The second command prints JSON with the turn and speed-ramp errors and their
sample counts. To compare performance against the target branch, use a fresh
output directory; the [performance guide](performance.md#compare-a-change)
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
cargo test --locked --example fidelity_metrics --test movement_turns --test physics
```

Performance comparisons need a quiet machine with matched builds and inputs.
Full local timing is not required to edit the reporting workflow. After a push,
the existing PR comment should retain its ID and identify the new head and run.
Repeated runs rebuild both revisions and retain full-state parity traces; the
wall-clock duration therefore includes compilation and serialization as well as
the timed workloads. Simulator speed alone does not predict report duration.
