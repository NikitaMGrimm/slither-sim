# Small-server check, 2026-10-27

Frozen before measurement. Compare v0.7.0 `4c2471b` with
`aee542100496b8cbc031b8ab19bb58e78bd8af2b`, identical Rust 1.99.0
locked release profile, debug=1, thin LTO and one codegen unit.
Synthetic local fixtures; no outside servers or machine configuration changes.

- Reuse the #204 protocol fixture: seed 7, 1,080 ticks, controls every six ticks,
  retry dead slots every 72 ticks, forced disconnect/rejoin at 720. Change only
  preset and seats: official 1/10/50/100, Battledome 8/16/32. Preserve host
  seat-based world initialization. No client bot policy or sockets in this arm.
- One warmup pair, then five alternating whole-process `instructions:u` pairs
  per size, including construction and drains. Require every counter >=99%
  scheduled; retain failures. No speed-based exclusions or replacements.
- Separate current-thread tick brackets: all 1,080 ticks and post-warmup ticks
  >=120, p50/p99/max by exact nearest rank. Keep death/reconnect/map bursts.
  Report the bracket overhead; it is not the whole-process counter scope.
- Full trace comparison at all seven sizes, including the requested official-10
  and Battledome-16, covering frame bytes/order, modeled timestamps,
  checkpoint state and RNG endpoints.
- Fixed gateway work: 4,000 passes at 0/1/2/5/10 idle loopback sockets;
  one warmup pair and five alternating user-instruction pairs. Inspect the
  no-socket fast path and account separately for kernel cost if measurable.
- Inspect pellet-index storage and per-broadcast/legacy dispatch overhead.
  If the net comparison regresses, isolate the responsible change with a
  narrowly matched control; do not change production code in this PR.
- Descriptive loopback holds: official stock 10/50, protocol-13 10, seed 7,
  10 s warmup +30 s hold, three alternating pairs per case. Reuse the existing
  drain-only legacy-capable load with identical steering/boost controls in
  both arms. Tick busy spans command intake through handover. Frame excess is
  receipt minus cumulative elapsed-header time, minus that connection's
  minimum within the hold (the EPYC recheck definition). Retain occupancy,
  traffic, errors, generator busy time and deferred ticks. Reuse input probes
  if practical; no capacity grade or claim of latency equivalence.
- Pin all task work to at most two logical CPUs; host/count/build on one,
  loopback load on the other. Cargo/Rayon/tests one job. Shared-machine WSL2
  timings are descriptive, even when instruction counts are repeatable.
- Raw arrays, traces, source archives, binaries, logs and environment records
  remain outside Git. Publish selected results and hashes only, with no
  personal paths or machine identifiers. Checks must work under Python `-O`.

Expected alternatives: optimizations may save work at every size, or fixed
maintenance may outweigh savings on small scenes. Lower aggregate work does
not guarantee lower p99/max tick work or delivery/input latency.

Premeasurement harness correction: an uncounted baseline pilot admitted only
14 simultaneous initial lives in Battledome-16, failing the borrowed fixture's
all-initial-admissions assertion. Preserve that pilot and change the fixture to
retain refused initial admissions, then use the existing 72-tick retry policy.
Report configured seats and actual population separately; no arena widening or
spawn-policy change. The official-10 pilot succeeded. Optional buffered input
probes are included for client IDs divisible by 64 (one client in these holds).
The first WSL build attempt could not resolve the Windows worktree's Git path;
commit-verified archives avoid changing the checkout.

Exploratory follow-up after the primary instruction runs: total fixture work
fell at every size, but the zero-socket gateway increased and official first-tick
work increased. Run one main-source diagnostic ablation that omits the readiness
snapshot in the gateway and builds the pellet index before tick zero. The two
changes affect separate executables/scopes. Compare zero-socket instructions in
one warmup plus five alternating pairs; inspect first-tick distributions and
require an official-10 full trace match for the prebuilt-index control. This is
attribution work, not a proposed fix or a new selected primary comparison.
The readiness ablation explained only part of the empty-pass increase, so a
second zero-socket-only ablation restores the old retain call after readiness
is omitted, isolating #208's remaining delivery-index maintenance. Use the same
warmup/five-pair counter gate; it is not a functional host configuration.
