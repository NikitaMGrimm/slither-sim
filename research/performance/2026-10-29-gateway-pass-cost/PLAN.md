# Gateway fixed work, frozen before attribution

Runtime baseline: `cc1bb1e9935094a00cf513413d7922d858dd5c1e`, main after #264.
Question: how much socket-thread CPU is removable fixed work per service pass,
while preserving every WebSocket byte, boundary, order and release opportunity?

## Source hypotheses, not measured CPU shares

`src/client/server.rs` has six full-session walks per stable gateway iteration:
read-budget rearm, readiness construction, service, explicit flush, retain/index,
and next-deadline selection. It also clears the slot index and counts occupancy.
`src/client/server/readiness.rs` constructs an 8-byte descriptor entry per
session and issues one nonblocking poll over those entries. Empty accept and
channel checks are constant per pass. Indexed delivery, actual parser reads,
framing and transport writes depend on activity rather than only session count.

The [earlier socket census](../2026-10-29-food-packet-bytes/SOCKETS.md) motivates
this investigation; its 2.760-ms whole-socket bracket is not an isolated flush,
poll or removable-work measurement. Its world fails the mature-class gate.
The [rejected epoll study](../2026-10-27-host-epoll-readiness/REPORT.md) is
counterevidence: its original saving did not reproduce and candidate bursts
remained unexplained. Do not select a readiness rewrite from that first subset.

## Attribution before implementation

Wait for the maintaining agent's explicit guest release. No local timing.
Local checks use at most two Cargo jobs; all heavy work/timing uses the authorized
guest, with one bounded target and no installs or system/security changes.

Reuse the [mature 500-real-socket capacity harness](../2026-10-28-capacity-checkpoint/REPRODUCE.md)
and unchanged received-frame client, gates, CPU clocks and census. Build separate
primary and diagnostic binaries. Record redacted complete process commands at
boundaries, keep the guest otherwise idle, and issue no status commands inside
counted windows. Preserve all failures and source/binary pins.

First bracket whole gateway phases: rearm, accept, both delivery-drain sites,
readiness construction, poll syscall, service loop, explicit flush loop,
retain/index, occupancy count and next-deadline scan. Count sessions, readable
results, due frames/bytes, parser calls, transport attempts, partial writes,
WouldBlock and conservative output-pending classes. Keep parents/children and
disjoint totals separate. Include delivery work in `recv_timeout`, outside poll.

Avoid a CPU-clock pair around every empty flush: that may cost more than the
operation. First classify whole flush-loop passes as proven-empty-only or
possibly-pending, retaining actual transport-attempt counts. No transport write
does not itself prove empty WebSocket state. Separate sampling or syscall-bracket
diagnostics may refine receives, sendto, framing and automatic/retry flush costs;
include their overhead and never substitute them for primary CPU/tail results.

Use gateway-thread CPU start/end coordinates for phase intervals and intersect
them with the same socket-clock boundaries used by aligned physics ticks.
Keep crossing phases and logger/timer residuals explicit. Do not assign a whole
crossing pass from one wall timestamp or add separate component p99s. Split
maintenance/other ticks using actual food-epoch transitions where identifiable.
Rank measured removable work, not inclusive necessary socket transmissions.

## Exactness and candidate selection

Only implement candidates supported by attribution, in one focused PR. If no
meaningful exact saving is supported, report numbers without a runtime PR.
Potential candidates are proven-empty flush hints, stable index/count retention,
idle-playing service skips and persistent Linux readiness.

A potential-output hint must cover every WebSocket read/write/close, automatic
Pong/Close, threshold drains, partial writes and WouldBlock retries, and clear
only when drained. Conservative false positives are allowed. New sessions must
retain their first service and flush opportunity. Lifecycle states, deadlines,
raw-read fairness, buffered-parser continuation, ascending session order and the
separate final flush loop remain exact. Keep the current readiness failure/error
fallback. Non-Linux production behavior stays unchanged. No reduced wake cadence,
later flushing, changed stock grouping or merged protocol-13 command messages.

Add focused scripted-transport comparisons at original service/flush boundaries
for new/pipelined sessions, Ping/Pong, peer/local Close, partial/threshold writes,
WouldBlock and recovery, login/HTTP/close deadlines, due/future queues, generation
fencing, slot reuse and compaction. Readiness changes additionally cover failure,
hangup/error/invalid fd, registration/removal and buffered continuation. Require
runtime mutation controls for every new hint; restore source, rerun focused
checks, then the full offline Rust group. Retain exact commands/exits and receipt
producer. No-socket fixture parity alone cannot validate a gateway change.

Require all 27 existing catalog/host traces, stock/protocol-13 payload mutation
controls, both mature warm streams and every repeated/direct-parent continuation.
Keep packet bytes, WebSocket boundaries, elapsed/sequence headers and due gates.

## Live confirmation and retention

After builds/checks, use immutable base/candidate hosts, the same client and
500 real sockets. Keep host threads on one core and clients on separate cores.
Warm full hosts from tick zero: the physics checkpoint omits maintenance and
recipient state. Use five alternating AB/BA/AB/BA/AB pairs of independent mature
cohorts, with the usual discarded control and unchanged client/queue/backpressure,
CPU-reconciliation and isolation gates. Every selected control window must stay
in length class. Preserve failed attempts; retry only for predefined class gates,
never for timing outcomes. Account full host/client warmup CPU and set an explicit
series ceiling before running; the older six-cohort budget is not a twelve-cohort
budget. Do not silently reduce the five-pair evidence bar.

Report gateway and aligned total-world CPU/tick, median/p99, per-arm means,
paired changes/ranges and maintenance/other splits. Independent live worlds make
the comparison descriptive. Keep diagnostic overhead separate; loopback CPU is
not a real-NIC budget. Selected values/table inputs belong with the final report;
full traces, binaries, inventories and failed preparations remain local-only.
Link earlier helpers rather than copying them. Retain rejected/instrumentation
patches, no production patch. Archive/hash/verify before pruning; remove the target
and leave no task processes. Add changelog/index and the actual model/role footer
if a runtime PR is proposed. Open/link it and do not merge.

## Attribution setup amendment, before mature collection

The diagnostic uses a no-phase-clock control window followed by coarse and fine
windows in one continued world. Apply the existing checkpoint quality predicates
to each window independently; retain each failure. An invalid fine window cannot
validate or erase a valid coarse one. These diagnostic windows are not primary
base/candidate comparisons. Small-socket smoke rows check recorder/math plumbing,
not the mature workload's quality gates or cost ranking.

The first mature preparation was stopped near tick 25,000, before any hold, to
move diagnostic session/empty-flush counter updates outside the per-session loops.
Its source, binary, partial warm telemetry and failure are retained; no timing
is selected. Fine timers also skip proven-empty flush calls. The corrected build
passes another finite smoke run. Full-array CPU coordinates and exact interval
intersections pass synthetic boundary/gap/flat-clock regressions.

## Candidate selected after attribution

The corrected mature attribution passes all original per-window quality gates.
Coarse poll CPU is 0.266 ms per maintenance tick and 0.162 otherwise, against
3.277/2.227 ms socket CPU. Rearm is 0.032/0.019 ms, retain/index 0.006/0.004,
occupancy 0.001/0.001 and deadline scan 0.002/0.001. About 70% of explicit flush
calls are proven empty, but no whole flush loop is empty: that diagnostic cannot
identify their isolated removable CPU. Fine transport-write brackets dominate
flush and include observer overhead. This supports testing the larger measured
fixed poll cost first, without an unmeasured empty-flush implementation.

Re-test the retained level-triggered epoll prototype on current main, with its
original snapshot timing, ascending service/flush order and poll fallback. Its
earlier rejection and tail bursts remain counterevidence, not acceptance. Keep
fresh base/candidate live pairs, body-class/CPU/client gates and any new tail
failures. No runtime PR if a meaningful exact saving fails to reproduce.

The measured recorder's record-limit edit also changed its seconds multiplier
from 1e9 to 4e9. Raw timestamps uniquely retain seconds and fractional nanoseconds:
divide by 4e9, require remainder <1e9, then combine seconds*1e9+remainder. Preserve
the module as run and failed first reduction. The selected decoder verifies every
timestamp, disjoint CPU budgets and independent canonical socket-clock coverage.
This is exact decoding, with no overhead subtraction or inferred timing values.
