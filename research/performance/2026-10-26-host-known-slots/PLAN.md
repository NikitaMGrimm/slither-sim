# Host known-slot enumeration

Frozen after the fresh main profile and before candidate timing, 2026-10-26.
Synthetic simulator traffic over loopback only; no AWS access.

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
