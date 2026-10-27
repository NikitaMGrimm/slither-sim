# Protocol-13 motion and browser workload

## Decision

No encoder or pacing change is selected. Under this one-core browser test,
NTL has larger large-snake corrections than stock on a matched stream. Reducing
other-snake traffic reduces the excess while preserving every retained peer's
movement byte and timestamp. This supports a client-workload contribution;
it does not establish a coordinate, rotation-scale or growth-layout defect.
The owner's unconstrained Chrome session has not been instrumented by this study.
Host frame pacing is assigned to a separate investigation.
The later socket-wake change and its [timing study](../2026-10-27-host-frame-timing/REPORT.md)
are outside these frozen recordings; the evidence branch is rebased onto main.

## Pins and method

Host/adapter: `368abe464c90c998d54c964f2d735ea8880193ea`, release build with
one build job and one inherited logical-CPU affinity bit. No production source
was changed. Raw data and fresh collection/analysis scripts are local-only in
durable private task scratch `protocol13-map-motion-20261027`; hashes below
identify material, not public downloads. No new outside-server connections.

The official reference is retained `02-legacy.jsonl.gz` from the
[catalog](../2026-10-26-protocol13-catalog/REPORT.md), first 100 seconds.
Local: seed 7, plain Battledome, 64 single-threaded bots, two remote seats,
`vision: 64000` (the owner's play-host setting, **not** default 1500).
Warm up 90 seconds, then record a stock Forager observer for 100 seconds.
All four lives are retained; choose the longest uninterrupted life by duration
before browser measurement: 34.860 seconds. Derive its protocol-13 messages
offline through the unchanged production adapter, keeping each broadcast's
timestamp and elapsed-header allocation. This is a matched wire transformation,
not two different simulations or an independently collected legacy socket.

Replay each stream separately to sandboxed bundled Chromium, real NTL 9.78 or
the retained stock browser. Loopback page/DNS mappings, fail-all external DNS,
deny proxy and HTTP allowlist block outside contact. Source is read only as
data; received challenge source is never executed by the relay/collector.
Viewport 1100x800, DPR 1, normal zoom. NTL uses its FPS-60 setting; stock keeps
its normal render loop. The native relay blocks on a high-resolution per-thread
timer, preserves WebSocket boundaries and original receipt timing, and logs
lateness. No busy wait or system-wide timer changes. Report relay errors rather
than assuming ideal playback; primary p99 lateness is below 1 ms.

Passive wrappers sample the actual `o9` / `redraw` renderer, not a second RAF.
Drawn logical head is `(xx+fx, yy+fy)`; log positional correction offsets,
speed, angle, scale, segment count and local object identity. Select the largest
visible living other snake. Exclude dead own/fade frames and identity changes.
Overlap stratum: 100–169 segments, speed below 9; population and speed still
differ within that stratum, so this is not an equivalence or causal-size test.

For 4–50 ms adjacent drawn frames, displacement error is
`abs(distance / dt - mean(sp) / 0.032)` in world units/s. Other frame gaps
are reported, not quietly clamped. Correction innovation is the magnitude of
the change to `fx/fy` across a relevant message, counted only inside eligible
same-selected-peer intervals. **It is not an instantaneous displayed jump:**
the smoothing offset is designed to preserve continuity. Frames and messages
are correlated; one recording/replay per primary condition is descriptive.

## Selected measurements

| Replay / overlap exposure | Draw gap p95 / p99 ms | Displacement error p95 units/s | Offset innovation p95 / p99 units | Innovations/s |
| --- | --- | --- | --- | --- |
| Official 13 → NTL, 24.81 s | 18.9 / 28.2 | 27.00 | 4.77 / 6.10 | 4.96 |
| Matched local 13 → NTL, 11.88 s | 42.6 / 76.9 | 250.71 | 19.15 / 34.09 | 4.46 |
| Same local 19 → stock, 14.10 s | 28.2 / 40.6 | 84.28 | 3.82 / 6.00 | 4.47 |
| Local 13 reduced scene → NTL, 12.62 s | 28.0 / 48.1 | 136.69 | 5.81 / 10.52 | 4.44 |
| Separate default-vision 13 → NTL, 31.11 s | 23.6 / 43.1 | 114.89 | 6.16 / 7.84 | 4.63 |

Reduced scene is a diagnostic, **not a proposed visibility policy**: retain own
and the five large peers seen in the matched views; omit other snake messages.
All retained snake bytes/timestamps and all food, minimap, HUD and pong traffic
are identical. The local NTL offset tail falls from 19.15 to 5.81 units p95,
near but not equal to the official 4.77. Displacement tails still exceed the
official replay: this study does **not** claim the requested jitter parity.
All five browser runs had zero page exceptions and zero lag-flag
exposure within the reported stratum.

The default-vision check repeats the host/Forager collection with `vision: 1500`,
retaining three lives and replaying the longest (42.446 s). It has a different
simulation workload, not identical target trajectories. Its smaller correction
tail supports the load hypothesis but does not establish official jitter parity.

In the full local NTL run, render/model/message-handler wall-time p95 was
11.6 / 0.2 / 0.3 ms per sampled large-snake frame; renderer p99 was 28.5 ms.
There were 65 live snakes and about 5,500 cached body points. This measures
callback wall time, including scheduling/GPU waits, not isolated CPU cost.
The reduced run has six live snakes; parsing/model work falls, but occasional
render tails remain. A separate extreme-zoom diagnostic shows larger stalls;
it is excluded from the normal-view table. No duplicate live-ID appearances
were found in the audited legacy socket or official reference.

## Wire and client-semantics checks

Independently decode the matched 19/13 streams: **10,192 anchors**, including
1,288 growth updates, **60,048 rotation tuples** and 39 tail removals.
Coordinates differ by at most 0.5 per axis (maximum Euclidean error 0.69748);
segment counts, growth choices, transmitted mass, rotation values and event
times match. This tests emitted bytes, not adapter internals. Integer legacy
step distributions match the catalog's roughly 42-unit steps and rounding tail.
No alternate speed/angle scale is justified.

Client-derived source inspection: NTL `GG` uses angle `2π/256`, speed `/18`;
`nG` treats `n/N` as growth and `g/G` as movement. These match the host output.
NTL `hG` starts at command byte 2 and does not consume elapsed-header deltas for
lag compensation. Its lag multiplier is driven by wall time/pongs; its model
step is capped at 40 ms. Large browser gaps can therefore under-advance its
prediction and produce later anchor corrections. Changing elapsed-header
allocation cannot fix that client behavior. This is not a host-pacing finding.

The nonzero official baseline is the part to retain: the same NTL prediction
and correction machinery is active there. It includes official gameplay and
recorded transport variation; do not attribute all baseline jitter solely to
NTL or aim for zero. The excess under full local visibility is sensitive to
client workload. No safe server-side kinematic fix is established.

## Retention, failures and next boundary

Full selected metadata, counts and quantiles are in [results.json](results.json).
It pins raw-recording, replay-observation and adapter-result hashes. Third-party
code/assets, raw names, addresses and complete captures are not published.

Retained excluded attempts: two renderers overloaded one core; elevated starting
mass overloaded admission; Python busy-wait and lower-priority relays missed
deadlines; an initial reduced scene kept the wrong own ID; early metrics counted
fade frames; a simultaneous stock observer died early; an early stock view had
zero large-snake exposure. Those attempts do not support protocol conclusions.
Native relay calibration, same-source stock playback and alive/object filtering
replace them; failed files remain in private scratch.

The minimal production change remains the merged [minimap fix](../2026-10-27-legacy-map-motion/REPORT.md).
Do not change shared physics, broadcast clocks or NTL assets from this evidence.
Before any new motion patch, reproduce a target-byte error or identify a bounded
client-load mechanism that can be changed without removing requested gameplay.
