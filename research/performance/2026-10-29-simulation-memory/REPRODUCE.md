# Reproduce the diagnostic

Run on an authorized idle Linux guest with existing Rust 1.99, locked cached
dependencies, Python 3.12+, a C compiler, binutils and permission to open
current-thread user PMU events. Install nothing and change no system settings.
There are no sockets or outside-server connections. Run builds, controls and
reductions before or after measured windows, never concurrently with them.

The runtime is main `539b37f3be969827b5eac67c2978e604f9e814e8`; the plan was
committed as `7260e8d3` before collection. The fixture is borrowed from
[mature covered bounds](../2026-10-28-mature-covered-bounds/fixture.rs), with the
same complete-backend fork recipe described in its [reproduction guide](../2026-10-28-mature-covered-bounds/REPRODUCE.md).
Do not substitute a physics checkpoint: it omits presentation lifetimes, hidden
births, recipient-held listings and sector interest.

From the checkout, prepare a fresh external study directory:

```sh
study=/tmp/slither-simulation-memory-reproduction
mkdir "$study"
git archive 539b37f3be969827b5eac67c2978e604f9e814e8 > "$study/main.tar"
cp research/performance/2026-10-29-simulation-memory/build.py "$study/"
cp research/performance/2026-10-29-simulation-memory/run.py "$study/"
cp research/performance/2026-10-29-simulation-memory/reduce.py "$study/"
cp research/performance/2026-10-29-simulation-memory/profile-repeat.py "$study/"
cp research/performance/2026-10-29-simulation-memory/select.py "$study/"
cp research/performance/2026-10-29-simulation-memory/tables.py "$study/"
cp research/performance/2026-10-29-simulation-memory/instrumentation.patch "$study/"
```

[build.py](build.py) extracts that archive, applies the measurement-only
[instrumentation](instrumentation.patch), transforms the borrowed fixture and
builds one example. It uses one target, `/tmp/slither-simulation-memory-target`,
portable release/thin-LTO code generation and debug symbols. It does not set
affinity or `target-cpu=native`. Verify the target is absent or belongs to this
task before starting; remove it after preserving results.

```sh
cd "$study"
python3 - <<'PY'
import json,time
from pathlib import Path
start=time.time()
Path('session.json').write_text(json.dumps({
    'startedUnix':start, 'measurementStopUnix':start+6600,
    'deadlineUnix':start+7200,
    'runtime':'539b37f3be969827b5eac67c2978e604f9e814e8',
    'frozenPlanCommit':'7260e8d3'}))
PY
python3 build.py
cc -O2 -DPROBE source/src/study_events.c -o event-probe
./event-probe > events.json
python3 run.py
python3 profile-repeat.py
python3 reduce.py
python3 select.py "$study"
cp tables.py selected/
python3 selected/tables.py
```

The two controls in `event-probe` perform eight million dependent loads over
4 KiB and 64 MiB arrays. Open failures retain errno; enabled/running times and
counts retain units. These controls test responsiveness, not the KVM event's
complete architectural semantics. No `perf` executable was available in this
session, including the existing alternate locations checked before collection;
no `perf stat -M` metrics could be collected. No L2 alias was exposed, and the
generic LLC-load-miss selector failed with `ENOENT`.

[run.py](run.py) warms stock and legacy separately to tick 193,000, at full
speed, then forks the complete backend for every 1,080-tick window. It records
redacted full process commands at each boundary. Each child opens fresh
task-bound events; the frozen parent never opens them. Instructions accompany
each of cycles, branch misses, L1d load misses, dTLB load misses, generic cache
misses and frontend stalled cycles. Three repetitions precede four separate
IP-sampling windows per protocol. [profile-repeat.py](profile-repeat.py) applies
the committed [sampling correction](SAMPLING.md), warming fresh identical
backends and repeating only L1d/generic-cache samples at one tenth the original
rate. Both series remain separate; no counter-only result is replaced.
Events surround `advance_tick_with` only;
policy decisions, respawns, frame draining, footprint serialization and result
serialization are outside. CPU clocks include wrapper overhead; no subtraction
or capacity comparison is made. Actual food-epoch transitions split ticks.

Sampling uses the same user-only event selectors, period 1,000,003 for cycles
and initially 1,009 for each miss event (10,007 in the corrected L1d/generic
profiles), a 1 MiB data ring, and per-tick drain after events
are disabled. Enabled/running time, lost records and throttle/unthrottle records
are retained. Samples have instruction pointers, without precise-IP support,
data addresses or call chains. `addr2line -i` provides static inline stacks;
recorded mappings and existing dynamic symbols identify shared-library samples
where possible. Interpret attribution as approximate overflow-IP location, not
an exact count of loads performed by each source line.

[reduce.py](reduce.py) requires the terminal `DONE`, checks same-protocol fork
summaries and scheduled fractions, and writes the selected counter, profile and
footprint results. Every repetition remains represented; no outcome-based
exclusions or speedup tests are applied. Profiles and counter-only runs are
kept separate. Footprints are untimed at the frozen start, deduplicating listing
`Arc` identities across world tables and all recipients. Counts/capacities are
measured; tree/allocator overhead and dynamic bytes actually touched are not.

The original build script read the identical retained donor fixture from the
preceding study's external directory and wrote the instrumentation directly.
The packaged builder references its committed copy and applies a patch instead.
The reconstruction check compares all generated Rust bytes and the C collector
against the measured sources, without publishing per-file hash maps.
[select.py](select.py) uses that original receipt when present; a fresh
reproduction instead computes and checks the published aggregate Rust/collector
identities directly, without needing a hand-written receipt or the old directory.

One initial collector build and partial stock warmup were stopped before any
mature measurement. The replacement records sampler scheduling time and disables
counters before draining samples. The initial attempt is preserved in the
local-only raw bundle; no measured result was overwritten. Raw binaries,
per-tick rows, mappings, process inventories and logs remain local-only.
