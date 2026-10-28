# Frozen hosted capacity protocol

Question: can current main sustain 142.857 complete 7 ms physics ticks/s with
2,281 occupied seats at official density on one logical CPU? Research only;
offline loopback, no runtime changes delivered. VM elapsed times are descriptive.

Primary runtime pin after #245 landed:
`d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`. The pre-merge warmup is retained
as an exploratory attempt, not pooled with primary results.

- Pin main immediately before building. Include #244; include #245 if merged,
  otherwise name the measured revision. Locked release, native CPU target,
  thin LTO, one codegen unit, debug=1; Cargo/Rayon one worker.
- Real CLI `host --stats 5`, seed 7, official world, official delivery,
  default link/processing distributions, two default A4 bot workers. All server
  threads inherit one logical CPU. The server's mandatory player slot means
  exactly 2,281 seats permit 2,280 background bots plus one player. Per owner's
  clarification, use one loopback A4 NetworkBackend client on another CPU;
  record its CPU separately. Reference: 500 background bots with the same
  socket client (501 occupied seats), explicitly report both counts.
- Reuse the public portable_bot client, host counters, /proc thread sampling
  from the previous capacity studies and user-instruction PMU brackets from
  population scaling. Archived-copy read-only hooks collect living lengths,
  populations, food/prey counts and per-tick wall durations. Never change
  admission, simulation, policy, decoder, packet timing or observations.
- Warm in 5,000 completed-tick blocks. After at least 30,000 ticks, require
  endpoint mean living length and p90 across three consecutive blocks each
  max/min <= 1.05. Report all blocks, including food drift. Ceiling 180,000
  ticks or 90 wall minutes; failure is unsettled, not a steady-state claim.
- After length screening, hold 300 wall seconds three times continuously,
  preserving each window. No compilation during warmup/holds. Record full
  process command lines at every hold start/end, plus CPU deltas for other
  processes. A busy competing worker invalidates timing and requires repetition;
  retain contaminated attempts. Ordinary service CPU remains attributed.
  For an explicit timing screen, any other surviving process consuming more
  than 3 CPU seconds across a 300-second hold (over 1% of one CPU) is busy;
  inspect full command lines and repeat that window. This threshold is fixed
  before primary holds. Retain aggregate guest interrupt/steal CPU context too;
  boundary process snapshots do not census short-lived processes completely.
- Record per-thread user/system CPU, affinity, ticks/s, deferred ticks, host
  simulation load, per-tick p50/p90/p95/p99/max wall duration and VmHWM.
  Tick work spans command/admission, world advance/broadcast and handover;
  sleep is excluded, scheduler preemption is included. Keep measurements and
  profiler runs separate. Sampling overhead and hook limitations stay explicit.
- Separately count current-thread user instructions for complete ticks and
  world phases (movement, collision construction/query, death food, pickup/cache,
  prey, ambient), spawn admission, broadcast, bot decode, frame and policy.
  PMU enabled/running must be >=99%; kernel instruction access is not assumed.
  Kernel CPU seconds are measured independently through /proc.
- Census/profile hooks are compiled into a research-only CLI copy. During
  warmup and timed holds, PMU phase brackets are inactive; after the holds a
  marker enables a separate 60-second instruction-count interval in that same
  mature world. No compile or world reset intervenes. Clock/census hooks and
  inactive bracket checks remain in descriptive elapsed-time measurements;
  report this overhead boundary rather than calling the binary uninstrumented.
- Within that separate profile, an external read-only PMU sampler also counts
  every server thread for 45 seconds. This supplies inclusive user-instruction
  totals, including socket and worker-loop work outside the phase brackets.
  These counters run on another CPU, require >=99% PMU scheduling and retain
  their exact independent start/end times. Do not combine differently edged
  totals as though they were an exact additive decomposition.
- Rank at most three byte-preserving optimization opportunities by current
  measured phase budgets, trace source owners, distinguish upper bounds from
  demonstrated savings. Do not promise that unimplemented candidates close a
  measured deficit. No runtime implementation in this PR.
- Keep bounded selected data, scripts, failed attempts, pins and report in Git;
  raw process lists/logs and binaries local-only. Public files must contain no
  private host identity, provider, kernel suffix, IP, username or home path.
  Run documentation checks and scan the diff before pushing. Open a research PR;
  do not merge.

## Owner-approved warmup amendment, before any timed hold

The owner authorized up to three hours of warmup per case and allowed more
vCPUs for warmup, provided every host thread is re-pinned before measurement.
The mean-length and p90 rule above remains unchanged: maximum/minimum <=1.05
over the last three 5,000-tick endpoint blocks, after at least 30,000 ticks.
No hold results had been viewed or collected when this amendment was written.

After an idempotent all-thread CPU-0 pin/readback test on the live host, warmup
may use CPUs 0, 2 and 3, leaving CPU 1 for the client/collector. Record both
pin operations and full process command lines. `taskset -a -p -c 0 PID`
re-pins all host threads before holds; verify every task's affinity and keep
checking throughout measurement. Discard 60 wall seconds after that pin for
queues/caches before the first 300-second hold. Actual hold length trajectories
remain visible; a multi-core warmup screen is not proof that overloaded
single-core control produces a stationary ecology indefinitely.

The amended ceiling is three elapsed warmup hours per case (including time
already spent warming the current host), replacing the original time/tick cap. A case failing the
screen at the ceiling still runs all holds, explicitly labelled unsettled.
No game state, configured rates, policy, wire path or runtime source changes.

An optional 15-second user-instruction IP sampler runs only after all holds,
within the existing diagnostic phase window. A warmup-only pilot checks PMU
access and ELF virtual-address translation. Preserve lost/unattributed weights;
do not symbolize kernel-context locations or infer callers of stripped shared
libraries. Inline compiler frames are not dynamic call stacks. This supplements
the exact phase brackets and changes no primary hold or settledness rule.

## Owner-requested accounting splits, collection unchanged

Before any holds, the owner requested both (a) all server work including bot
workers, and (b) simulation plus socket/I/O threads from those same holds.
For (b), simulation includes per-recipient encoding for every configured bot
and the socket player; socket CPU covers only the one real connection.
It does not measure 2,281 real sockets.

Report observed CPU shares and CPU milliseconds per executed tick. Interpolate
cumulative per-thread CPU counters from the approximately one-second telemetry
to the exact first/last retained census boundaries; retain the bracketing times
and interpolation weights. Required share of a 7 ms one-core budget is
`100 * CPU-ms-per-tick / 7`; implied solo throughput is
`1000 / CPU-ms-per-tick`. This is a same-work CPU-budget projection, not a new
run with bots moved to other cores or a qualification of 2,281 sockets.

Rank byte-preserving optimization candidates separately for the two splits,
using measured phase budgets and leaving unattributed work explicit. For (a),
separate decode/materialization from policy; do not assign worker-loop residual
to either. A retained socket-host per-connection cost may supply a clearly
labelled historical extrapolation, with its revision, platform/workload and
scaling limits; do not call it a measurement on this guest.

## Pristine CLI sanity check

Before inspecting primary hold results, add one supplementary 2,280-bot plus
one-player cold-start check after both primary cases. Use the pristine CLI and
client already built from the same archive, CPU 0 for every server thread and
CPU 1 for the client. Warm 30 wall seconds and retain 300 seconds of the CLI's
own statistics, process/thread CPU and full boundary process lists. No census
or PMU hooks exist in this pristine binary. This checks whether the basic
throughput shortfall also appears without research hooks. It is not a mature
capacity qualification or a paired estimate of hook overhead: live trajectories
and lengths differ and pristine lengths are not instrumented. Primary collection,
screening and holds remain unchanged.
