# Hosting sizing samples

Frozen before collection on 2026-10-26. Question: what host CPU and peak RSS do
short loopback socket workloads consume on one desktop core? These are sizing
samples, not a capacity qualification or a comparison of implementations.

- Runtime: `249cdb217d83a234895bc7eee4d98c3636c1cd76`, unmodified.
- Rust 1.99.0, Linux x86-64 under WSL2, locked release build, default features,
  thin LTO, one codegen unit; `CARGO_BUILD_JOBS=1`, `RAYON_NUM_THREADS=1`.
- Reuse unchanged `host_probe.rs` and `host_capacity_load.rs` from
  `research/performance/2026-10-26-host-capacity/`, registered as examples only
  in a temporary source snapshot. `examples/host_load.rs` measures background
  bots and a watcher, so it cannot supply the requested socket populations.
- Sequential order: Battledome 80, official 250, official 500 sockets. One
  attempt per case; 15 s warmup, 30 s measured hold, 2 s host teardown grace.
  A failed attempt remains recorded. No outside game servers.
- Host pinned to one logical CPU, one single-threaded generator to a second.
  Shared desktop load is uncontrolled. No background bots or added latency;
  seed 7, default preset and delivery. Do not tune admission or world settings.
- Sample host and generator `/proc/PID/stat` at 100 ms intervals. CPU is
  process user + system seconds across the hold, divided by elapsed wall time;
  100% means one logical CPU. Peak RSS is host `VmHWM` from process start
  through the end of the hold, including warmup. Record generator CPU separately.
- Keep requested count, mean/final playing population, admissions, errors,
  closes, traffic, executed/deferred ticks and simulated seconds. Verify
  gameplay packets were decoded during the hold; report failure or depletion
  beside resource numbers rather than calling it capacity.
- Preserve full local outputs outside Git; commit bounded summaries and source,
  harness and executable hashes. No fitted hypothesis or held-out claim, so
  repeated comparisons and held-out validation are inapplicable.

The Battledome admission implementation is being changed separately. Its
80-socket result must disclose the observed playing population and be refreshed
after that work; it is not evidence of 80 simultaneously living snakes.
