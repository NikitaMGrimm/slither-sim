# Population scaling protocol, frozen before selected runs

Synthetic local simulator; no external game connections and no runtime changes.
Pin main ef866d7 and Rust 1.99.0, default portable release, thin LTO, one codegen
unit, debuginfo=1. Initial preparation used WSL2 CPUs 6,7. After the maintaining
agent's
stop instruction for the owner's Windows workstation, all new computation moves
to a 4-vCPU Zen 5 EPYC KVM guest, taskset CPUs 2,3; each measured process uses
one CPU, Cargo jobs=2, Rayon=2. Keep perf_event_paranoid unchanged (2). No more builds, checks or
study workloads run on the owner's Windows workstation.

Measure N=500/1000/2000/5000/10000, seed 7. Identical rows below the cap run once.
The cap is the actual host's encode::MAX_RADIUS=32000, including its safety
margin inside the approximately 32767 mathematical u16 limit. At larger N,
scale initial official ecology by area, disable startup enlargement, keep
dynamic_radius enabled, and use the public recorded-radius API to apply the cap
after each seat-following update. No snake gets a different simulation rule.
The uncapped control uses Config::official(N) unchanged.

Admit all slots through an exact copy extracted from the host's spawn_pose_waited
function (waited=0); no clients or observations in physics runs. Public step and
respawn_body calls only. Retry refusals after two physics ticks (14 ms), stable
slot order; no socket queue or client death-notification delay. Count refusals,
attempts and successful admissions separately. Fresh lives start with host bodies.

Per-slot xorshift wander, heading changed every six ticks by at most 0.18 radians;
hold between decisions. Request boost one staggered 300-tick interval out of
eight (normal mass limits apply).
No targeting, safety, collision avoidance or distant-snake shortcuts. The boost
schedule provides ongoing mass turnover; report the resulting body distribution.
All living snakes receive an Action and are stepped every 7 ms tick.

Warm in 10000-tick blocks, minimum 3, maximum 24. Check the last three blocks'
mean segment count and p90 for max/min <=1.05; otherwise continue. This is an
operational convergence screen, not a stationarity proof. Retain all blocks and
report any failure at the maximum as unsettled, with no steady-state claim.
Save warmed world and driver continuation. Five 1000-tick repeated windows from
that continuation give median/range user instructions per tick; a preceding
1000-tick run is untallied. These repeats measure counter noise, not independent
world uncertainty. Retain every attempted run, including failures.

Pre-collection amendment: the v2 500-slot exploratory pilot failed convergence
at the original maximum of 12 blocks (840 simulated seconds), with a still
rising length distribution. Extended the ceiling to 24 blocks (1680 seconds)
before selected runs; retained the complete failed pilot. Controller unchanged.

Count current-thread user instructions around step and separately around respawn;
controller/statistics/serialization/setup excluded. Check enabled/running >=99%.
Cold restoration caches settle in the untallied prefix of each window (100 ticks).
Diagnostic overlay uses coarse step brackets and sampled per-head collision
query/candidate-test brackets. Separate diagnostic costs include hook overhead;
require identical terminal checkpoint and driver bytes versus the plain harness.
No changed production source files are committed.

Preparation amendment after the initial selected-harness attempt: uncounted
warmup uses conservative body boxes to avoid geometry calls that cannot affect
host clearance or its accumulated minimum margin. Exact tests, candidate order,
RNG and every physics tick remain unchanged. Counted admission always uses the
original search. Require byte-identical 500-slot warmup continuation against
the original long pilot, plus a same-seed warmup cross-check against original
search. Retain the original-search attempt and its completed 500 counts; new
selected runs use fresh output. This changes preparation cost, not the measured
algorithm or the controller/selection protocol.

Host-bot sample: actual SimulatorBackend, official 500, zero modeled link delay,
official delivery, 499 background Foragers plus one controlled scripted seat.
Warm 1000 decisions; measure 200 decisions, six ticks each. Five repeated seeded
processes plus one untallied run. Instrument actual broadcast/encoding,
receive_frame decoding, frame_for materialization and shared ForagerBot::decide.
Broadcast is a world-wide shared cost, not a separable per-bot marginal cost.
Report live decisions, dead/refused reconnects, visible entities and world steps.
No core capacity assertion follows from WSL2 elapsed time. Rank optimization
budgets as hypotheses, bounded by measured phase shares; native 7 ms acceptance
requires future measurements on the intended CPU with host/network reserve.

Censoring amendment from the maintaining agent: the uncapped 10,000 control
has a total
one CPU-hour ceiling and must be reported as unsettled. The terminated WSL
warmup consumed 3343.74 process CPU seconds without flushing a 10,000-tick
block; its reached tick count is unknown. Charge 60 additional seconds for
earlier diagnostic overhead, leaving at most 196.26 seconds. The isolated
diagnostic collection is limited to 180 aggregate child CPU seconds (including
setup, all cold onset probes and output); compilation is infrastructure outside
this simulation budget. No fresh full warmup for that control is allowed.

Use a separately pinned diagnostic to flush every reached tick's user
instructions, coarse phases, population/lengths and actual eater-grid bounds.
Cold probes bracket the grid-path onset; their placements differ from the main
host-admission trajectory, so they cannot replace the censored steady row.
Report the dense-grid rejection predicates and selected fallback explicitly.
The other six primary cases keep the original warmup/count protocol. The final
selected measurements and warmup-equivalence oracles are regenerated on the VM.

Owner resource amendment, 2026-10-28: the task's share drops to one logical
CPU, one active measurement process, Cargo jobs=1 and Rayon=1. Both in-flight
large warmups were stopped immediately and pinned to CPU 2. Their single-thread
world states total about 650 MB resident memory; resume one at a time without
changing simulation state, finish both warmups, then release them before the
checkpoint-loading count/profile children. Existing warmups never use Rayon;
their already-launched inactive Rayon environment value cannot be changed in
place. All new commands use CPU 2 and the new environment. Completed earlier
measurements and their original resource metadata remain retained. New
reproduction is fully sequential. Delete owned compiler targets after checking
the immutable copied executables; retain unique evidence and deduplicate only
byte-identical endpoint copies.

Memory-bound preparation amendment: the original checkpoint import retains a
generic JSON tree alongside decoded state and exceeded the new memory guard
on the mature 5k checkpoint. Keep that failure and the stopped RSS probe.
Append a study-only importer to archived source copies: validate record shapes
one at a time, then retain the original typed/configuration/state validation.
The production importer remains untouched. Every restore must reproduce the
complete canonical input checkpoint bytes before any counted tick. Compare
the 500 continuation against the original public importer, and require both
builds to pass a valid round trip and eight malformed-shape rejection controls.
Rebuild one consistent primary cohort and rerun its 500 counts; reuse only
hash-verified complete warmups, preserving their original preparation pins.
The completed bot and censored-control components keep their separate original
binary identities. No counted physics, admission, controller or selection rule
changes in this amendment.

Radius-event preparation correction: exact-multiple timestamps become later
than the accumulated simulation clock plus replay tolerance at tick 98,477.
Place each cap event halfway before its intended tick, preserving the initial
time-zero event. Validate the emitted plan through the full 241,100-tick
warmup/cache/count extent. Retain the interrupted untallied capped-10k count
and rebuild both primary arms. Reuse its completed 70,000-tick warmup only
after checking identical event-consumption indices on every tick through
71,100; replace only its canonical radius-update array and stream every other
checkpoint byte unchanged. Uncapped/500 fixtures need no change. Rerun all
primary counts with the new consistent pair; keep bot and censored-control
binary identities. This corrects a research arena schedule, not runtime code.

Final resource-method amendment: retain a full original-search reference on
the identical 100-cache/1000-count trajectory at every N. Existing five-repeat
500 and uncapped-5k references are reused; the capped-10k former untallied run
becomes its one original reference. This choice was recorded before inspecting
that run's instruction totals. Each remaining case gets one original reference.
Five physics repeats plus their untallied/profile runs use the exact admission
preparer outside the world-step PMU bracket. Every snake executes every normal
physics step. Require complete world/driver endpoint equality and identical
per-tick lifecycle/population census against every original reference. Report
baseline admission/combined costs only from original references, with n=1 or
n=5 explicit; never report private preparation cost as original server cost.
This avoids spending hours replaying the same host-admission work on the
one-CPU share. Original/reference binary pins remain distinct; all repeated
physics rows use one newly built pair. Keep all earlier and interrupted runs.
