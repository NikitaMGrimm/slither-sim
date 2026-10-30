# Reproduce the live obstacle-index recheck

Follow [#275's recipe](../2026-10-29-battledome-post-spawn/REPRODUCE.md) and its
linked #267/#263/#262/#258 tools. This study changes the pinned runtimes,
row order, conditional count and hold audit; the socket clients, treatment,
affinity, windows, canonical reducer and qualification gates are unchanged.
Full raw data, native sources/binaries, manifests, controllers and AWS receipts
are local-only verified archives outside Git, without a public download URL.
Hashes identify them and do not grant access.

## Stage and build remotely

Archive base `aeca19a9` and candidate `287d1dc4`; full hashes are in
[runtimes.json](runtimes.json). Use #267's preparation overlay and #275's
portable build configuration: Rust 1.99, locked dependencies, release debug=0,
empty Rust flags and no encoded/native flags. Verify the staged manifest before
building, clear package artifacts between arms and copy/hash each binary.
Require different host binaries, identical shared policy/client source pins,
and the boost-heading/rule-gate regression on both builds. Use the one candidate
client binary for every arm. All builds and reductions run on AWS.

Stage #275's [run_row.py](../2026-10-29-battledome-post-spawn/run_row.py)
unchanged, #267's original row wrapper under `shared267/`, and the linked
shared262/shared263 modules, host agent, delivery, metrics and reducer files.
Use a fresh directory per row, with private `job.json`, bound `pins.json`, the
chosen host and shared client. No endpoint or private identifier belongs here.

Verify same AZ/private traffic, one target CPU, generator topology/affinity,
creation tags, boot shutdown deadlines, terminate behavior and volume deletion
before collection. Follow the committed [plan](PLAN.md), including its generator
gates and budget, rather than changing qualification after seeing overload.

## Collect and hold audit

The host configuration remains shipped responsive Battledome, seed 7, 641
socket slots, zero in-process bots. Every received frame is decoded; shared
A4 steering stays unchanged and the received rule gates the boost override.
Warm 300 wall seconds at 80, then settle 60 and measure 90 seconds at each
80/160/320/480 stage through the requested target. Each ladder starts fresh;
earlier stages are calibration only. Keep complete transitions and deferrals.

The frozen order is base-320, candidate-320, candidate-480, base-480, then
candidate-320/base-320 if both repeats fit. Candidate-only 640 additionally
requires zero deferrals at 480 and enough remaining planned time. A hold needs
both the original marked primary window and canonical interior to have zero
deferrals; all client/queue/boost gates must pass. No historical calibration
replaces a primary, and no server overload row is discarded.

Retrieve each archive only after terminal closure/member verification and
atomic readiness. Verify local size/SHA, retain the source on retrieval error,
and reduce the original archive on AWS. The reducer must exactly reproduce
the original online tick ranges and independently qualify canonical intervals.

```sh
python select_results.py ROW/raw ANALYSIS_HARNESS ANALYSIS/LABEL.json ARM pins-master.json
```

[select_results.py](select_results.py) reuses #275's runtime-bound canonical
selector. Keep a complete source-verified receipt per archive. Produce each
hold record from that archive's final target entry in `run-summary.json`
(`windows[].result.deferredTicks`) and the selected canonical
`primary.deferredTicks`; both are interior-block counts. Prove whole-window
zero separately with the last `CENSUS` counter at/before the start mark and
first at/after the end mark in the archived `host-run/host.log`. Record the
enclosing difference and boundary coverage; a nonzero or missing enclosure
does not prove a hold. Include label, arm, seats and source archive digest.
Write `hold-gates.json`, actual `order.json`, per-row generator segments and
`not-measured.json` from the verified controller state.

[assemble_results.py](assemble_results.py) reuses #275's archive/master-pin
checks, then binds every hold record back to its verified archive, original
online result, enclosing census counters and canonical row. Pins include the inherited assembler/wrapper
once, without repeated per-file source maps.

```sh
python assemble_results.py PRIVATE selected-results.json
python tables.py selected-results.json
```

The [table wrapper](tables.py) retains every repeat as a separate numbered row
and labels an unrun 640 as candidate-only. All numeric inputs are bounded
selected summaries. Report the requested three-way slow-tail split by summing
the existing minimap and other exclusive categories; nested broadcast phases
overlap their parent and must not be added to it a second time.

[timing_diagnostics.py](timing_diagnostics.py) adds post-hoc wall/CPU context
from the same archived canonical tick range: busy-wall quantiles, the largest
wall tick's CPU clocks, slowest-wall 1% and bounded adjacent-census deferral
spans. Each span may contain multiple calls to the host's deferral function.
It does not select a new interval or change any gate. A wall-minus-CPU gap
includes waits and descheduling; it alone does not identify a kernel, scheduler
or hypervisor cause. The assembler reconciles diagnostic deferrals to the
canonical count and pins this helper once.

After counting, run the inherited synthetic reducer/delivery controls, archived
pin/receipt/hold mismatch controls and documentation checks on AWS. Close and
member-verify the final source/analysis archive, retrieve and verify size/SHA
before teardown, then terminate, remove task key/group and audit every enabled
region. Preserve failed attempts and protected/unrelated resources.

For this run, the maintaining agent explicitly retained the security group
because another worker's builder was attached, taking responsibility for its
deletion after that builder terminates. The final audit therefore proves zero
study instances, volumes and active/open Spot requests and records the remaining
group and unrelated resources separately; it does not claim empty global inventory.
