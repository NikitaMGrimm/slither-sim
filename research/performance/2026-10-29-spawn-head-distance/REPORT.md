# Exact head-to-edge comparisons in Battledome spawn searches

The exact comparison reduces total user instructions by **40.46%** on the
synthetic exhausted-search row in both protocols (five alternating pairs each).
Median per-window CPU p99 falls from **3.314 to 1.578 ms (19)** and **3.286 to
1.573 ms (13)**, with separated five-window ranges. Search/exhaustion ticks use
40.84% fewer instructions; the saving is also present in the slowest 1%.

The 80/160 controls change total instructions by at most about 0.01%, but their
CPU p50 is **2.5–3.8% higher (about 4–17 microseconds)** for these frozen parent
placements, with separated p50 ranges. Their p99/max ranges overlap. No CPU
improvement is claimed for those controls, and parent layout effects are not
isolated. These are AWS fixed-work results, not live capacity or latency claims.

The head runway predicate uses the existing exact `segment_distance_le` helper
instead of unconditionally calculating a `hypot` before comparing the distance.
Projection, inclusive thresholds and ambiguous/non-finite fallback retain their
original results. The existing helper's uncertainty band selects the original
`hypot` path; it is not an allowed output tolerance. The body and departure `swept_distance` comparisons remain
unchanged, including their separate `<=` and `>` behavior on NaN. Candidates,
random draws, ordering, admitted poses, refusals and retry clocks are unchanged.
No grid or admission-rule change is made.

[Frozen plan](PLAN.md), [Spot recovery](PLAN-RECOVERY.md), [attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md), [correctness and pins](correctness.json),
[all timing pairs](measurements.json), [verification](verification.json).

## Why this comparison

The [constant-boost live recheck](../2026-10-29-battledome-live-recheck/REPORT.md)
found spawn search still taking 63.8% of candidate-320's slow-tail CPU. That
phase did not identify which inner function should change. A separate failed
search profile of the existing held-ring overload fixture attributes 88.6% of
CPU-clock samples to the head predicate, including 63.5% in edge-distance
`hypot` calls; its `near` broad phase takes 1.5%. These are sample-count shares
from 3,393 samples, with warmup excluded and no reported lost samples.

The diagnostic counts 180 exhausted 4,096-pose searches: about 9,393 obstacle
visits and 245,946 edge-distance tests per search, or 2.16 obstacles and 56.51
edges per head call. Whole-obstacle bounds encompass the synthetic full rings,
and scans stop at the first blocker. The proposed 4,096 × 320 obstacle estimate
is not this fixture's observed work. An obstacle grid is deferred unless a later
profile supports it. [Selected profile](profile.json) pins the private archive
and records all figures. Diagnostic clocks materially perturb execution, so
their scope percentages are not used to rank uninstrumented work.

## Exactness

The independent linear head oracle consumes raw obstacle geometry and freezes
the old scalar projection, `hypot`, `near` arithmetic and inclusive predicate.
It calls neither Safety nor Bounds/Point distance helpers in its reference path.
The candidate adapter alone builds Safety. Tests exhaust a six-by-six coordinate
grid including signed zero, then cover random ordinary/full-bit-pattern values,
exact thresholds and adjacent floats, zero-length/repeated edges, empty worlds,
invalid obstacles, non-finite bounds/points, numerical overflow/underflow and
obstacle reorderings. That gives 1,156,612 exact head-verdict comparisons and
409,996 standalone segment-threshold comparisons. An independent frozen swept
oracle additionally checks 19,992 comparison verdicts in both directions; it
does not replace `>` with the complement of `<=`.

The focused wrong-equality mutation must fail these tests. The exact source is
restored in `finally`, its digest rechecked, and the tests must pass again.
An initial 50%-larger-width wire control was neutral on this fixture and failed
its sensitivity requirement. It remains a failed attempt. The replacement
wire mutation incorrectly rejects every head with nonempty neighbors; it must
alter actual queued wire bytes under both protocols. Baseline/candidate must
still admit after barrier release; this deliberately wrong treatment may leave
all waiting, but it must complete and change actual frames, not just a summary.

All 23 catalog, six host and two queued traces match complete bytes and terminal
records under the applicable stock/legacy protocols. The queued release case
admits 18 requests in each arm; the deliberately wrong refusal control admits
none and changes actual wire bytes in both protocols. The focused mutation fails
and the restored oracle passes. All 15 commands of the source-identical full
Rust group pass. Native focused tests, byte proofs and repeated-fork/direct-parent
checks also pass on the final Intel measurement pool. See the selected correctness
and verification receipts; no independent review is claimed.

Full checks use the uninstrumented candidate. Trace preparation and fixed-host
accessors are research-only overlays. The existing prefilter search oracle still
checks poses/errors/RNG, while the new raw-geometry oracle independently checks
the changed predicate that the old search oracle shares. Passing own checks
does not replace independent maintaining-agent review or human acceptance.

## Fixed-work measurements

| Row | Protocol | Instructions change median [range] | CPU p50 base → candidate ms | CPU p99 base → candidate ms | CPU max base → candidate ms |
| --- | --- | ---: | ---: | ---: | ---: |
| spawn | 19 | -40.46% [-40.46, -40.46] | 2.946 → 1.398 | 3.314 → 1.578 | 3.441 → 1.742 |
| spawn | 13 | -40.46% [-40.46, -40.46] | 2.909 → 1.392 | 3.286 → 1.573 | 3.343 → 1.636 |
| seats80 | 19 | +0.01% [+0.01, +0.01] | 0.157 → 0.163 | 0.447 → 0.444 | 0.771 → 0.772 |
| seats80 | 13 | -0.00% [-0.00, -0.00] | 0.158 → 0.162 | 0.452 → 0.444 | 0.781 → 0.766 |
| seats160 | 19 | -0.00% [-0.00, -0.00] | 0.478 → 0.495 | 0.994 → 1.010 | 1.494 → 1.457 |
| seats160 | 13 | -0.00% [-0.00, -0.00] | 0.481 → 0.497 | 1.148 → 1.158 | 1.580 → 1.575 |

| Row / protocol | Group | Instructions change median [range] | CPU p99 base → candidate ms |
| --- | --- | ---: | ---: |
| spawn / 19 | search | -40.84% [-40.84, -40.84] | 3.386 → 1.639 |
| spawn / 19 | exhausted | -40.84% [-40.84, -40.84] | 3.386 → 1.639 |
| spawn / 19 | other | -0.00% [-0.00, -0.00] | 0.051 → 0.056 |
| spawn / 19 | slowestCpu1Percent | -41.37% [-41.88, -40.97] | 3.441 → 1.742 |
| spawn / 13 | search | -40.84% [-40.84, -40.84] | 3.293 → 1.634 |
| spawn / 13 | exhausted | -40.84% [-40.84, -40.84] | 3.293 → 1.634 |
| spawn / 13 | other | +0.00% [+0.00, +0.00] | 0.051 → 0.048 |
| spawn / 13 | slowestCpu1Percent | -41.33% [-41.67, -41.18] | 3.343 → 1.636 |
| seats80 / 19 | search | -0.00% [-0.00, -0.00] | 0.447 → 0.444 |
| seats80 / 19 | other | +0.01% [+0.01, +0.01] | 0.373 → 0.372 |
| seats80 / 19 | slowestCpu1Percent | +0.10% [+0.10, +0.10] | 0.771 → 0.772 |
| seats80 / 13 | search | -0.00% [-0.00, -0.00] | 0.336 → 0.348 |
| seats80 / 13 | other | -0.00% [-0.00, -0.00] | 0.452 → 0.444 |
| seats80 / 13 | slowestCpu1Percent | -0.04% [-0.04, -0.04] | 0.781 → 0.766 |
| seats160 / 19 | search | +0.01% [+0.01, +0.01] | 0.814 → 0.817 |
| seats160 / 19 | other | -0.00% [-0.00, -0.00] | 0.994 → 1.010 |
| seats160 / 19 | slowestCpu1Percent | -0.11% [-0.11, -0.11] | 1.494 → 1.457 |
| seats160 / 13 | search | -0.01% [-0.01, -0.01] | 0.895 → 0.925 |
| seats160 / 13 | other | -0.00% [-0.00, -0.00] | 1.148 → 1.158 |
| seats160 / 13 | slowestCpu1Percent | +0.05% [+0.05, +0.05] | 1.580 → 1.575 |

| Row / protocol | CPU p50 range base / candidate ms | CPU p99 range base / candidate ms |
| --- | ---: | ---: |
| spawn / 19 | 2.933–2.952 / 1.365–1.409 | 3.306–3.356 / 1.547–1.593 |
| spawn / 13 | 2.902–2.927 / 1.390–1.408 | 3.250–3.303 / 1.562–1.580 |
| seats80 / 19 | 0.157–0.159 / 0.162–0.163 | 0.429–0.456 / 0.415–0.471 |
| seats80 / 13 | 0.157–0.159 / 0.161–0.163 | 0.421–0.461 / 0.426–0.479 |
| seats160 / 19 | 0.475–0.480 / 0.493–0.496 | 0.967–1.025 / 0.976–1.021 |
| seats160 / 13 | 0.481–0.482 / 0.492–0.501 | 1.136–1.180 / 1.132–1.279 |

CPU table cells are medians of five per-window quantiles, including each
window's maximum; they are not pooled global quantiles. All individual windows,
the discarded warmup, event groups and paired instruction ranges remain in the
selected JSON. Each arm's slowest 1% is selected independently by its own CPU
clock, so those tail groups need not contain identical tick indices. Whole
windows preserve exact work and bytes. Independent phase quantiles are not added.

The primary overload row is unchanged from the
[spawn-ring prefilter study](../2026-10-29-spawn-ring-prefilter/REPORT.md): seven
synthetic held rings, one observer, 72 waiting joins, warm 300 ticks and count
360 ticks from a frozen full host. Every measured window performs 180 refused
searches and 737,280 poses, exactly 4,096 per search, leaving all 72 waiting.
Held-pose resets and trace serialization are outside counters; actual command
processing, admission, world/broadcast and hand-over are inside. The separate
release trace removes the barriers and checks subsequent successful admissions.

The 80-/160-viewer controls hold separated synthetic bodies and periodically
queue one reconnect outside the interval. The real host admission runs inside
the interval, must finish that tick and leave no backlog, with all viewers alive.
Their initial arena is primed to the cohort using the normal seat-growth method
before viewers connect. The unprimed setup failed its no-backlog warmup gate and
is retained as a failed harness revision. They exercise ordinary admission and
broadcast overhead, not naturally grown
constant-boost worlds. A control's little or unchanged cost is not a capacity
claim. None of these rows measures real sockets, network jitter or spawn waits
in wall time; no live 320-seat speedup is inferred.

## Native context and limitations

Baseline `d65d89ca`; measured candidate `1601fa65`. Subsequent evidence/helper
commits retain the same production Rust source. Native source/archive/manifest
and copied binary digests appear once in correctness evidence. Release builds
use Rust 1.99, locked/offline dependencies and identical `-C target-cpu=native`
flags on both arms. One package's prior artifacts are cleared between arms.

After complete proofs, one pinned AWS virtualized Intel core runs each full-host
child; a separate physical core runs the controller. The c7i.xlarge has SMT
disabled at launch (two assigned cores, one thread each). No concurrent builds/tests or status
commands run during counting. One discarded AB warmup precedes five alternating
AB/BA/AB/BA/AB pairs per row/protocol. Every counted tick requires at least 99%
scheduled hardware `instructions:u` time. Repeated-fork, cross-arm and final
direct-parent continuation traces establish the frozen work. This is Frankfurt
Spot evidence, not dedicated physical-host timing or a Broadwell comparison.
The SSH guest is unused. No local native work is performed.

The first c7a.xlarge was reclaimed during native builds; its deleted root volume
left no retained final proof or performance row. The c8a.xlarge replacement
completed the source-identical full Rust group and native builds; both stage
archives were retrieved and verified before another Spot reclamation. Its later
wire-control observation is not selected evidence without its retrieved archive.
The c7i.xlarge final pool verifies the entire candidate source archive against
the retained Rust archive, rebuilds all native binaries and reruns focused
native mutation/restoration plus every byte trace before timing. Full Rust
receipts therefore come from the source-identical C8a run; focused/native byte
and timing evidence comes from C7i. Failed cache preparation and control attempts
remain recorded. No rows from different machines are pooled into a pair.

## Cost, retrieval and cleanup

All three attempts together are bounded by **$0.574**, including rounded
launch-to-cleanup compute at each bid cap and a shared $0.25 noncompute
allowance; this is a conservative bound, not an invoice. The full-timer launch
bound was $0.797, below the $1 cap. [Cost receipt](cost.json).

The final builder was explicitly terminated after verified retrieval, before
its 120-minute in-guest poweroff timer. Its root volume used
DeleteOnTermination and shutdown behavior was terminate. Every attempt's task
key pair and security group were deleted; protected resources were untouched.
At 17:26:49 UTC, the final read-only scan completed in all 17 enabled regions:
zero nonterminated instances, open/active Spot requests, available or other
volumes, Elastic IPs and project/task keys or groups. An earlier scan hit an
endpoint connection timeout; the complete read-only retry passed.
[Cleanup receipt](cleanup.json).

Full source trees, binaries, traces, inventories and logs are local-only, not
publicly downloadable. Digests identify archives without granting access.
Only closed, tar/gzip-read, hashed archives publish readiness. Retrieval verifies
size and SHA-256 locally; every manifest member is verified remotely before
readiness and source teardown. The final timing delta binds the already
retrieved source/byte-proof archive; selected JSON is independently checked
against the delta manifest. Retrieval pins and method appear in verification. Selected result
tables recompute with [tables.py](tables.py); earlier harnesses are linked rather
than copied. No host/cloud identifiers, credentials or endpoint are published.

The separate shrinking-marker reuse candidate remains parked on the pushed
`perf/pit-marker-reuse` reference branch at `58145ae5`, with no PR or timing.
Its estimated saving was too marginal to pursue; it is not part of this change.
