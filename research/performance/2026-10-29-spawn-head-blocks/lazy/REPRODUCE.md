# Reproduce the lazy revision

Follow the [original recipe](../REPRODUCE.md) on an authorized capped builder,
with default portable release flags. Main is `e860ec23`; the additional eager
head is `78f7d1ec`; final lazy source is `3d2965f5`. Preserve all failed attempts.
The original eager report/JSON remain unchanged beside this revision.

Run all seven `spawn::safety::head_tests`, including explicit lazy/eager
equivalence, untouched-cache and reuse checks. Require the same wrong-skip
mutation to fail the block-boundary test, restore the exact source in `finally`
and pass again. Run all 15 Rust commands remotely. Stage/build base, eager,
candidate and wrong-refusal mutation using [prepare.py](prepare.py) and the
original portable builder; pin each archive, instrumented manifest and binary.

Run the original 23 catalog/six host/two queued traces plus both wire controls
against main/lazy. Bind completed receipts with the original gate writer.
[run_pairs.py](run_pairs.py) extends #272's runner by reference with one eager
parent. Ready states and every repeated/full/direct-parent trace must match.
For each of the three rows and both protocols, run a discarded three-arm
warmup then main/eager/lazy and its reverse, alternating for five pairs.
Keep source/binary checks and >=99% PMU scheduling on every tick.

[reduce.py](reduce.py) reuses the original reducer for both main/lazy and
eager/lazy from the same windows and additionally retains CPU means per group.
Keep the original complete-Safety and raw-block construction diagnostics,
adding the eager arm to the alternating whole-build windows. Lazy whole-build
cost explicitly excludes first use; the main rows include actual initialization.
Do not run any probe/build/reduction concurrently with counting.

[select_evidence.py](select_evidence.py) validates three-arm work/trace equality
and source pins, reuses the original byte/check gates and refers to the original
profile instead of duplicating it. [tables.py](tables.py) prints all tables from
adjacent measurements, eager-measurements and construction JSON. Preserve raw
clocks/counters and all warmups privately. Archive/read/hash every member before
readiness, retrieve and SHA-verify before source deletion, and bind the final
timing delta to the immutable proof archive. Clean task resources and scan all
enabled regions through the limited profile. No local native work or guest use.
