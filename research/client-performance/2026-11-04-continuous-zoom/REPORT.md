# Continuous zoom without subpixel omission

The owner approved the continuous-zoom look on 2026-11-04. Bodies and glows
remain independently drawable at small scales instead of disappearing together.
This improves the authored threshold probes, but keeping more food still costs
CPU. The small arithmetic trim has no meaningful measured wall-time benefit;
all its paired ranges cross zero. The selected art frames remain above the
4.17 ms budget of 240 Hz. This study makes no presented-FPS acceptance claim.

## Rules and scope

Each food image uses its own extent, a one-displayed-physical-pixel floor and
true/drawn area alpha. The floor blends from exact stock at factor one to full
strength at 0.9. Snake decorations fade across 1-3 logical pixels and body shadows
across 2-6 after all geometry visits; names shrink a retained stock glyph raster
and fade below two physical pixels. These are authored, owner-approved look
choices, not inferred server mechanics. Motion, camera clocks and packets are
unchanged. No configuration switch remains.

The retained catalog/source classifications are reused. Subpixel eligibility
and its redundant scans are gone. The small CPU trim inlines the shared floor
helper and skips rectangle rewrites/ratios when an image already exceeds its
floor. Its frozen scalar oracle compares exact rectangle and gain bits for
settled, zero-scale and fading inputs. A larger projection/cache change is left
for separate work; these results do not establish that such a change would win.

The existing benchmark field `pelletsBeforeOmission` keeps its name for historical
receipts. It now counts the same in-bounds food as `submittedPellets` in continuous
zoom. Every selected non-main art callback has zero food omissions. In the measured composition, food budget selection was inactive below factor
one; the review amendment below corrects that admission discontinuity. Native
and generic planner tests cover the counts and retained-picture retry.

## Inputs, identities and access

- **Restricted community input:** retained-join-A, SHA-256
  `a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`,
  29,044 envelopes over 600 virtual callbacks / ten seconds. Only aggregate
  metrics and hashes are retained here. No address, names, positions or raw
  recording are published.
- **Authored stress input:** authored-wide-65535-v1, SHA-256
  `92a883e3814aa6c5738a09b5531d0396e79cdfbcdc517362cd4083af58f13860`,
  19 envelopes, 65,535 distinct food positions and 65 snakes.
- **Authored gallery:** 120 food in six size families and three named synthetic
  snakes, repository-generated procedural materials. No imported/official art.
  Animation is frozen and each zoom step starts from the same cloned world.

Access to the complete inputs, per-callback journals, load/process samples,
release binaries, offline replay driver and build journal is **restricted local
owner/reviewer scratch**. They remain outside Git at their original locations.
The driver never connects in these replay invocations. No new game connection
or visible window was used. The private driver itself was not independently
reviewed as part of this implementation.

`identities.json` records runtime revision, executable, common driver and
instrumented-source hashes for each arm. Main is `1e849de9`; the ungated approved
look before the trim is `ea2b1dde`; the measured trimmed source is `1aef092d`.
The initially published runtime at `7cca3d82` differed from that measured source
only in the shorthand of one `pixels_per_unit` field initializer, with no
arithmetic/control-flow change. The review fixes below are later source changes;
the timing tables continue to describe the original measured composition.
Both full-file identities are recorded honestly. `source-only-delta.patch` and
`food-painter-measured.rs.gz` retain that link; the test applies the actual patch
and verifies both hashes without old Git objects or current-working-file gates.
The compressed file is historical Rust source, not recording data. Later report,
test and documentation commits do not relabel the measured source.

## Release method

The private build journal archived each revision, added the same passive stage
counters and built sequentially after cleaning the client packages. Each Cargo
command used `CARGO_TARGET_DIR=D:/CargoTargets/community-lag`,
`CARGO_BUILD_JOBS=2` and no per-CPU flags. The build invocation was:

```text
cargo build --release --locked --manifest-path client/apps/wgpu-slice/Cargo.toml --features frame-profile,frame-bench --example join_profile --example continuous_scene
```

The examples require the retained restricted driver to be installed in the
archived source tree. The exact replay command shape was:

```text
<arm-binary> replay <retained-join-A input> <new output directory> 10 -24.118942187336824
<arm-binary> replay <retained-join-A input> <new output directory> 10 -1000
```

The first is factor 0.065, the second whole-map. Launches set
`PROFILE_SIZE=3840x2160`, `PROFILE_ADAPTER=rtx`, `PROFILE_BACKEND=vulkan`,
`PROFILE_MENUS=1`, `PROFILE_PHASES=1`; Native additionally sets `PROFILE_NATIVE=1`
(Original leaves it unset). The guardrail uses `PROFILE_ADAPTER=igpu` and
`PROFILE_BACKEND=dx12`. Worker/delay overrides were unset. Pixel hashing and
canonical parity were off during timing. The inactive canonical digest prints
as absent and is not used as evidence.

RTX means the RTX 5070 Ti / Vulkan primary; igpu means AMD integrated graphics /
DX12 guardrail. Both are offscreen 4K. Main/look/trimmed triples alternate order
by round for both quality modes and both views; the AMD guardrail alternates
look/trimmed pairs at factor 0.065 in Native. All attempts are retained. Any
observed rustc/link overlap excludes the entire corresponding matched block;
clean partner samples stay in the CSV with that reason. Cargo/test activity and
machine load are recorded; no quiet window or idle booking was used.

Each run processes all 600 callbacks with zero skips. The art window is callbacks
120-419 (2-7 virtual seconds). Each cell below is the median across five complete
clean blocks of that run's art median/p95. Per-run p95 uses sorted index
`floor((n-1)*0.95)`. Pair ranges retain the end-to-end trim's observed spread.
`elapsed_s` covers the whole process; allocation requests cover callbacks with a
process-wide counter, including workers, and are not live/peak-heap measurements.
Both remain per-run fields rather than separate performance claims.

<!-- SUMMARY -->
Retained attempts: 189; compile-overlapped: 100; clean partners excluded with their block: 19; selected samples: 70; complete selected blocks: 25. Selected sample median machine load: 7.0-30.1%.
<!-- /SUMMARY -->

## Timing and remaining cost

<!-- TIMING -->
| Adapter / view / quality | Main ms p50/p95 | Approved look ms p50/p95 | Trimmed ms p50/p95 | Trim p50 paired range ms |
| --- | ---: | ---: | ---: | ---: |
| igpu / far / native | n/a | 11.211/13.352 | 11.196/13.138 | -0.106 to +0.046 |
| rtx / far / native | 4.382/6.429 | 6.454/8.889 | 6.476/8.818 | -0.483 to +0.667 |
| rtx / far / original | 4.536/6.652 | 6.457/8.799 | 6.449/8.600 | -0.207 to +0.207 |
| rtx / whole / native | 3.173/4.727 | 5.302/7.554 | 5.311/7.415 | -0.156 to +0.066 |
| rtx / whole / original | 3.286/4.718 | 5.441/7.613 | 5.464/7.660 | -0.377 to +0.101 |
<!-- /TIMING -->

CPU pellet preparation sits inside CPU submit; food advance sits inside redraw.
Stage medians are not additive. GPU figures come from per-pass diagnostic
queries across world, any glow targets, blit and overlay. A missing separate
glow target is not a claim that glow drawing itself costs zero.

<!-- STAGES -->
| Adapter / view / quality | Pellet prepare: main / look / trimmed ms | Trimmed redraw / submit / GPU ms |
| --- | ---: | ---: |
| igpu / far / native | n/a / 1.866 / 1.873 | 2.069 / 3.815 / 3.099 |
| rtx / far / native | 0.228 / 1.846 / 1.834 | 2.080 / 3.527 / 0.237 |
| rtx / far / original | 0.234 / 1.771 / 1.778 | 2.071 / 3.508 / 0.259 |
| rtx / whole / native | 0.000 / 1.780 / 1.761 | 2.032 / 2.604 / 0.109 |
| rtx / whole / original | 0.000 / 1.791 / 1.767 | 2.027 / 2.660 / 0.185 |
<!-- /STAGES -->

Most additional far-view cost remains in per-food CPU math/preparation and
submission, rather than GPU capacity on the primary adapter. The extra food
work is intentional: main omits food that the new view retains. Source-family
classification reuse does not remove that dynamic work. The small floor trim
is within run-to-run variation; the AMD guardrail also shows no material change.
No 240 Hz, 170 Hz or 60 FPS acceptance is inferred from these synchronized,
offscreen callbacks. Real display cadence is a separate measurement.

## Image and pop controls

`image-controls.json` retains eight exact image-control configurations. Main vs
trimmed at factor one covers both qualities on RTX/Vulkan and AMD/DX12 at 4K;
approved look vs trimmed covers factor 0.065 and whole-map in both qualities on
RTX. Each configuration has 600 full-frame RGBA hash pairs, zero skips and more
than one actual frame hash. The two arms use different executables. These are
actual GPU readbacks rendered by the same wgpu renderer implementation, with
shared textures and fragment math; this is not an independent canonical renderer.
Native menu composition is driven, but variable egui painting is omitted in
both image arms. Timing includes egui. Hash equality establishes zero pixel
byte difference in these controls, including frames with fading/consumed food.

The slow gallery sweep uses 240 logarithmic intervals plus explicit threshold
probes, settling the camera at each factor. `pop.csv` retains the adjacent
microsteps straddling former cutoffs: each actual-factor difference is positive
and below 1e-6. The fixed ROI is x=1280..2559, y=600..1559 of 3840x2160.
RGB-byte sum is a light proxy, not linear radiometric energy. Coarse zoom steps
move edges normally; these finite central-crop probes do not prove universal
absence of pops, nor do they cover picture rejection at the hard budget.

<!-- POP -->
| Authored quality / arm | Microsteps | Max adjacent RGB byte | Max crop RGB-sum jump | Max relative jump % |
| --- | ---: | ---: | ---: | ---: |
| native / base | 12 | 255 | 888266 | 31.9597960 |
| native / candidate | 12 | 2 | 27 | 0.0003794 |
| original / base | 12 | 239 | 884790 | 30.2521715 |
| original / candidate | 12 | 1 | 15 | 0.0006569 |
<!-- /POP -->

The PR image is a matched, unretimed static gallery crop at factor
0.03758374042884443. It uses only authored objects and generated materials;
no community screenshot or private art pack is included. It is not physical
wheel/pacing evidence. Original filters a capped world raster; Native uses the
physical target, so their zoomed filtering can differ.

## Complete picture budget

<!-- CAPACITY -->
| Authored quality | Callbacks / skips | Visible food | Maximum picture charge / limit |
| --- | ---: | ---: | ---: |
| native | 600 / 0 | 65535 | 238567 / 262144 |
| original | 600 / 0 | 65535 | 238567 / 262144 |
<!-- /CAPACITY -->

These are fresh shape/budget controls, not timing comparisons (their runs
had compile overlap). Compact zoom-out uses its existing three paired quads per
food: 196,605 for this population, before other geometry. This fixture maximum
is not a universal upper bound. Generic/canonical expansion needs up to seven
commands per food (458,745), already over the 262,144-picture limit before snakes.
A non-admitted retained recipe can require that fallback. World preparation
rejects the complete picture; native release skips it and leaves the previous
presentation displayed. A backdrop also requires canonical expansion during menu
composition: on its draw quota failure, `compose_optional` skips the menu overlay
and the host submits the original world picture instead. Other geometry can also
consume the compact budget. No partial food selection or renewed size cutoff
hides the limit. Authored regressions check
retention, complete rejection, draw ordering and frozen retry inputs.

## Validation and limits

The final sequential `npm --prefix client run check` passed: core/native tests,
strict native and Wasm Clippy, desktop/Wasm builds and real synthetic Wasm ABI
checks. Focused floor/fade/body, census and quad-budget tests passed. The study
checks below are run locally; they are not an added CI game-server test.

Retained failed attempts: a redundant initializer lint was fixed; the unchanged
asynchronous startup/cancellation test failed and independently reproduced once
in ten runs on pristine main. It passed in the final sequential full run. A
concurrent cross-root debug check once reused a stale core artifact; that check
was invalidated, client packages were cleaned and checks rerun sequentially.
Release builds used sequential package cleans throughout; their frozen sources
and executables were not overwritten. No unrelated startup runtime fix is in
this change.

Physical wheel feel, real presented intervals, browser/WebGL pixels, 1080p
performance on this new composition and every possible density/backdrop are
unverified. The owner approved the local look; that is separate from a frame
budget or smoothness acceptance. No game-server connection is made by tests/CI.

Recompute every retained table/count and check the report from bounded data:

```text
python -B research/client-performance/2026-11-04-continuous-zoom/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-04-continuous-zoom -p test_study.py
```

The tests reject wrong arm identities, short/skipped/inactive image controls,
wrong configuration keys, malformed digests and false admission. Source identity
checks consume the archived source/patch, never a later current runtime file.
