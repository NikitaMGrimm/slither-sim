# Exact native pellet workers — 2026-11-03

Dense art preparation is faster with at most four participants including main.
RTX/Vulkan 4K Original art median/p95 falls from 5.164/7.533 to 3.928/6.656 ms;
Native falls from 4.917/7.465 to 3.705/6.669 ms. This does **not** meet the
4.167 ms high-refresh budget at p95, and it is not a displayed-FPS acceptance claim.

## Method and identities

Access: **local-only, restricted input and raw journals**. The input is
`retained-join-A`, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`.
The recording, frozen source directories, binaries, launchers and raw callback,
phase, allocation and machine-load journals remain in the external D: performance
scratch. They are not distributed with the repository. No new game-server session
was used. The authored rectangle contains 25,000 synthetic pellets.

Each replay consumes 29,044 envelopes in 600 virtual 60 Hz callbacks over ten
seconds. The art window is callbacks 120–419. Runs are unpaced, synchronized
**offscreen diagnostics**, not surface presentation. They follow native login/menu
composition and include egui painting in timing. Original uses the capped world
target; Native uses the requested physical size and the existing MSAA admission.
CPU stage sums include ingress, redraw, resource sync and submission; pellet
preparation is nested inside submission and must not be added to it again.
GPU values use the existing per-pass timestamp diagnostic mode.

The baseline contains #447, #448 and #450: its measured runtime files are
byte-identical to main `be40e228`. [identities.json](identities.json) retains the
actual frozen binary, source and driver hashes separately from the reviewed
source. The sealed final candidate corresponds to runtime `ec80d1ae`. The final
reviewed default additionally selects serial below four available logical CPUs:
two participants were not consistently faster at 4,096/8,192 pellets. Timing on
this host uses four participants or an explicit diagnostic override; those paths
are unchanged by the default-selection guard. No small-core hardware timing is
claimed. [measurement-delta.patch](measurement-delta.patch) exposes that guard,
passive instrumentation, comment and test differences without replacing measured
hashes. The earlier [source delta](source-only-delta.patch) remains historical.

The timing drivers differ only in the candidate worker override and diagnostic
canonical-output correction; [driver-delta.patch](driver-delta.patch) retains that
difference. Their native composition, drawing, hashing and allocation code is
shared. The image baseline is the separately sealed #450 image binary, except the
reconnect control uses the #450 timing binary with image hashing enabled. Its
binary and driver hashes are recorded separately.

All arms use release/thin LTO, two build jobs and the same configuration per pair.
Five alternating pairs per quality are retained for RTX/Vulkan 4K and AMD/DX12
1080p/4K. Five forced-20-ms-delay pairs per quality, two eight-burner pairs per
quality and two worker-disabled controls are also retained. Host CPU load and
cargo/compiler/link/test activity were sampled around every attempt; only
observed compiler/link overlap excludes an attempt, and its entire pair is excluded.
No noise-based exclusions or quiet windows were used. All attempts remain in
[runs.csv](runs.csv), with a cohort, arm, acceptance reason and binary identity.
`initial`, `range`, `bit` and `slab` are earlier compositions; only `final` supplies
the adoption tables below. Raw journals remain restricted outside Git.

Exact timing invocation, with the paths supplied by the local custodian:

```powershell
$env:CARGO_TARGET_DIR='D:/CargoTargets/community-lag'
$env:CARGO_BUILD_JOBS='2'
$env:CARGO_INCREMENTAL='0'
cargo build --locked --release --manifest-path "$frozenSource/client/apps/wgpu-slice/Cargo.toml" --features frame-profile --example join_profile
$env:PROFILE_SIZE='3840x2160'
$env:PROFILE_ADAPTER='rtx'
$env:PROFILE_BACKEND='vulkan'
$env:PROFILE_MENUS='1'
$env:PROFILE_PHASES='1'
$env:PROFILE_WORKERS='4'
$env:PROFILE_WORKER_DELAY_MS='0'
& $sealedArm replay $restrictedInput $freshOutput 10 0
```

Native adds `PROFILE_NATIVE=1`; AMD selects `igpu`/`dx12` and the table's physical
size. Forced delay sets `PROFILE_WORKER_DELAY_MS=20`; serial fallback sets
`PROFILE_WORKERS=1`. The alternating launcher clears other `PROFILE_*` settings.
The timing driver and launchers are local-only beside the sealed artifacts in the
external performance scratch. Recompute every adoption table without those inputs:

```text
python -B research/client-performance/2026-11-03-pellet-workers/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-03-pellet-workers -p test_study.py
```

## Results

Each cell is the median of per-run quantiles, not a pooled-frame quantile. A paired
delta subtracts the serial quantile from that same pair's pool quantile. Requested
bytes are decimal GB. Idle percentages use **one core**, not total-machine CPU.

| Adapter / physical size / quality | Pairs | Serial art median/p95 ms | Pool art median/p95 ms | Paired p95 delta median [range] ms |
| --- | ---: | ---: | ---: | ---: |
| rtx / 3840x2160 / original | 5 | 5.164/7.533 | 3.928/6.656 | -0.901 [-1.178, -0.737] |
| rtx / 3840x2160 / native | 5 | 4.917/7.465 | 3.705/6.669 | -0.874 [-1.389, -0.135] |
| igpu / 1920x1080 / original | 5 | 11.592/14.258 | 10.222/13.074 | -1.039 [-1.369, -0.902] |
| igpu / 1920x1080 / native | 5 | 11.684/14.116 | 10.483/13.276 | -0.779 [-1.814, -0.264] |
| igpu / 3840x2160 / original | 5 | 12.087/14.362 | 10.993/13.934 | -0.413 [-1.413, +0.429] |
| igpu / 3840x2160 / native | 5 | 15.183/17.593 | 14.177/17.221 | -0.628 [-0.776, -0.257] |

| RTX 4K / quality / arm | Art median range ms | Whole median/p95 ms | CPU stages ms | Pellet prepare ms | Submit ms | GPU ms | Rust requests / GB | Host load median [range] % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| original / serial | 5.090-5.228 | 4.824/8.629 | 4.780 | 2.193 | 2.770 | 0.229 | 3273416 / 10.909 | 8.8 [6.0, 22.4] |
| original / pool | 3.874-3.992 | 3.645/8.366 | 3.571 | 0.910 | 1.532 | 0.229 | 3327083 / 12.163 | 10.7 [6.3, 25.8] |
| native / serial | 4.847-4.990 | 4.673/8.741 | 4.491 | 2.137 | 2.521 | 0.243 | 3194859 / 10.822 | 9.1 [4.3, 19.6] |
| native / pool | 3.673-3.793 | 3.503/8.272 | 3.293 | 0.936 | 1.347 | 0.242 | 3248532 / 12.075 | 10.1 [7.2, 34.5] |

| Stress / quality | Pairs | Serial art median/p95 ms | Pool art median/p95 ms | Serial/pool whole p95 ms |
| --- | ---: | ---: | ---: | ---: |
| late20 / original | 5 | 5.097/7.479 | 5.178/7.871 | 8.539/9.787 |
| late20 / native | 5 | 4.863/7.465 | 4.933/7.593 | 8.562/9.452 |
| busy8 / original | 2 | 5.687/8.880 | 4.770/8.736 | 10.517/10.918 |
| busy8 / native | 2 | 5.701/8.712 | 4.233/7.188 | 10.855/10.217 |
| disabled / original | 2 | 5.089/7.687 | 5.081/7.904 | 8.507/9.279 |

| Final paced idle mode | Serial/pool process CPU % of one core | Serial/pool CPU ms per callback | Pool threads / observed idle CPU seconds |
| --- | ---: | ---: | ---: |
| menu | 20.106 / 17.744 | 0.900 / 0.795 | 0 / 0.000000 |
| sparse | 16.879 / 16.890 | 0.758 / 0.758 | 0 / 0.000000 |
| after-dense | 15.833 / 13.793 | 0.710 / 0.619 | 3 / 0.000000 |

| Visible / settled | Serial preparation ms | Two participants ms | Four participants ms |
| --- | ---: | ---: | ---: |
| 1024 / false | 0.1146 | 0.1095 | 0.0791 |
| 1024 / true | 0.0952 | 0.0890 | 0.0734 |
| 2048 / false | 0.2182 | 0.1764 | 0.1256 |
| 2048 / true | 0.1885 | 0.1501 | 0.1093 |
| 4096 / false | 0.4329 | 0.4281 | 0.3046 |
| 4096 / true | 0.3760 | 0.3878 | 0.2985 |
| 8192 / false | 0.8694 | 0.8496 | 0.5803 |
| 8192 / true | 0.7608 | 0.7788 | 0.5419 |

| First final pair / quality | Source rebuilds / 300 art callbacks | Slowest-16 source / ingress / redraw median ms | Slowest-30 whole callbacks: cold / dispatched |
| --- | ---: | ---: | ---: |
| original | 65 | 1.666/1.657/3.545 | 26 / 3 |
| native | 65 | 1.773/1.461/3.434 | 26 / 3 |

## Tail behavior and allocation cost

The originally reported 9.08→9.29 ms whole-replay p95 and 7.6→9.4 ms delayed art
p95 belong to the `initial` composition. The source-group metadata preparation
added for compact inputs was expensive even with workers disabled. The retained
`range` and `bit` probes tried range filling and cheaper metadata checks. Final
admission keeps the original numeric family comparison and checks exceptional
zero-half families once: those use the canonical fallback, preserving signed zero.
The first final pair still rebuilds source groups in 65/300 art callbacks. This is
measured source-group rebuilding, not a hypothetical future-cache justification.

Normal final art p95 improves in every RTX pair. Whole-replay p95 also improves
in the normal final cohort, but **not in every stress control**. Forced-late art
p95 remains higher (Original 7.479→7.871 ms; Native 7.465→7.593 ms), and whole-run
p95 is higher too. The disabled control also has a residual tail cost. Copying,
chunk setup and source lookup/admission are not free; cold raster/menu variability
also remains. There is no 20 ms rendezvous: a claimed, blocked chunk is recomputed
by main without waiting, and a job that contributes no chunks triggers 32 callbacks
of serial backoff. This establishes nonblocking result adoption, **not immunity to
OS scheduling, allocator contention or unrelated work**. Eight ordinary-priority
CPU burners retain the median gain; Original's whole-run p95 is nevertheless a
little worse. These negative controls are kept, not excluded as noise.

The process-wide Rust global allocator uses atomic counters, including worker
allocations, startup and teardown/drain. It includes #450's per-spawn 280-byte
metadata payload plus Arc ownership allocation. Original requested allocation
rises from 10.909 to 12.163 GB: compact input copies and chunk result vectors add
about 1.25 GB cumulatively over the replay. This is expected temporary allocation
traffic, **not peak resident memory or a leak measurement**. Vendor/OS allocations
outside Rust's allocator are not counted. The `slab` probe eliminated roughly
52,000 requests but added about 0.35 ms preparation, so it was rejected. Reusing
job storage could be separate work; no reuse scheme is hidden in this PR.

## Correctness and lifecycle

Final image validation and full-check results are recorded below.
Actual world-presenter image hashes compare both qualities, replay and authored
rectangle, RTX/Vulkan and AMD/DX12, forced serial fallback, delayed workers and an
offline reconnect at callback 300. Native menu composition runs in these controls;
**egui raster painting is omitted identically in both image arms**. This is not an
independent renderer comparison: both arms share core presentation policy, texture
sources and shader/fragment math. The candidate computes and submits its actual
parallel output. Timing runs include egui and do not hash pixels.

The authored GPU test separately compares serial/parallel world pictures through
Original/Native, a nonempty nonblack prefix (canonical fallback), an overlay,
resize, consumed food, world replacement and explicit device destruction/renderer
drop on both native backends. CPU regressions cover 2/3/4 participant ordered
instance bytes/flags/f64 coverage, sparse admission, first-error order, a claimed
chunk held behind a gate, 20 ms lateness and caught worker panic. Late results own
only numeric inputs and immutable source groups, cannot mutate the displayed
world, and are ignored after cancellation. Drop disconnects every receiver before
joining all workers. Sparse/menu frames create no pool; post-dense workers block
on receives. The final idle controls observed three sleeping workers and zero
added worker CPU ticks, with sparse process CPU essentially unchanged. The initial
thread counter had an HRESULT bug; those observations remain marked invalid in
[idle.csv](idle.csv). Actual four-core hardware and spontaneous device-loss
recovery are unverified; explicit teardown is verified.

No canonical-quad gate is claimed from the timing accumulator. Earlier values of
`d1fba762150c532c` were inactive metadata; the final diagnostic prints `absent`.
Actual image controls and sent-byte digests are separate. [parity.json](parity.json)
retains image identities/counts; [callback-stages.csv](callback-stages.csv) contains
bounded stage-only timelines without world data. No raw capture, copied community
layout or player information is published. Tests remain offline.

## Validation status and remaining work

- Final 9,600/9,600 world-presenter image pairs matched exactly, with zero skipped callbacks.
- Full `npm run check` passed; final focused release checks passed (14 tests; four explicit GPU/density tests ignored by that invocation). Final authored offscreen lifecycle checks were then run explicitly and passed on RTX/Vulkan and AMD/DX12.
- Study recomputation and five study tests pass, including rejection of bad arm identities, short/skipped image histories, wrong configurations and empty/malformed active digests.

Main-thread CPU stages remain about 3.3–3.6 ms at the median; the preparation
critical path is roughly 0.9 ms. This gets the normal median near the owner's
refresh budget but leaves p95 well above it. GPU work is unchanged. No motion,
shader approximation, frame limiter, adapter default, wheel-zoom admission or menu
composition behavior changes. Source-group rebuilds (about 1.7 ms in the slowest
art callbacks here) and ingress bursts are next; an incremental food-planning
cache has **not** been implemented. Real presented intervals, actual owner play,
real small-core hardware and browser GPU presentation are unverified. Wasm remains
serial and is covered by the full build/smoke/Clippy check, not by browser-pixel
acceptance.
