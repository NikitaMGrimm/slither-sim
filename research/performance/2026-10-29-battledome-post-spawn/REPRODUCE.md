# Reproduce the post-spawn live comparison

Use [#267's full recipe](../2026-10-29-battledome-live-recheck/REPRODUCE.md) and
its linked #263/#262/#258 helpers by reference. This study changes only runtime
pins, primary order/counts and the portable profile's debug/flag settings.
Keep failed attempts, closed archives and complete source manifests private.

Archive base `3a223676` and candidate `a1535902` with `git archive`. Resolve
full hashes from [runtimes.json](runtimes.json). Stage each through #267's
`prepare.py`, preserving its boost treatment, `main.tar` and source manifest.
Build remotely with the repository's default portable release settings,
`RUSTFLAGS=''`, no encoded flags and release debug=0. Clear this package's
artifacts between arms, copy/hash both executables and retain full command
logs. Require identical shared A4/client source hashes and pass the native
`high_duty_preserves_heading_and_rule_gate` regression in both builds. Use
the candidate client binary for every server arm.

Stage this study's [run_row.py](run_row.py) as each row entry point, with #267's
original `run_row.py` under `shared267/`, plus the existing shared262/shared263
modules and metrics/reduce/delivery files. Keep #267's `host_agent.py`, which
releases its control port after final bundle retrieval. Use fresh row folders,
private endpoints in `job.json`, the selected host/candidate client under
`bin/`, and bound `pins.json`. Host configuration and six fully decoding
workers remain unchanged. Verify one target vCPU, generator physical topology,
both terminate timers and delete-on-termination volumes before collecting.

Follow [PLAN.md](PLAN.md). Do not pool calibrations into primary results or
discard overloaded server rows. Every client/queue/boost gate must pass. Do not
relax the 42-ms lateness gate or use a thin generator. Retain whole-run deferrals
outside marked intervals. Fresh worlds have matched scheduled wall prehistory,
not fixed simulation age or identical ecology.

After terminal archive readiness, immediately retrieve and size/SHA-verify the
closed row archive. Run reduction **on AWS**: extract that archive into a fresh
`raw/`, verify required files, and write a complete source-verified retrieval
receipt beside the archive. Use [select_results.py](select_results.py), which
wraps #267's archive-bound canonical selector with the new runtime constants:

```sh
python select_results.py ROW/raw ANALYSIS_HARNESS ANALYSIS/LABEL.json ARM pins-master.json
```

The analysis harness holds this arm's `main.tar` and the linked reducer,
metrics, shared248 and shared256 helpers. Replay original online ranges
exactly; require canonical task-clock reconciliation, complete inventories,
decoding/headroom/queue gates and received boost duty. Selection records the
archive digest at selection; [assemble_results.py](assemble_results.py) requires
that same digest to match the complete receipt and actual archive. Write
`order.json`, `generator-segments.json` and `not-measured.json` from verified
collection state, then assemble remotely. Each generator segment has its own
topology/market; no last-generator counts describe other rows.

Run [tables.py](tables.py) on `selected-results.json`. Keep all cited means,
quantiles, slow-tail partitions, calibrated boundaries and deferral ledgers.
Run #262's synthetic `check.py` against the unchanged reducer/delivery modules,
and documentation checks, on AWS after client counting stops. Verify the full
native-source and final analysis archives (closed tar/gzip, every member hash)
before readiness/retrieval, and verify downloads before source deletion.

Terminate task instances, cancel task Spot requests, delete task key/group
and scan every enabled region through the limited profile. Never touch
unrelated or protected resources. Stop on denied/expired credentials instead
of changing profiles. No owner play hold, local work or SSH guest time.
