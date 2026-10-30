# Reproduce the official-500 acceptance

This is a paid, real-socket capacity qualification, not an optimization comparison.
Follow the unchanged criteria and labelled amendment in [PLAN.md](PLAN.md).
Cloud collection requires separate authorization and its own bounded budget.
Raw logs, cloud receipts, command inventories and native binaries are local-only,
outside repositories; hashes do not make those files publicly downloadable.

## Stage and build

Start from the [c7a ladder recipe](../2026-10-29-c7a-mature-capacity/REPRODUCE.md).
Stage its `metrics.py`, `window.py`, `generator_boundaries.py`, `check_harness.py`
and portable `build.sh` in an external directory. Keep its `prepare.py` as
`prepare258.py`, and its `run_ladder.py` and `host_agent.py` under `shared258/`.
Reconstruct `shared256/` from the published
[#256 harness patch](../2026-10-28-capacity-checkpoint/harness.patch), retaining
`build.py`, `client.rs`, `timing.rs` and `select.py`. Copy the four linked #248
helpers exactly as the ladder recipe describes. Reuse
[#281's atomic archive publisher](../2026-10-29-battledome-experience/delivery.py)
as `delivery.py`; do not copy that study's Battledome runtime/policy changes.

Add this study's Python files. Archive the pinned main from the execution binding
as `main.tar` with `git archive --format=tar`; supply `input.json` with its
`runtime`. On the generator install the ordinary build toolchain and Rust 1.99.0,
then run the linked `build.sh`. It applies the ladder overlay followed by this
study's read-only changes and builds host/clients with locked dependencies,
`-C target-cpu=x86-64`, thin LTO and one codegen unit. No native targeting.
Use the one bounded target, copy/hash both executables, then delete the target.
Both independent pairs use identical host/client binaries.

Before starting any world, verify that linked #256 files equal the reconstructed
published files and that this study's collector bytes equal the frozen commit.
Run on the generator:

```sh
python3 -m py_compile acceptance.py run_acceptance.py select.py host_agent.py prepare.py
python3 check_harness.py /srv/slither-study
python3 check_acceptance.py
python3 check_markers.py
```

The synthetic controls exercise aligned sums versus component quantiles, exact
class bounds, a report-only p99 above seven ms, end-class drift and generator
rejection. They are synthetic contract checks, not capacity evidence.

## Provision and collect

Use Frankfurt Spot with profile `slither-bench`, the exact types, launch tags,
private same-AZ network, bid ceilings, deletion settings and verified boot timers
in the plan. Do not expose the game/control/artifact ports to the Internet.
Keep each fresh pair's identifiers, SSH configuration and `job.json` outside Git.
That job contains private host/generator addresses and the game/control ports.
Verify one host vCPU, eight generator vCPUs, timer bounds, terminate-on-shutdown,
root-volume deletion and `CLK_TCK=100` before work. No host tuning.

Transfer the host over the private network and verify its SHA-256 on both
instances. Start `host_agent.py` on the server and `generator_boundaries.py`
followed by `run_acceptance.py` on the generator. Start `marker_tokens.py` on the
generator before counting, on both runs: the linked control writer appends a
newline, and the metric accumulator requires bare tokens. The normalizer touches
only those two report markers, runs on control CPU 7, and journals its work.
The linked host observer runs
every host thread on its sole vCPU. Six received-frame A4 workers use generator
CPUs 0-5; control uses CPU 7. There are exactly 500 socket slots and zero host bots.

The runner keeps the ladder's natural tick-zero warmup, 100 resident/400 rotating
clients, no forced churn after tick 180,000, and the #256 mature screen. It settles
ten seconds, then counts exactly 1,800 seconds. No checkpoint, thin clients,
population ladder, artificial latency or forced counted churn is used.

CPU selection uses complete interior 100-tick blocks. Deferrals use the exterior
census bracket covering the requested interval; final class uses the first census
at/after its end. Retain both intervals and offsets. Frame-gap, header-gap residual
and frame-excess use the [earlier calculation](../2026-10-29-battledome-experience/prepare.py)
with 1-microsecond bins, accumulated only during counting and published once per
worker afterward. Preserve actual client metric intervals. End publication may
take up to thirty seconds outside the counted window. CPU p99 and these client
tails are reported, not gated; CPU/task reconciliation remains a validity check.

Wait for terminal `archiveReady`, not merely a phase/status change. The archive
publisher closes, gzip-reads and hashes the archive before readiness. Run the
selector on the generator, after collection and before teardown:

```sh
python3 select.py
```

It must exactly reproduce the online result, verify source/binary/configuration
receipts, client progress, command inventories, one-core affinity and task-clock
reconciliation. Retrieve and verify the raw archive and selected result. Only a
passing first run authorizes the second fresh pair under this plan. Preserve all
failures; only the single predeclared infrastructure replacement is available.

## Reduce, report and clean up

To replay privately retained data, stage the same linked harness, extract that
run's `load-run`, `host-run`, `pins.json` and `run-summary.json` beside `select.py`,
then run it again. Keep each run's selector output separately. Use
[assemble.py](assemble.py) to wrap one or both results without pooling, then
[tables.py](tables.py) to recompute report tables:

```sh
python3 assemble.py selected-results.json PRIVATE_RUN1_SELECTED.json
# Add PRIVATE_RUN2_SELECTED.json only if that independent run was authorized.
python3 tables.py selected-results.json
```

No access to cloud resources is needed for reduction.

The retained run-1 archive is local-only, SHA-256
`4002cf60512b63558e163e252db84f9f33782fb17efe415c35d70dda3a6323f9`
(12,205,258 bytes); retrieval was verified before termination. The selected
JSON retains the binary/source bindings without a per-file hash map. The final
post-collection checks ran on the AWS generator:

```sh
python3 check_acceptance.py
python3 check_markers.py
# From the source checkout, with this study and its index row overlaid:
python3 -B -m unittest discover -s tools -p test_documentation.py
```

Both synthetic checks and all seven documentation tests passed. Recomputed
tables matched the collection-side tables byte-for-byte. The retained client log
contains neither `CLIENT_ERROR` nor `panicked at`; the failing client predicate
is the selected worst one-second input-lateness p99, not CPU headroom or queues.
No new Rust test matrix was run for this research-only change.

For the labelled post-result diagnosis, use the same verified retained archive:

```sh
python3 diagnose_lateness.py PRIVATE_RUN1_RESULTS.tar.gz lateness-diagnosis.json
```

This short standard-library reduction preserves the frozen report selection,
computes per-report join/death deltas and the six violating worker intervals,
and binds them to histogram publication and nearby aggregate host samples.
It checks its worst p99 against the original result and publishes no raw
addresses or cloud identities. It cannot recover unrecorded per-connection
event times or reconnect durations. Per the maintainer's scoped exception,
this diagnosis reduction ran on Windows (under one second), with no build,
Rust test, guest access or new AWS collection.

Stop and retrieve within the fixed ninety-minute per-pair plan. Cancel task Spot
requests by recorded instance/request identity, terminate the instances, verify
task volumes disappear, and remove the task key and security group. Scan every
enabled region and retain the private inventories, including unrelated resources
left untouched. Publish selected cleanup counts and conservative cost inputs,
not addresses, resource identifiers, account details or personal paths.
