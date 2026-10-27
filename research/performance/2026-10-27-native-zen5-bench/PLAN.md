# Native Zen 5 one-core bench plan, 2026-10-27

Frozen before qualification. Measure the as-shipped host at v0.7.0
`4c2471ba9afff3609faae964cdaad49e3209da68`, #211
`65ff713731cbd436bf5c19a25705fa77b0ebedcc`, and main at setup
`9960f4f84d1b831d4468486b1e0524710b7b9c45`.
This is a bounded capacity observation, with no prediction of a winner or
statistical speedup claim. No runtime source changes are proposed.

Machine description for publication: a 4-vCPU Zen 5 EPYC KVM guest (Debian 13).
Never export its hostname, kernel suffix, provider, account names or addresses.
Use only loopback, no TLS, no artificial latency, seed 7, zero host bots,
default presets. All files stay under `~/slither-bench/` on the measurement
machine; no system packages or kernel/security/settings changes.

## Schedule

Build from local `git archive` snapshots, Rust 1.99.0, `cargo build --release
--locked`, thin LTO, one codegen unit, `CARGO_PROFILE_RELEASE_DEBUG=1`, no
CPU-specific flags. One fresh target at a time, remove it after copying and
hashing binaries. Use the #178 load source with #201 admission accounting and
the existing handoff study's sampled-input tracing. Freeze one load binary
built against v0.7.0 for all arms. Reuse #201 buffered ticks and 1 ms CLI
counters, original frame-excess/CPU definitions and nearest-rank reduction.

An eight-client, three-second-warmup/eight-second official smoke for each host
is excluded from qualification. Then exactly one fresh-world 15 s warmup plus
120 s hold per row, sequentially:

1. Official 500: v0.7.0, #211, pinned main.
2. Official 375: pinned main, #211, v0.7.0.
3. Battledome 80: v0.7.0, #211, pinned main.

Host and every inherited thread are pinned to CPU 0 before startup; eight load
shards are round-robin pinned to CPUs 1, 2, 3. Observer processes use CPUs 1–3.
Read back every extant thread's allowed CPUs and last CPU once per second and
at hold boundaries. Record generator process CPU, per-core aggregate CPU,
poll/control lateness, errors, admissions, and time-weighted playing counts.
Stop qualification if generator saturation compromises the workload; retain the
attempt and declare an adjustment before rerunning. No speed-based repeats.

Grade: excess p99 <=10 ms and no new deferred ticks, conditional on valid
decoded traffic and occupancy. Report CPU ms/executed tick, core CPU %, excess
p99/max, busy tick p99/max, deferred counts, occupancy and peak RSS including
warmup. Sampled enqueue/read/ingest includes match/loss counts. Keep Battledome
admission waits and censored requests. Preserve all failed or incomplete attempts.

Compare descriptively with #201/#211 c7a/c8a holds; loopback on one VM differs
from their two-instance NIC path. One seed and one hold per row do not estimate
variance, causality, long-run capacity, human-browser behavior or larger snakes.
Full raw receipts, ticks, logs, snapshots and binaries stay outside Git; publish
selected sanitized results and raw-file hashes. No outside game servers.
