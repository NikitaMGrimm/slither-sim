# Crowded frame preparation reuse

The selected change reuses exact minimap byte-blend arithmetic and defers the
complete advance snapshot until the control callback accepts. Packet ingress,
food appearance, snake geometry and the GPU renderer are unchanged.

This follows the [food snapshot](../2026-11-01-food-snapshots/REPORT.md) and
[motion transaction](../2026-11-01-motion-transactions/REPORT.md) studies. Their
11.20 ms GPU p95 came from a different partial live input, not this fixed replay;
it is context rather than a paired GPU baseline here. Remaining packet ingress
belongs to a separate change.

## Question and baseline profile (recorded before implementation)

Which CPU/GPU stages exceed the crowded frame budget, and can repeated
preparation work be removed while preserving the ordered picture, filtering
and blend arithmetic?
Baseline is main `7ce9569285e2b6aec22f725675865ee5b3fe9a08` with passive
`frame-profile` markers around the control probe, snapshot and advance.

[excluded-diagnostics.csv](excluded-diagnostics.csv) retains the four original
per-run summaries behind the following table, including CPU/GPU quantiles, peak
quads, counts, allocation totals and host-load/compile flags. No measurement was
rerun for this addition. `python recompute.py --check` regenerates and verifies
this table as well as the existing retained-data controls.

One excluded diagnostic invocation per cell uses production LocalGame,
procedural artwork, 960x540 Original/MSAA4, and offscreen DX12 timestamps on
AMD Radeon(TM) Graphics (IntegratedGpu), driver 32.0.21001.9024. No window,
socket, outside website or stock artwork is used. The host was busy with other
compilation; these figures rank candidates and are not the A/B result.

<!-- BEGIN EXCLUDED DIAGNOSTIC BASELINE -->
| Workload / view | CPU prepare median / p95 ms | GPU passes median / p95 ms | Peak prepared quads |
| --- | ---: | ---: | ---: |
| Restricted replay / default | 22.938 / 27.760 | 42.376 / 66.884 | 118,747 |
| Restricted replay / whole map | 12.473 / 18.218 | 0.705 / 0.783 | 1,196 |
| Authored 30,000 food, 40 snakes / default | 11.425 / 14.273 | 30.303 / 54.676 | 35,870 |
| Authored 30,000 food, 40 snakes / whole map | 28.611 / 35.017 | 82.173 / 87.637 | 218,273 |
<!-- END EXCLUDED DIAGNOSTIC BASELINE -->

The synthetic arena is small enough that whole-map food remains above existing
LOD thresholds; the restricted replay's much larger map culls most subpixel food.
These are intentionally different stress cases. Received maxima in the replay
are 31,467 food keys and 40 snakes across 32,106 messages / 366 callbacks.

Ranked default-replay CPU stages, median / p95 milliseconds:

1. Entity advance including its inner snapshot: 4.185 / 5.703.
2. Food planning: 4.171 / 4.996.
3. Minimap composition: 3.197 / 3.721.
4. Resource planning: 1.682 / 2.381.
5. Bodies/faces: 1.417 / 2.102.
6. Read-only control probe including its discarded snapshot: 1.016 / 1.618.
7. Curves including draw transaction snapshot: 0.658 / 0.957.
8. Outer advance snapshot: 0.374 / 0.645.
9. Nicknames: 0.296 / 0.595; prey planning: 0.288 / 0.344.

Preparation also includes other control/session work, HUD completion, final geometry validation
and bookkeeping outside these observer spans. Quantiles are not additive.
GPU submission CPU and blend/overdraw work remain separate from preparation.
The first candidate targets the dominant default-view GPU work: adjacent quads
currently split whenever their individual texture identities differ, even when
their dimensions and sampling policy match.

Restricted recordings, raw traces, process paths, binaries and failed attempts
remain owner-local outside Git. Only aggregate counts/timings may be published;
no server identity, names, coordinates or recording bytes belong in this study.

## Rejected GPU candidate

An equal-dimension texture-array prototype kept the ordered picture and native
sampling/blend paths but could not batch across alternating inner/outer glow
dimensions. Its initial default-replay GPU median was 42.982 ms versus 42.376 ms
in the prior diagnostic; synthetic whole-map medians were 88.882 versus 82.173 ms.
These single non-alternating diagnostic observations were collected under
compilation and do not establish an improvement or a reliable regression size.
The candidate also added a per-vertex layer word, increasing upload bytes by 20%.
It was rejected before the primary comparison and is absent from shipped code.

The prototype passed three explicit Vulkan iGPU checks: in-flight vertex/target
reuse, black-shadow composition, and a new array test comparing individually
bound versus grouped images, mixed sampling/blends, revision restoration and a
group larger than the device layer limit. Those checks concern only the rejected
candidate. Its owner-local patch SHA-256 is
`60093f991fc933b488e8b1cb75cbacf1e2aeab77f9f8e85fd504c487fb769827`;
executables, patch, build logs and failed/inconclusive diagnostics are retained
outside Git. No performance claim is made for it.

## Rejected composition cache and stale artifact

A full unmarked-image cache keyed by image bytes and layer styles missed during
the replay's nearly continuous crossfade. A forced-fresh diagnostic still spent
3.503 ms median in default-view minimap composition, so this cache was replaced.
Its patch and run journals remain owner-local. An earlier release executable
had incorrectly reused baseline libraries after restoration preserved old file
timestamps; its candidate parity/profile runs are invalid and retained separately.
The stage observer exposed its still-allocating control probe. Package-only
release cleanup and verbose recompilation corrected that build issue. No primary
A/B runs had begun. Final measurements use fresh executable pins below.

## Selected implementation

The minimap compositor retains its already opacity-adjusted procedural background.
Each image-layer call finds one present nonzero source color and builds a
1,024-byte table for all destination byte values and four channels using the
original floating-point expression and rounding. Matching source pixels use that
exact table; zero RGBA pixels preserve destination bytes, and other source colors
keep the original scalar blend. Layer opacity may change every callback. There
is no new palette restriction, change to layer order, or approximation of alpha.
The marker, tile/revision decisions, resource ordering and label remain unchanged.

`advance_with_control` computes the same frame step on a copied small clock and
calls control once with the same pre-motion view. A rejected probe returns before
copying the world; an accepted callback snapshots and commits the staged clock
before the original action/entity sequence. Invalid clocks/actions and callback
errors leave the complete model unchanged. No ingress transaction is changed.

## Method and reproduction

The retained [probe patch](probe.patch) adds a diagnostic-only offline example
and authored synthetic scene; apply it to the chosen revision. On the baseline,
also apply this change's observer-only `game.rs`/`render.rs` diff, preserving the
baseline core/minimap implementation. Both executable
sides use the same driver source, Rust 1.99.0 release/thin-LTO profile and
`frame-profile,latency-rig` features. No recorder is attached during primary
timing. Every Cargo invocation uses a task-owned process environment with
`CARGO_TARGET_DIR=D:/CargoTargets/frame-prep` and `CARGO_BUILD_JOBS=2`.
The final executable/library build is forced fresh; source restoration must
update timestamps or clean the affected task-owned package artifacts.

Build `slither-wgpu-slice --example frame_stage_probe` with those features.
Its arguments are `replay|synthetic INPUT NEW_DIRECTORY SECONDS ZOOM_NOTCHES`;
`live` is rejected and transport effects are in-memory acknowledgements. The
synthetic mode does not read INPUT. Default zoom uses 0, whole-map uses -120.
The replay runs the complete 6.1-second schedule (366 callbacks); primary
synthetic runs use 1.6666666666666667 seconds (100 callbacks), excluding the first
40 for 60 measured pictures. The synthetic world is authored: 30,000 food and
40 snakes on a small arena, 24-cell minimap, with settled spawn animation and
no further ingress. It is also a small-minimap control for the replay's map cost.

Five before/after pairs alternate within each workload/view on the same host.
No quiet period is reserved and no other process is stopped. Each side's
calling-thread CPU preparation, resource sync, submission and GPU pass intervals
are recorded; timestamp resolution/completion waits, logging and serialization
are outside those CPU phase timers. The callback wall interval includes those
GPU waits and ingress. `cpu_supply_ns` is the per-callback sum of preparation,
sync and submission, excluding GPU waiting and packet ingress. None is display FPS.
Populated replay summaries select model time >=1 second and >=10,000 received
food keys. Per-run p95 uses nearest rank. Tables aggregate the five per-run
medians/p95s, rather than pooling quantiles or summing stage quantiles.

Process classes and CPU load are sampled around each run and throughout it.
The selected CSV retains process-class maxima, observed compile overlap and load
range. All successful primary runs are included, even with observed background
work; no slow sample is removed. Full process paths/journals remain owner-local.
Snapshots do not establish absence between observations. Executables, raw samples,
restricted inputs, failed and inconclusive attempts remain outside Git.

[Stage summaries](stages.csv) retain one baseline observer run per cell and one
final CPU-only observer run per cell. Baseline GPU and final CPU-only profiling
have different scheduling overhead and are excluded from primary A/B tables.
They rank operations and confirm that minimap/probe work actually changed:
default replay minimap median is 3.197 -> 0.202 ms; control probe is reduced to
about 1 microsecond. Primary runs leave observers and output hashing disabled.

## Primary results

[runs.csv](runs.csv) contains all 40 per-run summaries. Run
`python recompute.py` in this directory (standard library only) to recompute the
tables, pair checks, ranges and stage ranking. All runs completed without picture
skips. Every replay processed all 32,106 messages / 366 callbacks, with 302 meeting
the populated selection. Each synthetic run measured 60 of its 100 callbacks.
CPU-load snapshots ranged from **5% to 94%**; **33/40** runs observed Cargo,
rustc or link processes, including this task's remaining checks early in the
series. All are retained in the aggregates.

CPU preparation is lower in all five replay pairs at each view. The default-view
median paired saving is **3.952 ms** (range **0.528-4.411 ms**); whole-map is
**2.491 ms** (**1.817-3.122 ms**). These paired results and the large removed
minimap/probe stage support a bounded improvement on this input despite host drift.
The synthetic small-map controls show no reliable preparation difference relative
to their run variation; their raw changes are reported without a speed claim.

| Workload / view | Before CPU prepare median / p95 ms | After median / p95 ms | Run median range before / after ms |
| --- | ---: | ---: | --- |
| Replay / default | 17.783 / 24.327 | 14.110 / 20.558 | 15.545-19.986 / 13.567-16.544 |
| Replay / whole map | 6.890 / 9.815 | 4.220 / 6.047 | 6.094-8.691 / 3.820-5.568 |
| Synthetic / default | 8.399 / 10.472 | 8.523 / 11.327 | 7.460-11.669 / 7.041-11.448 |
| Synthetic / whole map | 24.314 / 30.810 | 22.715 / 27.550 | 21.692-30.927 / 21.621-29.979 |

The median-of-run-median replay preparation reductions are **20.65% default**
and **38.75% whole map**. These differ from a median of individual pair ratios;
the recompute script retains both paired millisecond savings and side aggregates.
Historical 21.6 ms preparation figures and the initial observer profile use other
host/build conditions and are not substituted for this series' measured control.

| Workload / view | Before GPU median / p95 ms | After median / p95 ms | Run median range before / after ms |
| --- | ---: | ---: | --- |
| Replay / default | 42.289 / 67.066 | 42.603 / 66.882 | 41.895-42.895 / 42.319-43.573 |
| Replay / whole map | 0.711 / 0.778 | 0.725 / 0.780 | 0.683-0.735 / 0.708-0.985 |
| Synthetic / default | 31.864 / 36.564 | 33.181 / 37.250 | 31.550-33.403 / 30.728-33.696 |
| Synthetic / whole map | 83.047 / 87.867 | 82.501 / 89.064 | 81.211-86.090 / 81.423-84.601 |

There is no demonstrated GPU improvement; the renderer and its inputs remain
unchanged. Default replay CPU supply (prepare + sync + submit) median/p95 falls
**35.872/44.933 -> 31.269/41.378 ms**. Whole-map supply is
**7.551/10.615 -> 4.919/6.820 ms**. Full callback wall, including ingress and
serialized GPU completion/query waits, is **98.034/153.042 -> 92.960/142.251 ms**
default and **11.107/57.988 -> 7.987/53.896 ms** whole-map. Those waits make the
callback figures diagnostic throughput, not gameplay frame pacing.

Requested calling-thread allocation bytes over each complete replay fall from
**52.670 to 51.211 GB** default and **49.083 to 47.623 GB** whole-map (decimal GB).
These include unchanged ingress and wgpu-side requested allocations, not live
heap, driver heaps or GPU residency. The authored read-only probe regression
independently allocates zero with 41 retained snakes and preserves the full model.

## Exactness and checks

[parity.csv](parity.csv) records four complete validation comparisons. Across
1,092 paired callbacks, all 18 compared fields match exactly, including received
counts, draw/draw-call counts, lag/zoom, own raw/corrected motion and speed. The
complete Debug representation of every world/overlay draw and resource image,
together with every sent command, produces matching per-cell 64-bit digests.
Those are consistency checks rather than cryptographic proofs. Six synthetic
960x540 RGBA readback pairs also match byte-for-byte. Hash/readback runs are
excluded from the timing series; restricted pictures are not published.

- Four allocation tests and eight control-boundary tests passed.
- Seven minimap tests passed, including all 256 destination bytes across source
  colors/alphas/fades, mixed/transparent planes, and 0/24/80/512-cell dimensions.
- Full `npm run check` passed on the final implementation: source guards,
  JavaScript, core/native tests, strict native/profiling/Wasm Clippy, Canvas ABI,
  desktop/Wasm builds and generated WebGPU bindings.
- All seven repository documentation tests passed. The diagnostic patch applies
  cleanly with `git apply --check`; no official assets or recording bytes are added.
- An initial allocation fixture sent opaque food without enabling food support;
  it correctly retained no food. The regression now uses 41 admitted snakes.
  An earlier cache-test Clippy loop warning was corrected by the final table tests.
- A summary-export assertion initially assumed 306 populated replay callbacks;
  the unchanged selection yields 302 in all 20 raw journals. The assertion was
  corrected after checking every journal; no filter, sample or timing changed.

## Remaining limits

### Rebase validation (2026-11-02)

The implementation was rebased onto `d325df3b`, retaining the received-snapshot
and pellet-raster/mip changes. The measurements, digests and full `npm run check`
above remain historical evidence for the original implementation revision;
they were not rerun or relabelled. On the rebased source, 79 core library,
4 allocation, 8 control-boundary, 7 minimap, 9 food-ingress, 1 pellet-quality and
1 feel-trace tests pass. Strict core/native/profiling/Wasm Clippy and seven
documentation tests pass. `recompute.py --check` now also verifies the excluded
diagnostic baseline table from its four retained per-run CSV summaries. This
documentation correction adds no measurements.

This fixes part of frame preparation. Default CPU preparation p95 still exceeds
16.7 ms, and default GPU p95 is about 67 ms on this replay. Entity advance, food
planning, final geometry validation, resource planning and body work remain CPU
targets; per-pellet glow drawing, binding runs and overdraw remain GPU targets.
Packet-ingress bursts remain separate work. The old recording lacks recurring
pongs; both sides retain the same lag schedule. There is no 60 Hz, physical input,
scanout, owner-play, official/mod comparison, stock-art, 1080p/4K, discrete-GPU,
other-iGPU or browser-runtime acceptance claim. Independent maintainer review is
still required. No live server was contacted and no recording was collected.

## Provenance

- Baseline source: `7ce9569285e2b6aec22f725675865ee5b3fe9a08`, plus the same passive observer markers.
- Baseline executable SHA-256: `c4d82574ad071e2ccd1a0f3160a0cf6d664e7437e1af8b4894db2265f476a16e`.
- Final candidate executable SHA-256: `52c02ce9447a03a02823350f7ac52a64c50f253d641a1ef3367df2f53b1786dc`.
- Candidate core source blob: `0a17523c8d9f5e230d810a894d78378cbb52986b`.
- Candidate minimap source blob: `7ed6df3a445b6cfb5282c704302fc4bc262831b9`.
- Restricted input SHA-256: `84d4c361ee9feb5369166858cabdca857f7f740a3f19d146cbd3e0eaf8902c82` (owner-local access only).
- Windows x86-64, Ryzen 7 9800X3D CPU; DX12 AMD Radeon(TM) Graphics IntegratedGpu,
  driver 32.0.21001.9024, 960x540 Original/MSAA4 procedural presentation.
