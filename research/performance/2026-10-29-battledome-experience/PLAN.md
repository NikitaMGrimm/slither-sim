# One-core Battledome experience plan

Frozen before paid collection on 2026-10-29. Runtime/source archive:
`165a5bb764aba6cbf39f42503e090f9b5e3c4d22`. Production sources stay unchanged.
Use the shipped Battledome world, responsive delivery, default arena/admission,
zero in-process bots, seed 7, 7 ms ticks, and one extra human socket slot.
Build a read-only archived-source measurement overlay on the generator with
Rust 1.99.0, locked release dependencies and portable x86-64 flags.

## Infrastructure and stop conditions

One c7a.medium Spot server and one eight-vCPU Spot generator in the same
Frankfurt availability zone, private bot/control traffic. All host threads
share its single vCPU; six received-frame client workers use generator cores
0–5 and control uses core 7. No use of the separate SSH guest, no local builds.
Verify the existing AWS login first; stop on expiry without refreshing or
obtaining replacement credentials. Public access and raw cloud metadata remain
private. Never touch unrelated resources, including the protected pre-existing
key and group (name redacted for publication).

Plan at most 100 minutes including build, collection and at most 60 minutes of
human hold. Each boot installs a 130-minute shutdown timer, with shutdown
behavior terminate, one-time Spot/terminate-on-interruption, and deleted root
volumes. Bid ceilings $0.06/h server and $0.30/h generator give at most $0.78
compute through both timers; reserve $2 for disks, IPv4 and bounded egress,
under the $3 whole-task cap. Only this pair; no automatic Spot replacement.

## Workload and selection

Reuse [#258](../2026-10-29-c7a-mature-capacity/REPRODUCE.md)'s source overlays,
aligned simulation/socket CPU clocks, remote host agent and real protocol-19
client. Every frame is decoded; every decision materializes the received view
and calls the unchanged pit-aware shared A4 policy at 42 ms, including its boost
choice. Physical-food/rules opt-in and normal pings remain. No thin or
authoritative-policy input, checkpoint, edited ecology or forced churn.

Start 80 bots and grow naturally for five minutes. Battledome's size floor,
pit deaths and free boosting have a different length distribution from the
official mature-class screen; report the actual census rather than impose that
screen. Ascend 80, 120, 160, 240, 320, 480, 640, stopping at the first clear
degradation: deferred ticks; world CPU p50 >=6.8 ms; mean alive below 90% of
target after settling; pending login age >=30 s; client frame-gap p99 >100 ms;
or generator/queue failure. Each step settles 60 s and counts 90 s. Keep failed
rows as limits. Descend once to 80, settle 30 s and count 90 s to expose world
history effects. No interpolation, competitor speed ratio, uptime qualification
or optimization saving is claimed. This is one evolving-world exploratory
ladder; no fitted mechanics or held-out law is selected.

## Measurements

Select complete 100-tick interior blocks. Sum simulation and socket task CPU
at each consecutive tick edge before p50/p99/max. Retain wall busy time,
deferrals, queue errors/trends, process/task CPU reconciliation and observer
overhead. Per-tick read-only phases retain movement, collision build/query,
death food, pickup, prey, refill, admission and broadcast. Nested broadcast
clocks distinguish shared food, coverage, recipients, maps, board and markers;
do not add nested times. Mark food-epoch, minimap, board, marker changes and
strict/fallback/refused admission searches. Rank the slowest 1% by aligned world
CPU and inspect cadence/phase means, also checking wall-time slow ticks.
Instrumentation cost is included; no uninstrumented speed claim.

Clients collect frame-receipt gaps while continuously playing at their real
network location. Pool microsecond-floor histograms; report p50/p99/max,
gaps >84 ms (twice the normal six-tick motion interval), >100 ms and >250 ms.
These are application receipt gaps, including client scheduling and packet
opportunities, not isolated network latency. Record input-lateness and CPU on
every worker: require p50 <70%, p99 <80%, worst one-second lateness p99 <42 ms,
all six workers healthy and no decode/write failures. Input-to-effect latency
is optional only if an existing cheap received-frame measurement is available;
do not call ping or receipt gaps input-to-effect.

Record initial and respawn login-to-first-playing waits, completed quantiles
and pending/censored ages. Exclude the fixed 28 ms reconnect delay from those
waits, disclose handshake/worker scheduling bounds and report observed early
deaths (<3 s, <10 s) without inferring unfair geometry from deaths alone.
Record living/occupied seats, radius, length/food census, actual boosting
alive-tick fraction, boost action share, boost-food births and surviving count.
Floor boost makes no pellets; time boosting and food births are separate.

## Human hold, cleanup and publication

When listening and publicly reachable through the supplied single-address
game rule, write the endpoint only to the requested scratch endpoint file.
Retain once-per-second public-playing occupancy and label affected windows.
After measuring, keep the same server at 80 A4 bots until the done file appears,
60 minutes of hold elapse, or the planned/self-termination boundary approaches.
Then delete the endpoint file, retrieve/hash private raw results, terminate
both instances, cancel task Spot requests by recorded request/instance identity,
delete task keys/groups, verify deleted volumes and scan every enabled region
for instances, open/active Spot requests, volumes and Elastic IPs. Retain
unavailable-region errors as unavailable. Publish bounded selected results,
table reducers and this study's harness deltas; link prior scripts, no patches.

Rank observed causes by CPU share within slow ticks and by measured spawn
outcomes. Byte-identical optimization candidates are proposals requiring exact
trace controls in a later implementation; timing, admission or rule changes
require the maintaining agent's approval on a local play server first.

## Measurement correction after the first 80-seat window

The first expanded phase-log serialization dominated the slowest 1%, repeating
every 100 ticks. Retain that failed measurement and interrupted 120-seat stream.
Before further counted windows, buffer expanded CPU/census records in memory and
serialize only after counting is disabled, preserving tick-edge timestamps.
Restart the natural world on the same pair with the corrected measurement
executable. All sampling, load, validity, price and original launch-relative
timer/hold limits remain unchanged. This correction authorizes no additional
instance pair or budget extension.

## Recovery from controller teardown

The same-pair restart's controller omitted its local `access` variable and
entered teardown before any corrected measurement. Preserve that error and
the first expanded-log attempt. After verified first-pair cleanup, allow one
replacement pair with the original complete controller and buffered overlay.
The replacement has the same 100-minute plan/130-minute boot timers and at most
60-minute human hold. No pairs overlap. Bound the first pair's compute below
$0.12 using recorded termination, plus $0.78 for both replacement timers and
the original shared $2 noncompute reserve: maximum $2.90. Verify session identity
before launch; stop on expiry. No other recovery or budget extension is planned.

## Receipt-cadence correction before replacement counting

Source inspection confirms that straight, unchanged snakes can legitimately
wait about 200 ms for their next anchor. Raw >100 ms receipt gaps therefore do
not identify stutter. Retain those gaps and add protocol elapsed-header-relative
telemetry: receipt gap minus emitted server-time increment, and receipt time
minus cumulative server time relative to the connection's running minimum.
Report both; call only header-relative excess a delivery-delay diagnostic.
The 100 ms p99 degradation stop applies to frame excess; retain raw gaps as an
inspected diagnostic. This corrects cadence interpretation without changing
policy inputs or outputs. Preserve the replacement's initial warmup and restart
on the same pair before any counted window, with no timer/resource extension.
Add a synthetic normal-210-ms/5-ms-excess control.

## Final infrastructure recovery before qualified collection

The replacement's same-pair telemetry restart lost one SSH status connection;
the fail-fast controller terminated it before counting. Retain that operational
failure. Fresh all-region preflight verifies no remaining resources. Allow one
final nonoverlapping pair with four bounded read-only SSH reconnect attempts;
process-launch commands are not replayed. Prior compute is below $0.15, final
full timers cost at most $0.78, and the shared noncompute reserve is $2: aggregate
maximum $2.93. Same source, clients, ladder and 100/130-minute durations.
No session refresh, gate relaxation or additional concurrent server.

Buffered JSON is converted to a string before a bounded stderr write to avoid
fragmented formatting syscalls. Reset received-frame excess baselines at each
counted window boundary; retain all deferrals and raw gaps, including uncounted
flush intervals. The natural warmup restarts from tick zero without a checkpoint.

## Complete-report client selection after inspecting the 320-row maximum

The online end-timestamp filter can retain a first one-second report containing
pre-RESET delay history. Preserve online metrics/gates exactly. Additionally
recompute every row's public client statistics from whole worker reports with
both preceding and ending timestamps inside its CPU interval. Freeze this
uniform boundary rule before upper-count selection; no host ticks or online
stop are altered. Add a synthetic contaminated-first-report control. Retain the
original online maxima and all deferrals outside counted windows as limits.

## Maintaining-agent-authorized recovery after archive loss

At 03:42 UTC the maintaining agent explicitly authorizes one further collection,
counting all prior costs under $3. Three earlier pairs are terminated. Verify
their keys/groups and regional inventories before launching one fresh pair.
Use the same workload and ladder, with whole-client-report selection online
and a newline-terminated log reader. Publish terminal status only after complete,
verified archive/digest publication. The controller preserves source instances
until a complete local download, matching size/SHA-256 and required-file checks
succeed. Boot termination timers remain the final resource safeguard.

New planned duration 60 minutes, boot timers 90 minutes (plus 30 minutes).
Prior bid-cap compute upper bound $0.2923 + new full timers $0.54 + shared
noncompute reserve $1.50 = $2.3323. The maintaining agent says the human is asleep
and the done file exists: skip the play hold and tear down immediately after
measuring and verified retrieval. Stop on AWS login expiry without workaround.

## Canonical-log completeness check

The 480 online reducer drained only 81.2 seconds of a 90.2-second marked window
before computing its provisional result. Preserve that original tick range and
online stop decision. After complete archive retrieval, use the canonical server
log for all complete 100-tick interior blocks in every marked window. Reproduce
the original online result on its exact retained range, retain both aggregates,
and reconcile the canonical window against task CPU before qualification.
Synthetic delayed-stream controls must recover the full canonical window while
reproducing its shorter online subset. This restores the frozen window design;
no collection or workload changes, no new cloud pair or repeat.
