# On-demand direct release comparison

**Direct release performance** explicitly compares a published release with an
exact candidate commit. Unlike [PR preview reports and release consolidation](pr-reports.md),
it runs only on demand, with no two-hour mode. It changes no release-plz files,
PR bodies, release guards or publication behavior.

## Request and identity

An owner with measurement authorization dispatches `release-perf.yml` on the
default branch with three required inputs:

| Input | Meaning |
| --- | --- |
| `baseline_tag` | Previous published release's version tag, such as `v0.3.1`. |
| `baseline_commit` | Full lowercase commit SHA behind that tag. A mismatch fails pinning. |
| `candidate_commit` | Explicit full commit SHA, normally main or the rolling release PR's exact head. Branch names are rejected. |

Before building, read-only pinning verifies the published release and resolves
both commits/trees. The dispatch's default-branch revision pins the shared harness,
collector and workflow. The immutable request records all three revisions, release
ID, run/attempt, protocol and inputs. Candidates are exact commits, never
two-parent merge previews. Rerun all jobs for a new attempt; measurement-only
retries cannot consume another attempt's request.

Detached runtime worktrees receive the trusted harness's measurement examples,
selected tests and fixtures through `perf_build.overlay`; runtime code and
lockfiles are not adapted. `perf_build` copies executables and records source/tree,
overlay hashes, lockfiles, toolchain, commands, targets and binary hashes, checked
around measurement. Builds use fresh separate targets, two jobs, explicit
`networking,parallel` with default features off and identical release settings.
Both sources share the job's stable compiler; its exact version is retained,
but is not pinned across runs.

Catalog names, operation counts, trace windows, seeds, definitions and operations
must match. Incompatible sources, unavailable features, build errors or mismatched
catalogs are **Unavailable**: no older harness, asymmetric features, retry or
selection of successful attempts.

## Evidence and limits

The collector reuses `perf_compare.parity`/`sequential`, selected
`movement_turns`/`physics` regressions and fidelity helpers. Missing traces suppress
that row's timing claim; changed parity remains changed behavior. Raw artifacts
keep every attempted sample, failure, selected test outcome, residual, full trace,
copied binary and identity. `evidence.json` hashes bind retained files to the
request and both runtimes: consistency, not scientific validity.

The [v2 contract](performance-statistics-v2.md) owns process blocks, workload/look
limits, error allocation, sampling caps and failure rules. The declared runner is
`ubuntu-latest`, matching [PR comparisons](pr-reports.md#calibration), selected by
the owner on 2026-10-25 without a separate Windows-versus-Ubuntu confirmation run.
The request records it.

Before preparation, fix `job_cutoff = job_start + 1800 - 90`. Git preparation,
builds, regressions, fidelity, metadata, catalogs and parity check remaining time
before starting children and bound them by it. Preparation/build children use the
shared process-tree owner with strict stop/reap verification. A cutoff leaves
Unavailable and partial logs, never a fresh preparation timeout. Worktree cleanup
uses bounded waits within the finalization reserve.

Sampling follows preparation at `min(stage_start + 1500, job_cutoff)`; setup and
builds consume the job budget. There are no interim comments or background
collector. The step `exec`s the collector so cancellation's SIGINT reaches it.
Cleanup kills/reaps session-isolated build/regression groups before unconditional
diagnostic upload; later runner SIGTERM/forced kill can interrupt this guarantee.

The protocol's intervals remain conditional; hosted-runner calibration and power
are unestablished. Synthetic operation costs are neither official-server evidence
nor a production distribution. Direct runs have their own interval family;
never combine their budgets or percentages with PR results.

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

**Completed results are never reused.** Later requests run afresh; comments,
PR numbers, expired artifacts and failed runs cannot satisfy them. Suppression
identifies the original active run and makes no evidence claim. Cross-run reuse
would require an immutable evidence index, retention and revalidation policy.

## Trusted release evidence publication

Build/run jobs have only `contents: read`, no persisted credentials or candidate
write token. A separate completed `workflow_run` executes default-branch publisher
code, independently verifying workflow/event, default-branch ancestry,
run/attempt, published baseline, exact commits/trees and request key. ZIP readers
accept only bounded regular report/request files without extracting controlled
paths. Fixed wording renders validated fields, ignoring candidate Markdown and
preserving Unavailable/Undecided.

If pinning found exactly one open rolling release PR, each source run owns a
**Direct release performance** comment linking its run/artifacts and exact
revisions. It writes only while the pinned PR head is current; otherwise results
stay in summaries/artifacts rather than attaching to another candidate.
Failed/cancelled runs with retained requests publish Unavailable, never partial
claims.

Publisher concurrency uses the source run ID; comment ownership includes that
ID and input key. Distinct runs cannot evict pending publication or overwrite
each other. Attempts of one run share the single-pending queue; stale attempts
are rejected and the latest owns its comment, regardless of dispatch order.
See GitHub's [concurrency policy](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).
`queue: max` is unnecessary and unsupported by locally verified actionlint 1.7.12.
Publication changes no release body, release-plz PR body, guard, tag or asset.

This evidence measures the recorded candidate and overlaid harness. A later merge
or publication with a different tree is a different candidate; the report never
claims to measure that shipped tree. Artifacts have 14-day retention; expired
evidence is not reusable and the original run identity remains in the comment.
