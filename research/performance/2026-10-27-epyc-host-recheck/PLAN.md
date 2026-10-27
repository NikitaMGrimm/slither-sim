# One-core host recheck, 2026-10-27

Freeze before collection. Synthetic protocol-19 socket clients only; no outside
game server. Source baseline is `48977ec` (the runtime measured by #178), current
is `ca7ac2f`, and optional vision is `a979b10`. The latter diverges before
#196/#197/#199 and is a separate branch sample, not an isolated vision comparison.

Question: does current main qualify 500 official clients on one c7a core?
Prediction to test: the current official-500 hold meets the unchanged #178 grade.
One seed/hold per arm is descriptive, without a confidence interval or attribution
to individual optimizations. C8a is an architectural control, not EPYC 9645 hardware.

## Frozen schedule

- One Frankfurt Spot session, separate c7a.4xlarge generator; sequential
  c7a.medium then c8a.medium servers, one exposed core/thread per server.
- c7a: baseline official-500, 120 s; current official-500, 120 s.
  If current passes, confirm current official-500 for 300 s. If either current
  500 hold fails, descend to current 375 then 250 for 300 s, stopping at the
  first passing count. Report the highest tested passing count and its duration.
- c7a: baseline Battledome-80, 300 s; current Battledome-80, 300 s;
  vision Battledome-80, 300 s if its build/smoke succeeds without substantive
  adaptation. Retain any optional failure and skip rather than change the branch.
- c8a: current official-500 then baseline official-500, 120 s each.
- First validate each binary with an 8-client/8 s smoke, excluded from grades.
  No performance-based repeat or exclusion. Infrastructure failures stay retained;
  one bounded retry of an interrupted hold is allowed in this same session.

## Contract and measurements

Reuse #178's exact load executable, challenge/framing/decoder, eight pinned shards,
seed 7, zero bots, default presets, no TLS/added link delay, no boost, 100 ms
steering and 250 ms death rejoin. Clients begin before a shared future epoch;
warmup is 15 s from that epoch, as in #178. Host uses `--stats 5` and the same
public-counter/1 ms diagnostic sampler and `/proc` resource sampler.

The grade remains pooled per-connection-normalized frame-excess p99 <=10 ms and
zero newly deferred ticks, subject to valid decoded traffic and occupancy.
Also report receipt excess p50/p99/max, own-update gap p99/max, executed ticks/s,
CPU ms/executed tick, process CPU/RSS, operator stats, playing population, errors,
deaths, admissions, initial/respawn waits and pending requests.

Add equal study-only instrumentation to all server binaries: each executed tick's
busy wall duration (command intake through delivery handover) and start-to-start
gap, buffered in memory and written after shutdown. Report nearest-rank p99/max
for tick starts inside the hold. These have no new pass threshold: #178 did not
retain per-tick durations. The timer includes preemption, not only CPU.

## Budget and preservation

Local work uses at most two logical CPUs and `CARGO_BUILD_JOBS=1`; release builds
run on the generator with Rust 1.99, locked dependencies, thin LTO, one codegen
unit and line-table debug info. Copy each binary before the next build.

Cap $8. At most one generator and one server run concurrently. Every instance
uses one-time Spot, root delete-on-termination, terminate-on-shutdown and a
verified 120-minute in-instance hard timer (planned session <=90 min +30).
Even timer exhaustion at launch caps (.65 + .06 + .08)/h for two hours totals
$1.58 compute; reserve $2 for storage, IPv4 and <=10 GiB downloads, below $8.
No credentials are obtained or printed. Stop if the existing login expires.

Retain all raw artifacts outside Git with hashes; publish selected results,
source/build pins, failures and evidence-access limits without cloud identifiers.
Delete only created resources. Attempt every commercial region after teardown:
instances (all non-terminated states), volumes, open/active Spot requests, EIPs,
and created groups/keys. Disabled/API-inaccessible regions are reported explicitly
without claiming a verified inventory or enabling regions.
