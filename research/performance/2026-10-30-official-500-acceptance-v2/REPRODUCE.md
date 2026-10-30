# Reproduce v2

Reuse [v1's recipe](https://github.com/NikitaMGrimm/slither-sim/blob/55f2921594aff00044624e47a8a1f90c4ee0b073/research/performance/2026-10-30-official-500-acceptance/REPRODUCE.md)
and its linked c7a ladder/#256 files. Stage the original v1 `prepare.py`,
`acceptance.py`, `run_acceptance.py` and `check_acceptance.py` in `sharedv1/` from
commit `55f2921594aff00044624e47a8a1f90c4ee0b073`; use its `select.py`,
`host_agent.py`, `marker_tokens.py`, `check_markers.py`, `assemble.py` and
`tables.py` unchanged at the staging root. Add this study's files there. Do not
copy v1's raw results, source-hash maps or cloud identifiers into Git.

Keep the original `sharedv1/acceptance.py` and root `select.py` staged through
the build, initial checks and run-1 startup below. Do not stage the corrected
helper yet: the runner imports it before starting its world.

On the first fresh AWS generator, before warmup, run the portable locked build
on the pinned main archive. `prepare.py` applies v1 and this study's generator
changes. Retain one bounded Cargo target and remove it after build. Run:

```sh
rustc --edition=2021 --test background.rs -o background-test
./background-test
rustc --edition=2021 --test --cfg synchronous_mutation background.rs -o background-mutant
# Must fail: synchronous dispatch stalls the 42 ms decision schedule.
./background-mutant
python3 check_harness.py /srv/slither-study
python3 check_gates.py
python3 check_markers.py
```

The deliberate 500 ms connect and publication jobs must leave at least fifteen
other-seat decisions in 650 ms with no lateness reaching 42 ms. The mutation
executes those jobs synchronously and must fail. These are synthetic generator
contract checks, not host capacity evidence. Commit the final plan/harness after
these checks and before starting any host world. Verify the staged files match
that commit. No access to the private bench host or local Rust work is permitted.

Follow the frozen plan for both fresh pairs and the unchanged one-second gate,
including boundary-overlapping reports. Run the metric normalizer before
counting. Each worker keeps the same A4 policy, full received-frame path and
42 ms decisions. Its connector and publisher background threads inherit that
worker's generator CPU; the host and observer remain on the host's sole vCPU.
Connection completion records include close/connect elapsed time; ordinary and
histogram output is serialized away from decisions, with bounded queue depth.
Queue overflow or a worker failure fails collection rather than hiding load.

After run 1 starts and its status reaches `warmup` (confirming that the running
process has already imported the original helper), apply the labelled
pre-reduction correction: replace `sharedv1/acceptance.py`, root `select.py`
and `sharedv1/check_quantiles.py` with files from v1 commit
`fa10b79368e4176f03287de7605d2058c5948665`. Run
`python3 sharedv1/check_quantiles.py` before offline reduction. Do not restart
run 1. It retains the original helper in memory; a new selector process imports
the corrected helper from disk. All other linked source stays at the original
pin. Stage the corrected files before starting run 2, as the plan specifies.
Corrected replay compares every gate and non-tail value exactly while allowing
only declared report-tail/derived seven-ms fields to differ.

After collection, use the unchanged v1 selector/assembly/tables recipe. Keep
both runs separate, retrieve and verify each raw archive, and verify teardown
before launching another pair. The selector calls this study's `acceptance.py`,
which changes only final maturity to the prospective lower bounds; start screen,
deferrals, queues, input lateness, accounting and report-only tails are retained.
Finish by scanning every enabled AWS region and deleting the task key/group.

Historical limitation: the v2 worker headroom metric omitted connector/publisher
CPU. This recipe reproduces that executed harness, not the corrected v3 gate.
No v2 result was reduced or qualified; see the report's post-result limitation.

## Retained v2 evidence and limit

This execution never produced an eligible full-window archive: the generator
was reclaimed during counting, after the sole replacement had been consumed.
Its client log and complete run summary are unavailable after configured root
deletion. Therefore the full-window selector/assembly steps above cannot be
replayed for this attempt. No partial interval replaces them.

The selected results instead curate the retained control status, entry screen,
binary bindings and the AWS state reason `Server.SpotInstanceTermination`.
The available host archive is local-only, SHA-256
`68675c54b7f8475c9545ab8431337499440a15843f58c9aa69fa1fbee5cc9603`
(2,631,076 bytes), verified against the remote digest before teardown. The
preflight archive (test/build logs and pins) is local-only, SHA-256
`75f5791874408c39a003bddf2098020ac30134e863d77de7a005c610374c71b6`
(6,506 bytes). No CPU/cadence table is derived from those partial records.
Selected cleanup/cost receipts retain all-region counts and conservative
lifetime/bid inputs; cloud identities and complete API receipts stay private.
