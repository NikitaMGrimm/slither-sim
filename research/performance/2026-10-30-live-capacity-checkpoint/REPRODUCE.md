# Reproduce the live checkpoint

This is a capacity study, not a new fixed-work optimization. Follow [PLAN.md](PLAN.md)
and [CALIBRATION-PLAN.md](CALIBRATION-PLAN.md). All builds, socket loads, checks and
reductions run on the authorized Linux guest. Use an otherwise idle guest for
counted windows. No outside game servers, package installs or system changes.
Python 3.11.4+ is required for the archived builder's `tarfile` data filter.

The exact runtime archives are A `551cb1030ea2bf84f83bbeb167233b917900929a`,
B `b750004aeb8f11be61dc483a2ce458c6194f116f`, and the common client runtime
`d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`. Create them with `git archive --format=tar`
from the repository, named `base.tar`, `candidate.tar` and `client-base.tar`.
Transfer them and the study scripts to fresh external output storage. Keep the
common client's `client.rs` byte-for-byte as extracted from #256's harness.

Set `REPO` to the checkout and `OUT` to that fresh external directory. `STUDY`
is this study directory. Reuse, rather than duplicate, [#256's harness patch](../2026-10-28-capacity-checkpoint/harness.patch)
and [#248's helpers](../2026-10-28-official-density-capacity/prepare.py):

```sh
cd "$OUT"
git -c core.autocrlf=false apply "$REPO/research/performance/2026-10-28-capacity-checkpoint/harness.patch"
ln -s "$REPO/research/performance/2026-10-28-official-density-capacity" shared248
cp "$STUDY/calibration.py" "$STUDY/prepare_harness.py" "$STUDY/live_series.py" .
cp "$STUDY/analyze.py" "$STUDY/check_analysis.py" "$STUDY/check_quality.py" .
cp "$STUDY/build_fine.py" "$STUDY/run_fine.py" "$STUDY/fine_series.py" .
cp "$STUDY/verify_sources.py" "$STUDY/analyze_fine.py" .
mkdir publication
python3 prepare_harness.py "$OUT"
```

The adapter makes exact, asserted replacements to the linked scripts: one bounded
target, the separate common-client archive, disabled coarse phase/maintenance
exports, the extra post-primary coarse window, calibrated isolation and complete
retry classification. It builds only the needed host executable per arm. No
production checkout files change.

## Calibration and freeze

The actual run committed the calibration protocol as `b682c392` before observing
idle activity, then committed the measured numeric gate/full plan as `165d172d`
before any host timing. For a fresh repetition, run calibration with all other
guest work idle, keep its complete output, and commit the new numeric gate and
plan before host/client timing. Do not overwrite or silently reuse this study's
calibration as a fresh observation. The script settles 20 seconds then takes the
fixed twelve 30-second windows:

```sh
python3 calibration.py > calibration.log 2>&1
python3 - <<'PY'
import json
from pathlib import Path
r=Path.cwd();v=json.loads((r/'calibration.json').read_text())
assert v['idleQualificationPasses']
gate={
 'calibrationPasses':True,'calibrationWindows':v['plannedWindows'],
 'idleEnvelopeRate':v['idleEnvelopeRate'],
 'excessRateAllowance':v['excessRateAllowance'],
 'counterQuantumSeconds':v['counterQuantumSeconds'],
 'endpointQuantumCount':v['endpointQuantumCount'],
 'formula':'W * (idleEnvelopeRate + 0.01) + 2 * counterQuantumSeconds',
 'exampleThirtySecondAllowanceSeconds':v['exampleThirtySecondAllowanceSeconds'],
 'calibrationPlanCommit':'b682c392f783d1c1a544463389e6e4c620fd9512',
}
(r/'isolation-gate.json').write_text(json.dumps(gate,indent=2)+'\n')
PY
```

The last commit field above records this run's protocol; replace it with the
new protocol commit when preregistering a different run. Here the idle-envelope
rate was 0.002998926999628939, with 10-ms counter quanta. The selected primary
windows are about 29.4 seconds, so their allowance is about 0.402 seconds, rather
than the 30-second example's 0.410 seconds. No background CPU is subtracted.

## Build, source reconstruction and preflight

Use the existing locked/offline dependencies. The single target must be absent
and task-owned before building. Both builds use Rust 1.99.0, native-target release,
thin LTO, one codegen unit and line tables, as in #256. Copy/hashes precede timing.

```sh
test ! -e /tmp/slither-live-checkpoint-target
python3 build.py shared248 base
python3 build.py shared248 candidate
python3 - <<'PY'
from pathlib import Path
import shutil
p=Path('/tmp/slither-live-checkpoint-target')
assert p.resolve()==p and not p.is_symlink()
shutil.rmtree(p)
PY
python3 client_build.py
python3 verify_sources.py "$OUT"
python3 run.py base smoke-base 8
python3 select.py "$OUT" smoke-base-selected.json smoke-base --preflight
python3 run.py candidate smoke-candidate 8
python3 select.py "$OUT" smoke-candidate-selected.json smoke-candidate --preflight
python3 run.py candidate pilot-candidate-3 500 --pilot
python3 select.py "$OUT" preflight.json pilot-candidate-3 --preflight
python3 check_controls.py
python3 check_analysis.py "$OUT"
```

`client_build.py` removes the target and writes the cleanup receipt. One immutable
common client is used by both hosts; the per-arm client build is intentionally
omitted. Source reconstruction compares every Rust source byte, and checks the
common client's production sources against its original archive.

The metrology mutations double tick CPU without changing process accounting,
saturate synthetic client CPU, and impose a growing synthetic kernel queue. All
must fail the existing selector at the intended gate. The separate arithmetic
controls test a known 72-tick tail, exact phase sums, food flags and detail-export
contamination; all deliberate perturbations must be detected.

## Detached primary series

After committing the plan and passing preflight, launch the original read-only
vCPU sampler and the new frozen series controller. Detach stdin/stdout/stderr so
an SSH reporting interruption cannot stop collection:

```sh
python3 - <<'PY'
from pathlib import Path
import json,subprocess,time
r=Path.cwd()
for script,receipt,log in [('core_cpu.py','core-controller.json','core-controller.log'),
                           ('live_series.py','controller.json','cohorts.log')]:
 with (r/log).open('xb') as out:
  p=subprocess.Popen(['python3',str(r/script)],stdin=subprocess.DEVNULL,
                     stdout=out,stderr=subprocess.STDOUT,start_new_session=True)
 (r/receipt).write_text(json.dumps({'pid':p.pid,'unix':time.time()})+'\n')
PY
```

The controller runs A,B,B,A,A,B, each from tick zero, with the original forced
warmup and screen, five-second settle and 30-second primary hold. It retains one
20-second coarse export and one 15-second syscall diagnostic after each primary.
Only complete interior 100-tick blocks qualify. There is one class-only fresh
retry per cohort, at most one isolation-only retry globally, no retry for CPU or
p99, and a six-CPU-hour ceiling including the 600-second setup reserve. Any
other quality failure stops collection. Never enable the old `--thin` fallback.

The first A attempt here failed class only. The old selector stopped at class
before computing isolation. Before its retry reached a primary window, an audit
with only that class assertion bypassed confirmed all other gates, and the
adapter was updated to evaluate class and isolation together before naming a
retry class. Neither predicate changed; simultaneous failures stop. Four
synthetic combinations verify this behavior on a completed mature run:

```sh
python3 check_quality.py "$OUT" base-pair-1-attempt-1
python3 analyze.py "$OUT"
python3 check_boundaries.py "$OUT"
```

The original failed log is unchanged. `verification.json` retains its class range
and other-gate audit; it is never capacity evidence. A fresh run without a failed
attempt may use a completed, otherwise-qualified mature case for the synthetic
combination control.

## Optional nested diagnostic and reporting

After all six primary holds qualify, `fine_series.py` builds one separate
current-main host, reconstructs its complete source, removes the target, resumes
the vCPU sampler and runs one fresh mature cohort. Its windows are 15 seconds
of broad clocks, 20 seconds of coarse export and 20 seconds of inclusive nested
clocks. Both diagnostic windows face the same class/client/queue/isolation checks.
The separate allowance is 7,200 CPU seconds with a 100-second reserve. There is
no diagnostic retry or capacity claim from its extra timers.

```sh
python3 fine_series.py > fine-series-controller.log 2>&1
python3 analyze_fine.py "$OUT"
python3 verify_sources.py "$OUT"
python3 "$STUDY/tables.py" "$OUT/publication"
```

The actual controller was detached in the same way as the primary controller.
`analyze.py` keeps primary ticks and coarse ticks separate, validates 8,400 actual
food flags per arm, reports the original socket CPU bins and post-report markers,
and exports only whole-source/archive/binary identities. `analyze_fine.py`
exports inclusive nested scope means and separate quality status. Fine scopes
must not be summed or substituted into primary p99.

Run the documentation test on the integrated publication checkout, check the
public diff for whitespace/private identifiers, then archive and verify all raw
runs, failed attempts, process inventories, sources and binaries before removing
only this task's guest directories. Raw access is local-only, with its aggregate
receipt in `archive-receipt.json`; no per-file source-hash maps are published.
