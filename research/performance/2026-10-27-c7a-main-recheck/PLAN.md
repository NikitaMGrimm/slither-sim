# Same-session c7a main recheck, 2026-10-27

Frozen before collection. Question: does current main meet the existing delivery
grade for 500 official socket players on one c7a.medium core? Expect lower user
CPU from recent optimizations; whether kernel/scheduling tails pass is unknown.
No causal attribution to individual changes or confidence interval is planned.

## Arms and order

- C: current origin/main `4532a76f22958ae3abcca801e90edd30ed9f466f`.
- B: same-session control `65ff713` (resolve full SHA before building).
- One c7a.medium host, separate c7a.4xlarge generator, Frankfurt Spot.
- Eight-client/eight-second smoke per arm, three-second warmup, excluded.
- Qualification: B official-500, then C official-500, 120 seconds each.
- If C passes: second official-500 pair in C-B order, 120 seconds each.
- If C misses: one C official-500 diagnostic, socket nice +5, 120 seconds.
- If budget/time remains: C official-375, then B official-375, 120 seconds each.
- Each hold starts a fresh world. No performance-based exclusions or retries.
  Infrastructure failures remain evidence; stop and report, no automatic recovery.

## Measurement contract

Reuse #211's #201/#178 exact frozen load executable, instrumentation and reducer:
eight pinned shards, seed 7, 15-second warmup, common epoch eight seconds ahead,
100 ms steering, no boost, TLS, added latency or host bots; death/rejoin unchanged.
No outside game servers. Keep operator `--stats 5` lines. Grade is pooled frame
excess p99 <=10 ms and zero newly deferred ticks, with valid traffic/occupancy.
Tick busy p99/max are diagnostics, not an extra pass threshold. Exact nearest
ranks and half-open tick-start windows remain unchanged. Failed control remains
reported; a control miss is expected, not a prerequisite to discard a main pass.

Add user/system counters to the existing /proc sampler, at the same boundaries,
for process and each thread. Report CPU per executed tick, share of one core,
sim/socket and user/system splits, ticks/s, deferrals, peak RSS including warmup,
operator load, playing population, excess p50/p99/max, busy/gap p99/max and errors.
Nice +5 changes only socket thread priority, verified before warmup finishes;
default arms retain normal priority. No runtime change is proposed.

## Build, bounds and retention

Build each instrumented pinned snapshot on the generator, Rust 1.99 locked release,
thin LTO, one codegen unit, line-table debug info, fresh per-arm targets and
CARGO_BUILD_JOBS=1. Freeze source, instrumentation, worker, binary and plan hashes.
Local orchestration/analysis is limited to two logical CPUs; no local build.

Hard cost cap $3. One server and one generator; terminate-on-shutdown and
delete-on-terminate 24 GiB roots; verified 120-minute timers (planned <=90 minutes
plus 30-minute margin). Compute caps (.65+.06)*2 = $1.42; reserve $1.20 for disks,
IPv4 and <=5 GiB downloads, below $3 even if timers expire. Before every hold,
check existing login and conservative projected spend; stop optional work at
$2.80. If login expires, stop without obtaining credentials or working around it.

Retain all attempts and raw logs locally outside Git; publish bounded selected
JSON, hashes, methods, failures and limits. Delete created resources, then scan
every commercial region for nonterminated instances, volumes, open/active Spot
requests, EIPs and created groups/keys. Disabled/API-inaccessible regions must be
reported as unverified, without enabling them. Never print credentials/identity
or publish cloud identifiers. No merge. Update hosting only if results change it.
