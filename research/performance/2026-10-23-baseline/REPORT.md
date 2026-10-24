# Initial 0.2.0 cost report, October 23, 2026

This is a historical study, separated from the maintained guides without new
measurements or changed conclusions. Source paths and local output addresses in
the record refer to the original study, not this archive directory. Full raw
outputs are local-only references, not downloadable evidence in this checkout.

The original workload and metric definitions are [frozen in the source guide](https://github.com/NikitaMGrimm/slither-sim/blob/0ab867149da6a3f0ae3242aab4b8df6950e965eb/docs/performance.md).

The [selected measurements](results.json) retain individual
samples, complete workload configs, phase profiles, source/toolchain/binary hashes
and trace digests. Raw outputs remain in the performance worktree's ignored
`outputs/perf-bb56d94/baseline-1`. Runtime sources match `bb56d94`; harness commit
`3129fcf011df5fa5361280ce700ffcc62baf587e` adds only measurement files.

Machine: Ryzen 7 9800X3D, 8 cores/16 logical CPUs, approximately 31 GiB RAM,
Windows 11 Pro 10.0.26200. Toolchain: rustc 1.99.0 (`b940084d7`, LLVM 23.1.1),
Cargo 1.99.0, Python 3.12.9. Locked release build, task-owned target, two build
jobs, no `RUSTFLAGS`. Binaries were copied and SHA-256 pinned before timing.
Health and research peers agreed to hold CPU-heavy activity for the initial
window around 14:28–14:30 UTC. Pre-run aggregate CPU was 10%; this was a shared
desktop, not an isolated machine. Repeated wall times varied substantially.
Treat this as an exploratory cost baseline, not a stable speed comparison.

Each native sample contains 1,000 iterations; three measured fresh processes
followed one untallied warmup. Inner wall and throughput use the median total
operation time; CPU uses median process total; peak is the largest process working
set; p99 is the median of per-run p99 values. CPU totals include setup, reset and
teardown. Particularly short CPU measurements are too quantized for comparisons.

| Case | Inner wall, ms | Operations/s | Process CPU, s | Peak MiB | p99 operation, µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| step-default | 2.96 | 337,804 | 0.015625 | 4.68 | 18.4 |
| step-food | 103.89 | 9,625 | 0.109375 | 6.60 | 717.9 |
| step-body | 44.35 | 22,546 | 0.062500 | 5.09 | 134.8 |
| observe | 1,715.01 | 583 | 1.593750 | 7.66 | 4,579.9 |
| portable | 600.75 | 1,665 | 0.437500 | 7.09 | 2,759.0 |
| protocol | 222.30 | 4,499 | 0.234375 | 6.58 | 1,031.5 |

These operations do different work: portable iterations advance six ticks and
produce eight frames; protocol iterations reconstruct nine frames from a tape.
Do not compare their iteration rates as equivalent simulation throughput.
All step samples retained 4/32/64 living snakes with zero deaths. Food minima
were 316/11,213/350 respectively; dense food started below its 12,000 attempts.
Body counts and final growth/boost changes are retained in the JSON report.

The 100-decision CLI cases used three samples plus warmup. Local portable CLI
wall time was 1.164 s median (client CPU 0.265625 s, peak 8.51 MiB, response p99
30.97 ms). Hosted wall time was 4.875 s median (client CPU 0.171875 s, peak
7.75 MiB, response p99 68.33 ms). The client was playing for all 100 local frames
and 100/100/99 hosted frames. Real-time hosted conditions differ between samples;
these response latencies include caller polling and scheduling. The host process
used 0.0625 CPU s median and peaked at 6.29 MiB; client and host peaks are separate
process lifetime maxima, not a simultaneous combined-memory measurement.

The separate phase profile found food pickup at 62.07 ms and ambient refill at
19.45 ms of 96.92 ms for 1,000 dense-food ticks. The body case spent 32.39 ms of
50.72 ms in collision processing: 4,032,000 candidate pairs, zero candidate narrow
body anchors. This measures bounds/pair overhead, not expensive contacting bodies.
The profiles include instrumentation overhead and are not the unprofiled totals.

Ranked next work, considering measured cost, scope and parity risk:

1. **Collision bounds lookup:** remove per-pair binary searches of stable living
   slots. The body profile supports investigating this small change first; keep
   join order, broad-phase arithmetic and narrow geometry unchanged. A speedup
   remains unproven until matched runs.
2. **Observation materialization:** the immutable batch costs about 1.6 CPU s per
   1,000 iterations. Profile index/filter/sort/clone costs before changing it;
   visibility, ordered food and detached observations are contract constraints.
3. **Food scan/refill:** the dense-food profile makes this the main core cost for
   that fixture. Investigate scans and allocation; preserve tie order, RNG
   consumption and public food mutation support. No numerical retuning.
4. **Portable broadcast/frame construction:** separate encoding, receipt state,
   recording and frame copies before optimizing. The initial tape is too short
   to characterize long-lived reconstruction or minimap delta costs.

For the longer follow-up workload, see [retained client processing](../../../docs/performance-workloads.md#retained-client-workload).
