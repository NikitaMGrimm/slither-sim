# Host known-slot enumeration

Frozen after the fresh main profile and before candidate timing, 2026-10-26.
Synthetic simulator traffic over loopback only; no cloud compute used.

- Baseline: main `ce54c607ad5d98a633f2411ff5f0ea1e6bd2a874`.
- Profile: 500 socket clients, seed 7, default official world, no bots,
  15 s warmup and 45 s hold, WSL2 Linux x86-64. All host threads on CPU 0;
  three load processes on CPUs 2, 4 and 6. Four logical CPUs total.
  Rust 1.99.0, locked release build, thin LTO, one codegen unit, debug level 1.
  `perf record -e cpu-clock:u -F 199 --call-graph dwarf,16384` covers user
  CPU only. Kernel work is excluded. Profile timings are not speed evidence.
- Hypothesis: maintaining ascending occupied `KnownSnakes` slots removes a
  per-recipient scan of all world slots, retaining direct lookup and exact
  ascending recipient packet order. No mechanics, schedules or radius changes.
- Before timing: compare complete decompressed deterministic traces for
  `portable` and `protocol` (32 operations each), `client-official-500`
  (1 decision after its internal warmup), `client-battledome-64` (4 decisions),
  and retained `step_all` and `next_frame` (384 decisions each). Require equal
  bytes, not hashes alone. Retain full traces locally.
- Timing: independently build and copy base and candidate binaries. Use the
  baseline load executable for both. Each fresh host has a 15 s warmup.
  Run one untallied 15 s hold per revision, then five adjacent alternating
  pairs of 45 s holds (base/candidate, candidate/base, etc.). Divide host
  process user+system CPU by executed ticks; retain population and deferrals.
  Report every complete pair and its spread, descriptively, without a
  significance or native one-core capacity claim. Retain failed attempts.
- Keep `CARGO_BUILD_JOBS=2`, `RAYON_NUM_THREADS=2`, bounded affinity and test
  threads. Finish builds and checks before paired timing; no concurrent tests.
- After timing: repeat the separate 500-client user-CPU profile to identify
  remaining costs. Do not infer performance from changed sample percentages.

## Supplementary diagnostic, before its execution

The first three socket pairs have mixed CPU ratios (including a 14.9% candidate
increase). Retain all five frozen pairs. After they finish, probe support for
unprivileged `instructions:u`. If supported, run the unchanged deterministic
`client-official-500` fixture for 100 decisions, one untallied pair followed by
three alternating pairs, on one pinned core. Retain complete perf-stat output
and report whole-process instructions (including identical construction and
internal warmup), separately from socket CPU. If unsupported, retain that failure
and make no instruction-count claim. This is an exploratory diagnostic prompted
by socket timing variability, not a replacement or new gate for those results.

## Owner-added Battledome workloads, before execution

Keep the official-500 round. Add 40, 64 and 80 actual loopback socket clients,
no bots, exact default `battledome` preset (fixed radius 3,584). Use the same
15-second warmup, discarded warmup pair, five alternating 45-second pairs and
separate baseline/candidate profiles for each count. All host threads share
one CPU; three generator shards share the other three allowed logical CPUs.
Never alter admission rules. Both arms use one adapted baseline load binary:
the original steering plus staggered 3-second boost bursts every 8 seconds.
Retain waiting logins until the hold ends, without the old generator's setup
timeout or its three-pending-handshake limit suppressing queued logins. Report
alive and waiting populations, completed/censored login-to-setup waits, boost
controls, CPU/tick, frame excess p50/p99 and deferrals. Frame excess uses the
capacity study's unchanged per-connection minimum normalization. No claim of
serving N living players when admission exposes fewer than N.

Before timing, extend the shared deterministic client fixture to Battledome
40, 64 and 80 with boosting, retaining its existing death/reconnect lifecycle,
and require complete byte equality. Its synthetic initialization differs from
real socket admission; report that boundary separately.

The exploratory instruction diagnostic also includes all three added Battledome
counts, using the shared extended client fixture and the same 100 decisions,
untallied pair and three alternating measured pairs per workload.
