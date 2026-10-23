# PR comparison experiment

The `ci/report-preview` branch tests an advisory Actions comment on its draft PR.
Keep `@coderabbitai ignore` in that PR's description throughout the experiment;
do not change the repository's CodeRabbit configuration. This workflow neither
merges changes nor joins the existing `Required checks` dependency list.

Each PR update measures the merge commit against its first parent (the exact
target revision used for that merge). The report records that pair and the PR
head. A Windows runner builds release executables for both revisions with the
same toolchain, after copying the PR's benchmark and fidelity examples, selected
test sources and fixtures into a fresh, detached baseline worktree. Both runtimes
therefore see identical measurement code and evidence. Cargo lockfiles can differ
and their hashes are retained. Compilation failures make results unavailable.

The performance table reuses the six [baseline workloads](performance.md).
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
and `physics` and six separate 32-iteration canonical traces. Test source changes,
new tests and removals are disclosed; removed tests are never counted as fixes.
Ignored tests stay distinct from passes. A changed trace means changed behavior,
which may be appropriate for a fidelity correction. No overall accuracy score
or empirical claim about unmeasured server mechanics is produced.
The denominator includes every declared test in these two selected source files,
including cases absent from both execution logs after an abort. The table marks
those cases unavailable and labels the report incomplete.

The comment is updated in place, keyed by a hidden marker and bot author.
Before posting, the publisher checks the artifact's PR/repository/head/merge
identity, current open PR head and target, and prior published run/attempt.
Only the named same-repository branch can run it. Measurement jobs have read-only
repository access; the separate publisher has comment write access and treats
downloaded artifacts only as data. Both use pinned actions and leave checkout
credentials unpersisted. No personal token or external benchmarking service is
needed. All measurements, input/binary hashes, traces and logs are uploaded for
14 days, including partial output on failure. Numerical deltas do not fail the
job; broken report generation remains visible.

This branch-only publisher deliberately skips forks. A production fork publisher
would use a trusted `workflow_run` workflow already installed on the default
branch, with validation of originating runs and artifact data. This experiment
does not install anything on `main` or prove that future fork path.

Local focused checks (standard library for the Python reporting code):

```sh
uv run --locked --only-group tools python -m unittest discover -s tools -p test_pr_report.py
cargo test --locked --example fidelity_metrics --test movement_turns --test physics
```

Real performance collection belongs on the isolated Actions runner while other
agents use the shared desktop. Full local timing is not required to edit/report
the workflow. To verify updating, push another change to the experiment branch
and check that the existing comment ID is retained with the new head and run.
