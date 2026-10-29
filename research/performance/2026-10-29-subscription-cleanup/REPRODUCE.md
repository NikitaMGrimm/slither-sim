# Reproduce the cleanup skip comparison

Reuse the [food-diff fixed-work builder and execution helpers](../2026-10-29-food-diffs/REPRODUCE.md)
and its measurement-only `harness.patch`, including the earlier harnesses linked
there. The backend fixture, controls, warmup, stream comparison, fork requests,
counter, trace matrix and selectors are unchanged. Do not copy the physical-world
checkpoint into a fresh backend: it does not restore the food/recipient history.

Archive base `23e987f3` and candidate `7a384183` as `base.tar` and `candidate.tar`.
Use one bounded target, `/tmp/slither-maintenance-round2-target`, in the linked
builder instead of its historical target path. Run focused visibility tests,
each of the three deliberate oracle mutations in the frozen plan, restore the
candidate source, then `python3 tools/check.py rust`. The oracle's original
full-sweep helper is independent of the new hint. Run docs checks as well.

```sh
python3 build.py base
python3 build.py candidate
python3 build.py mutation
python3 parity.py
python3 measure.py short base candidate --warm 0 --ticks 1080
python3 run_fixed.py stock
python3 run_fixed.py legacy
python3 select_fixed.py
```

The mutation arm uses the linked harness's radius perturbation, never production
code. In addition to complete-trace rejection, compare tag-2 frame records alone
to require encoded wire bytes to differ in stock and protocol 13. All successful
traces require their terminal record. One discarded AB warmup precedes five
alternating pairs. Report maintenance and other ticks separately. CPU includes
the same clock/PMU wrappers; no overhead is subtracted. Preserve full redacted
process command lines at each boundary and run no status commands during counting.

Between cohorts, archive the complete session with numeric ownership and blank
account metadata. Copy it outside the guest, verify SHA-256, then compare every
prospective deleted file against that archive before pruning duplicate proof
traces. Keep the first base proof locally for the unchanged selector. Preserve
failures and all raw material externally; no trace is silently discarded. After
all builds and measurements finish, verify there are no task executables running
and remove the sole task target. Full artifacts are local-only.

`tables.py` reuses the earlier study's table calculation against this study's
selected results, without copying that script. No new live capacity hold runs.
