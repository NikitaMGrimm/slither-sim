# Broadcast-local food diffs, 2026-10-29

Broadcast-local diff reuse preserves byte-identical output and reduces mature
maintenance user instructions by **7.55% with stock clients and 6.84% with
protocol 13**. Whole-window instruction reductions are 1.09% and 1.00%. Fixed
CPU means/p99s fall descriptively, with overlapping timing ranges. **The live
world-tail saving is unconfirmed, and the 7 ms target is not qualified.** The
planned balanced socket confirmation fails body-class drift in its last control
window; all three earlier qualified windows are reported without a causal
speedup claim.

[Selected results](results.json), [table calculation](tables.py), [frozen plan](PLAN.md),
[reproduction and linked earlier harnesses](REPRODUCE.md).

## Problem and change

The [500-socket checkpoint](../2026-10-28-capacity-checkpoint/REPORT.md) identifies
the shared nominal 504-ms food-maintenance opportunity as the dominant p99 event.
This change computes a sector's previous-to-current food diff once per broadcast,
then copies its encoded packets for recipients holding those exact listing `Arc`s.
The key includes the sector and current-listing address, retaining both identities.
Older views take the existing diff. Physical/decorated views remain distinct;
pickups are fixed for the broadcast. Dropping it drops all cached transitions,
including after errors. Packet timing, order, framing and bytes remain unchanged.

Owned packet bytes are still allocated/copied per recipient. Subscription cleanup,
listing construction and socket writes are unchanged. Additional cache memory is
bounded by the current physical/decorated listings touched in one broadcast;
no RSS improvement is claimed.

## Before: measured tail budget

The unchanged checkpoint's separate coarse top-1% window has 28 ticks. These
aligned disjoint conditional means add to world work, in CPU ms/tick:

| Work | Before |
| --- | ---: |
| Socket task | 3.905 |
| Broadcast | 3.452 |
| Subscription advance | 0.768 |
| All bracketed physics | 0.584 |
| Commands and handover | 0.049 |
| Simulation residual, including reporting overhead | 1.009 |

Its **separate fine** top-1% window brackets recipient food at 1.634 ms,
preparation at 0.613, snake visibility/encoding at 1.307, and emission/grouping
at 0.252. These nest inside fine broadcast and cannot be added to the coarse
parent. Fine clocks add overhead. This ranked food diffs ahead of subscription
cleanup and preparation as the first exact-reuse candidate. Collection/sort/dedup
is only part of preparation; no isolated listing cost or saving is inferred.

## Fixed work and byte identity

Runtime base is `692cd561714b8af3ea95adb3adcbd807b3a48734` (v0.8.0); the measured
implementation is `15792fa591c796492226fb64c1e97b98b6ef086a`. Later source edits
add only a report-link comment; integrated upstream #257 updates package metadata
to 0.8.1. Timings remain pinned to the measured v0.8.0 revisions. Heavy work uses
a 4-vCPU Zen 5 EPYC KVM guest,
Rust 1.99, locked offline dependencies, native CPU targeting, release/thin LTO
and one codegen unit. Selected receipts bind source and copied-binary identities.

The retained fixture warms at full speed without sockets or wall pacing, from
tick zero to 193,000 with every recipient active. It qualifies at 499 living,
mean length 55.3307, p90 176 and p99 247. Count the next 360 complete advances;
controls/admission/draining remain untimed. The short guard starts at tick zero
and counts 1,080 advances. One discarded AB pair precedes five alternating
AB/BA/AB/BA/AB pairs, each fork starting from its runtime's frozen full backend.

The validated physical-world checkpoint omits hidden-food visibility, presentation
lifetimes, shared listing handles and recipients' held listings/sector interest.
Restoring it alone cannot continue this path. Full-backend forks preserve that
state and natural wire history, avoiding a warmup per pair.

Every warm byte is compared live, then two repeated fork traces per runtime and
both direct parents agree in full: physical states, actions, ordered frame/time
bytes, RNG and lifecycle endpoints. CPU intervals include identical clock/PMU
wrappers; no overhead is subtracted. User instructions exclude kernel work.
These descriptive forked-fixture costs do not establish real-player capacity.

| Fixture | Ticks | M instructions/tick, before to after | Median reduction (five-pair range) |
| --- | --- | ---: | ---: |
| Mature 19 | Maintenance | 19.8109 -> 18.3150 | 7.551126% (7.551120-7.551128) |
| Mature 19 | Other | 8.3879 -> 8.3145 | 0.874593% (0.874587-0.874595) |
| Mature 19 | All | 8.5466 -> 8.4534 | 1.089541% (1.089534-1.089543) |
| Mature 13 | Maintenance | 20.9838 -> 19.5483 | 6.840962% (6.840959-6.840964) |
| Mature 13 | Other | 8.6981 -> 8.6286 | 0.798744% (0.798741-0.798767) |
| Mature 13 | All | 8.8687 -> 8.7802 | 0.997303% (0.997300-0.997325) |
| Short 19 | Maintenance | 17.8252 -> 14.6593 | 17.760839% (17.760798-17.760906) |
| Short 19 | Other | 5.8794 -> 5.7910 | 1.504440% (1.504399-1.504457) |
| Short 19 | All | 6.0453 -> 5.9142 | 2.170181% (2.170146-2.170198) |

Each mature window has five maintenance and 355 other ticks; short windows have
15 and 1,065. These repeats measure counter repeatability on one fixed scene,
not production-world variability. CPU values below are medians of five run means
(or per-run p99s), in ms. Timing variation overlaps between arms; instruction
reductions are not claimed as equal CPU reductions.

| Fixture | Maintenance CPU mean, before to after | Other CPU mean | All-tick CPU p99 |
| --- | ---: | ---: | ---: |
| Mature 19 | 4.856 -> 4.553 | 1.840 -> 1.767 | 4.781 -> 4.515 |
| Mature 13 | 4.794 -> 4.594 | 1.886 -> 1.807 | 4.828 -> 4.553 |
| Short 19 | 3.435 -> 3.170 | 1.490 -> 1.484 | 3.516 -> 3.395 |

The short guard's other-tick p99 rises from 2.907 to 3.042 ms despite its
slightly lower mean. Timing ranges overlap; no uniform CPU improvement is
claimed across individual tick classes.

All 27 catalog/host traces match complete decompressed bytes, with terminal-record
checks. Both stock/protocol-13 deliberate wire perturbations are detected. Mature
stock compares 5,876,276,676 warm bytes per arm; protocol 13 compares
10,132,210,980. Every proof trace and direct-parent continuation agrees. Filtering
the mutation traces to encoded frame payloads alone also detects differences
(first changed frames 21 and 567); this control does not rely on physical-state
records changing.

## Real sockets

The first three whole windows pass the unchanged body, client, queue, quiet-
process and CPU-reconciliation gates. Values below are observed CPU ms/tick.

| Window | Cache | Interior ticks | Maintenance ticks | Maintenance mean | Other mean | World p99 |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Off | 2,000 | 28 | 9.381 | 4.116 | 8.767 |
| 2 | On | 2,100 | 29 | 9.382 | 4.181 | 8.999 |
| 3 | On | 2,000 | 28 | 9.254 | 4.231 | 8.712 |

The final off window reaches mean length **65.224**, above the fixed limit 65.
The following fine window spans **65.192-66.776**, also outside the class. Both
fail qualification. The original all-window selection failure is preserved; a
post hoc reduction applies the same selector to every whole primary window,
retaining all three that pass. It does not complete the planned ABBA comparison
or establish a world-p99 improvement. No threshold or timed window is shortened
to rescue the attempt.

Recorded host/client/main-controller CPU is **2,255.69 seconds** (37.59 CPU
minutes), below 3,600 seconds; this figure excludes the small context sampler.
Another full warmup at this observed cost would exceed the remaining allowance,
so it is not run.

The checkpoint's full received-frame A4 clients and original body/client/queue/
CPU-reconciliation/quiet-process gates are reused. To bound warmup to about one
CPU-hour, one research binary runs a single mature world through four 15-second
cache-off/on/on/off windows with two-second settling gaps. Disabled calls the
original diff. This is a same-process control in an evolving world, not two
immutable production binaries or identical live trajectories. Fine clocks and
socket-call counters are disabled in primary windows; the separate fine
diagnostic follows. Its clock/counter overhead is not subtracted.

## Remaining measured work

The following **out-of-class explanatory diagnostic** ranks the remaining work;
it is not an accepted matched mature comparison. It keeps the full 500 clients
and passes preflight client/queue/accounting checks, while explicitly failing
the body-class gate. Its 25 maintenance ticks have these nested CPU means:

| Work | CPU ms/maintenance tick |
| --- | ---: |
| Socket task (disjoint from simulation) | 2.760 |
| Recipient food (within broadcast) | 1.741 |
| Snake visibility/encoding (within broadcast) | 1.008 |
| Subscription advance and processing flags | 1.000 |
| Preparation (within broadcast) | 0.820 |

Next candidates, ranked by this observed budget:

1. **Socket service and writes.** Investigate repeated service/flush work while
   preserving message boundaries. There are only 0.881 underlying TCP writes
   per recipient per maintenance interval, against 6.544 `Open::flush` calls;
   many flushes do not write. Loopback sends charge receive-side processing to
   the sender, so these CPU costs cannot be extrapolated to a real NIC.
2. **Remaining recipient food ownership/bookkeeping.** Repeated comparisons are
   shared, but each recipient still allocates/copies each cached packet and
   updates its held view. Test feeding shared bytes into unchanged framing.
3. **Snake visibility/encoding**, about 1.01 ms here, nearly tied with
   subscription cleanup at 1.00 ms. The latter has a particularly direct exact
   skip candidate; preparation is smaller and still needs an isolated listing
   profile before changing collection/sorting.

Diagnostic WebSocket messages / TCP writes / `Open::flush` calls per recipient
are **0.973 / 0.881 / 6.544** on maintenance intervals versus
**0.718 / 0.719 / 3.689** otherwise. These divide interval totals by 500; they
are not identical per-recipient counts, and socket work crosses tick boundaries.
Fine clock/counter overhead and the larger body class limit interpretation.

A subscription skip is sound only if its hint is a lower bound on every
non-current sector's `last_seen`. For fixed finite `now`, rounded floating
subtraction is monotone: larger `last_seen` cannot give larger elapsed time.
Thus `now - oldest <= RETENTION_SECONDS` proves none expires. Preserve this
subtraction, strict expiry boundary and touch-before-cleanup order; an added
future deadline need not round identically. No subscription change lands here.

## Checks, failed attempts and limits

The complete offline Rust group passes on implementation `15792fa5` (v0.8.0),
including all feature combinations,
loopback host contracts and existing pit/handoff/reconnect regressions. The new
oracle covers older recipients, physical/decorated views, pickups with unchanged
handles, sector identity and per-broadcast pickup invalidation. Removing the
previous-listing identity guard makes it fail at runtime. Stock/protocol-13 wire
perturbations are detected. Final documentation/format/privacy checks accompany
the PR. No independent review or outside-server evidence is claimed; the
maintaining agent reviews and lands the PR.

Failed attempts remain local-only: a mutation receipt lookup, a diagnostic timer
anchor preparation, an incomplete short proof after persistent disk filled, and
a mature-controller progress-logging failure. The failed short/mature cohorts
are excluded and repeated fresh. A selector display error occurred after valid
short data was written and was corrected outside measurement. Only this task's
externally copied, SHA-256-verified artifacts were removed; earlier studies were
left intact. Later traces use bounded temporary storage.

The sole task Cargo target is verified absent and no task workload executables
remain after collection. All 25 temporary proof traces survived the access pause. Early and final
archives are copied externally with matching SHA-256 hashes. Final source and
binary rechecks pass; the context sampler exits zero. Full traces, raw tick samples, inventories, sources,
copied binaries and failed attempts are local-only. Selected values and enough
code to recompute the tables are retained here; hashes identify artifacts rather
than establishing scientific or delivery acceptance.
