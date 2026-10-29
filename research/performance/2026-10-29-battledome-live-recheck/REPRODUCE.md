# Reproduce the live tail comparison

Full per-tick/client metric logs, native binaries, commands, process inventories, source
manifests and AWS receipts are **local-only**, outside Git. They are not publicly
downloadable. Digests identify them without granting access. Published JSON
contains bounded aggregates and one source-manifest digest per arm.

## Stage and build

Follow [#263's recipe](../2026-10-29-battledome-boost/REPRODUCE.md), including
the linked #262/#258/#256 helpers, in fresh external staging. Reuse, by reference:

- #263 `prepare.py` as `prepare263.py` and its `host_agent.py` under `shared263/`;
- #262 `prepare.py` as `prepare262.py`, `run_ladder.py` under `shared262/`, and
  its `reduce.py`, `delivery.py` and `check.py`;
- the earlier portable `build.sh`, `metrics.py`, `shared248/`, `shared256/`
  and preparation helpers from those recipes.

Add this study's `prepare.py`, `run_row.py` and `host_agent.py`. Prepare each
arm separately, supplying `input.json` with its `runtime` and a `main.tar`
made by `git archive --format=tar` at that revision. The base is the parent of
#265's merge; the candidate includes both #265 and #266. Exact revisions,
archive digests and instrumented-source manifest digests are in
[selected results](selected-results.json). `prepare.py` preserves the boost
overlay and records the actual archive's runtime rather than its donor's pin.

On the generator build each arm with Rust 1.99.0, locked portable release,
`RUSTFLAGS='-C target-cpu=x86-64'` and release `debug=1`. Clear inherited encoded
Rust flags. Use at most four build jobs on that eight-vCPU machine. Clean this
package's release artifacts between arms, copy and hash each executable before
the next build, and retain the complete build log. Run the native
`high_duty_preserves_heading_and_rule_gate` example regression for both arms.
Verify the staged source manifest before building. The host binaries must
differ, shared policy/source hashes must match, and **the candidate's client
executable serves both arms**. No production source is edited.

## Collect

The [frozen plan](PLAN.md), [first interruption amendment](PLAN-ADDENDUM.md)
and [same-target recovery](PLAN-RECOVERY.md) preserve the failed methods;
[the physical-core repeat](PLAN-FINAL-REPEAT.md) also failed qualification;
[the Zen 5 pool amendment](PLAN-ZEN5-POOL.md) owns the final sequence's resource
and qualification decisions. They specify order, timing, access,
resource limits and stop rules. One c7a.medium server core handles every host
task; six received-frame workers use logical CPUs 0–5, with control on CPU 7.
Initially both machines are Frankfurt Spot in the same AZ, using private traffic. Verify
140-minute boot poweroff timers, terminate-on-shutdown and DeleteOnTermination
volumes before work. The final generator is c8a.4xlarge: verify 16 physical
cores/16 vCPUs/one thread per core, with the same six workers on CPUs 0–5 and
control on CPU 7. Extra CPUs are unused by clients. Check quota for the 17
combined vCPUs and poweroff deadlines within 142 minutes of launch, including
boot. The first three primary rows use this pair. After the later Spot losses,
[the approved exception](PLAN-ONDEMAND-GENERATOR.md) uses exactly one On-Demand
c8a.2xlarge for the remaining rows, retaining the same Spot target and six workers.
Its boot timer is 90 minutes (launch bound 92); the final target timer is
180 minutes from original launch (bound 182), installed/verified before the
previous timer is stopped. Base-320 is first; both 480 rows must fit the fixed
remaining planned time/cap or are marked not measured. No further extension.
Earlier c8a interruptions and the c7i qualification failure remain separate;
do not pool them or relax the 42 ms input-lateness gate. Rebuild and require
identical source/native-binary pins before counting.
Stop on actual AWS CLI session expiry; no manual renewal.

Each row gets a new directory, fresh host and fresh clients. Private `job.json`
contains endpoints/ports plus `arm`, `target`, `runtime` and `label`. Stage the
chosen host, shared client and row-specific pins under `bin/`/`pins.json`.
Run `host_agent.py` on the server and `run_row.py` on the generator. The host
configuration remains shipped responsive Battledome, seed 7, 641 socket slots
and zero in-process bots. No delivery override or authoritative bot input.

The plan calls for six fresh ladders in the predeclared order. Each warms 300 seconds
at 80, then settles 60 and measures 90 seconds per stage through its target.
Only the final stage is a primary comparison. Retain the earlier calibrations,
transitions and all deferrals. Require every calibration and primary window to
pass generator/queue gates and at least 99% living-tick boosting; host overload
alone does not discard a row. No play hold or bench guest is used.

Each terminal archive must be closed, gzip-read and hashed before atomic
readiness publication. Retrieve only after readiness, verify size/SHA-256 and
required raw sources, then release that row's control server. Preserve source
instances on retrieval failure. Terminate only after all required retrievals
verify, then delete task resources and scan all enabled regions. Never delete
unrelated or protected mail-filter resources.

## Reduce and recompute

For each arm, create an external analysis harness containing its original
`main.tar`, canonical `reduce.py`, `metrics.py`, `shared248/` and `shared256/`.
Keep the private native master receipt as `build-pins-private.json`. Run:

```sh
python -X utf8 select_results.py PRIVATE/rows/LABEL/raw ANALYSIS_HARNESS PRIVATE/analysis/LABEL.json ARM PRIVATE/build-pins-private.json
python -X utf8 assemble_results.py PRIVATE selected-results.json INTERRUPTED_PRIVATE EARLIER_REPEAT_PRIVATE PHYSICAL_CORE_FAILURE_PRIVATE
python -X utf8 tables.py selected-results.json
```

Repeat the first command for the four verified labels from `assemble_results.py`'s order.
The verified prefix ends at base-320; 480 is not measured after credential expiry.
The assembler accepts only a four-row prefix or the complete six-row plan and
does not mutate the original private collector state.
The optional interrupted root retains the two earlier completed 160-seat
reductions separately, verifies their archives/source identity and records the
hash-verified surviving host archive. It does not reconstruct the generator
receipts already deleted by Spot. The retained target supplies all four primary
windows, with the disclosed Spot-to-On-Demand generator transition. The earlier-repeat root adds its three qualified windows as bounded
supplementary values, verifies the surviving fourth-row host archive, and retains
the complete c7i row's unqualified online diagnostics. The physical-core failure
root adds the rejected c7a-160 gate diagnostic and verified archive receipt.
Earlier rows are not
pooled or substituted, and unqualified diagnostics are not live-tail evidence.
The selector invokes [#262's canonical selector](../2026-10-29-battledome-experience/select_results.py),
binding the runtime and normalizing Windows source-key separators. Its in-memory
reducer replaces only the queue-variance square `dx**2` with `dx*dx`, which
exactly reproduces the original Linux float on Windows. No tolerance is added:
the complete original online result must still compare exactly, and the
transformed source digest is retained once. It checks the actual arm archive, host binary,
shared client and policy against the native master receipt. It exactly reruns
online tick ranges, then uses complete 100-tick interior blocks from the final
host log, whole client-report boundaries and independent task-clock reconciliation.
All boundary inventories, decoding progress and generator gates remain checked.

The assembler rechecks all four primary archives, supplementary/failure archives and both staged source
manifests. The report's exclusive slow-tail categories use means over each
row's own largest aligned-CPU 1%: admission; socket plus broadcast minus nested
minimap; nested minimap; remaining world CPU. Independent quantiles are never
added. The minimap scope includes recipient gating and shared work.

Use the linked #262 `check.py` with this staged reducer/delivery on `sys.path`
to rerun its synthetic aligned-clock, cadence, histogram, headroom, concurrent
log-tail, archive-failure and source-preservation controls. The native treatment
regression verifies heading bits and the received-rule gate, not human play.
Documentation checks need no Rust build.

Frame jitter is receipt-gap minus elapsed-header increment. Frame excess is
receipt time minus cumulative elapsed headers relative to a reset running
minimum. Residual stalls count gaps above 20/100/250 ms; sparse normal emissions
are not automatically stalls. These metrics include server slowdown and client
delivery/scheduling, and do not identify absolute network or input-to-effect
latency. Completed spawn waits start at login enqueue and exclude fixed
reconnect delay and the preceding TCP/upgrade handshake; pending requests are
censored separately. See the donor recipe for the precise quantile convention
and online-versus-canonical selection details.
