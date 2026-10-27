# Persistent readiness native screen

Keep only a small two-file Linux readiness change with unchanged service,
delivery, flush and wait timing. Per-open registration avoids fd-reuse aliasing;
compaction updates the event's current index. Keep poll as failure fallback.
Test against the old poll snapshot after writes, partial drains, reindexing,
hangup, descriptor replacement and registration failures.

Baseline origin/main at experiment freeze, candidate only the readiness change.
Freeze source and compiled binary hashes before holds. Use an unchanged copy of
the native run.sh under the authorized capacity directory and the same shared
lock. A private bench copy changes only its owned paths, port to 18934, shard
count to one and load affinity to CPU1. Host CPU0; no other heavy work during
holds. Same frozen generator and telemetry/validity gates for both arms.

After a short smoke per arm, three 120-second official-500 pairs after 15-second
warmup, order AB, BA, AB. No performance-based exclusions. Each pair must reduce
total host CPU milliseconds per executed tick, with valid traffic/affinity and
generator headroom. If not consistent, reject without more searching. Retain
all failed attempts and diagnose harness failures before any recovery. Do not
change security/system settings, install packages, use forbidden ports, merge,
or contact outside game servers. Record frame-excess and tick tails as controls,
not a capacity qualification. SSH only; no provisioning CLI.

## Held-PR follow-up, frozen before collection (2026-10-28)

Keep the original six holds and measured runtime refs unchanged. Add four pairs
BA, AB, BA, AB (pairs4–7), using the same official500 seed7, stock19 load,
15s warmup, 120s primary window, run.sh and shared lock, hostCPU0/loadCPU1.
Rebuild both pinned sources with identical additive probes; these are diagnostic
builds, reported separately from the original binary hashes. Keep every attempt;
no replacement, performance exclusion or extra pair based on outcomes.

Buffer per-second world snapshots (alive, body anchors, logical segments, mass
in segment units, food, radius, cumulative births/deaths), executed/deferred ticks
and exact scheduled backlog on the simulation thread. Sample process/thread CPU,
per-CPU busy/steal and all-process CPU/comm/start identity (no command lines).
Continue the same controls/host for30s beyond the primary window and retain
post-window telemetry separately; primary counters and frame records stay clipped
to120s. Record instrumentation hashes and validity gates. No runtime policy change.

Predeclared saturation episode: any newly deferred tick, or at least two adjacent
approximately1s samples with hostCPU>=95% of one core and active backlog>=4ticks
in both samples, increasing over the two-sample span. Report marginal episodes
and primary/post intervals separately; this definition supplements all raw tails.

Decision rule requested by the owner: if saturation recurs only in candidate
holds across all seven pairs, keep #235 held and investigate a mechanism. If
episodes occur in both arms, or the original episode does not recur in the four
new pairs, report environmental/world-driven evidence for the owner's landing
decision. Absence of recurrence does not establish a cause or latency improvement.
If validity/instrumentation fails, retain the attempt and stop for diagnosis;
do not replace it silently. Retained historical gaps remain unavailable.
