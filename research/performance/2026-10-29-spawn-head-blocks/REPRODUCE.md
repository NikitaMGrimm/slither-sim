# Reproduce the portable head-block comparison

Use an authorized remote builder only. Baseline `3c55991d` is runtime-identical
to merged #272 (`e860ec23`); candidate `ea5869fa`. Preserve a source archive,
complete manifests and actual command exit receipts. Keep production source
and raw traces outside Git; selected pins do not imply public archive access.

Profile baseline first using the previous study's fixed-host preparation and a
separate debug-info portable release build of `study_fixed`. Add acknowledged
perf FIFO enable/disable calls around the window after the 300-tick warmup.
Use a wholly blocked 3,600-tick window (not the default release-half parity
entry point), `cpu-clock:u`, 499 Hz and DWARF stacks. Require 1,800 exhausted
searches, 7,372,800 poses and 72 waiters. Classify inlined head stacks before
outer scopes; keep the raw profile, log, instrumentation and failed first run
in the private archive. This diagnostic never supplies timing results.

Run six focused `spawn::safety::head_tests` tests, then mutate only the block
skip by making its `bounds.near(query, self.head_width + 1.)` conjunction false.
The new block-boundary test must fail. Restore the exact source in `finally`,
verify its digest and require all focused tests to pass. Run all 15 commands
from `tools/check.py rust`, serially, with bounded remote Cargo/test workers.
Keep failed CLI invocations and distinguish them from passing mutation proof.

Reuse [#272's preparation](../2026-10-29-spawn-head-distance/REPRODUCE.md),
the prefilter study's build/queue recipe and the minimap study's trace matrix.
This study's [prepare.py](prepare.py) adds only construction diagnostics;
[build_staged.py](build_staged.py) wraps the earlier builder with empty
`RUSTFLAGS` and an extra diagnostic binary. Remove inherited encoded flags.
Use the default portable release profile on both arms. Build offline after a
locked fetch, verify staged source manifests, clean this package between arms
and pin copied executables. Require 23 catalog/six host/two queued complete
byte and terminal comparisons plus protocol-13/19 wrong-refusal wire controls.
Bind actual Rust check receipts and byte proofs with the previous `write_gates.py`.

Run the unchanged #272 `run_pairs.py` for `spawn`, `seats80`, `seats160`, each
with protocols 19 and 13. Require PMU scheduling >=99% on every tick, full
work/trace equality, repeated frozen forks and direct-parent continuation.
Keep one warmup and five AB/BA/AB/BA/AB pairs. No other builds, tests, probes
or reduction may overlap counting. Reduce remotely with the prefilter study's
`tables.py OUTPUT ROW` and retain each row's selected JSON.

Run `study_build PROTOCOL ROW` in five alternating whole-Safety construction
pairs after a discarded warmup. Set `STUDY_SEATS=160` for that row. Its frozen
world must remain unchanged. A separate candidate-only invocation with
`STUDY_BLOCK_ONLY=1` measures block building over existing edge arrays. These
diagnostics sample 16 repeats × 64 samples; the main rows retain construction
inside their ordinary counters. Raw diagnostic JSON preserves every sample.

Run [select_evidence.py](select_evidence.py) remotely after complete proofs and
timing. [tables.py](tables.py) reads selected `measurements.json` and its adjacent
`construction.json` and prints all report tables. Raw profiles, traces, samples,
logs, inventory and complete instrumented manifests remain private. Publish
one digest per instrumented manifest, not repeated file maps.

Before readiness, close/read each tar/gzip archive and verify every manifest
member remotely. Retrieve and size/SHA-verify immutable proof archives before
counting; the final timing/results delta references that source-proof archive.
Independently bind selected JSON to the verified delta manifest. Never delete
source before verified retrieval. Terminate task resources, delete the task
key/group and scan every enabled region through the limited profile; preserve
unrelated resources. Retain lean retrieval, cost and cleanup receipts.
