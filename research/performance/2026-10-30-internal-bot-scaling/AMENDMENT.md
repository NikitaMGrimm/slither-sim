# Maintainer update before the remaining counted rows

The maintainer requested bounded completion, achieved speed even if 10k cannot
run in real time, slow-tick sampled attribution at both 10k and botstorm-12k,
and optional botstorm rows only when 12k misses real time. This amendment was
written during 10k warmup, before any 10k or botstorm counted window.

- Keep the original 10k warmup guard and its qualification verdict. Do not run
  the previously proposed extra warmup in WARMUP_FOLLOWUP_PLAN.md. If the guard
  fires, count the same verification, five pairs and phase diagnostics at the
  final checkpoint, explicitly labelled **unsettled ecology**. Never select a
  snapshot by timing. No further warmup retry for performance.
- Apply the same bounded-snapshot rule to botstorm if its original warmup screen
  fails. It still gets five counted continuations, with the failed screen shown.
- Run botstorm 6k, then 9k, only if 12k's median whole-window unpaced throughput
  across five counted windows is below `1000/7` ticks/s. This includes the cheap
  center-policy loop; world and admission costs are also reported separately.
  Report per-tick 7-ms deadline misses separately. This supersedes the earlier
  optional 30-wall-minute cost gate, without changing policy, radius or density.
- Add a separate debug-info diagnostic for A4 10k and botstorm 12k. Study-only
  monotonic timestamps bracket admission and world work; perf uses the same
  clock. Retain all-world and slowest-1% world+admission CPU tick samples, with
  unresolved stacks explicit. Measure timestamp/debug and sampling overhead
  against ordinary rows; do not replace the portable counted measurements.
- Explicitly test the pickup grid's fixed/food-relative limits at both radii.
  Read-only checkpoint guard arithmetic alone is snapshot evidence. Combine it
  with sampled stacks and per-tick phase/work counts; distinguish inference
  from a directly observed per-tick branch.

Use “a 4-vCPU Zen 5 EPYC KVM guest” only. Describe the anonymous reported
comparison as “a 12k-snake single-core reference world (64,800 across,
center-steering bots)”; its hardware and older tick rate remain unknown.
No production source changes or implementation proposals are adopted.
