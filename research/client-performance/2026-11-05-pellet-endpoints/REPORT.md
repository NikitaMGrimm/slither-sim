# Exact pellet rectangle endpoints

The timing tables and original controls below describe **e066bc33**, before
#462. They were not re-timed or relabeled after rebasing onto **60bac3ea**.
Fresh matched image/census/canonical controls for the rebased code are in
the rebase-validation section at the end.

<!-- RESULT -->
CPU pellet preparation is faster in this measured workload. At RTX/Vulkan 4K
Native and factor 0.065, median/p95 preparation falls from **1.764/2.223 to
1.567/2.043 ms** (11.2% at the median). Callback median/p95 moves
from 6.241/8.357 to 6.103/8.195 ms. It remains above the 4.17 ms / 240 Hz
budget; individual callback tails do not uniformly improve.
<!-- /RESULT -->

The retained body/glow path previously converted all four corners independently.
It now converts each axis endpoint once and assembles the same six vertices.
The f64 coordinate operations and f32 submission boundary stay in their original
order. Identity-transform signed zeros cannot survive normalization as different
output coordinates. Invalid/overflowing results take the unchanged general
projector, preserving its first error. Continuous floors, area alpha, culling,
source families, coverage accumulation order, shaders and census are unchanged.
The existing worker pool and Wasm serial path use the same helper.

## Profile and rejected attempts

A separate current-main diagnostic at 4K Native/factor 0.065 measured pellet
preparation at **1.829/2.448 ms median/p95**, including **1.630/1.895 ms** of
projection math. Its callback was 6.843/9.376 ms. This exploratory run is excluded
from the matched adoption table. The profile maps the dominant preparation scope
to `pellet_cpu::project`, `glow_pair`, `exact_instance` and their rectangle
conversion; it does not establish that the floor helper alone dominates.

Two source-family geometry-cache pilots were rejected. Copying the large cached
layout did not improve preparation and overlapped compilation. Borrowing it
instead was clean but raised the math median from 1.612 to 1.682 ms, with no
meaningful callback improvement. None of that cache code is delivered. The
endpoint pilot also overlapped compilation; it motivated this comparison but is
not pooled with it. `exploratory-attempts.json` retains bounded summaries; complete
sources, binaries and journals remain in local scratch.

The diagnostic also reproduced an **88.358 ms** cold callback. Its redraw was
82.729 ms and its GPU pass 0.071 ms. Consecutive preparation spans were 15.995 ms
between arena/name completion, 27.236 ms in resource materialization, 20.821 ms
in announcement/minimap append, and 18.200 ms before redraw completion. This
localizes that observed spike to CPU presentation/resource/HUD setup. It is not a
complete allocation or function-level attribution, does not explain the older
196 ms hitch, and supplies no shader-prewarming claim. Cold spikes and join
fade/spinner behavior are not fixed by this change.

## Matched release comparison

<!-- SUMMARY -->
Retained timing attempts: 110; selected: 50 in 25 complete blocks; compiler/link overlap: 44; clean partners excluded with their block: 12; lock-loss exclusions: 4. Selected per-run median machine load: 9.8–96.4% (median 24.4%).
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / view / quality | Callback before ms p50/p95 | After ms p50/p95 | Pellet prepare ms p50/p95, before → after | Paired callback p50 delta range ms |
| --- | ---: | ---: | ---: | ---: |
| igpu / far / native | 11.636/13.908 | 11.113/13.107 | 1.870/2.385 → 1.584/2.047 | -2.709 to -0.162 |
| rtx / far / native | 6.241/8.357 | 6.103/8.195 | 1.764/2.223 → 1.567/2.043 | -0.481 to +0.447 |
| rtx / far / original | 6.583/8.592 | 6.189/8.354 | 1.773/2.240 → 1.507/1.989 | -12.116 to -0.215 |
| rtx / whole / native | 5.765/8.483 | 5.223/7.437 | 1.801/2.481 → 1.524/2.083 | -1.704 to -0.018 |
| rtx / whole / original | 5.899/8.423 | 5.484/8.092 | 1.815/2.273 → 1.539/2.038 | -8.418 to +1.144 |
<!-- /TIMING -->

<!-- SPREAD -->
| Adapter / view / quality | Paired pellet p50 delta range ms | Paired callback p95 delta range ms | Per-run callback p95 range ms, before → after |
| --- | ---: | ---: | ---: |
| igpu / far / native | -0.585 to -0.232 | -3.359 to -0.020 | 13.436–16.893 → 12.504–13.756 |
| rtx / far / native | -0.302 to -0.062 | -0.640 to +6.506 | 8.242–8.585 → 7.918–14.747 |
| rtx / far / original | -1.772 to -0.220 | -30.410 to +0.018 | 8.412–39.016 → 8.217–8.610 |
| rtx / whole / native | -0.563 to -0.180 | -2.817 to +0.056 | 7.381–12.876 → 7.113–11.455 |
| rtx / whole / original | -1.491 to +0.008 | -27.241 to +1.675 | 7.625–39.645 → 7.276–12.404 |
<!-- /SPREAD -->

Slow clean blocks remain selected, including an Original baseline at 96.4%
median machine load. Large callback reductions in those blocks cannot be
attributed entirely to the small projector change. Native/far block 2 has
callback p95 **8.242 → 14.747 ms** and a 53.083 ms maximum art callback in
the endpoint arm. That callback records 13.867 ms ingress, 11.431 ms redraw
and 25.246 ms submit, with a 0.232 ms GPU pass. Across that art window the
pool still contributed 154–229 chunks per callback and only 0–1 late chunks.
The phase record localizes the inflation to CPU scopes; its cause remains
unresolved. This block is retained and is not excluded as noise.

Each table value is the median of five per-run quantiles, not a pooled-frame
percentile. Per-run p95 uses sorted index `floor((n-1)*0.95)`. The art window is
callbacks 120–419, selected before timing: 300 callbacks per run. The paired range
subtracts baseline from endpoint callback medians within each block. Preparation
is nested in submit; phase medians must not be added. `runs.csv` retains phase
median/p95, load and all matched attempts, including exclusions.

Both sides use baseline main **e066bc334e04c7f8159c4c6a4893083fbd284f05**,
Rust 1.99.0 release/thin LTO, two Cargo jobs and no CPU-specific flags. Each arm
was archived and the two client release packages cleaned before building, then
its executable sealed. Every Cargo invocation used per-command
`CARGO_TARGET_DIR=D:/CargoTargets/perf-spikes`, `CARGO_BUILD_JOBS=2`, and
`CARGO_INCREMENTAL=0`. The common local driver build was:

```text
cargo build --offline --release --locked -p slither-wgpu-slice --features frame-profile,frame-bench --example join_profile --example continuous_scene
```

The replay invocation is `<sealed-arm> replay <retained-join-A> <new-directory> 10
<notches>`, with `PROFILE_SIZE=3840x2160`, `PROFILE_MENUS=1`, `PROFILE_PHASES=1`.
Far uses `ln(0.065)/ln(1.12)` notches; whole-map uses -1000. Native sets
`PROFILE_NATIVE=1`; Original leaves it unset. RTX 5070 Ti uses
`PROFILE_ADAPTER=rtx`, `PROFILE_BACKEND=vulkan`; the same PC's AMD integrated GPU
uses `igpu`/`dx12`. Worker/delay overrides, pixel hashing and canonical hashing
are unset for timing. The inactive timing quad accumulator is not parity proof.

The host is Windows with a Ryzen 7 9800X3D. A/B and B/A block order alternates,
including configuration order. Five complete clean blocks are required per cell;
the initial attempt ceiling is twelve. After it was exhausted with four clean
AMD pairs, `PLAN-SUPPLEMENT.json` allowed up to four AMD-only blocks; the second
supplemental block supplied the fifth clean pair. Earlier selections were kept. CPU load and compiler/link/Cargo/test activity are
sampled during each attempt. Observed rustc/link overlap excludes a block,
including its clean partner. No noise-based selection or idle booking is used.
The first series aborted when the shared lock disappeared before its third
block; all four runs in that series are retained as `lock_lost` exclusions,
separately from their original compiler-status observations. Original summaries
remain unchanged in a local backup. The retry holds a read-shared file handle
that prevents deletion/writes for the lease; it changes no ACL or system setting.
Each timing series acquires the shared `TIMING_LOCK` with purpose/UTC start time,
checks its age before each block, binds every selected run to the lease digest
and its start/end age, bounds each owned child to 120 seconds, and
releases the lock in `finally`; the series stops before its 60-minute ceiling.

## Inputs, exact controls and identities

**Restricted timing only:** retained-join-A, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`,
29,044 envelopes and 600 virtual 60 Hz callbacks. No community image is rendered
for validation, saved or published. Timing has no pixel readbacks. Only aggregate
timings and consistency hashes are retained here; raw frames, positions, player
information and the recording remain outside Git.

**Authored validation:** the common driver's 25,000-food protocol-13
rectangle uses repository-generated procedural materials and synthetic packets.
All **8,400/8,400** actual full-frame RGBA hash pairs match, with 600 callbacks,
zero skips, multiple distinct images and identical complete food-census bytes in
every configuration. This covers factor one, 0.065 and whole-map, Native and
Original, RTX/Vulkan and AMD/DX12 at 4K; two additional RTX Native/far controls
force serial submission or 20 ms worker delay. Each arm uses a different sealed
executable. The matrix includes spawn fading and sector removal.

These readbacks use the production `WorldPresenter` and actual retained output.
Native menu composition is driven, while egui raster painting is omitted equally
in the image arms. Timing includes egui. Both arms share textures and fragment
math: this is an exact cross-revision control, not an independent renderer.
Output is 3840x2160; Original retains its capped world raster and production
MSAA, while Native uses physical resolution and production MSAA admission.

Separate active CPU controls match **67,254,126** projected quads, canonical
frame/resource/send digests and 600 callbacks per arm. `canonical-controls.json`
records their nonempty active digests. Native tests compare endpoint bits/errors
to the unchanged general projector and retain the existing packed rectangle,
alpha, source-identity, consumed-food, census, capacity and frozen-retry controls.

`identities.json` binds every run and image arm to its distinct executable and
common driver. It distinguishes runtime files from passive instrumentation in
food advance, phase observers and pellet preparation. The projector file itself
has no timing instrumentation. `sources.json.gz` contains historical authored
Rust source, not recordings: baseline, measured endpoints and delivered source,
plus frozen bases/instrumented files for the four passive changes exposed in
`measurement.patch` (zero context).
The original delivered file (`24f610b2`) differs from the sealed endpoint source only inside its test
module, adding the independent general-projector regression. Recomputing checks
these archived identities and equal production prefixes, without consulting later
working files or requiring historical branch objects after squash merge.

Access to complete journals, source archives, release binaries, launchers and the
local replay driver is **local owner/reviewer scratch only**, under
`D:/slither-scratch/client-lane-20261105/perf/`. All attempts are retained. Replay
and image invocations open no socket or visible window. No restricted art pack
is loaded or published. The full test suite also uses authored loopback servers.

## Validation and limits

The full local `npm run check` passed: Node behavior/tooling tests, core/native
tests, strict native/Wasm Clippy, desktop/Wasm builds and real synthetic Wasm ABI
checks. The new 1,200-case endpoint regression passed against the unchanged
general projector, including exact error results. Seven documentation tests also
passed. Recomputation and all eight selected-data tests pass. The initial fresh-worktree check lacked `ws`;
the locked dependency was installed from the offline npm cache. Its next run
found `wasm-bindgen` absent from PATH; subsequent checking uses the existing
pinned 0.2.129 executable through per-command `WASM_BINDGEN`. Both failed logs
are retained, without unrelated source fixes.

This is a CPU preparation optimization, not a 240 Hz, physical pacing,
owner-play or browser GPU acceptance claim. The iGPU is a guardrail on this same
host, not a measurement of a weaker CPU or a four-core PC. GPU work, cold resource
setup, join fade/spinner and other callback costs remain.

Recompute the selected table and validate the retained controls without the
recording or GPU:

```text
uv run --offline --no-project python -B research/client-performance/2026-11-05-pellet-endpoints/recompute.py --check
uv run --offline --no-project python -B -m unittest discover -s research/client-performance/2026-11-05-pellet-endpoints -p test_study.py
```


## #462 rebase validation

The implementation was rebased cleanly onto main
`60bac3eae6967afc0f62325ad2f48d034f950159`, which changes HUD visibility and
picture issue/retry. Range-diff kept the endpoint optimization unchanged. The old
head remains in a local backup branch; earlier evidence files remain byte-identical.

Fresh release arms use #462 as the baseline and source epoch
`7322fd1676d1cab059c2412fcfffd2ecb2eb90a6` as the endpoint candidate. Subsequent edits add
selected receipts/checks/documentation only; runtime source hashes still match.
The common private driver now calls `take_frame_with_menu` with NativeLogin's
`needs_menu_pass` observation in both arms, instead of the old menu-unaware
`take_frame` call. The small adapter change is retained in `rebase-driver.patch`.
The same two-job release build, D: target directory and authored 25,000-food
rectangle were used. These runs are validation only; their elapsed-time journals
are excluded from timing claims. No restricted community recording was replayed.

All **8,400/8,400 fresh image pairs** and complete prepared-food census bytes
match exactly, with zero skipped callbacks: factors one, 0.065 and whole-map,
Original/Native, RTX/Vulkan and AMD/DX12 at 4K, plus forced-serial and 20 ms
late-worker RTX Native/far controls. Every configuration has 600 unique callback
indices and multiple distinct actual full-frame hashes. Retained submission is
active in **346 callbacks** per arm/configuration; menus are
active in **104 callbacks**. This guards against a blanket-hide/empty-picture
comparison. Egui raster painting remains omitted equally in image arms.

Fresh, executable-bound active canonical controls also match across 600 callbacks
per arm and **67,254,126 projected quads**. These are freshly executed #462
receipts; this fixture's count matches the historical control, while the records
and executable bindings remain separate. The unchanged general
projector remains the endpoint bit/error oracle. Full local `npm run check`
passed again on the rebased runtime, including #462's picture-issue/menu/retry
tests, native/Wasm Clippy, desktop/Wasm builds and synthetic ABI checks.

`rebase-controls.json` records distinct executable identities, a common updated
driver, the complete image matrix, active canonical launch/log receipts and
hashes of the six unchanged historical evidence files. `rebase-sources.json.gz`
freezes the selected caller/projector and instrumented sources. The checker
validates these archives without later working-file or old-Git-object gates.
Four additional study regressions reject incomplete rebased image matrices,
reuse of an old executable, source corruption and timing relabeling.

This refresh establishes exact output controls on the new base. It establishes
no new performance, browser presentation or physical pacing result for #462.
