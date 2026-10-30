# Reproduce v4

Follow [PLAN.md](PLAN.md), with collection code frozen at
`2abc874e275c7e9ed62d7cb358fc9cc8efe1d6d2`. The initial plan predates
provisioning; its documented systemd verifier corrections precede tick zero.
Do not reinterpret failed v3 or change the acceptance gates.

Reuse the linked v3/v2/v1/c7a/#256 staging and runner instructions from the
[pinned v3 plan](https://github.com/NikitaMGrimm/slither-sim/blob/bbc7d18c9d5052903bf3b07ba4067a48e8f040d5/research/performance/2026-10-30-official-500-acceptance-v3/PLAN.md).
Add v3's `check_replay.py` from `651f9526`, and overlay this study's
`host_agent.py`, `quiet_image.py`, `host_diagnostics.py`. Keep generator code,
the nearest-rank helpers, config, socket behavior and file telemetry unchanged.
Verify the exact portable host/client hashes in the plan. This execution reuses
v3's verified binaries; no new Rust build or Cargo target was needed.

On each fresh generator before its world, run [check_host.py](check_host.py)
from the staged root, then the inherited controls:

```sh
taskset -c 6 python3 check_host.py
taskset -c 6 python3 check_headroom.py
taskset -c 6 python3 check_gates.py
taskset -c 6 python3 sharedv1/check_quantiles.py
```

The public focused check is byte-identical to `v4_preflight.py` in the retained
pre-world proof archive. It covers valid cached-load/runtime-mask state,
rejected active/unmasked state, protected service exclusions and live all-process
CPU/full-proc-stat capture. Documentation checks also run on AWS before worlds.

Provision only within the plan's tagged two-pair, on-demand, timer and $3 bounds.
Start the host wrapper, which applies the host-only policy before starting the
server/observer/diagnostic process. Confirm readiness before starting the
unchanged generator sequence. No manual service changes after tick zero.
During warmup, retain an additional pre-window state/timer snapshot:

```python
from pathlib import Path
import json
import quiet_image as q
root = Path('host-run/image-policy')
policy = json.loads((root / 'policy.json').read_text())
state = q.record(root, 'pre-window', policy['recordedUnits'])
q.verify(state, policy['maskedUnits'], policy['servicesStopped'])
```

Run this with host permissions before the counted window, allowing the unchanged
ten-second settle. During counting leave only the frozen sampler/observer and
normal workload. The wrapper stops and verifies units after the world and retains
the journal before making the host bundle available. Preserve all raw process,
start-time, complete `/proc/stat`, journal and unit/timer data outside Git.

After the full archive is closed, on the generator, enforce the entire result
comparison **before** the inherited selector, then reduce the descriptive host
context with [diagnostics.py](diagnostics.py):

```sh
taskset -c 6 python3 check_replay.py . results.tar.gz full-replay.json
taskset -c 6 python3 select.py
taskset -c 6 python3 diagnostics.py results.tar.gz host-diagnostics.json
```

Retrieve/hash raw archive, selected result, full-replay and diagnostic receipts.
Verify first-pair teardown before launching the second fresh pair. Keep both
complete results regardless of pass/fail. No replacement, retry or exclusion.
Run the inherited v1 `assemble.py` on the two selected files and `tables.py` on
its output; set market to `On-demand` and record this study's frozen collector
commit without changing run values. Keep all gated fields and tails intact.
The extra host context uses full one-second covering intervals, matched process
CPU and signed global/task counter residuals; none is an acceptance gate.

Final documentation checks run on AWS after timing. Delete task instances,
volumes, key and group, scan every enabled region, preserve unrelated resources
and record capped-rate lifetimes plus the full noncompute reserve. Publish only
the selected results, proof/archive/access bindings, cost/cleanup and report.
No cloud identities, raw journal/process logs, root README or changelog change.
