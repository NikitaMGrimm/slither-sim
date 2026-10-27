# One-core host after indexed pickup/delivery, 2026-10-27

Frozen before collection. Question: does main `65ff713` meet the #201 delivery
grade with 500 official socket clients on one c7a.medium core? Prediction:
official-500 passes; retain misses without changing thresholds or repeating for
speed. Compare descriptively with #201's `ca7ac2f`, without a fresh baseline arm.

## Schedule and contract

One Frankfurt Spot session, separate c7a.4xlarge load generator; sequential
c7a.medium then c8a.medium servers. Exactly one qualification hold per row:

1. c7a official-500, 120 seconds; official-375, 300 seconds.
2. c7a default Battledome-80, 300 seconds.
3. c8a official-500, 120 seconds, only while budget remains.

First validate the current binary with an eight-client/eight-second official
smoke (three-second warmup), excluded from qualification. Required holds use
#201's exact frozen #178 load executable, eight pinned shards, seed 7, 15-second
warmup after a shared future epoch, 100 ms steering, no boost/TLS/added latency,
zero host bots and fresh worlds. Clients die and rejoin with the same 250 ms
delay. No outside game servers. No optional branch or additional ladder counts.

Grade: pooled frame-excess p99 <=10 ms and zero new deferred ticks, subject to
valid decoded traffic and occupancy. Busy tick p99/max remain diagnostic, with
no separate threshold. Reuse #201's study-only buffered tick instrumentation,
1 ms public-counter sampler, --stats 5 and /proc resource sampler unchanged.

Report CPU ms/executed tick, process share of one core, simulation/socket thread
split, deferred ticks, ticks/s, peak RSS including warmup, operator load, frame
excess p50/p99/max, tick busy/start-gap p99/max and playing population. Preserve
Battledome initial/respawn waits, pending requests, deaths and admissions. Exact
nearest ranks and half-open tick windows match #201. No performance-based
repeats/exclusions. Retain infrastructure failures; stop if AWS login expires.

## Build, budget and evidence

Pin main's complete Git SHA; instrument an isolated snapshot using #201's
instrument.py and #178 harness.patch. Build only the server on the generator:
Rust 1.99, locked release, thin LTO, one codegen unit, line-table debug info,
CARGO_BUILD_JOBS=1 and a fresh target directory. Copy/hash before use. Local
analysis/orchestration uses at most two logical CPUs.

Hard cap $3. One generator and at most one server concurrently; each has a
verified 120-minute shutdown timer, terminate-on-shutdown and delete-on-terminate
root disk. Planned duration <=90 minutes plus 30-minute margin. Maximum compute
at launch caps (.65+.06+.08)*2 = $1.58 even if all three timers expire. Reserve
$1.20 for <=5 GiB total downloaded evidence, disks, IPv4 and minor transfers.
Skip later holds if conservative projected cost reaches $2.80. Raw artifacts
remain outside Git; selected results include source/binary/harness/archive hashes.

Delete created resources, then attempt every commercial region: non-terminated
instances, open/active Spot requests, volumes, EIPs and created groups/keys.
Report disabled/API-inaccessible regions explicitly, without enabling them or
claiming their inventories were verified. Never print credentials or publish
cloud identifiers. One seed/hold per row supports a bounded capacity observation,
not causal attribution, a confidence interval, human-browser or long-uptime SLO.
