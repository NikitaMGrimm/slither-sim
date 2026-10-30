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

## Pickup-guard clarification before diagnostic execution

The maintainer requests all three decline conditions, not just the fixed cap.
Use existing public event recording in the already planned diagnostic copies.
After a complete step, head, heading, speed, integer segments and alive status
still equal their pickup-phase values: later food/prey gains change fam and
derived score/radius, not these circle inputs. `kiosk.finish_score` changes only
displayed length/score. Source pins bind these statements to this runtime.

Pickup-entry food length is exactly final food length plus Food pickup events
minus ambient birth events. Boost/death births occur before pickup and have a
source; ambient births follow it and have no source. Event recording is
non-authoritative and excluded from checkpoints. Require diagnostic final-state
identity with primary rows and identical guard traces across plain, sampled
and profiled diagnostic continuations.

After the world clock's end, fold the unchanged circles with the same 128-unit
cover arithmetic. Record box cells, input food length, summed cover area,
the fixed and food-relative limits, each decline flag, and derived dense/
hashed-indexed/linear branch for all 512 ticks. The actual food-relative limit
is `max(4*food.len(),4096)`; report that floor rather than silently omitting it.
The normal worlds are within the finite geometry/u32-count guards; retain those
checks too. This adds no timing run and changes no `src/` file. Event-recording
overhead inside step and audit work outside it are included in the separately
reported diagnostic/primary instrumentation ratios.
