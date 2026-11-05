# Exact menu crossfade preparation

## Result

<!-- RESULT -->
RTX/Vulkan at factor 0.065, Native 4K: tail callback p95 falls from 58.8 to 18.3 ms (medians across 9 accepted pairs). After the first two cold callbacks, tail p95 is 17.5 ms; ordinary gameplay p95 is 9.6 ms. The long expansion plateau is removed; the tail still has measurable submission overhead.
<!-- /RESULT -->

The join crossfade and spinner tail previously expanded retained food into
canonical image commands, copied those into generic draws, then projected and
packed them on the callback thread. This study measures an exact replacement:
keep the retained inputs through composition and prepare the same canonical
vertices directly. The page remains below the food outlines and the cover
remains above the world, below the independently fading HUD.

Native pictures with at least 2,048 visible foods use up to four participants,
including the caller, for independent projection and vertex stores. The caller
merges each coverage contribution in the original primitive order. Workers fill
the renderer's retained vertex chunks directly, avoiding intermediate packed
vertex arrays and their copies. Chunk claims balance the different outline,
body and glow costs; immutable source/texture lookup is shared per family.
The existing outline and glow admission, chunk boundaries, per-chunk sorting, shaders,
texture sampling and GPU blend operations remain unchanged. Wasm and smaller
pictures use the serial stream. Coincident surrounding additive draws that
could pair across a food barrier also keep that stream.

Large native command ranges record one render bundle per original vertex
chunk, using at most four threads. Encoders are created and finished on their
own threads; the caller executes completed bundles in the original chunk order.
Direct passes and bundles use the same command emitter. Small ranges and Wasm
retain direct serial recording.

The spinner rasterizer indexes path edges by the rows they can cross. Each
scanline still visits those edges in path order, using the original crossing
test, intersection arithmetic, stable sort, coverage and blend operations.
It avoids walking the entire ribbon path on every scanline.

The full seven-command admission charge remains, including at far zoom. Failed
menu admission preserves the original issued picture. No food selection or
visual approximation is introduced.

<!-- SUMMARY -->
Runs: 138; pairs: 69; pairs excluded for observed compiler overlap: 30. Accepted-pair run median machine load: 13.2-44.7%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / view / quality | Pairs | Tail callback p50 before → after (ms) | Tail callback p95 before → after (ms) | Post-cold tail p95 before → after (ms) | Gameplay callback p95 before → after (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| igpu / far / native | 8 | 38.7 [35.2, 49.4] → 14.9 [14.4, 16.4] | 66.1 [63.7, 84.4] → 23.1 [22.2, 26.0] | 67.9 [65.7, 89.8] → 23.1 [22.2, 26.0] | 14.3 [13.6, 20.6] → 14.3 [13.9, 15.7] |
| rtx / far / native | 9 | 30.8 [30.0, 36.1] → 9.5 [9.1, 10.2] | 58.8 [54.9, 65.1] → 18.3 [17.8, 19.4] | 64.6 [57.0, 70.7] → 17.5 [16.7, 18.0] | 9.8 [9.3, 11.2] → 9.6 [8.8, 10.6] |
| rtx / far / original | 9 | 30.9 [30.1, 32.6] → 9.5 [9.1, 10.5] | 58.4 [55.3, 64.2] → 18.3 [16.9, 18.9] | 59.5 [56.7, 73.6] → 17.5 [16.3, 18.9] | 9.9 [9.7, 11.2] → 9.3 [8.8, 13.9] |
| rtx / stock / native | 6 | 15.3 [14.9, 17.4] → 5.7 [5.6, 6.2] | 43.0 [40.3, 49.1] → 14.2 [13.1, 15.2] | 43.6 [40.5, 49.7] → 13.3 [12.8, 13.9] | 7.2 [6.4, 7.6] → 6.7 [6.1, 7.1] |
| rtx / whole / native | 7 | 29.1 [27.9, 31.2] → 8.3 [8.0, 9.5] | 54.1 [50.9, 71.7] → 15.6 [14.8, 17.9] | 56.8 [52.0, 72.0] → 14.8 [14.6, 17.1] | 9.3 [7.4, 16.1] → 8.4 [7.7, 9.0] |
<!-- /TIMING -->

Each cell is the median [minimum, maximum] across accepted runs of a per-run
callback quantile. The join tail is callbacks 60 through 101, including the
first two cold gameplay callbacks; gameplay is callbacks 120 through 419.
The separate post-cold tail column uses callbacks 62 through 101. Its window
was declared before collecting delivery-arm timing; it excludes no later spike.
The replay advances at a fixed 60-callback model schedule. Wall time includes
GPU completion waits and offscreen egui painting. These are synchronized
offscreen callbacks, not presented-frame pacing or interactive acceptance.

## Exactness

The authored 25,000-food rectangle runs for 600 callbacks at factor one,
0.065 and whole-map, in Native and Original, on RTX/Vulkan and AMD/DX12 at
3840×2160. Both arms compose menus and omit egui painting for image controls.
`image-controls.json` retains full-sequence RGBA SHA-256 identities and each
individual callback 60–101 hash, explicitly covering the crossfade and spinner
tail. The driver compares all 600 full-frame hashes, food census bytes and
numeric observation digests for each configuration. Three CPU-only canonical
controls compare the full numeric and projected-quad digests as well.

`menu_stream_matches_canonical_primitives_and_preserves_retry_picture` checks
each projected float by its bits across animation, zoom and viewport scales.
The explicit offscreen test
`prepared_bytes_states_and_coverage_match_the_canonical_stream` compares the
parallel preparation's complete vertex bytes, draw states and coverage values
with the original canonical stream on the authored rectangle, then compares
full rendered pixels from bundled and direct submission. Existing menu
admission tests retain the overlay and failed-picture controls.
`spinner_scanline_index_matches_the_full_edge_walk` compares every raster byte
with the original full-edge scan at five times and four scales through 5×.

## Method and access

The retained restricted join replay is used locally for timing only. No
recording, identifying metadata or rendered output from it is published.
All image controls use authored input and procedural materials; no official
art is loaded. No window, server or network connection is opened by the driver.

The baseline is main `2a4938cb`, including the cold-resource and batched-upload
changes. Both arms use the same retained offline driver and passive phase
overlay, release profile, Rust 1.99.0,
`CARGO_TARGET_DIR=D:/CargoTargets/join-fade`, `CARGO_BUILD_JOBS=2`.
`identities.json` binds the source archives, relevant runtime files, driver and
executables. Complete archives, binaries, raw journals, load samples, build logs
and failed attempts stay outside Git in the task's `perf/join-fade` scratch.

Timing reverses arm order on alternate pairs, with phase instrumentation off.
System load, process names and GPU engine load are sampled every 250 ms. Both
runs of any pair with an observed rustc or linker process are excluded; those
runs remain in `runs.csv`. There is no quiet-window booking. Timing holds the
lane's shared lock with a purpose and UTC start, releasing it within one hour.

Earlier candidates, rejected build/archive attempts and failed check logs remain
in scratch. None is used as an accepted timing arm. Missing npm dependencies and
the pinned wasm-bindgen executable were resolved before the final checks.

Focused menu, canonical packing and spinner regressions passed, including the
explicit offscreen bundled/direct comparison. The final `npm run check` passed,
including native and Wasm Clippy and builds. Documentation checks passed. No
independent review or interactive play acceptance is claimed.

```text
python -B research/client-performance/2026-11-05-join-fade/recompute.py --check
```
