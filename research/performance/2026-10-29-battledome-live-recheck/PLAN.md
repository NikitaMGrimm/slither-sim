# Frozen real-socket comparison after #265/#266

Base is `cc1bb1e9935094a00cf513413d7922d858dd5c1e`, the parent of #265's
merge commit `78de98b8`. Candidate is `539b37f3be969827b5eac67c2978e604f9e814e8`,
containing #265 and #266. Freeze these before collection; no production edits.
Marker reuse is parked, not included in either arm.

Reuse #263's shipped responsive Battledome configuration, seed 7, 641 reserved
remote slots, zero in-process bots, real protocol-19 WebSockets and six fully
decoding received-frame A4 workers on a second eight-vCPU machine. Override
only A4's boost output when received free-floor-boost rules permit it; preserve
heading bits and the shared policy. Use one identical client executable for
both server arms. Target the earlier approximately 99.7–99.9% living-tick boost
duty, record actual duty and boost-food births, and require at least 99% for
treatment qualification. Keep invalid rows rather than silently replacing them.

One Frankfurt same-AZ Spot pair: c7a.medium server, eight-vCPU generator,
private bot/control/artifact traffic. Pin all server tasks to its single vCPU;
six generator workers use cores 0–5 and controller core 7. No SSH bench guest.
No owner play hold. Preserve the existing restricted management/game access;
publish no addresses or AWS identifiers.

Primary order is **base-160, candidate-160, candidate-320, base-320, base-480,
candidate-480**. Each primary row starts a fresh world and fresh clients. Warm
naturally at 80 for 300 seconds, then follow the 80/160/320/480 ladder only up
to that row's target, retaining each 60-second settle and 90-second marked
stage exactly as in #263. Earlier stages are retained calibration/history,
not extra primary replicates. This gives matched scheduled wall-time prehistory
per seat count without pausing hosts/clients or competing hosts on one core.
Actual virtual age, bodies and food can diverge; retain their census.
Only one host/client cohort runs at a time. Six fresh ladders take about 75
minutes; reserve build/retrieval time within a planned 110-minute task.

Use portable Rust 1.99 x86-64 release builds, identical overlays and flags,
native boost-rule regression and source/policy checks. Rebuild this package
per arm before copying/hashing binaries. Keep clock/phase overhead in both
arms. Reuse #263's complete 100-tick interior blocks, whole client reports,
generator/queue/error gates and independent task-clock reconciliation.

Report aligned simulation+socket CPU p50/p99/max, deferrals, frame residual
jitter p99 and stalls/excess, completed respawn p50/p95/max plus pending waits,
alive/occupied counts, body/food census, actual boost duty/births and all-row
deferrals. Select each row's own slowest CPU 1% before attribution. Categories:
admission search; socket CPU plus broadcast excluding nested minimap; minimap;
other world CPU. Nested phase quantiles must not be added. State what dominates
320 and 480. One window per arm/count is a descriptive live check, not a
replicated causal speed ratio, capacity maximum or human-play acceptance.

Keep all host overload rows. Stop remaining collection for process failure,
invalid generator/queues or treatment; retain failed rows and prior calibrations.
No replacement pair is automatic. $3 cap for this collection. Bid ceilings
$0.06/h server + $0.30/h generator, 140-minute boot-installed terminate timers
(110 planned +30), terminate-on-shutdown and DeleteOnTermination disks bound
full-timer compute to $0.84. Reserve $1.40 noncompute: total ceiling $2.24.
Verify timers/attributes before work. Stop immediately on an expired CLI
session; do not wait for login or refresh/reconfigure credentials manually.

Close, gzip-read and hash each row archive before atomic readiness. Download
only after readiness; verify exact size/digest and required sources before
deleting any source. Preserve all rows locally before teardown. Terminate the
pair, cancel own open/active Spot requests, delete own keys/groups/volumes and
scan every enabled region for nonterminated instances, active/open requests,
volumes and Elastic IPs. Never touch the protected mail-filter resources.
Record unavailable coverage explicitly and do not delete unrelated resources.

Publish a study PR with one index/changelog entry, lean selected values and
recomputation code, links to reused harnesses, no per-file source maps and only
the closing assistance footer. Do not merge. Record parked marker branch
`perf/pit-marker-reuse` at `58145ae5ed33b912efe4151eb9e4d3414d78c235`: no PR,
no guest time; its local work is not a live optimization in this comparison.
