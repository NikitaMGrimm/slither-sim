# On-demand direct release comparison

The **Direct release performance** workflow is a second, explicit measurement
layer: a published release versus an exact candidate commit. It is separate from
the [automatic PR preview reports](pr-reports.md) and their cheap release
consolidation. Neither release metadata refreshes nor pushes start this workflow.
It has no two-hour mode and never changes release-plz files, PR bodies, release
guards or publication behavior.

## Request and identity

An owner with measurement authorization dispatches `release-perf.yml` on the
default branch with three required inputs:

| Input | Meaning |
| --- | --- |
| `baseline_tag` | Previous published release's version tag, such as `v0.3.1`. |
| `baseline_commit` | Full lowercase commit SHA behind that tag. A mismatch fails pinning. |
| `candidate_commit` | Explicit full commit SHA, normally main or the rolling release PR's exact head. Branch names are rejected. |

Before any build, the read-only pinning job verifies the published release and
resolves both commits and trees. The dispatch's default-branch revision pins the
shared harness, collector and workflow. The immutable request artifact records
all three revisions, the release ID, run/attempt, protocol and declared inputs.
The measurement job never interprets a candidate as a two-parent PR merge preview.
Rerun all jobs to obtain a new request for a new attempt; a measurement-only retry
cannot consume another attempt's request.

Both exact runtime commits are detached worktrees. The pinned trusted harness's
measurement examples, selected regression sources and fixtures are copied into
both through `perf_build.overlay`. No runtime code or lockfile is adapted.
`perf_build` builds and copies executables, records source/tree, overlay hashes,
lockfiles, toolchain, commands, targets and binary hashes, and verifies copied
binaries around measurement. The workflow uses fresh, separate baseline/candidate
targets, two build jobs, explicit `networking,parallel` with default features off,
and identical release profile settings. The stable compiler installed for that
job is shared by both sources and its exact version is retained. This is not a
toolchain pin across different runs.

The same catalog names, operation counts, trace windows, seeds, resolved workload
definitions and operations are required. Incompatible old sources, unavailable
features, build errors or mismatched catalogs produce **Unavailable**. There is
no fallback to an older harness, asymmetric feature selection, retry or selected
successful attempt.

## Evidence and limits

The collector reuses `perf_compare.parity`, selected `movement_turns`/`physics`
regressions and fidelity helpers, and `perf_compare.sequential`. It does not own
a new sampler. Missing trace evidence suppresses that row's timing claim;
changed parity remains visible as changed behavior. The raw artifact preserves
all attempted samples, failures, selected test outcomes, residuals, full traces,
copied binaries and identities. `evidence.json` binds retained files by hash to
the original request, including both runtime identities. Hashes establish
consistency, not scientific validity.

The [v2 contract](performance-statistics-v2.md) applies unchanged: Windows, one
block per fresh process, at most ten workloads, fourteen covered looks, and the
fixed error allocation. Builds/regressions/parity precede the sampling stage.
The shared job cutoff is fixed before preparation at `job_start + 1800 - 90`.
Git preparation, builds, regressions, fidelity, build metadata, catalogs and parity
check the remaining allowance before starting children and bound those children
by it. Preparation/build children use the shared process-tree owner with strict
stop/reap verification. A cutoff leaves Unavailable and retained partial logs;
it cannot start a fresh independent preparation timeout. Worktree cleanup uses
bounded waits within the finalization reserve.
Sampling uses `min(stage_start + 1500, job_cutoff)`; setup and builds consume the
job budget. The final covered prefix survives a normal sampling time cap.
Sampler or binary-validation failure suppresses final claims. There are no
interim comments or background collector, so a cancelled foreground measurement
step is stopped before unconditional diagnostic upload. Frozen v1 studies keep
their original contract and are never relabelled.

Intervals remain conditional on independent normal constant-variance log errors
with constant change and order effects. Hosted-runner calibration and power are
not established. These synthetic operation costs are neither official-server
evidence nor a production workload distribution. This direct run has its own
interval family; PR percentages and interval budgets are never combined with it.

## Redundant requests

The request key is SHA-256 of canonical declared measurement inputs: repository,
published tag/commit/tree, candidate commit/tree, harness commit/tree, protocol,
runner, toolchain selector, explicit features/profile/settings, operation block
policy, suite and budgets. PR number, comment text and outcome labels are absent.
Before measuring, an identical older queued/in-progress declaration on the same
default-branch harness revision suppresses a concurrent duplicate. The scan is
bounded to 100 queued and 100 active runs. Input-keyed concurrency permits one
active and one pending measurement for duplicates that escape that scan. Under
GitHub's default `queue: single`, another matching arrival replaces the pending
duplicate; it does not cancel the active measurement. This is suppression, not
a guarantee that every duplicate will be measured.

There is deliberately **no completed-result reuse**. A later request runs afresh;
an existing favorable comment, PR number, expired artifact or failed run cannot
satisfy it. Suppression makes no evidence claim and identifies the original active
run. Robust cross-run reuse would need an immutable evidence index, retention and
revalidation policy; that machinery is outside this focused layer.

## Trusted release evidence publication

Jobs that build or run either source have `contents: read` only, no persisted
checkout credentials and no write token passed to candidate code. A separate
completed `workflow_run` job executes only default-branch publisher code. It
verifies the source workflow, event, default-branch ancestry, run/attempt,
published baseline, exact commit/trees and canonical request key independently.
ZIP readers accept only bounded regular report/request files and never extract
artifact-controlled paths. Candidate Markdown is ignored; the publisher renders
validated fields with fixed wording and retains Unavailable/Undecided outcomes.

When pinning found one open rolling release PR, the publisher adds a distinctly
owned **Direct release performance** comment per source run there, linking the original run and
artifact evidence with all exact revisions. It writes only while that pinned PR
head remains current. Otherwise the result stays in the workflow summary and
artifacts; it is not attached to a different candidate. A failed/cancelled source
run with a retained request publishes Unavailable, never partial timing claims.
Publisher concurrency is scoped to the source run ID, and comment ownership
includes that ID and the input key. Distinct completed runs therefore cannot
replace each other's pending publication or overwrite each other's evidence.
Only attempts of the same source run share the single-pending queue; stale
attempts are rejected and the latest attempt owns that run's comment. This does
not depend on dispatch order. See GitHub's [current concurrency policy](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).
The documented `queue: max` option is unnecessary for this policy; the locally
verified actionlint 1.7.12 does not support that newer field.
Reports for older rerun attempts cannot overwrite newer evidence. No release
body, release-plz PR body, guard, tag or release asset is changed.

This evidence measures the recorded candidate and overlaid harness. A later merge
or publication with a different tree is a different candidate; the report never
claims to measure that shipped tree. Artifacts have 14-day retention; expired
evidence is not reusable and the original run identity remains in the comment.
