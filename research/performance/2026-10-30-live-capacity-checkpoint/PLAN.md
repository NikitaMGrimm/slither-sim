# Live mature-500 checkpoint after compact histories

Freeze before any host/client timing. Research only: no runtime optimization,
mechanics change, outside server or production source edit. The maintaining
agent asks whether official-500 p99 now fits the one-core seven-ms budget and
what leads the current slow tail. Do not infer a same-trajectory causal speedup
from independently evolving live worlds.

## Arms, client and host setup

A is 551cb1030ea2bf84f83bbeb167233b917900929a, #256's post-#255 main.
B is current main at this task's start,
b750004aeb8f11be61dc483a2ce458c6194f116f. It includes compact occupied snake
histories; #277's rejected retained-sector layout is absent. The pending #277
research-only merge does not alter the pinned runtime.

Reuse #256's archived-copy hooks, full received-frame client and #248 helpers
by reference. Build one common client from d127acd1f0e9db2100c8a1dd5c2fa43e2deff506
with the unchanged #256 client.rs; use the exact same copied executable for
both hosts. No authoritative host observations drive clients. All 500 seats
use real loopback TCP WebSockets, protocol 19, physical-food/rules opt-in,
received-frame A4, ordinary 42-ms controls and normal pings/reconnects. Zero
in-process bots. No thin-client substitution.

Keep #256's native-target release flags, thin LTO, one codegen unit and line
tables on both arms for this same-core checkpoint. This is a measurement build,
not a proposed portable-runtime policy. Simulation and socket tasks share
vCPU 0; the three client workers use vCPUs 1/2/3; CLI/low-rate collectors use
vCPU 3. Read back affinity at startup and throughout every run. All package,
build, test, timing and reduction work runs on the SSH guest, with existing
locked/offline tools only. Use one bounded /tmp/slither-live-checkpoint-target,
verify copied binaries and remove it before any live timing. No installs,
system/security/limit changes, provider CLI, cloud or local-play-server work.

## Calibrated isolation

CALIBRATION-PLAN.md was committed as b682c392 before observations. All twelve
fixed idle windows qualify. Largest-other CPU ranges 0.07–0.09 seconds.
R_idle is 0.002998926999628939 CPU seconds per wall second; USER_HZ is 100.
Freeze the threshold for selected duration W as:

W * (0.002998926999628939 + 0.01) + 0.02 CPU seconds.

The 30-second example is 0.4099678099888682 seconds. Preserve the raw observed
largest/summed other-user CPU and each allowance component. No subtraction from
host CPU, process-name allowlist or post-result threshold change. Counter
quantization and sampled process lifetimes remain limitations. IRQ/softirq and
steal are separately reported, not added to task CPU as disjoint work.

## Cohorts, maturity and validity

The retained #256 result has three fresh cohorts per arm, so use exactly three
fresh pairs in AB/BA/AB order: A,B,B,A,A,B. No discarded mature pair. Before
primary collection, run an eight-client smoke and one 500-client short-world
pilot to verify this metrology and full-client headroom; neither is mature
capacity evidence. Freeze/check the binaries, synthetic negative controls and
source reconstruction before accepted primary holds.

Each fresh host warms from tick zero with the original seed-7 official world,
seven-ms physics, seat-following radius and no artificial delay. Keep 100
logical clients resident; retire/rejoin the other 400 round-robin, two per
100-tick census, stopping forced churn at tick 180,000. Do not restore/fork
multithreaded sockets or edit authoritative bodies, food, RNG or policy.

At or after tick 193,000 require three successive 500-tick endpoints with
at least 490 living, mean length 45–65, p90 156–196 and p99 at least 200.
Discard five seconds after screening, then collect one fixed 30-second primary
hold. Every sampled census in its complete 100-tick blocks must stay in class.
Keep all raw drift and deviations. This is a mature-class screen, not stationarity
or a long-duration uptime qualification. Each warmup is bounded at 45 minutes.

Preserve #256's received-frame client/transport gates: all workers remain alive,
no unexpected established-session/decode errors, worker CPU medians below 90%
and p99 below 98%, aggregate client CPU below 2.9 cores, sampled one-second input
lateness p99 below 42 ms, stable queues under the original 8-KiB/s slope and
512-KiB level allowances, no write WouldBlock/buffer-full events, and at least
495 players at socket samples. Keep the original admission-refusal accounting
and CPU reconciliation tolerance, max(0.03 ms/tick, 2%). Report exact observations.
No full-client failure is repaired by silently thinning the policy.

One class-only fresh retry per cohort is permitted, as in #256. Additionally,
pre-authorize at most one isolation-only fresh retry over the whole series,
using the same frozen gate after the rejected cohort ends. Keep all failed
attempts and their CPU charged. Any other quality failure stops collection.
Never retry because of CPU time, p99, deferrals, throughput or an unfavorable
comparison. Host backpressure/queue failures are retained negative capacity
evidence, not grounds to hunt for a favorable window.

Primary collection has the original six-CPU-hour allowance, including a
conservative 600 CPU seconds for builds/calibration/pilots/checks and all
failed/active host-plus-client work. Charge termination/controller tails
conservatively. Do not start a cohort if the remaining budget cannot support
the normal observed warmup cost. Retain incomplete runs if the ceiling stops
collection. The optional finer current-main attribution has a separate
two-CPU-hour allowance. No timing-result-driven cohort count expansion.

## CPU and tail measurement

Keep aligned simulation and socket CPU clocks at consecutive completed-tick
boundaries, including user and kernel CPU. Sum each tick's components before
world quantiles; component p99s cannot be added. Retain p50/p99/max, mean,
per-thread user/system CPU, nested broadcast/physics, throughput, deferrals,
client input cadence/frame counts/receipt ages and application/kernel queues.
The client's existing one-second receipt-age samples are not a per-frame
inter-arrival distribution. No new client control or decoding path.

Capacity windows keep optional syscall/fine clocks and per-tick phase exports
disabled. Existing broad phase clocks stay unchanged. Add only a disabled
coarse-export flag to the archived timing helper. After each primary hold,
collect a separate 20-second coarse phase window, then the original 15-second
syscall diagnostic. The coarse export copies existing nine phase-clock deltas
and actual maintenance flags per tick, adding no phase clock calls. Its extra
recording/serialization overhead is diagnostic-only. Keep its body/client
quality status separate; an invalid coarse window cannot replace a valid
primary window or supply a capacity claim.

Validate the source-derived primary food cadence against all coarse actual
event flags, requiring at least 2,000 validated ticks per arm. Classify primary
maintenance and other ticks without moving socket work between tick bins.
Rank the slowest ceil(1% * ticks) primary ticks by aligned world CPU, with
simulation/socket conditional means, actual/validated maintenance associations,
neighboring socket intervals and post-report markers. Rank coarse broadcast,
physics/admission and unbracketed simulation work separately; nested scopes
are not additive. Preserve observer CPU, allocated to the next tick where
applicable, without subtraction. Report post-report contamination explicitly.

If the primary series is complete and allowance remains, use one separately
built current-main diagnostic host and the same common client, re-warming from
tick zero with the same mature screen. Collect coarse then fine nested clocks
to rank recipient food, held-listing membership refresh/ownership, shared
subscription-union updates, subscription advancement, preparation and snake
encoding. Fine timers add overhead and do not qualify seven-ms capacity.
No extra optimized candidate, source-layout experiment or runtime change.

## Reporting and delivery

State plainly whether current-main primary world p99 fits seven ms in every
qualified cohort, with margins and ranges. Report all failed/unqualified
attempts, client/backpressure conditions, maturity drift and budgets. These
short asynchronous live cohorts are descriptive; no pooled fixed-work result,
exact paired speedup, confidence interval or sustained-uptime claim.

Describe the machine only as a 4-vCPU Zen 5 EPYC KVM guest. Loopback TCP may
charge receiver work to sender syscalls and overstate real-NIC socket costs.
Full process commands are retained privately at boundaries with the account
redacted before writing. Never publish host/provider/kernel/account/personal
path/IP details. Keep only cited values plus table recomputation and whole-source/
binary identities, no per-file source-hash maps. Link old scripts instead of
copying them. Archive and verify retained raw data before deleting only this
task's files/target, announce guest release, add an Added changelog entry and
research index row, and open one research PR with Change/Validation and one
accurate agent-assistance footer last. No top AI note. Do not merge.
