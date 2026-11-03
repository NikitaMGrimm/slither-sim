# Retained pellets through wheel zoom — 2026-11-03

**Access: public bounded summaries; restricted local input and full journals.**
The recording, per-frame world data and pictures remain local and are not part of
this study bundle. No new server connection was made.

Dense food at non-default wheel zoom fell back to generic per-image records.
The retained path now freezes the exact user zoom with the queued picture and
uses the shared core's existing eligibility and image-projection policy.
There is no shader, animation, smoothing, effect, quality or LOD change.

## Composition and input

The final comparison starts at main `140a373a0174654a0ecdafdfeb5c22e137b2102f`,
including source-group reuse (#456) and death-return menu composition (#455).
The candidate runtime is `f1baee46257f0c654eca350e7a27aa123504be36`;
subsequent study-only commits leave it unchanged. This is based directly on main,
not stacked on an unmerged PR.

Neutral local input **retained-join-A**, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`,
contains 29,044 envelopes. Every run processes 600 virtual 60 Hz callbacks over
ten seconds, with no skips. The art window is callbacks 120–419 (2–7 s).
Default, −1 notch, −4 notches and a clamped whole-map request (−1000 notches)
use the same input. The whole-map camera settles near zoom 0.0147.

Both arms are release builds with the same private replay driver and passive
instrumentation. [identities.json](identities.json) records each binary, driver,
runtime source and instrumented source identity separately.
[measurement-delta.patch](measurement-delta.patch) retains the instrumentation
delta (zero context; apply with `git apply --unidiff-zero`). It does not alter
production expressions. Historical identities are not current-file hash gates.

## Method and reproduction

The maintained local-only replay driver and launchers live with the restricted
samples in the task's external scratch archive. Access requires the owner's
local archive; no identifying path is published. The build was performed from
separate archived source trees, sequentially with a package clean before each
arm to avoid shared-target artifact reuse:

```powershell
$env:CARGO_TARGET_DIR='D:/CargoTargets/community-lag'
$env:CARGO_BUILD_JOBS='2'
$env:CARGO_INCREMENTAL='0'
cargo clean --release --manifest-path client/apps/wgpu-slice/Cargo.toml -p slither-client -p slither-wgpu-slice
cargo build --release --locked --manifest-path client/apps/wgpu-slice/Cargo.toml --features frame-profile --example join_profile
```

From the restricted launch directory, the exact alternating timing invocation was:

```powershell
python -B zoom_current_timing.py *> zoom-current-timing.log
```

For each arm it runs the following command, with `PROFILE_SIZE=3840x2160`,
`PROFILE_ADAPTER=rtx`, `PROFILE_BACKEND=vulkan`, `PROFILE_PHASES=1` and
`PROFILE_MENUS=1` (Native adds `PROFILE_NATIVE=1`; Original leaves it unset):

```text
<arm-binary> replay <retained-join-A> <fresh-output-directory> 10 <wheel-notches>
```

The AMD guardrails select `PROFILE_ADAPTER=igpu`, `PROFILE_BACKEND=dx12` at
1920×1080 and 3840×2160. Both arms have the same settings. The launcher clears
all unrelated `PROFILE_*` flags, including `PROFILE_PARITY`. Inactive canonical
digests print **absent** and are not evidence. Separate explicitly active
controls are described below.

Arm order alternates within pairs; quality and notch order also reverse between
rounds. Five accepted pairs per RTX configuration and three per AMD guardrail
were required. Every attempt and its process/load journal remains local. Only
observed rustc/link overlap excludes a sample; a pair is selected only when both
arms are accepted. Cargo/test activity and system CPU load are recorded, not
used to discard otherwise successful samples. [runs.csv](runs.csv) retains one
row per attempted sample, selection and reason, binary identity, callback
quantiles, process-wide Rust requested allocation bytes and load ranges.

`wall_ns` spans the callback's poll, ingress, redraw, native menu composition,
resource sync, submission and GPU completion waits. Native egui preparation,
painting and its wait are included. Runs are unpaced, offscreen and synchronized;
these are **not presented frame intervals or FPS**. Wall time includes the
per-callback timestamp resolve/readback, so it is not an uninstrumented host
frame time. End-of-callback diagnostic
world scans and CSV writes occur outside `wall_ns` but affect elapsed time.
CPU scopes sum poll/ingress/redraw/sync/submit, excluding menu/egui and explicit
GPU waits. The offscreen output format is RGBA8Unorm, not a swapchain.
Food plan is nested in redraw; pellet preparation is nested in submit.
Stage medians do not add to the whole-frame median. GPU timestamps use per-pass
mode (`begin_diagnostic_frame`) and include the glow passes; CPU egui wait is in
wall time, while its GPU work is outside the recorded world-pass total.

Per-run p95 uses sorted index `floor((n-1)*0.95)`; tables show the median of
per-run quantiles, not pooled frames. Pair ranges preserve background drift.
Process allocations count Rust requests across all threads from launch to exit,
including driver/diagnostic overhead; they do not measure driver/native heap
allocations or live/peak memory.

Recompute all tables without the recording, GPU, historical Git objects or a build:

```text
python -B research/client-performance/2026-11-03-retained-pellet-zoom/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-03-retained-pellet-zoom -p test_study.py
```

## Results on the final composition

| Adapter / size / notches / quality | Pairs | Base art median/p95 ms | Retained art median/p95 ms | Paired p95 reduction median [range] ms |
| --- | ---: | ---: | ---: | ---: |
| rtx / 3840x2160 / -1 / original | 5 | 24.095/27.686 | 4.138/6.139 | +20.925 [+19.951, +27.541] |
| rtx / 3840x2160 / -1 / native | 5 | 23.555/27.570 | 3.868/5.816 | +21.594 [+20.656, +23.578] |
| rtx / 3840x2160 / -4 / original | 5 | 27.142/32.165 | 4.878/7.179 | +24.985 [+22.615, +34.116] |
| rtx / 3840x2160 / -4 / native | 5 | 27.716/32.080 | 4.498/6.369 | +25.791 [+23.255, +35.163] |
| rtx / 3840x2160 / -1000 / original | 5 | 3.067/4.614 | 3.251/4.872 | -0.290 [-0.437, -0.104] |
| rtx / 3840x2160 / -1000 / native | 5 | 3.109/4.725 | 3.116/4.632 | +0.007 [-0.131, +0.447] |
| rtx / 3840x2160 / 0 / original | 5 | 4.108/6.110 | 4.168/6.435 | -0.069 [-0.822, +0.533] |
| rtx / 3840x2160 / 0 / native | 5 | 4.436/6.301 | 4.145/5.932 | -0.042 [-0.956, +0.572] |
| igpu / 1920x1080 / -4 / original | 3 | 34.804/37.261 | 10.902/12.321 | +25.020 [+24.904, +31.006] |
| igpu / 1920x1080 / -4 / native | 3 | 35.345/38.118 | 11.487/13.168 | +24.886 [+24.185, +25.403] |
| igpu / 3840x2160 / -4 / original | 3 | 35.227/38.194 | 11.101/12.907 | +25.262 [+24.616, +26.203] |
| igpu / 3840x2160 / -4 / native | 3 | 38.256/41.539 | 13.968/15.405 | +26.246 [+25.407, +28.924] |

| RTX 4K / notches / quality / arm | Food plan ms | Pellet prepare ms | All submit ms | CPU scopes ms | GPU / glow ms | Whole median/p95 ms | Process Rust GB | Load median [range] % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| -1 / original / base | 5.257 | 0.000 | 10.619 | 23.492 | 0.157/0.063 | 18.163/26.758 | 9.680 | 11.0 [2.2, 30.2] |
| -1 / original / candidate | 0.065 | 1.013 | 1.756 | 3.764 | 0.237/0.121 | 3.837/6.485 | 10.036 | 14.7 [4.8, 54.4] |
| -1 / native / base | 5.186 | 0.000 | 10.302 | 22.840 | 0.223/0.094 | 17.848/26.241 | 9.666 | 13.3 [6.1, 43.4] |
| -1 / native / candidate | 0.064 | 0.913 | 1.416 | 3.436 | 0.233/0.096 | 3.637/6.419 | 9.954 | 12.4 [6.2, 32.7] |
| -4 / original / base | 5.568 | 0.000 | 12.745 | 26.462 | 0.166/0.060 | 16.417/30.572 | 10.093 | 11.8 [6.2, 44.1] |
| -4 / original / candidate | 0.072 | 1.191 | 2.234 | 4.439 | 0.262/0.125 | 4.504/7.529 | 10.493 | 17.0 [6.9, 43.0] |
| -4 / native / base | 5.733 | 0.000 | 12.512 | 26.883 | 0.249/0.086 | 16.157/31.266 | 10.084 | 13.1 [7.1, 42.3] |
| -4 / native / candidate | 0.067 | 1.114 | 1.896 | 4.034 | 0.287/0.109 | 4.199/6.924 | 10.433 | 14.8 [7.2, 33.5] |
| -1000 / original / base | 0.523 | 0.000 | 0.304 | 2.675 | 0.050/0.000 | 2.939/4.653 | 7.735 | 14.1 [5.5, 30.1] |
| -1000 / original / candidate | 0.615 | 0.000 | 0.306 | 2.878 | 0.050/0.000 | 3.116/4.822 | 7.735 | 12.9 [5.0, 40.4] |
| -1000 / native / base | 0.538 | 0.000 | 0.257 | 2.772 | 0.012/0.000 | 2.959/4.853 | 7.728 | 13.5 [6.9, 48.9] |
| -1000 / native / candidate | 0.606 | 0.000 | 0.251 | 2.762 | 0.012/0.000 | 2.955/4.760 | 7.728 | 13.4 [7.0, 46.7] |
| 0 / original / base | 0.041 | 1.028 | 1.718 | 3.748 | 0.229/0.116 | 3.850/7.872 | 12.182 | 14.0 [6.4, 44.9] |
| 0 / original / candidate | 0.044 | 1.058 | 1.734 | 3.799 | 0.226/0.116 | 3.856/8.392 | 12.182 | 16.4 [2.7, 48.7] |
| 0 / native / base | 0.051 | 1.053 | 1.585 | 3.967 | 0.244/0.105 | 3.882/8.076 | 12.095 | 13.8 [7.9, 48.9] |
| 0 / native / candidate | 0.045 | 1.072 | 1.559 | 3.700 | 0.244/0.105 | 3.814/8.132 | 12.095 | 15.5 [2.7, 42.8] |

| AMD / size / quality (-4) | Base CPU / GPU ms | Retained CPU / GPU ms | Base / retained process Rust GB |
| --- | ---: | ---: | ---: |
| 1920x1080 / original | 26.879/4.076 | 4.333/4.789 | 10.092/10.492 |
| 1920x1080 / native | 26.579/5.089 | 4.195/5.735 | 10.083/10.475 |
| 3840x2160 / original | 27.090/4.623 | 4.274/5.373 | 10.092/10.492 |
| 3840x2160 / native | 27.104/7.571 | 4.173/8.025 | 10.083/10.432 |

| Candidate RTX 4K / notches / quality, slowest 16 art callbacks | Wall ms | Ingress ms | Food advance / clone ms | Source rebuild ms | Pellet prepare ms | Submit ms | GPU ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| -1 / original | 6.514 | 1.571 | 1.211/0.588 | 0.409 | 1.323 | 2.169 | 0.235 |
| -1 / native | 6.100 | 1.595 | 1.213/0.587 | 0.397 | 1.346 | 1.794 | 0.210 |
| -4 / original | 7.441 | 1.622 | 1.265/0.662 | 0.413 | 1.603 | 2.878 | 0.265 |
| -4 / native | 6.780 | 1.440 | 1.287/0.662 | 0.392 | 1.578 | 2.343 | 0.286 |
| -1000 / original | 5.277 | 1.692 | 1.243/0.631 | 0.000 | 0.000 | 0.364 | 0.050 |
| -1000 / native | 4.936 | 1.608 | 1.210/0.586 | 0.000 | 0.000 | 0.273 | 0.012 |
| 0 / original | 7.043 | 1.550 | 1.389/0.683 | 0.424 | 1.474 | 2.223 | 0.225 |
| 0 / native | 6.394 | 1.633 | 1.227/0.605 | 0.425 | 1.393 | 1.877 | 0.230 |

The final set contains 104 accepted samples (52 complete pairs), with no
rustc/link exclusions. At -4 Original, median callback cost falls 82.0%
(27.142 -> 4.878 ms); p95 falls 77.7% (32.165 -> 7.179 ms). Food planning
falls 5.568 -> 0.072 ms and submission 12.745 -> 2.234 ms. The improvement
comes from avoiding generic pellet records, their repeated resource discovery,
validation and packing; there is no comparable GPU reduction. At this view,
CPU scopes fall 26.462 -> 4.439 ms while GPU rises 0.166 -> 0.262 ms.
Afterward about 1.2 ms is compact pellet preparation (inside submit), with
food advancement and other presentation/menu/diagnostic work remaining.

There is **no default-view performance claim**: paired default p95 differences
straddle zero. The empty whole-map Original control has a small regression:
paired p95 is 0.290 ms worse (range 0.104-0.437 ms worse). Its extra eligibility
scan raises food planning 0.523 -> 0.615 ms; it prevents the much larger rejected
source-rebuild spikes. Native whole-map differences straddle zero. This known
cost is retained rather than hidden by pooling views.

AMD total callback cost improves despite GPU increases of 0.45-0.75 ms. The
retained path also requests more process-wide Rust bytes in the zoomed views
(about 0.29-0.40 GB over the ten-second diagnostic run); this is a CPU win,
not an allocation reduction. Default/empty-LOD controls have effectively the
same requested bytes between arms.

The final table takes the median of each run's **slowest 16 art callbacks**,
then the median across runs. At -4, ingress is about 1.4-1.6 ms, food advance
1.3 ms (including 0.66 ms cloning), pellet preparation 1.6 ms (inside 2.3-2.9 ms
submit), and source rebuilding about 0.4 ms. These scopes overlap and their
medians must not be added. GPU remains about 0.27 ms. Those CPU bursts and
remaining presentation/menu/diagnostic work leave p95 above the 4.17 ms target.

## Picture and behavior controls

[parity.json](parity.json) binds both image arms to the final binaries and all
12 expected configurations: RTX 4K default/−1/−4/whole-map in both qualities,
plus AMD 1080p/4K −4 in both qualities. Each has 600 callbacks and zero skips.
The 4,800 RTX pairs and 600 AMD 4K Native pairs are byte-identical. The other
1,800 AMD frame controls differ in image hashes; rerendering the candidate
against its generic canonical submission path bounds the maximum RGB difference
to **1 byte**, within the existing 2-byte paired-addition allowance. This is
rounding-bounded parity, not byte identity on those AMD configurations.

These comparisons render the actual retained submission and generic canonical
submission; the latter reconstructs the original image stream via
`Batch::canonical`. They share core policy, textures and fragment math and are
not independent renderers. Image controls omit egui identically in both arms;
native menu composition is still driven. Timing includes egui as described above.
No private community pictures or layouts are published.

[canonical-controls.json](canonical-controls.json) separately enables
`PROFILE_PARITY=1` at −4 notches, at 1080p and 4K. Both distinct binaries process
600 callbacks / 29,044 envelopes / zero skips and **48,556,857 actual quads** per
run. Vertex, canonical-stream and sent-command digests match. These controls
exercise canonical reconstruction, not instanced GPU order; actual-image controls
and the authored projection oracle cover that separately.

The authored synthetic 25,000-pellet rectangle covers zoom factors 1, 1/1.12,
1/1.12^4, 0.05 and 1.25, exact packed rectangle/alpha bits, canonical draw order,
logical seven-command budgets and insertion barriers. Additional regressions
cover fully omitted food, unused missing resources, frozen queued retries and
membership changes. [mutation-checks.json](mutation-checks.json) records intended
assertion failures after restoring stock-only admission, ignoring sprite zoom,
and removing the empty-LOD guard. Original source bytes were restored and the
focused tests passed again.

`npm --prefix client run check` passed, including strict native/Wasm Clippy,
native tests/builds, Wasm builds, authored Canvas ABI checks and repository guard.
The browser's shared implementation compiles; browser WebGPU/WebGL runtime and
owner play/presented cadence were not exercised. Tests never connect outside.

## Earlier diagnostics, rejected pilot and summary repair

[initial-profile.csv](initial-profile.csv) contains 16 earlier-main diagnostic
samples (runtime pin in `identities.json`). All overlapped compilation, so these
only motivated stage attribution and are not adoption evidence. Later pre-rebase
pairs and image controls remain local and are not pooled into the final tables.
All headline numbers above were repeated on main including #455 and #456.

An initial zoom pilot unnecessarily rebuilt source groups while every pellet
was omitted at whole-map LOD, worsening tail time. The candidate now detects
that empty picture before grouping and keeps the generic validation path.
The rejected pilot and all journals remain local.

**2026-11-03 summary correction:** the first private summary launcher emitted
optional stage/allocation/whole-window columns in arm-dependent order without
binding them to its CSV header. [summary-correction.json](summary-correction.json)
records the affected columns. Original summaries were preserved; callback,
phase, allocation and load journals were intact. The launcher now zero-fills a
common schema and binds rows to header names. The public summaries here are
reconstructed from original journals, not the malformed CSV tails.

## Limits and remaining work

The whole-map input puts every art pellet below the **existing** subpixel LOD
cutoff. It therefore tests the empty-LOD fallback, not a whole-map scene with
many drawable pellets. The synthetic cases cover broader factors and admission
boundaries, but cannot establish whole-map crowded-server performance generally.

The retained path still declines unsupported recipes, quality blends, overload
pressure or a nonempty prefix. Generic planning remains available. Join fade and
spinner/menu fallback behavior was left unchanged. No new join/fade/spinner work
was started for this task. High-refresh p95 remains above 4.17 ms; this study
makes no 240 Hz, smoothness, browser or owner-play acceptance claim.
