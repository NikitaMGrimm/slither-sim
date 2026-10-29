# Mature simulation memory diagnostic

Runtime: current main `539b37f3be969827b5eac67c2978e604f9e814e8`.
Question: do cache/TLB misses or pointer-heavy working sets explain the mature
simulation thread's low IPC, and which portable layouts deserve investigation?
This is diagnostic-only: no production change, capacity claim or fitted rule.

Use the existing frozen full-backend fixture, no sockets, 500 players, stock and
protocol 13. Warm one backend per protocol at full speed to tick 193,000, then
fork that complete backend for fixed-work repetitions. Preserve hidden food,
held listings and sector-interest state. Reuse earlier fixture/build helpers by
reference, changing only diagnostic instrumentation and execution flags.

All builds, tests, traces and reductions run on the SSH guest. Use one bounded
Cargo target, existing offline dependencies, no installs or system/security/limit
changes. No outside game servers or AWS. Record redacted complete process
commands at measurement boundaries. Keep the guest otherwise idle while counting.
Bound the guest session to about two hours, reserving the last ten minutes for
checks, archival and cleanup; stop new measurements at 110 minutes.

Use portable release code generation, not `target-cpu=native`, SIMD intrinsics or
CPU-specific runtime paths. Do not pin threads, including in this diagnostic's
driver: task-bound PMU events follow the single simulation thread. Report any
resulting scheduling/frequency limitations. Architecture-specific event names
may be observed as diagnostics only, never turned into a production dependency.

First inspect existing `perf` executables, sysfs event descriptions and permitted
`perf_event_open` events. Do not install perf. Try instructions, cycles, branch
misses, L1d load misses, the closest exposed L2/LLC event and dTLB load misses.
Record each exact encoding, open failure, time enabled/running and multiplexing.
Use small cache-resident and larger streaming/pointer controls to detect an
exposed event that never counts; zero unsupported data is not evidence of no
misses. Try Zen top-down metrics only if the existing perf/PMU offers them.

Collect at least three fixed-work repetitions per protocol where the budget
permits, with maintenance identified by actual food-epoch changes. Keep whole
tick and maintenance/ordinary counter totals separate. Counter wrappers and
profilers have overhead; do not mix their CPU times with uninstrumented capacity
results or sum nested budgets/quantiles.

Attribute miss samples to functions with `perf record` when available. Otherwise
use task-bound overflow samples with instruction pointers, or coarse function
counter brackets if sampling is unavailable. If misses cannot be sampled, label
cycle samples as a CPU profile rather than a miss profile. Record lost samples,
event scheduling and symbolization limits. Avoid attributing fixture serialization
or policy/control work to simulation: events are enabled only inside advance.

Measure type sizes, lengths/capacities and unique shared allocations in an untimed
snapshot, including snake bodies/bounds, known-snake tables, sector listings and
recipient state. Separate measured payload/capacity bytes, allocation/container
overhead estimates and transient buffers. Deduplicate shared listings. Estimate
the touched set from source loops and observed counts; do not equate RSS or total
retained bytes with a tick's working set. Compare descriptively with generic
256 KiB, 512 KiB, 1 MiB and 2 MiB private-cache sizes, without tuning for one size.

Rank only portable candidates: compact/contiguous layouts, fewer allocations and
pointer chases, smaller touched sets. Support each with measured events/profile
and footprint/access evidence, or explicitly label it a source-derived hypothesis.
Expected effects may be estimated bytes/accesses removed; do not invent speedups.
Unavailable counters cannot establish either memory-boundness or its absence.

Keep all failures/raw profiles/binaries local-only and bounded. Retain selected
values and what recomputes tables, with whole-artifact pins rather than per-file
source-hash maps. Keep instrumentation as a patch; link earlier helpers instead
of copying them. Open a short research PR only if the diagnostic findings are
worth preserving, with only the assistance footer in its body. Do not merge.
Verify copied archives before pruning task-owned files, remove the target and
stop task processes. Tell the maintaining agent when the guest is released.
