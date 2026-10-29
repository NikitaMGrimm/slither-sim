# Reproduce retained food-listing ownership

Reuse the [food-diff fixed-work builder and execution helpers](../2026-10-29-food-diffs/REPRODUCE.md)
and measurement-only `harness.patch`, including its linked earlier harnesses.
The backend fixture, controls, warmup, complete-stream comparer, fork requests,
counter, 27-row matrix and selector are unchanged. Use one bounded target,
`/tmp/slither-maintenance-round2-target`, instead of the builder's old target.
Archive base `23eddca8` and implementation `a15bfc7a` as `base.tar` and
`candidate.tar` in the external execution directory. Both include #261. The
study scripts extract archives with `tarfile` filters, so use Python 3.11.4 or
later.

Reconstruct the external execution layout described by that earlier study and
bind its historical helper paths to that layout. Its `setup.py` populates the
fork/comparison helpers; its `build.py` supplies the common measurement overlay.
Copy the exact study [check runner](check_held.py) and
[payload-only mutation verifier](verify_wire_mutation.py) alongside the archives.
It runs the food oracles, then separately drops unchanged-handle pickups,
ignores a changed listing, and ignores held-listing identity. Each control must
fail at runtime. A `finally` restores the source after each; restored oracles
must pass, followed by `python3 tools/check.py rust`. Every command/expected
failure/exit is written to `checks/results.json`. The runner then writes
`required-checks-pass.json` with `passed` and those `checks`, which the unchanged
selector reads. Its complete primary receipt is at `fixed.checks` in
[results.json](results.json). The food-diff harness's `check.py` is not its producer.

```sh
python3 setup.py
python3 check_held.py
python3 build.py base
python3 build.py candidate
python3 build.py mutation
python3 parity.py
python3 verify_wire_mutation.py
python3 measure.py short base candidate --warm 0 --ticks 1080
python3 run_fixed.py stock
python3 run_fixed.py legacy
python3 select_fixed.py
```

The radius perturbation is the linked harness's existing mutation. Compare
tag-2 host-frame payload records alone as well as complete traces to require
wire-byte changes in stock and protocol 13. All valid traces need terminal
records. Keep the exact retirement+pickup+fresh-state test and all existing
older-view, pit, handoff, reconnect and ordered full-walk oracles active.

The physics checkpoint omits maintenance-path state. Warm each full backend
once per arm/protocol with every recipient active and no sockets/wall pacing,
then fork for each pair. Compare every warm byte and all repeated/direct-parent
continuations. Count 1,080 short advances from zero and 360 mature advances
after 193,000 warm ticks. One discarded AB warmup precedes five alternating
pairs. Select actual food-epoch transitions separately. CPU includes identical
clock/PMU wrappers with no correction.

Finish builds, checks and socket diagnostics before timing; record full redacted
process commands at boundaries. Keep the guest otherwise idle and issue no
status commands during counted windows. Recheck copied binaries against their
source/build identities. Remove the sole target after builds/parity finish;
immutable copied binaries suffice for timing.

Archive complete cohorts externally, verify SHA-256, then compare every duplicate
proof against its archive before pruning. Keep the first base proof through
selection. Raw data and failed preparations are local-only. `tables.py` reuses
the earlier calculator against selected primary pairs. No live capacity hold runs.

For the rejected variants, start from `23e987f3` and apply the corresponding
patch linked in the report. Use its own check runner and a fresh external output
directory; never substitute its pairs or receipts into the primary row. Their
earlier frozen baseline may be reused only after every archive/source/binary
identity is rechecked. Primary current-main arms are freshly built instead.

[count_food_packets.py](count_food_packets.py) parses the retained v1 archive's
mature base proof (`packet-bytes/sessions/stock/proof-base-0.trace` and `.json`).
Put that local-only archive at `packet-v1-complete.tar.gz` alongside the script,
or bind the input paths to a newly reproduced base proof. It validates framing,
terminal records, frame totals and the actual maintenance flags. Header ticks
precede the advance; samples name its successor. `RawSimulator::take` drains
generated frames including future deliveries. The selected census recomputes
counts without measuring allocation CPU.

The separate [socket analysis](SOCKETS.md) retains its counter/build/runner and
selected counts. It never supplies CPU/tail samples for this comparison.
