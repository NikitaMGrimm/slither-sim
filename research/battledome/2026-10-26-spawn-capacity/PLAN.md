# Spawn capacity comparison (frozen before collection)

Origin: local-simulator. Base: main 901b02e2e71144c6ffd35291ddb6603b2c6a1ca7.
All traffic goes to a loopback Host. No outside game servers.

Compare control (fixed 3584, original strict admission), A (fixed arena,
strict first, fallback tiers at 2/3/4 world seconds), B (same tiers plus
3584*sqrt(max(occupied+queued,32)/32) growth, proportional pit and band).
Every search has at most 4096 candidates; one search per host tick, existing
14 ms failed-search backoff. Tier widths/horizons are in the source snapshot.
All tiers retain contact+40 and the 3-second normal head runway. Strict always
wins over a retained relaxed candidate. Relaxed candidates maximize body margin.

Exploratory screening: seed 7, 80 seats, 20 wall seconds per arm. Retain all runs.
Main comparison: seats 40/64/80, seeds 7/29/47/71/101, 60 wall seconds each,
sequential, rotate arm order by seed and seat. Four actual socket probes plus
seats-4 background A4 bots, one bot worker, no added latency, default ecology.
Each probe stays alive for 5 seconds, uses A4 at 42 ms and pings at 250 ms,
then disconnects for 100 ms and rejoins. Dead probes rejoin too. Full pending
requests remain censored; no timeout successes or silent retries are imputed.

CARGO_BUILD_JOBS=2, RAYON_NUM_THREADS=2. Long-run affinity 0x30. Shared Windows
Ryzen 7 9800X3D, 16 logical CPUs; initial total utilization about 16%.
Admission CPU: Windows GetThreadTimes sum around searches divided by host ticks;
100 ns units with coarse scheduler accounting, includes conservative cached
queries and candidate generation, excludes telemetry emitted after the sample.
Also retain elapsed search p95/max and total host busy time. No isolated speedup.

Report joined and respawn wait p50/p95/max and pending count separately; socket
waits use wall request-to-first-own-appearance. Queue waits use world ticks.
Count all births, strict 600-unit/2-second head-path intrusion per birth,
complete 3-world-second followup lives and early deaths by cause; censor births
less than 3 s before shutdown unless they die first. Record live count at 1 s,
late 30-60 s population, admission/death counts, host ticks/deferred and errors.

Selection: first require zero observed early deaths and zero strict intrusions
across measured seeds, prefer full late population, then shorter tail waits,
then CPU. Do not call a failed gate inconclusive capacity success. If no arm
passes, retain failures and adjust only in a separately labelled exploratory
round. These bot/probe runs do not prove 80 simultaneous humans, one-core
network delivery, collision immunity, or supremacy over a community server.

Feel checks: 16 and 32 target seats keep exactly the base wall, pit and band,
same food density, floor, scoring, pit entry/exit and physics. Analytic normal
spawn-to-pit radial travel and area per seat are unchanged there. Check the
actual live pit raster cells at multiple grown radii and decorations at the
live edge. Radius changes never move snakes themselves.

# Amendment before the main comparison

R1 sqrt area growth failed (one early pit death and 45 strict intrusions at 80).
R2 linear population radius recovered 80 late seats but had 10 strict intrusions.
R3 keeps strict admission and scales wall radius by
  1 + 1.5 * (max(seats,32)/32 - 1)
relative to the configured circular base. This extra perimeter/band area is a
spawn-packing allowance, not constant-density growth. At 16-32 nothing changes;
at 40/64/80 target wall radii are 4928/8960/11648. A 20-second 80-seat seed-7
screen had 0 early deaths, 0 strict intrusions, bot waits .294/.525/.567 s.

The main comparison now uses that R3 strict-growth arm, the original strict
fixed-arena arm, and R1 fixed-arena relaxation, with unchanged seeds, seat counts,
60-second duration and five runs per cell. Both earlier growth arms remain
rejected exploratory attempts. R1 control bot waits are invalid because its
telemetry zeroed queue age when disabling tiers; all R1 files are retained.
R2 onward preserves the real queue age separately. The main binary adds request
and cancellation events to count pending/censored waits. No failed trial is
replaced. Seeds 29/47/71/101 were not used in screening; seed 7 was.

The growth arm uses only strict admission: geometry, rather than predictive
safety, changes at high populations. The fixed arm retains all strict-first
2/3/4-second tiers. Admission CPU includes its margin ranking and strict scan.
The control is the current-main strict policy with fixed geometry and the same
instrumentation. Its minimap receives the added pit edge; A4 does not read that
map, and the spawn RNG and exact strict geometry are unchanged. No bit-identical
whole-host baseline performance claim will be made.

The comparison logger repeated retained death history on every tick. It did
not invent physics deaths, but inflated telemetry cost and required deduplication
by (victim,time). Three completed 40-seat seed-7 runs remain exploratory; the
in-progress 64-seat fixed run was aborted to correct logging before main
collection. Its partial raw events remain. No completed run is overwritten.

# Final comparison amendment, before collection

The preliminary strict-growth 40-seat run admitted all seats but its last
background startup join waited 10.535 s. Add a host-only space allowance above
32 seats: queue age >=2 s grows the wall to 1.5 times the seat target; >=4 s to
2 times it. Pit and band scale too. Keep every strict check, the 4096 search
budget, socket-first queue, 14 ms failure backoff, gradual shrink and 3% dead
zone. The allowance lives in the existing live radius, and request age stays
in the host queue. No core checkpoint field is added. Request age never lowers
contact, head-path or departure clearance. Protocol validation covers the
largest 2x allowance for the configured seats.

Only circular worlds with minScore use this policy; generic circular official
worlds retain population sizing and fixed pit/spawn settings. This resolves the
existing generic-ring regressions without changing the measured Battledome arms.
The simulator keeps death history; corrected telemetry emits only new entries.
Offline analysis also deduplicates (victim,time), requiring nonnegative age.
The earlier apparent R1 early pit death was a telemetry counting error; the
corrected R1 pilot has zero early deaths and still fails on strict intrusions.
All completed preliminary runs and the interrupted partial run are retained.

Main comparison identifiers now use confirm-*; unchanged seats 40/64/80, seeds
7/29/47/71/101, 60 wall seconds, rotated arm order, five sequential runs/cell.
The four non-screen seeds remain unused by policy selection. All other methods
and limits from PLAN.md and AMENDMENT.md apply. No capacity/latency gate is
waived; report nonzero early deaths, strict intrusions and pending requests.

# Pit-growth correction, before the corrected comparison

The complete first 45-run comparison is retained in initial-comparison-results.json.
Its strict-growth 64-seat seed-101 arm had three real early pit deaths (ages
1.764, 0.357 and 2.604 s), so that growth candidate is REJECTED by the frozen
safety gate. A controlled synthetic repro with no opponents reproduced a pit
death at 0.651 s when an expanding edge overtook an outside newborn.

Fix: in dynamic circular floor worlds, only a physical inward crossing counts
as new pit entry. Growth over an unentered snake does not enter it; it can leave
that engulfed area. Lives initialized inside start entered, and an actual later
inward crossing commits the snake to the unchanged one-way exit rule. Already
entered lives retain that state. Existing pitEntered plus live radius carries
this exactly through checkpoints; no saved field, immunity or timer is added.
Fixed-pit worlds retain their original entry test and initial flags. The new
regression checks engulfment, exact continuation, later re-entry and lethal
exit; all 39 Battledome tests pass. The earlier failure is not reclassified as
a logger error; those three deaths are real failed safety outcomes.

Corrected comparison: seeds 7/29/47/71/101; 40/64/80 seats; 60 wall seconds;
strict fixed control versus corrected strict growth, alternating pair order
by seed and seat, 30 sequential runs. Run IDs are corrected-*. The original
15 fixed-relaxation runs remain the rejected A comparison: their pit is fixed,
so the correction does not change that physics path. Initial control/growth
runs remain retained separately and are not pooled with corrected results.
These seeds have now all been examined; this is corrective exploration, not
an independent held-out confirmation. No safety failure is waived.

Same host/probe/bot protocol, affinity 0x30, CARGO_BUILD_JOBS=2 and
RAYON_NUM_THREADS=2. Builds/checks use separate affinity 0xC0. Add birth head,
wall/pit radius and death head geometry to local telemetry, identically in the
corrected control and growth arms. No algorithm/seed changes to fix a result.
Select only with zero early deaths and zero strict head-path intrusions;
report pending requests, all causes, all errors and CPU/elapsed per tick.
The known reader/host shutdown race stays in the unchanged probe harness;
error timing is not logged and no reset will be silently excluded.

Owner-provided community-spawn clarification is inspiration only: a two-second
spot reuse policy, 500-unit clearance with crowded fallbacks, between-spot
placement and modest population growth. We did not contact that server or
measure its real players, death rate or hardware. Strict-first A was measured;
B retains strict safety and explicitly trades density above 32 for space.

# Affinity correction, before the qualified comparison

The requested 0x30 mask was not actually applied by the original Python FFI:
GetCurrentProcess used ctypes' default integer return type, and the affinity
function lacked HANDLE/SIZE_T argument declarations. The unchecked call failed.
A live PowerShell check observed 0xFFFF on the corrected runner and its host.
The first 45-run round and all corrected-* runs are therefore UNQUALIFIED for
the requested resource limit. Their raw files, terminal results and interrupted
attempts are retained; no numeric CPU or wait result from them enters the final
qualified comparison. Their causal safety counterexamples remain useful.
This is an agent harness error, not a simulator failure or a waived limit.

Use explicit Win32 ctypes types, assert every call, verify the parent mask,
then verify GetProcessAffinityMask on EVERY child process before waiting for
its result. Fail and terminate the child if its mask is not exactly 0x30.
Also independently spot-check active host masks with PowerShell.

Run 45 new sequential 60-second holds, IDs qualified-*, all three policies
control/fixed/growth, seats40/64/80, seeds7/29/47/71/101, rotated arm order by
seed and seat. The copied comparison-corrected.exe is unchanged. No policy,
geometry, bot, probe or safety gate changes. Every policy/seat gets five runs
under verified two-logical-CPU affinity. The completed Rust checks already use
PowerShell's checked process affinity0xC0 and need no rebuild for this wrapper fix.
All old cohorts are retained separately; do not pool them with qualified data.
Report all errors, censored requests, early-death causes, intrusions, phase CPU,
search elapsed/tick, actual population and deferrals. Zero CPU counter values
remain below resolution, not zero cost. Selection still requires zero observed
early deaths and strict intrusions, then favors population/waits and cost.

# Final moving-edge correction and paired confirmation

The first affinity-verified round is retained in qualified-first-round-results.json.
Growth/40/seed7 recorded one real pit death at age2.534 s, so that candidate
fails safety. A controlled repro shows an unentered life overtaken by growth
can be falsely armed again while the pit slowly shrinks: the old test compared
its pre-step head to the NEW smaller edge. A horizontal grazing chord reproduced
a pit death at world time0.014. Keep both failures; do not waive them.

Fix: capture the pre-update pit radius. For an unentered life in a moving
circular floor world, physical entry requires its starting head to be outside
BOTH old and new edges; then keep the ordinary swept test against the live edge.
Growth passing over it and shrink passing inside it cannot count as entering.
Already entered lives and actual later entry/exit remain one-way. All40
Battledome regressions pass, including failing-before/passing-after grow and
shrink cases and exact continuation. No timer, immunity or new saved state.

Stop the failed growth round. Relaxed A is also stopped for strict head-path
failures; retain ALL SIX complete affinity-verified A holds (two seeds7/29 per
seat40/64/80) as the rejected comparison. Do not repeat an unsafe policy to
increase its sample count. Its fixed-pit physics/admission path is unchanged
by this moving-edge fix. Its cost observations are descriptive, not a five-run
performance verdict. Other old growth/control rows stay in the failed record.

Final confirmation: 30 NEW sequential60-wall-second holds, strict control and
final strict growth, seats40/64/80, seeds7/29/47/71/101. Alternate pair order by
seed/seat. IDs final-*. Verified mask0x30 in parent and EVERY child, plus external
PowerShell spot checks. Same copied production-profile instrumented binary,
probe/bot controls and parameters; no further policy tuning. Keep every outcome.
The final table contains five holds/cell for control and growth, two/cell for
rejected A, with counts explicit. Selection still requires zero observed early
deaths/strict intrusions in the growth arm; retain pending/censored/errors.
All seeds have been seen; this is adaptive corrective exploration, no independent
held-out claim. Source/checks, numeric precision and qualification limits remain.
# Recent-birth growth hold and fresh comparison

The final-* round is stopped and retained in before-growth-hold-results.json.
Growth/40/seed7 had a real pit death at age 2.891 s. The physical-entry fixes
remain necessary but do not prevent a later actual crossing near a newborn
when the pit has recently expanded. This safety failure is not waived.

Before increasing the wall for queued requests, the host now waits until the
latest successful birth has its full three-second departure window. While
expansion is required but held, it pauses admissions too, so repeated births
cannot postpone growth forever. No search is performed during this hold.
This is transient host state; the core checkpoint has no new field. Startup
growth before births is allowed. The ordinary seat controller remains intact.

The real host regression queued_expansion_leaves_recent_births_their_departure_window
and Raw growth atomicity test pass, along with the focused host/Battledome/
checkpoint/seat-radius/world-rules checks. The change is frozen for another
30 sequential 60-wall-second holds: strict control and strict growth, seeds
7/29/47/71/101 and seats40/64/80, alternating pair order. IDs protected-* use
a newly built and copied instrumented binary from current production sources;
old instrumented files must not replace the host hold. Verify parent and every
child affinity0x30, plus independent PowerShell spot checks. Builds/checks0xC0.

Keep all completed results, errors, pending requests and failed attempts.
Select only if fresh growth has zero observed deaths within3s and zero strict
head-path intrusions. The six qualified fixed-relaxation holds remain rejected
and are not rerun. Final comparison36 holds: five/cell for each strict policy,
two/cell for unsafe relaxation. All seeds are seen; this remains adaptive
corrective exploration without independent held-out or human-play validation.

# Delivery integration and source-review correction

Final delivery fast-forwards main from249cdb2 to150d4b1. Only the changelog
conflicted; preserve main release0.6.1 and put this change under Unreleased.
Recovery stash06e7461078d36e19001b3b0520abddcfd9f9eed0 is retained. Main's
known-slot enumeration change168 is left intact; no admission/physics owner
changed between these main revisions.

Independent source review found redundant decorative removal/addition on
sub-cell pit shrinkage. sector_changes now compares decorations by wire key
and style; physical food still compares exact pickup identities. A real
one-tick shrink regression failed before and passes after, with actual cell
movement as positive control; a same-cell physical replacement still sends
remove/add. Four food tests and ten pit tests pass. Final fullRust reruns on0xC0.

The protected-* binary stays frozen on249cdb2 plus the final growth-hold
policy and measurement instrumentation. Its CPU timing remains search-only,
not delivery broadcast performance or a final-build all-socket grade. Main's
known-slot optimization and decorative packet suppression are not timed in
that binary. Production and measurement hashes remain separate. Their
scheduling-dependent socket worlds are not claimed bit-identical on the
later delivery. Selection safety remains based on all30 declared protected
holds, with all prior failures retained. No metrics are retroactively tuned.

# Whole-admission timer and integrated delivery confirmation

The protected-* cohort times candidate searches only. Retain every completed
hold and its failures in pre-delivery-results.json; do not call this full
admission CPU. Its transient host hold remains unchanged. Final delivery
integrated main150d4b1 and suppressed redundant sub-cell decoration packets.

Freeze a new instrumented production-profile binary on the integrated source.
Measure both the entire Simulation::admit_waiting call (including queue scans,
growth/holds, successful connects and nested telemetry) and spawn_pose searches.
Accumulate full-call CPU/elapsed counters without I/O in the admission guard;
emit counters after each actual world tick, outside the measured admission
phase. Keep GetThreadTimes coarse-resolution caveats and elapsed ceilings.
No per-waiter searches, production timer or policy change is added.

Run30 new sequential60-second paired control/growth holds, IDs delivery-*,
seeds7/29/47/71/101 x40/64/80, alternating pair order; parent and every child
must be verified0x30. Build/check0xC0, two jobs/test/Rayon threads. Keep all
outcomes/errors/censoring. The selected new growth must have zero deaths within
3s and zero strict path intrusions. The six rejected fixed-relaxation runs
remain valid descriptive search-only results; full admission was not collected
for that rejected arm, and its CPU sample is not used to rank policies. No
independent held-out or one-core real-human grade is claimed.

# Final PR rebase after the measured build

PR176 initially conflicted with new main174 because both added an Unreleased
changelog section. Rebase onto3a23dc9 preserves both entries. Runtime source
merged automatically: main adds shared prey-visibility bounds, while our only
broadcast change remains the live-radius minimap call. Default Battledome prey
count is zero. No admission or policy tuning is done after measurements.

Keep numerical tables pinned to final measured main150d4b1. Retain original
measured production, instrumented sources and current production hashes as
separate identities. Rerun completeRust group and docs on rebased delivery.
The PR head and report permalink will update after verification; no PR merge.
