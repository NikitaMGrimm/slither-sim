# Loopback latency rig

This manual Windows rig runs 120 background Foragers in the local Battledome
host, plus one production native client. It opens no window and reads no
external art. It uses authored client materials, an offscreen 960x540 MSAA
target, production `LocalGame` and `native::Transport`, and the existing
[`latency-trace-v1`](latency-trace.md) recorder. Tests and CI do not run acquisition.

## Reproduce

Use Rust 1.99, Node 24, PowerShell 7 and a host checkout with the nondefault
`latency-rig` feature described in [the host guide](../../docs/raw-server.md).
The host hook may be built from its unmerged topic branch. From the repository
root, this command builds release binaries sequentially, then captures one run:

```powershell
node client/tools/latency-rig.mjs --build-host-root C:/path/to/host-checkout --output C:/external/evidence/baseline-default --seconds 120 --zoom 0 --region pit-edge
```

The output parent must exist; the output directory must be new and outside the
checkout. Builds set `CARGO_TARGET_DIR=D:/CargoTargets/latency-rig` and
`CARGO_BUILD_JOBS=2` only in each child command's environment. No WSL is used.
Do not use the build option during a reserved measurement window. Prebuild with:

```powershell
& { $env:CARGO_TARGET_DIR='D:/CargoTargets/latency-rig'; $env:CARGO_BUILD_JOBS='2'; cargo build --release --locked --features latency-rig --bin slither-sim --manifest-path C:/path/to/host-checkout/Cargo.toml }
& { $env:CARGO_TARGET_DIR='D:/CargoTargets/latency-rig'; $env:CARGO_BUILD_JOBS='2'; cargo build --release --locked -p slither-wgpu-slice --features latency-rig --bin slither-client-latency }
```

Run prebuilt binaries with one command:

```powershell
node client/tools/latency-rig.mjs --host-bin D:/CargoTargets/latency-rig/release/slither-sim.exe --client-bin D:/CargoTargets/latency-rig/release/slither-client-latency.exe --output C:/external/evidence/baseline-default --seconds 120 --zoom 0 --region pit-edge
```

Repeat in a fresh directory with `--zoom -17` for zoom-out. Sixty seconds is the
default; the runner accepts 10-120 seconds. `--without-truth` works with a normal
prebuilt host and reports packet-relative errors and pipeline stages only.
It never treats received packets as authoritative truth.

The runner freezes its config/selection/analysis plan before acquisition, pins
binary copies and SHA-256 hashes, retains source revision/diff, starts the host
with `host --config` and `127.0.0.1:0`, and parses its complete listening
announcement. It rejects ports 8766, 18884 and 18885. The same bound listener is
used throughout; there is no release/rebind port race. The host uses seed 7,
120 bots, two bot threads, one player and the default Battledome preset with no
artificial player link delay. Truth mode stops/exports the host after client
duration + 12 seconds. Both children and the load sampler are reaped on normal
completion, interruption and failure. All attempts, logs and failures survive.

The shared scratch parent must be below 1.25 GiB before another capture, reserving
0.75 GiB for the new bounded host trace and client output. Keep the entire study
below 2 GiB, including binaries and failed attempts. Full recordings stay out of
Git. Do not overwrite or delete earlier attempts to obtain a passing result.

## Approach and reference

The original `--region inner` plan below retains its strict 0.7-pit band and
two seconds of warmup after entry. A separate `--region pit-edge` analysis
accepts head radius at most 1.1 times the live authoritative pit radius, matching
the requested safe pit-edge alternative. It warms a reference for two seconds
from its first observation, including approach observations. Zoom-out samples
also wait two seconds after actual camera zoom falls below 0.5; a requested zoom
that never activates produces no zoom-out geometry samples. Control still aims
at 0.5 pit and arms boost at 0.7 pit. Zoom is requested as soon as play and own
state exist, independently of approach success. The first baseline binary armed
zoom only at 0.7 pit; retain that distinction when comparing attempts.
Region choice is frozen in each new
capture's plan. Retrospective pit-edge results must be labelled supplemental and
kept alongside the original results, never overwrite them.

The client joins normally; no spawn placement or bot takeover is needed. A
scripted vector field points inward outside an orbit and tangentially at its
target radius, 0.5 of the live pit radius. The harness derives the pit from the
frozen Battledome preset's 1784/3584 proportional boundary and the received live
wall radius. "Near the middle" means head distance below 0.7 pit radius. Once
inside, the orbit stays well away from the one-way exit boundary. Other snakes
can still kill it: deaths and failed approaches are retained, with normal
session rejoin and distinct appearance tokens.

Select the nearest received, living drawn bot within 1,200 world units, breaking
distance ties by wire ID. Keep it until disappearance, life replacement or
leaving that radius, then start another segment. Captures store wire ID and
appearance token; authoritative interpolation requires matching live host life
counters on both ticks. Each client token is bound once to a unique host life
whose head comes within three units of its latest packet head in the preceding
500 ms of truth. This accounts for integer wire quantization and tick spacing;
missing or ambiguous matches are excluded. The lookback is an acquisition bound,
not a claim about maximum network delay. Later host lives cannot rebind an old
client token. Reused wire IDs never bridge host lives. Missing,
offscreen-culled or retained dead appearances produce no head sample.

The rig routes a synthetic pointer callback immediately before each scheduled
60 Hz redraw and toggles Space during seconds 4-6 of each eight-second cycle
only after reaching the middle. These enter production input policy; no direct
heading/boost bytes are invented. Source coordinates, callback timestamps,
decision dependencies and exact sent bytes remain independently auditable.
These are callback-to-wire observations, not physical input-device latency.

For deliberate boost coverage, add `--scripted-reference`. This is a separate
workload, not a replacement for the natural Forager baseline. It reserves a
second player seat and starts one extra bot through the host binary's existing
`slither-client-json-v2` network API, before the measured client joins. All 120
background Foragers remain. No host changes or takeover are needed. The probe
accepts only this bot's reported wire ID within the same 1,200-unit distance;
missing reference exposure remains missing. The `formation-fan-v3` controller
alternates two-second straight normal, turning normal, straight boost and turning
boost requests from play. When it receives the primary player, it follows a
target 250 units lateral to that head. Otherwise it approaches the center. Like
the primary rig, it derives live pit geometry from the frozen Battledome wall
ratio. A bounded received-head/body lookahead fan avoids immediate obstacles and
the pit exit; it cannot guarantee survival. Requested actions do not establish
actual strata: authoritative headings and boost remain the classifier, including
acceleration/deceleration phases. Deaths reconnect explicitly and retained
control journals carry life IDs, reported pit and wall radius. This extra JSON
client adds CPU/load; compare such runs separately. Earlier controller attempts
that waited for deep entry or used a 600-unit formation remain distinct failed
coverage attempts; never relabel them as this controller.
The host recorder runs for client duration + 24 seconds in this mode.

## Clocks and endpoints

Each numeric preparation records `SystemTime` UNIX nanoseconds before/after an
existing process-`Instant` latency stamp. These bracket a mapping between the
two domains for that exact picture. The host records UNIX/Instant step brackets
and optional raw Windows QPC. The client joins UNIX to UNIX and checks adjacent
UNIX spacing against monotonic spacing in both processes; a discrepancy above
1 ms rejects the capture. QPC in the host is retained but is not required for
this join. Nanosecond strings parse as BigInt; no epoch passes through a JS
Number. Startup also retains the smallest positive system-time increment from
10,000 consecutive reads as an empirical granularity observation. Windows'
nominal system-time storage quantum is 100 ns; recorded
bracket widths and clock variation bound the observations, not timestamp
accuracy or synchronization accuracy.

The head journal copies the selected appearance's actual first f32 body painter
center, corrected head and latest committed packet head from the same numeric
prepared draw, before the objects are consumed by rendering. Coordinates are
translated from wire-centered presentation coordinates back to the host origin.
It never prepares a second picture. Frame identity/generation join the immutable
geometry to the existing issue/submit/acknowledgement events. Superseded pictures
are excluded from submitted geometry metrics. The bounded, preallocated scalar
journal reports overflow explicitly; output is written after the run.
This is a prepared-geometry proxy. A visible body can retain a head outside the
viewport; the journal does not establish that head pixels were visible. Neither
the geometry nor offscreen completion is a visible-display latency endpoint.

Offscreen GPU work is completed synchronously after each picture, matching the
existing `--headless` completion policy. This introduces serialized GPU waits.
There is **no surface acquire, present, compositor, scanout or visible-pixel
endpoint**. The last real queue submit is the geometry comparison endpoint;
offscreen acknowledgement/completion time is reported separately. No fake
present events or GPU timestamp claims are added to latency-trace-v1.

The analyzer uses the existing CPU-sink lifecycle mode for the absent surface,
then additionally requires actual native raw-read joins and a real submit for
each accepted offscreen picture. This does not certify desktop completeness.

## Analysis and hygiene

`results.json` includes nearest-rank median/p90/p99 for:

- Receive to apply, apply duration, preparation duration, preparation end to
  last submit, receive to **first** consuming submit, and submit to offscreen
  acknowledgement. `latestMessageAgeAtSubmitMs` separately retains watermark age:
  repeated frames reusing a watermark do not create new pipeline observations.
- Admitted pointer/boost callback to successfully flushed heading/boost effect.
  Flush is transport/OS acceptance, not server receipt; suppressed/coalesced
  callbacks do not become invented sends.
- Reference painter head versus latest admitted packet, including signed
  tangent/speed lag-equivalent and Euclidean error. Prediction can make this
  value negative; it measures packet-relative displacement.
- Authoritative head error at preparation and last submit. Truth is linearly
  interpolated between post-step host positions, only within a live life and a
  tick gap at most 30 ms. The step bracket and interpolation gap remain reported.

Report straight/turning, normal/boost and each zoom separately. Straight means
at most 0.05 radians of reference-heading change over the preceding 100 ms;
turning means at least 0.15 radians. Intermediate motion is counted separately.
With truth, use authoritative motion/boost; interim runs use client presentation
motion as a provisional classifier. Exclude the first two seconds of each
selected near-middle reference segment for approach/zoom settling. Missing
strata remain missing. Frame samples are correlated descriptive observations;
`sparseP99` flags fewer than 1,000 observations. No percentile sums are used.
During a turn, projected spatial error divided by speed is a **lag-equivalent,
not an identified temporal delay**. Neither interpolation nor smoothing is truth.

Trace completion and motion coverage are separate. `measurementCoverage` names
every missing stratum, reports per-stratum frame counts, and distinguishes all
four strata being present from all four having at least 1,000 frames. The latter
is only the descriptive sparse-p99 threshold; correlated frames do not become
independent observations. A trace can be complete while the requested matrix is
incomplete. Failed approaches and missing boost exposure remain part of the
results, not successful measurement rows with invented zeros.

The companion PowerShell sampler checks cargo/rustc roughly every 200 ms,
including machine-wide process IDs, and records machine CPU via CIM about
once a second. Five consecutive compiler-free observations are required before
starting the host. Any observed overlap invalidates and aborts the attempt.
CIM calls add gaps to process sampling and consume some CPU; this is recorded
instrumentation, not a guarantee against unobserved sub-sample compiler runs.
Respect separately coordinated machine timing windows as well. Host stderr
retains tick rate, deferred/lost time, simulation load and configured population.

Reanalyze retained evidence without launching any process or game connection:

```powershell
node client/tools/latency-rig-analysis.mjs C:/external/evidence/baseline-default
node client/tools/latency-rig-analysis.mjs C:/external/evidence/baseline-default --pit-edge
node client/tools/latency-rig-audit.mjs C:/external/evidence/baseline-default
node client/tools/latency-rig-poses.mjs C:/external/evidence/baseline-default
node --test client/tools/latency-rig.test.mjs client/tools/latency-trace.test.mjs
```

`heads.poses.jsonl` is a separate bounded scalar receipt journal. The header is
`client-latency-poses-v1` with `complete` and `dropped`; each row contains
`generation`, decimal-string native `message` ID, command index, wire snake `id`
and `kind` (0 appearance, 1 removal, 2 position anchor, 3 heading/target/speed).
The final row has `end:true` and the exact `records` count. It uses the production
borrowed command reader; no payload bytes or replacement position decoder are
recorded. Message IDs join original `message` and `apply_end` latency stamps.
`reference-cadence.json` separates reference-specific anchor, motion and any-pose
intervals at both endpoints, deduplicating bundled commands per message/channel.
Intervals never bridge selection gaps, removals or replacement appearances.
The initial baseline binary lacks this sidecar: its head-cache refresh cadence
is only frame-observed, with endpoint bounds in the independent anomaly audit.

For storage, `node client/tools/latency-rig-storage.mjs FULLPATH.jsonl ...`
creates new gzip files, verifies decompression against the original SHA-256 and
byte count, writes a receipt, then reclaims the uncompressed duplicate. The
analyzers read either form. A failure retains the original; an existing archive
is never overwritten. Retain the archive and its receipt with the run.

## Latency levers

Rank levers from a valid baseline before changing production behavior. This rig
can measure ingress scheduling, input-to-flush, preparation CPU time and native
socket work. Surface `desired_maximum_frame_latency`, Fifo/Mailbox/Immediate,
tearing, desktop event-loop pacing and physical pointer sampling still require
the corresponding window/surface evidence. Do not infer their gains from an
offscreen run. Default smoothing remains unchanged; any visual/feel candidate
must be an explicit setting off by default and await owner judgement.

The November 2026 loopback baseline exposed two measurement pitfalls. A held
position anchor produced a roughly 230 ms packet-relative sawtooth even though
authoritative tangent/speed errors were only a few milliseconds. Exact reference
receipt journals separated those anchors from changed motion reports, whose
median was about 42 ms. Also, repeatedly aging the last consumed message on every
frame produced an apparent ~180 ms p99. Joining each message to its first
consuming submission instead gave p99 27.80–28.41 ms across four 120-second
natural-reference confirmation runs. That remaining distribution belongs to this
fixed-cadence, serialized-GPU rig; it does not establish desktop swapchain delay.
Keep full per-run quantiles, load, missing strata and immutable captures in the
external study directory. No production speedup or smoothing preference follows
from these corrections.
