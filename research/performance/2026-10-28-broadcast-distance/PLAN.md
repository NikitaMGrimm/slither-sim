# Fixed-work broadcast attribution and exact comparison

Runtime baseline: `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`. Read the
capacity report from PR #248 before collection. No mature-length holds,
outside game servers, running play servers, package installs or guest system
changes. One task-owned Cargo target on a filesystem with sufficient space;
retain failed attempts and remove that target when finished.

Before production edits, use the existing 1,080-tick seed-7 official-500
host/wire fixture with all protocol-19 and all protocol-13 recipients.
Add an otherwise identical 2,281-seat stock row if admission succeeds cheaply.
Use the existing `perf_baseline client-official-500 24` row too. These are
short synthetic fixed-work worlds, not the mature A4 worlds in #248.

Separate diagnostic builds bracket current-thread user instructions in world
advance, food-interest advance, broadcast, snake coverage, per-snake encoding,
food updates and prey updates. Nested scopes overlap. Per-candidate brackets
add overhead; neither their parent total nor interposed total is a plain
performance result. Retain calls and instruction sums, not only percentages.
An LD_PRELOAD shim uses the existing gcc to wrap hypot/fmod/atan2/sin/cos/
sincos and other libm entry points, keying calls and instruction brackets by
return address and current phase. Resolve call sites with addr2line, including
inline locations. These libm brackets include read-boundary/wrapper overhead;
report that limit and use separate plain phase counts as the denominator.
Do not equate these instruction fractions with time or with #248's samples.

Attribution-method amendment before any paired speedup samples: additionally
calibrate the libm bracket with the same no-IPA measurement routine calling
a one-`ret` control, separately for each function signature. Verify each empty
function's sole instruction by disassembly and require 64 identical calibration
counts per process. Then actual libm instructions are raw bracket minus control
plus one. Retain raw and corrected sums, controls and >=99% PMU coverage.
Use the plain executables for this calibrated caller run, avoiding the phase
overlay's temporary coverage vector and recipient-level counter overhead.
This avoids assigning unresolved callchain sample weight to guessed callers;
the thin-shim IP/callchain sample remains an independent, explicitly partial
diagnostic. Keep the earlier uncalibrated run as a separate pilot.

Also collect calibrated, phase-tagged callers for stock-500 and stock-2,281.
Cross-check the broadcast owners' call counts and corrected instructions
against the plain caller run, then use their tick-only scopes to exclude
initialization and outer admission work. Normalize those math totals by the
separate plain host-tick brackets. Retain both runs; do not replace a failed
measurement with a favorable one.

Choose a focused production change only after this attribution. Require the
original full decompressed bytes for every available deterministic workload's
registered trace window, plus stock, mixed and protocol-13 host/wire fixtures.
Keep terminal markers and host checkpoint/frame counters. A deliberate runtime
perturbation must fail the same comparison; retain its result and restore the
candidate exactly. Test edge cases relevant to the predicate/encoding changed.

Build copied release executables with identical locked offline recipe, portable
CPU settings, thin LTO, one codegen unit and debuginfo=1. Retain archives,
generated-source hashes, build logs, toolchain and binary hashes. Complete
builds and traces before the comparison. Run one discarded baseline-first
warmup pair, then five pairs ordered AB, BA, AB, BA, AB for every fixed-work
row. Each side starts a fresh same-seed process. Require at least 99% PMU
running/enabled time and identical host summaries. Count the outer fixture
invocation separately from the host's 1,080 tick brackets. Its bracket excludes
process startup and final counter-file writing. Report medians
and the entire paired range; no sample replacement or elapsed-time claim.

Pin each measurement process to one logical CPU, leave the guest otherwise
idle, and retain full process command lines at every boundary outside Git.
Public artifacts contain selected source/counter data and hashes only; scan
for private machine, account and path details before pushing. Run the focused
tests followed by the Rust check group, plus final documentation checks.
Open focused PRs for maintaining-agent review and do not merge.
