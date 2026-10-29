# Frozen live recheck after #272/#273

Main before #272, `3a223676`, versus `a1535902`, with #265/#266 in both.
Research only. Reuse #267's actual protocol-19 received-frame A4 clients,
constant-boost override, host instrumentation, delivery metrics and canonical
selection by reference. Every frame is decoded, A4 steering is unchanged and
the received rule gates the study-only boost output. Shipped Battledome arena/
delivery defaults; no authoritative bot observations or delivery override.

Build both hosts remotely with the default portable release profile, Rust
1.99, locked dependencies and no native CPU flags. Use the candidate client
for both arms, verifying identical shared policy and client source. Run the
boost heading/rule-gate regression for both builds. No production edits.

One c7a.medium target core, one same-AZ c8a generator, Frankfurt Spot first.
Retain #267's six workers on CPUs 0–5 and controller CPU 7, with enough physical
cores; do not relax its input-lateness/headroom/decoding/queue gates. Fresh
world per row, seed 7, 641 slots and zero in-process bots. Warm 300 seconds at
80, settle 60 and measure 90 seconds at each natural ladder stage through the
target (80/160/320/480, plus 640 if enabled). Earlier stages are calibration.

Primary order: base-320, candidate-320, candidate-480, base-480. One window
per arm/count. Add base-640/candidate-640 only if candidate-480 has zero
deferrals and both rows plus retrieval fit the remaining planned time/cap.
Skip 160 primary unless 320 reveals a validity anomaly. Overload alone does
not invalidate a row; invalid generator/queue/boost qualification does.

Report aligned simulation+socket CPU p50/p99/max, measured and full-run
deferrals, frame jitter/excess/stalls, completed respawn waits, censored pending
age/count, alive population and actual boost share/births. Attribute each row's
slowest aligned-CPU 1% exclusively to spawn, sockets+broadcast (excluding nested
minimap), minimap and other. Identify the leading phase at 320 and 480 plainly.
These are descriptive single-window comparisons of independently evolving
worlds, not an isolated speed ratio or capacity claim beyond measured rows.

Use only the limited profile. Creation tags on instance, volume, SG and key;
terminate shutdown behavior, DeleteOnTermination and planned-duration+30-minute
in-guest poweroff timers. Initial 140-minute timers support 110 planned minutes.
Combined cap $3 including failures, replacement and noncompute. A single
On-Demand generator is preapproved only after repeated Spot reclamation and
only within the remaining cap. Never extend a timer or budget to hide failure.
No local builds, runs or reduction; no SSH guest, owner play hold or endpoint.

Archive only after closure/read/hash verification. Retain raw inputs and failed
attempts; bind each selected row to its own archive and complete retrieval
receipt. Perform reduction on AWS before teardown. Retrieve/verify required
sources before deleting them, then terminate task instances, delete task key/
SG and scan every enabled region for instances, Spot requests, volumes, EIPs
and task resources. Preserve protected/unrelated resources. Publish lean
manifest/archive pins and selected results with a template-compliant study PR;
do not merge. Keep all cloud/host identifiers and addresses private.
