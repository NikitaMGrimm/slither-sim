# Internal A4 scaling on the normal map

Measurement only. Runtime `d1bab317` (full source pin recorded at build), portable
repository release profile, Rust 1.99, no production source edits. Synthetic
in-process worlds, seed 7, `Config::official(N)` unchanged: adaptive radius,
official density, prey and 7 ms physics. There is no wire delivery layer, HUD,
socket, decoder, frame adapter or modeled latency. `Timing::default()` is zero.

Populations: 1,000, 2,500, 5,000 and 10,000. Maintainer-frozen cadence: every
six ticks, staggered by `(tick + slot) % 6 == 0`. Six reusable sparse observation
buffers retain each phase's allocations across cycles. An explicitly unselected
smoke run checks compilation and accounting first. `Config::official(10000)`
sets radius 67,000; no stock-client radius cap is applied.

Initial placement uses public `Placement::Admitted` (64 poses, preferred
600-unit clearance). Later deaths retry in ascending slot order, at most 64
poses, the same preferred clearance and positive-margin fallback, with a
one-virtual-second refusal backoff. The harness uses public `spawn_margin`,
which computes a full margin rather than the host's private bounded-margin
early exit. Report this difference and separate admission from engine cost;
this is not a socket-host capacity measurement.

One simulation thread on CPU 0. Serial mode observes and runs direct
`bot::Forager::decide` there. Parallel mode uses the existing parallel batch
observation path in a three-thread Rayon pool on CPUs 1–3, followed by direct
A4 on those same workers. Barriers preserve the same observation/action tick;
there is no speculative or stale-state overlap. One reusable policy per worker.
Warmup may use this parallel mode; it must match serial action/state digests
in a focused smoke comparison. No policy override, forced boost or intents.

Warm until length and food statistics stabilize. Census every 1,000 ticks;
evaluate consecutive 30-virtual-second bins after at least 300 virtual seconds.
Require the last four bins' mean lengths and food counts within 5% of their
four-bin mean, p90 length within 10%, and at least 99% living. Preserve the full
warmup trajectory. A 1,800-virtual-second or two-wall-hour per-population limit
is a censored warmup, not a stabilized row; report it and retain its census.
Never select a more favorable warm snapshot based on timing.

Freeze a complete checkpoint plus harness RNG, tick, retry clocks and held
actions. On each population, discard one serial/parallel pair, then run five
alternating pairs of 512 ticks from that state. Save all pairs and final-state
digests. Separate 512-tick verification runs retain full action digests, keeping
action hashing outside throughput measurements. Record world CPU/wall, admission/lifecycle CPU/wall,
observation CPU/wall, A4 CPU/wall, whole-window wall/process CPU and memory.
Report per-bot cost per actual observation/decision, cadence-amortized per-tick
cost, whole-window throughput and the reciprocal isolated simulation cost.
Also run simulation-only replay from the same checkpoint and a full held-action
tape produced by the separate serial verification run. Run the same admission
and physics, with observation and A4 absent; require the final state to match.
Replay decoding is outside world/admission brackets and included in whole-window
wall throughput. Rotate its position around each serial/parallel pair. Report
both its actual unpaced throughput and the reciprocal world/admission cost.
Neither establishes a wall-paced sustained-capacity claim.

Run matched 512-tick ordinary/profiled diagnostics separately using the existing
`StepProfile`: movement, collisions, death processing, food pickup, prey and
ambient refill. These are wall-clock phase brackets; report their reconciliation
with same-thread CPU and the profiled-versus-unprofiled overhead. Keep residual
work explicit. Do not relabel phase wall time as exact CPU. Use a separate
sampled CPU profile if attribution or preemption makes that distinction material.
HUD and wire phases are structurally absent. Record working RSS and peak RSS,
world-only and observation/policy-buffer census, body quantiles, food by origin,
prey, deaths/respawns/refusals and adaptive radius.

Timing requires an otherwise idle guest. No builds, analysis or status polling
during timed windows. No long local work, no AWS and no outside server traffic.
Raw checkpoints, binaries and samples stay outside Git; selected lean data,
reproduction code, failed attempts and a table reducer accompany the report.
Archive and verify before cleaning task-owned guest storage. Research index row
only, no changelog, no implementation, template PR body and one assistance footer.
