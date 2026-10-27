# One-core delivery-tail diagnosis, frozen before AWS collection

Retained #211 data at `65ff713` is examined first. Its receipts have no packet
family/send timestamps: correlate 100 ms wall-time bins with buffered tick
completion lateness; report that limitation, not an exact late-send attribution.
Tick lateness is actual tick start/end minus its 7 ms schedule, minimum-normalized
within the hold. Derive map/leaderboard due ticks from their source clocks.

Fresh runtime baseline is `587b597`, main at task start (#210 beyond #211's
runtime). Same frozen #178/#201 load executable, seed 7, zero bots, eight pinned
shards on a separate c7a.4xlarge, 15 s warmup, official-500, no boost/TLS/latency.
All host threads share the c7a.medium's one CPU. No outside game server.

## Bounded schedule

1. Build a fresh source snapshot with #201 counters/tick diagnostics and optional
   buffered stage tracing. No traces/I/O inside normal qualification ticks.
2. Eight-client/eight-second smoke, excluded from grades.
3. Unprofiled baseline official-500 for 120 s.
4. Diagnostic baseline official-500 for 90 s, with a 60 s system-wide kernel/user
   profile starting 15 s into the hold. Enable socket poll/representative slot-0
   queue/send/packet-family traces, schedstats, /proc thread user/system/sched
   snapshots, softirq/interrupt counts and mpstat. No speed grade from profiling.
5. Socket nice +5, unprofiled official-500 for 120 s. Apply after thread creation
   before warmup; verify effective priorities. This is an experimental OS setting,
   not yet a production default.
6. At most two further cheap candidates/holds, each 120 s, chosen and recorded
   before running from the profile and preceding result. No performance-based
   repetitions; retain every outcome. If nice +5 misses but improves simulation
   lateness, nice +10 is eligible. Fewer writes/batched reads only if supported
   by the measured hot path and a small behavior-safe prototype.
7. If affordable, sequential c8a.medium diagnostic official-500, same profile,
   to investigate the prior socket/simulation architecture ratios. This is a
   diagnostic control, not an additional capacity qualification.

Unchanged grade: pooled frame-excess p99 <=10 ms, zero new deferred ticks, valid
decoded traffic/occupancy. Tick tails are diagnostics. Report total/thread CPU,
user/kernel CPU, delivery/tick lateness, RSS, counts and profiler overhead limits.
One hold per candidate is descriptive; do not claim a stable production SLO.
Freeze any added experiment in a dated amendment before its collection.

## Budget, preservation and delivery

One Frankfurt Spot session, at most one generator plus one server concurrently.
All instances have verified 120-minute termination timers, terminate-on-shutdown
and root delete-on-terminate. Max compute at caps (.65+.06+.08)*2 = $1.58;
reserve $1.20 for storage/IPv4 and <=5 GiB total evidence downloads, below $3.
Check projected spend and existing STS login before each hold. Stop if login
expires; obtain no credentials. One infrastructure replacement is allowed within
this same budget, with failed/partial attempts retained and disclosed.

Build on generator, Rust 1.99, locked release/thin LTO/one codegen unit/debug line
tables, CARGO_BUILD_JOBS=1, fresh target. Local work <=2 logical CPUs. Prefer
cpu-clock if guest cycles are unavailable; record -a -g with DWARF user stacks
and kernel symbols. Only the ephemeral server's perf/kptr/schedstat settings may
change, and profiling settings are disabled for unprofiled candidates.

Preserve raw data outside Git with hashes, including failures. Publish only a
small selected study without cloud/personal identifiers. If a cheap fix works,
implement it on this branch, run the usual Rust checks and open an evidence PR;
do not merge. Otherwise report the diagnosis and rejected prototypes. Afterwards
delete created resources and attempt every commercial region's instances,
volumes, open/active Spot requests, EIPs and created groups/keys. Explicitly
identify disabled/API-inaccessible regions without asserting their inventory.
