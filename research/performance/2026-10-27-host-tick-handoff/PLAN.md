# Single-core host handoff, frozen before collection

Runtime base: main `40ce9fac41db8b3043b827056f21ebefe27a706a` at task start.
Question: can single-core turn-taking qualify official-500 without moving delay
to input handling? Experimental env switch only; default concurrent and multi-
core paths retain immediate handoff. No outside game servers.

## Schedule

One Frankfurt Spot session; separate c7a.4xlarge generator. One unprofiled hold
per arm, same #201 grade and 120 s duration, 15 s warmup, seed 7, eight shards,
zero bots, default official preset, no boost/TLS/added latency. Fresh worlds.

1. Validate builds and 500-seat protocol traces (19, mixed 13/19, 13); eight-
   client/eight-second socket smokes (three-second warmup), not qualification.
2. c7a.medium baseline concurrent main, 500 sockets /120 s.
3. c7a.medium candidate turn-taking /120 s: simulation owns each tick through
   full handoff; socket pass reads inputs/sends due frames before next tick.
   Socket idle polling cannot run inside the tick. Future due frames may run
   between ticks while holding the same exclusion gate. Default early handoff
   is unchanged when the switch is unset.
4. If turn-taking misses, c7a candidate inline /120 s: the simulation thread
   executes a nonblocking gateway pass after each tick, without a socket thread.
5. c7a baseline binary with ephemeral ENA interrupt moderation /120 s: inspect
   supported settings, request adaptive-rx on plus rx-usecs/tx-usecs 64 when
   supported. Freeze exact successful setting before its hold; record readback,
   interrupts/softirqs/allowance counters, revert before other controls. No code
   or security-mitigation default change.
6. If a candidate qualifies, sequential c8a.medium baseline and that candidate
   /120 s each, then c8a.large two-core baseline/candidate default concurrent
   /120 s each to confirm Zen 5 and the default multi-core path. Same one-hold
   rule, with no performance repeats. No additional candidate tuning.

Grade: pooled per-connection-normalized frame-excess p99 <=10 ms, no new
deferred ticks, valid decoded traffic and occupancy. Tick busy/phase tails,
ticks/s, total/thread user/kernel CPU, peak RSS and playing counts are diagnostic.
No new threshold or relaxed gate. Retain every smoke/failure/hold.

## Inputs and parity

Reuse the frozen #201 load source/control/decoder, adding only buffered samples
for client IDs divisible by 64. Record command acceptance by the client socket,
actual server read/forward and consumption by the simulation before physics.
Match per-client ordered steering bytes across streams, retain unmatched samples,
and report send-to-ingest, read-to-ingest p50/p95/p99/max plus target tick timing.
Cross-machine send-to-ingest depends on measured chrony clock synchronization;
these are application boundaries, not NIC wire times. Trace overhead is equal
in all arms. No nonce/protocol/control changes or extra messages.

Freeze source/build/patch/harness/binary hashes. Compare deterministic per-
recipient payload/timing traces and final checkpoints across all three schedule
modes with fixed inputs, including reconnects and modern/legacy frames. Same
Rust 1.99 locked release, thin LTO, one codegen unit, debug line tables, fresh
targets and CARGO_BUILD_JOBS=1. Builds/checks on generator where practical;
local work <=2 logical CPUs. Successful Rust changes need the usual Rust group,
rebase onto main, repeated parity after conflict resolution, and confirmation
before PR. Preserve another worker's framing edits; do not modify its checkout.

## Budget and preservation

Hard cap $3. One generator and at most one server concurrently. Verified
120-minute hard shutdown/terminate timers, root delete-on-terminate. Max compute
at all launch caps (.65+.06+.08+.15)*2 = $1.88; reserve $1.10 for disks/IPv4 and
<=5 GiB total downloaded evidence. Check existing STS and conservative projected
spend before each hold; stop if session expires, obtain no credentials.
One infrastructure recovery is allowed only inside the same cap, retaining the
partial attempt. Do not repeat for speed or claim capacity from profiler timing.

Retain full logs/traces/sources/binaries outside Git; publish selected results
and hashes without cloud identifiers. If no variant passes, report all outcomes
and retain prototype as a research patch, with no runtime default/hosting change.
Afterwards delete created resources and attempt every commercial region's
instances, volumes, open/active Spot requests, EIPs and created groups/key pairs.
Disabled/API-inaccessible regions remain explicitly unverified, not enabled.
