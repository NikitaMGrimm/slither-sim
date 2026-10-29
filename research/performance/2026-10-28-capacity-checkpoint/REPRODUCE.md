# Reproduce the 500-socket checkpoint

The runtime pins are #248 base `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`
and merged #255 main `551cb1030ea2bf84f83bbeb167233b917900929a`. Use fresh
external output storage and existing locked/offline dependencies on Linux x86-64.
All heavy work runs on the authorized guest; no packages or system settings
change. Do not run any other host, build or timing job during counted windows.

Reuse these earlier sources by reference; they are not duplicated in this study:

- [#248 prepare.py](../2026-10-28-official-density-capacity/prepare.py): exact
  replacement/hash helpers for archived copies.
- [#248 observe.rs](../2026-10-28-official-density-capacity/observe.rs): original
  read-only census and busy-wall duration stream.
- [#248 run.py](../2026-10-28-official-density-capacity/run.py): process/task
  snapshots, including user/system CPU and affinity readback.
- [#248 select_results.py](../2026-10-28-official-density-capacity/select_results.py):
  percentile definition, CPU interpolation and reconciliation helpers.

Apply this study's harness patch in the external output directory. It contains
only the new archived-copy timing hooks, socket client, build/run/select helpers
and their checks. Nothing changes the production tree. Point `shared248` at the
earlier study directory in the current checkout (a symlink or execution-only copy
is sufficient). Build both archived hosts and one common client; use that exact
client binary for both hosts.

```sh
git archive --format=tar -o "$OUT/base.tar" d127acd1f0e9db2100c8a1dd5c2fa43e2deff506
git archive --format=tar -o "$OUT/candidate.tar" 551cb1030ea2bf84f83bbeb167233b917900929a
cd "$OUT"
git -c core.autocrlf=false apply "$STUDY/harness.patch"
ln -s "$PRIOR_STUDY" shared248
python3 build.py shared248 base
python3 build.py shared248 candidate
python3 client_build.py
python3 run.py candidate pilot-candidate-3 500 --pilot
python3 select.py "$OUT" preflight.json pilot-candidate-3 --preflight
python3 check_controls.py
python3 reconstruct.py
```

The single bounded `official-density-target` is removed after binary copying and
pinning. The actual run also removed it between host and common-client builds
to bound disk use; this does not change the runtime sources. Preserve failed
builds/runs, and never reuse an existing run output directory.

Start `core_cpu.py` before primary collection and `stages.py` afterward. For SSH,
detach stdin, stdout and stderr to task-owned logs with a new process session;
the collector must survive a lost reporting connection. No manual repinning is
needed: every simulation/socket task is pinned to vCPU 0, client workers to
vCPUs 1/2/3, and CLI/collector work to vCPU 3. The sampler records vCPU-0 IRQ,
softirq and steal context separately from process CPU clocks.

For a fresh output directory, register detached controllers this way after
preflight and the budget input below are present:

```sh
python3 - <<'PY'
from pathlib import Path
import subprocess, json
root = Path.cwd()
for script, receipt, log in [
    ('core_cpu.py', 'core-controller.json', 'core-controller.log'),
    ('stages.py', 'controller.json', 'cohorts.log'),
    ('diagnostic_stages.py', 'diagnostic-controller.json', 'diagnostic-stages.log'),
]:
    with (root / log).open('xb') as stream:
        process = subprocess.Popen(
            ['python3', str(root / script)], stdin=subprocess.DEVNULL,
            stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
    (root / receipt).write_text(json.dumps({'pid': process.pid}) + '\n')
PY
```

The original long-hold run is retained as a drift control. For a fresh amended
reproduction, provide its recorded host/client CPU seconds in
`drift-control-cpu.json` (zero if the control is not repeated); this is budget
accounting, not a timing input. `stages.py` alternates three independent fresh
cohorts per runtime, re-warms from tick zero, keeps all recipients/sockets active,
requires the same mature screen, discards five seconds and counts 30 seconds.
A separate 15-second syscall diagnostic follows each cohort. Every sampled
census in the window must stay in class. One retry is allowed for class drift,
never for a timing outcome. The controller enforces the six-CPU-hour allowance
and a 45-minute ceiling for each new warmup. No checkpoint restoration is used.

The driver normally runs the full received-frame A4 policy. `run.py`'s `--thin`
option retains decoding, frame materialization, 42 ms inputs and ping/lifecycle
behavior but replaces only the policy; it switches after warmup before settling.
Use it only in a separately retained matched rerun of both arms when the full
clients fail their headroom screen. Never combine different measured client modes.

`select.py` requires complete cases, mature screening, CPU reconciliation,
client headroom/cadence, stable send queues and no unexpected session failures or
write backpressure. It retains aggregates and the values needed for report
tables. Raw tick arrays, logs, machine/process inventories and binaries remain
local-only. Component quantiles are computed separately; add aligned per-tick
values before computing the combined world quantiles.

After collection, create `CORE_STOP`, verify the sampler and all task processes
have ended, recheck source/binary pins and the absent target, and preserve the
complete failed-attempt records. Run documentation tests and privacy/whitespace
checks before publishing only the report, patch and selected results.

For the requested explanatory diagnostics, start `diagnostic_stages.py` only
after `stages.py` is registered in `controller.json`. It waits for all six
accepted capacity cohorts and their CPU receipt before doing any build. It
builds separate archived-copy hosts with `build_diagnostics.py`, removes the
single bounded target, and runs `run_diagnostics.py` for both runtimes. The
ordinary host/client binaries and the primary collector remain unchanged.
Each diagnostic repeats the same tick-zero warmup, then runs a 12-second
`perf_window.py` instruction/cycle group inside a 15-second collection interval,
followed by a separate 20-second coarse phase/event window and a ten-second
fine broadcast-subphase window. The common client
and all affinity/readiness checks stay the same. `core_detail.py` observes the
host vCPU every five ms on a client-side vCPU; counter resolution remains
`USER_HZ`. It shuts down with its runner, including on failure.

`select.py --preflight --diagnostic-windows` checks client headroom, queues and
CPU-clock reconciliation separately for all three diagnostic windows.
`analyze_diagnostics.py` selects PMU/CPU ratios and conditional tail aggregates.
`check_diagnostic_math.py` checks its interpolation and top-1%/2% arithmetic on a
synthetic 72-tick periodic control. Detailed phase clocks introduce overhead;
these windows cannot replace the primary capacity measurements. Account their
builds and two additional warmups against the documented separate two-CPU-hour
diagnostic ceiling, alongside the original six-CPU-hour capacity receipt.
Retain any failure.

After a diagnostic has measured at least 2,000 food-maintenance event flags,
run `primary_tail.py "$OUT"`. It first verifies the source-derived floating-point
food cadence against every available diagnostic flag. It then reduces the
unchanged primary traces into top-1%/2% CPU and cadence aggregates, requiring
their p99s and tick counts to match the original accepted selection. This
read-only analysis runs outside counted windows. It corroborates primary tail
periodicity without importing fine-timer or detailed-logger overhead.

The first diagnostic reduction exposed an unbracketed final telemetry edge.
Diagnostic selection uses only complete blocks with surrounding task telemetry;
the revised runner takes an extra post-window snapshot before cleanup. The
primary selection path is unchanged. Keep its original failure record.

`repeat_base_diagnostic.py` is the one body-class repeat used in this session,
queued after the initial base/current diagnostics, retaining all previous CPU
against the same two-hour diagnostic ceiling. It is optional on a fresh run:
invoke it only when the first base PMU window has left the primary class.
It reads this session's resumed-controller receipt,
`diagnostic-controller-boundary-resume.json`; a fresh run has only
`diagnostic-controller.json`, so point the script at that receipt instead. In
this session the repeat qualified for length but failed client cadence, so its
host measurements are excluded. The final explanatory selection is therefore:

```sh
python3 analyze_diagnostics.py "$OUT" diagnostic-base-1 diagnostic-candidate-1
python3 primary_tail.py "$OUT"
python3 reconstruct.py
python3 check_diagnostic_math.py
python3 check_boundaries.py "$OUT"
```

Label the first base PMU window as drifted. Never silently substitute the
rejected repeat, relax its 42-ms client gate, or claim a matched-body PMU pair.
