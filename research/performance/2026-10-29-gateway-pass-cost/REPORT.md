# Gateway socket-task attribution after #264

**Live confirmation is incomplete; no runtime change or speedup is proposed.**
Attribution supports testing persistent readiness first, and the candidate passes
the offline correctness bar. The frozen live series stops at the first primary
base window's marginal competing-process CPU failure. No primary pair completes.
The candidate is retained as an exact patch; production source is restored to
the integrated baseline. [Plan](PLAN.md), [reproduction](REPRODUCE.md),
[table inputs](attribution.json), [table script](tables.py).

Baseline runtime cc1bb1e9935094a00cf513413d7922d858dd5c1e, mature official-density
500 real loopback WebSockets, one host core on a 4-vCPU Zen 5 EPYC KVM guest.
This is a gateway cost diagnostic, not an independent production capacity claim.

## Method and controls

Reuse the capacity-checkpoint common received-frame client and full tick-zero
warmup. Stop forced churn at 180,000 ticks; require the existing mature screen
after at least 193,000 ticks. Record actual class/cadence/headroom/backpressure,
CPU reconciliation, isolation and binary identities; retain every failed window.
Separate continued-world windows use no gateway phase clocks, coarse whole-loop
clocks, then fine nested I/O/WebSocket clocks. These modes are not base/candidate
arms. Every new timer, recorder and classification cost remains included.

Phase intervals use gateway-thread CPU coordinates, intersected with the same
CPU boundaries that produce per-tick socket CPU. This assigns crossing work
without a wall-time approximation. Actual food-epoch flags identify maintenance.
Coarse parent/child intervals and fine WebSocket/transport intervals are nested;
only disjoint phases can be summed. Retain unassigned clock/logger/queue-telemetry
work explicitly. Never add component p99s.

Counter updates for session/empty-flush loops are batched outside those loops.
Proven-empty flush calls have no per-call CPU timer, including fine mode. A
conservative diagnostic-only hint labels pending output; it suppresses no flush.
It marks before every read/write/close and clears only on successful flush.
Fine transport clocks include wrappers and observation cost, not pure kernel
instruction time. Empty-only whole-loop timing is an upper bound containing the
loop/classification/clock overhead, not an isolated removable-per-call cost.

## Baseline attribution

All three diagnostic windows pass the existing mature-class, client cadence/
headroom, queue/backpressure, CPU-reconciliation and isolation gates. Coarse
sampled mean length spans 56.174-58.868, p90 181-187 and p99 257-261; all 500
clients remain represented, with 499-500 living. Thirty selected maintenance
and 2,170 other ticks supply the coarse conditional means.

| Coarse CPU bracket, ms/tick | Maintenance | Other |
| --- | ---: | ---: |
| Gateway thread | 3.277074 | 2.227096 |
| poll syscall | 0.265919 | 0.161593 |
| Readiness-array construction | 0.003906 | 0.002222 |
| Rearm | 0.031637 | 0.018675 |
| Accept | 0.027680 | 0.018642 |
| Delivery drain in poll | 0.000958 | 0.000623 |
| Delivery after channel wait | 0.080047 | 0.075394 |
| Per-session service, inclusive | 0.575353 | 0.358215 |
| Explicit flush loop, inclusive | 2.207058 | 1.545264 |
| Retain/index | 0.006259 | 0.004420 |
| Occupancy | 0.001039 | 0.000621 |
| Next deadline | 0.002083 | 0.001408 |
| Channel wait CPU | 0.038570 | 0.019387 |
| Unassigned observer/telemetry work | 0.036566 | 0.020632 |

The full poll-pass parent is 3.150002/2.125868 ms, containing most rows above;
do not add it again. Root intervals cover 99.774% of gateway CPU in the coarse
window and 99.908% in fine mode. Residual CPU is retained rather than normalized.

Coarse whole-window counters cover 6,193 complete gateway passes, 3,096,409
explicit flushes, 2,171,680 proven-empty calls and 924,729 possibly-pending calls:
**70.135% proven empty**. Nevertheless every flush loop has at least one pending
recipient, so the planned all-empty-loop timing cannot isolate empty-call CPU.
Counts omit incomplete selected edge records and are not allocated between
maintenance and other ticks. No empty-flush saving is asserted from the count.

Fine mode selects 18 maintenance/1,282 other ticks. Transport-write brackets
consume 1.908101/1.655520 ms/tick and receive brackets 0.262941/0.133540. They
include std/libc wrappers and timer/recording overhead, rather than isolated
kernel CPU. WebSocket reads, writes and flushes are inclusive parents; the
reducer subtracts only transport intervals actually contained in each parent.
The earlier direct syscall witness maps this build family's writes to sendto;
this new diagnostic is wrapper-level attribution, not a new syscall-entry trace.
The receive bracket includes the transport Read wrapper; the earlier witness
did not enumerate receives, so it supplies no new recvfrom-entry count.

| Fine WebSocket CPU excluding contained transport intervals, ms/tick | Maintenance | Other |
| --- | ---: | ---: |
| Read/parser | 0.251662 | 0.141655 |
| Write/framing | 0.279594 | 0.185968 |
| Explicit flush, possibly pending calls only | 0.232288 | 0.175564 |
| Close | 0 | 0 |

These are wrapper-exclusive diagnostics with timer/recorder overhead, not pure
tungstenite instruction counts. They neither isolate empty flushes nor establish
that all framing work is removable. Coarse complete records count 773,769 binary
WebSocket writes, 773,777 transport writes, 422,970 transport-read attempts and
zero write-block events. Threshold/control writes need not match application
message counts. Necessary transmission still dominates the gateway budget.

The measured removable fixed-cost ranking is poll first (0.266/0.162 ms), then
read-budget rearm (0.032/0.019); retain/index, occupancy, array construction and
deadline scans are small. Accept's fixed failed syscall is 0.028/0.019, but
changing listener checks would need a separate exact snapshot/admission proof.
Service/flush are large inclusive budgets containing necessary I/O, not
removable per-session overhead. Byte/framing versus kernel costs require the
separately labelled fine window and cannot be subtracted across distinct worlds.

## Candidate

Re-test the retained level-triggered epoll implementation on current main:
zero-timeout query at the original snapshot point, original ascending service
and separate flush passes, registration tied to each Open lifetime, MOD after
compaction, and original poll fallback after any create/register/query failure.
Non-Linux paths and all output/framing functions remain unchanged. The earlier
rejected experiment stays counterevidence. New tests cover query-failure
fallback and 500 repeated multi-socket reorder/partial-drain snapshots against
poll, in addition to the retained creation/registration/reused-fd regressions.

The [retained runtime/test patch](rejected-readiness.patch) applies to
`539b37f3` and reconstructs all three candidate source/test files byte for byte.
[Its receipt](rejected-readiness.json) pins the digest and both runtime/test refs.
The source restoration matches the baseline exactly; there is no changelog entry
because no runtime change is proposed.

## Live attempt and frozen stop

The series plans discarded AB, then five AB/BA/AB/BA/AB pairs. Each independent
world warms from tick zero to at least tick 193,000, discards five seconds, counts
30 seconds and runs a separate 15-second diagnostic. Host tasks share CPU 0;
the unchanged full received-frame client uses CPUs 1/2/3. No new gateway phase
clock runs in the selected hold. The 12-CPU-hour ceiling includes full warmups.

Four cohorts complete, consuming **2.648503 CPU-hours** in total:

1. Discarded base passes all original quality gates.
2. Discarded candidate attempt 1 fails the class gate: p90 segments reaches 197
   against the maximum 196; its sampled mean remains 57.098–60.568. Its sole
   permitted class retry is launched without consulting timing.
3. Discarded candidate attempt 2 passes all original gates.
4. First primary base passes body/client/cadence/queue/reconciliation checks,
   then fails the original competing-process CPU gate. One other userspace
   process consumes **0.300 CPU-seconds** over **29.405192 seconds**, against
   **0.294052 seconds** allowed. Total other userspace CPU is 0.390 seconds.

The retained counters advance in 10 ms increments. The excess is about 6 ms, below one counter
quantum, so this is a marginal gate failure and does not establish material
contamination. The frozen plan permits no non-class retry or post-result change
to that threshold. The controller stops, with **zero completed primary pairs**.
No candidate primary window is launched and no discarded or failed window is
promoted. This provides neither a qualified speedup nor a candidate regression.

[live-incomplete.json](live-incomplete.json) retains the excluded windows, their
original quality values and actual-maintenance/ordinary splits. The diagnostic
reduction exposes the rejected isolation row while retaining its failed status;
it never creates an accepted receipt. All reduced tick flags match actual food
events and their all-tick CPU summaries match the original selector's values.

| Excluded window | Gateway mean ms/tick | World mean | World median | World p99 |
| --- | ---: | ---: | ---: | ---: |
| Discarded base | 2.287454 | 5.361890 | 5.353180 | 8.875310 |
| Discarded candidate retry | 1.893281 | 4.417851 | 4.295710 | 7.710990 |
| Isolation-failed first primary base | 1.964309 | 4.385030 | 4.297340 | 8.186940 |

These independent worlds supply no paired effect estimate. Their gateway means
on maintenance/other ticks are 3.327472/2.272636, 2.633108/1.882740 and
3.220756/1.946408 ms respectively. Full median/p99 splits remain in the selected
JSON; no component quantiles are added. No five-pair instruction or live-tail
result is invented from this incomplete series.

During the first discarded candidate warmup, the SSH connection service reports
a cooldown until 11:08:35 UTC. The detached controller resumes the same warmup
after reconnection; this interruption occurs before any counted hold and is
retained. It supplies no reason to relax a later gate.


## Selection and limits

Primary comparisons refresh both arms to main `539b37f3`, candidate `da7e65c2`,
before any candidate timing. Attribution remains at its original pin. Later
`afd7e03a` adds four transport tests only; the checked production prefix and
readiness source match the measured candidate archive exactly. Runtime, archive,
compiler, build, selected overlay and host-binary pins are in [pins.json](pins.json).

Current-main validation passes:

- All 27 catalog/host traces compare byte for byte. Stock and protocol-13
  payload-only perturbations are detected at frames 21 and 567 respectively.
- Short, mature stock and mature protocol-13 complete warm streams compare
  9,719,935 / 5,876,276,676 / 10,132,210,980 bytes per arm. Repeated fork and
  direct-parent continuations match in full; proof digests remain in the results.
- The Linux server suite passes, then each deliberate missing compaction MOD,
  readable-event assignment and query fallback fails with exit 101. Source is
  restored, the focused suite passes and `python3 tools/check.py rust` passes.
- Four added transport tests cover buffered Pong, peer Close/slot release,
  partial write followed by WouldBlock/retry without readability, and a new
  Open's already-buffered output. Blocking the new session's buffered parser
  fails with exit 101; restored tests and the full Rust group pass again.

The receipt producers are retained in the measurement patch and documented in
[REPRODUCE.md](REPRODUCE.md); no receipt is inferred from a successful build.
[fixed-results.json](fixed-results.json), [payload-controls.json](payload-controls.json)
and [final-tests.json](final-tests.json) retain the selected checks and outputs.

The no-socket fixed row has five alternating user-instruction pairs per case.
Median all-tick changes are +0.00500% short, -0.00333% mature stock and +0.01841%
mature protocol 13. Maintenance/other changes are -0.00422%/-0.00331% stock and
+0.06649%/+0.01694% protocol 13. This fixture never calls gateway readiness:
these tiny differences supply no socket speed or instruction-saving claim.
Only the real-socket comparisons can decide this candidate.

Independent original harness quality predicates apply per diagnostic window.
A fine-window failure cannot erase or validate a coarse-window outcome. Selected
values must remain in mature class; invalid data stays labelled diagnostic only.
Eight-socket smoke rows validate records and math, not the 500-player workload.
Whole-window event counters come from complete selected CPU records, excluding
edge-crossing records. Their events are not invented at maintenance boundaries.

Loopback sends can charge receive-side work to the sender. This machine and
workload do not establish c7a/NIC costs. The earlier epoll study remains rejected
because its saving failed to reproduce and candidate-only bursts were unexplained.
No new exact candidate is justified solely by that historical first subset.

## Setup attempts and retention

One prepare-only run omitted the linked client source and failed before Cargo;
its partial source/log remain. Smoke 1 exposed a record-volume ceiling and an
unwanted per-empty-flush fine timer; both were corrected, with old binary/source
and pins retained. Smoke 2 validated the corrected coordinates. Mature warmup 1
was stopped near tick 25,000 before any hold to batch per-session counters.
The corrected binary passed Smoke 3 before fresh mature warmup 2. None of those
preparations supplies a selected mature timing result.

Raw sources, binaries, ticks, complete traces, command inventories and failed
attempts remain **local-only**. These digests identify the retained gzip tar
archives; they are not public download links:

- `attribution-complete.tar.gz`, 64,414,721 bytes:
  `95cfa4a074e5312ca3b4d6644b3a25a1d625d4f197d2f77ec5bf3b8d5e4c7f38`.
- `pre-live-complete.tar.gz`, 720,788,229 bytes:
  `325a0099cae4025980b27aaf9f42007ecc8b9c15a649e12d9266a8202f6831f3`.
- `final-complete.tar.gz`, 78,866,176 bytes:
  `d4a4d38020b737a20f699387d1c9b6a99bec25f8c6674f25f57d4dd96fefab6e`.

Each archive is copied outside the guest and SHA-256 verified before pruning.
Every prospective removed file/link is compared with the archive. The first
pre-live prune stops safely on two source-archive symlinks; the helper is corrected
to validate their stored link targets separately, then the comparison passes.
The final cleanup verifies 2,721 entries, exact primary source/binary pins and the
common client. All task processes stop, the sole Cargo target is absent, and the
task root is removed. Read-only earlier-study donor directories are untouched.
The guest is released before completing the local report.

## Timestamp encoding incident

A record-limit replacement also changed the as-run module's clock seconds
multiplier from1e9 to4e9. The first reduction consequently had no phase/tick
intersections and is preserved as failed. The raw seconds/fraction representation
is injective: divmod by4e9, require fractional remainder below1e9, then combine
seconds*1e9+remainder. The selected decoder checks every raw timestamp, disjoint
nonnegative CPU budgets and canonical independent socket-clock coverage. No
sample is approximated and no observation overhead is subtracted. Preserve
the actual measured module and its hash rather than silently fixing it.

Six math tests cover exact recovery at second boundaries, gaps/flat clocks,
crossing intervals, nested parents and aligned world quantiles. Four deliberate
analytical faults each fail at runtime; original and restored analysis pass.

## Remaining measured budget

Polling remains the largest isolated fixed cost: 0.266/0.162 ms per
maintenance/other tick. A new live attempt needs a predeclared treatment of
10 ms process-counter quantization, then the full five qualified pairs.
Read-budget rearm is next at 0.032/0.019 ms; retain/index is only 0.006/0.004 ms.
The 2.207/1.545 ms flush loop and 0.575/0.358 ms service loop are inclusive
budgets. Their proven-empty/idle subsets need isolated cost evidence before a
skip is justified. This study implements neither skip.
