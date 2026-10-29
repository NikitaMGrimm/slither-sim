# Frozen live recheck of the obstacle index

Research only: base `aeca19a9` versus candidate `287d1dc4` (#278). Full hashes
are in `runtimes.json`. Both contain #272/#273/#274; candidate adds only #278.
No production edits. Reuse #275's real protocol-19 sockets, fully decoded
received-frame shared A4 steering and received-rule-gated constant-boost override.
Shipped Battledome arena/delivery defaults, seed 7, 641 socket slots and zero
in-process bots. No authoritative bot observations, thin clients or delivery override.

Both hosts build remotely with Rust 1.99, locked dependencies, default portable
release, debug=0, empty Rust flags and no encoded/native flags. Use the same
candidate client binary for both arms; require matching shared policy/client
source hashes and both boost-heading/rule-gate regression passes before counting.

One Frankfurt Spot c7a.medium target (one vCPU, all server tasks on CPU 0), plus
one same-AZ Spot c8a.4xlarge generator. Verify 16 physical/vCPUs and one thread
per core. Keep #275's six decoding workers on CPUs 0–5 and controller on CPU 7.
Generator CPU/headroom, 42-ms input-lateness, decode, queue and delivery gates
are unchanged; every calibration and primary must pass and boost duty must be
at least 99% of living ticks. Generator failure invalidates that row, not an
overloaded server. Preserve all failures and Spot interruptions separately.

Every row starts a fresh host and clients. Warm 300 wall seconds at 80, then
settle 60 and measure 90 seconds at each natural stage 80/160/320/480 through
the requested target. Earlier stages are calibration, never substitute primary
replicates. Retain transitions, full-run deferrals and independently grown ecology.

Frozen priority/order: base-320, candidate-320, candidate-480, base-480;
then candidate-320, base-320 if both repeats plus retrieval fit. The 320 series
is AB/BA when complete, with the two 480 rows between its pairs. Otherwise it
is a descriptive single-window comparison. Do not pool away order differences.
Finally run candidate-640 alone only if candidate-480 holds and that ladder
plus retrieval fits. Never run base-640 or a 160 primary this round.

"Holds" means valid generator/queue/boost gates and zero deferred ticks in
both the original marked 90-second primary window and its canonical interior
interval. A zero only after dropping interval edges is insufficient. This is
only an observation for that window, not a sustainable-capacity claim. Preserve
all qualified overloaded rows. Optional repeat-pair admission requires at least
30 planned minutes remaining; candidate-640 requires at least 21. Counting must
stop by 130 minutes from launch; reserve the final 30 minutes for safe export,
retrieval and cleanup. No timer extension to obtain another result.

Report aligned simulation+socket CPU p50/p99/max, measured and full-run deferrals,
frame-gap jitter/excess/stalls, respawn p50/p95/max, pending count/age, alive seats,
boost share/births, generator qualification and human count. Attribute each
canonical row's slowest aligned-CPU 1% exclusively to spawn, sockets+broadcast
(excluding nested minimap), minimap and other. Also report the requested
three-way partition with minimap included in other. If spawn no longer leads,
rank the next two measured phase leads, distinguish proposals from causes,
and implement neither. No extrapolation beyond measured rows.

Hard combined cap $3, including every failed/replacement attempt. Limited AWS
profile only. Spot only, creation tag project=slither-sim on instances, volumes,
key and security group; terminate-on-shutdown, DeleteOnTermination volumes and
verified in-guest 160-minute poweroff timers (130 planned plus 30 safety).
Initial bid caps $0.06/hour target and $0.50/hour generator; 162 launch-to-timer
minutes plus $0.65 noncompute reserve gives $2.162 for a full initial pair.
Only replace a reclaimed resource if all spent compute plus the replacement's
full timer and reserve remain below $3; otherwise stop and report missing rows.
No On-Demand exception is assumed. Never touch protected or unrelated resources.

No local builds, bot runs or heavy analysis; no shared SSH guest, owner play
hold or public endpoint. Reduction and synthetic controls run on AWS after
counting stops. Archive readiness is atomic and published only after closure,
member verification and hashing; retrieve/verify size, SHA and required sources
before deleting any source. Bind every selected row to its own archive, complete
receipt and arm/master build pins. Publish one manifest pin per arm, no per-file
maps. Terminate task resources, cancel their Spot requests, remove task key/group,
and scan all enabled regions for instances, open/active Spot requests, volumes,
Elastic IPs and task resources. Keep cloud identifiers, addresses and host names
private. Template study PR with footer, no merge.
