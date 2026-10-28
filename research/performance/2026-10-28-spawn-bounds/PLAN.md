# Admission bounds plan, 2026-10-28

Baseline: current main `0aba6f97070a06c73b604869761d4c5c14965a78`.
Compare unchanged exact predicates/order against cached per-snake boxes.
Expect less admission work at high populations; make no capacity, latency,
official-server, or playing-strength claim. Keep realistic small/500-seat
results even when admission is negligible or total counts increase.

Use a 4-vCPU Zen 5 EPYC KVM guest, Rust 1.99, locked portable release builds
(thin LTO, one codegen unit, debug=1), one Cargo/Rayon/test worker. Count each
process on one logical CPU. Finish checks/builds before measurement. Retain
an untallied warmup pair and five adjacent alternating pairs, including every
failed attempt. Record enabled/running PMU time; require at least 99% scheduling.

- All deterministic rows in the existing full workload catalog: use its
  registered trace and sample sizes, literal full-file parity and terminal
  report, five whole-process `instructions:u` pairs. Socket rows have no
  deterministic trace. Supplement them with five alternating diagnostic pairs
  of the catalog's 286-tick loopback host windows, driver on a separate CPU;
  count whole simulation ticks and nested spawn search on the simulation
  thread. Their wall-clock-driven worlds are unpaired trajectories, so those
  counts establish admission share, not a behavior-preserving speed estimate.
- Separate diagnostic matrix runs bracket measured operations and client
  admission disjointly. Divide client decisions by six physics ticks; report
  admission and their combined total per tick. Diagnostics include PMU hooks;
  uninstrumented process pairs establish the overall instruction result.
- Reuse #242's harness from `cae1d7a5a9c9e44f2f9565164a410020fbb7fd0b`
  and its existing warmed worlds. Keep its memory-bounded checked importer,
  driver, 100 untallied continuation ticks, admission search and controller.
  Count the original production admission path; never set its fast-admission
  switch in measured windows. Add only a shared count-length parameter:
  1,000 measured ticks at 2,000; 20 at capped 5,000 because the full baseline
  window is disproportionately expensive. Compare complete endpoint checkpoint
  and driver bytes plus every tick's lifecycle/population. Preserve the shorter
  5,000 window as a limit, not a long-run estimate.
- Exhaustive bit/boolean oracle with long/crossing bodies, contact/clearance
  boundaries, excluded slots, wall margin and unusual coordinates. Exercise
  all public geometry edits, lifecycle, clone/restore and between-tick births.
  Reverse broad-phase rejection deliberately; require an oracle failure, then
  restore and rerun it. Run the repository Rust check group and docs test.

Keep raw logs, counters, source snapshots, binaries and compressed matrix
traces outside Git; publish selected counts, identities, reproduction and
limits. Remove task-owned Rust targets after verification. Do not disclose
machine/user identifiers or absolute bench paths. No outside game servers,
system changes, release, or merge.
