# Operator-only c7a official-500 tuning, 2026-10-28

Frozen before collection. Question: can current main satisfy the existing
official-500 delivery gate on one c7a.medium core using only socket niceness and
ENA moderation? Expect D to have the strongest chance; retain every miss and
invalid attempt. Two short repetitions cannot establish a long-uptime guarantee.

## Runtime, load and gate

Pin origin/main at task start: `4dd66ab00ab8b0acb2b9ea1432a43f9fd31a8196`.
This is documentation/research-only #232 atop `4532a76`; verify the complete diff
contains no runtime/build files, then reuse #232's hashed instrumented server and
exact #201/#178 frozen load executable. Record both build and task-start commits.
No runtime change. A failed source-identity check requires an AWS build, jobs=1,
before collection, not silent reuse. Never push to the owner's #234 branch.

One Frankfurt Spot c7a.medium server throughout, separate c7a.4xlarge generator.
Official preset, 500 sockets, zero bots, seed 7, eight pinned load shards, 100 ms
steering, no boost/TLS/added latency, same 250 ms death rejoin. Fresh world each
hold, 15 s warmup after a shared future epoch, 120 s hold, --stats 5 retained.
One eight-client/eight-second smoke (three-second warmup), excluded from grading.
No outside game servers. Use the unchanged #201 gate/reducer: pooled frame-excess
p99 <=10 ms and zero newly deferred ticks, with valid traffic/population. Tick
busy p99/max are diagnostics, with no additional threshold. Exact nearest ranks,
half-open tick-start windows and CPU per executed tick follow #232/#201.

## Arms and fixed sequence

Capture boot ethtool -c before changes; this is the default state, including RX
adaptive flag, RX usecs and TX usecs. Restore it before applying every arm.

- A: boot NIC state, socket nice 0.
- B: boot NIC state, socket nice +5.
- C: boot NIC state with RX usecs=64, socket nice 0. Preserve TX and the default
  adaptive flag; this matches #217 if boot is again adaptive on/RX20/TX64.
- D: C plus socket nice +5.
- E: boot NIC state with adaptive RX on, socket nice 0, only if a pre-hold
  ethtool command succeeds and readback confirms on. If boot was already on,
  E is an explicitly labeled duplicate-default control; still collect both holds.

Run A1 B1 C1 D1 [E1], then [E2] D2 C2 B2 A2 on the same server. No replacement
holds or result-based repeats/exclusions. If E is unsupported, omit only E and
record the failed capability probe. If a requested setting cannot be verified,
stop before dependent qualification and retain partial evidence. No recovery run.
An arm passes only if BOTH its valid holds pass the unchanged gate. Do not pool
default/E aliases or select a single favorable repetition.

Record full ethtool readback before restoration, after application, before warmup
ends, and after each hold. Verify actual niceness through /proc before warmup
ends and at hold boundaries; other host threads stay nice 0/SCHED_OTHER and all
host threads use CPU [0]. Set nice unprivileged on the socket thread only. Verify
the NIC's adaptive/RX/TX values against each arm, not only command exit status.

## Recording, budget and retention

Record every hold's grade, excess p50/p99/max, CPU ms/tick and share of one core,
sim/socket and user/system splits, tick busy p99/max, deferrals, tick rate, playing
population, peak RSS, operator load, generator errors/CPU and network IRQ/s.
IRQ/s uses the host /proc/interrupts Tx-Rx queue counters at the same resource
boundaries; preserve total interrupts too. No profiling during qualification.

Hard cap $2. One generator/server, each with a verified 90-minute hard shutdown
timer (planned <=60 minutes plus 30-minute margin), terminate-on-shutdown and
delete-on-terminate root storage. Compute caps (.65+.06)*1.5=$1.065; reserve $.70
for disks, IPv4 and <=3 GiB evidence downloads. Check existing login and projected
remaining cost before each hold; stop later work at $1.80. If login expires,
stop/report without obtaining credentials. No kernel boot or mitigation changes.

Local work is light orchestration/reduction on one logical CPU, no local builds;
respect the owner's total Windows budget of three CPUs and use RAM sparingly.
Run the large client-frame reductions on the generator. Retain raw receipts,
all attempts and full settings logs outside Git; selected JSON and archive hashes
go in one research PR with report/plan/reducer and a plain index row. No changelog
or hosting edits. Delete created resources and attempt every commercial region:
instances, volumes, open/active Spot requests, EIPs and created groups/keys.
Report disabled/API-inaccessible inventories unverified; do not enable regions.
Read-only cleanup retries may recover API errors, retaining failed scans.
Never print credentials, cloud/account identifiers or IPs in chat or public files.
