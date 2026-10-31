# Studio panel draw records

Menu admission counts quad records, not GPU draw calls. Adjacent matching quads
already share GPU draw runs. `MenuFrame::draw_counts(preview_count)` attributes
the successfully prepared records to login wordmark/controls/text, studio
shade/surface/chrome/active section, and other panels. Text is counted after
clipping, including zero-opacity records that the producer still emits.

`reported_menu()` matches the earlier transition counter: frame records plus
the page backdrop. `admitted_menu()` also includes the optional world cover;
this is the count checked against 512. Preview commands are separate, and
`combined()` includes them without charging them to the Menu limit. Phase and
section belong to the caller's fixed-clock fixture, not to the raster cache.

The studio surface keeps the original bounded raster resources, corners,
mirroring and shadow strips. Only rectangles entirely inside the opaque flat
fill can combine into a stretched region. The old slice grid partitions that
region; an integer pixel-area check requires a full rectangular union after
clipping. Small surfaces, non-flat themes and unions that fail this check keep
the original records. The remaining edge records keep their source crops,
geometry, alpha and order. Preserving these records also preserves repeated
shadow/corner samples that an ordinary nine-slice replacement would change.

This path is limited to the studio panel. Settings retains its separate slicing
path, and the builder controls, wordmark, shader and timing stay unchanged.
Menu still allows 64 cache slots, 13 MiB and 512 records; preview and active-picture
budgets are unchanged. The optimization reduces records, not retained raster
bytes; it makes no frame-time or FPS claim.

The finite Windows exporter prepares both Built-in and early Builder entry at
fixed clocks, return transitions and explicitly clipped surfaces. Its matrix is
three logical layouts (960x540, 360x640 and 960x360) at DPR
1/1.25/1.5/2/3/4/5/6/7/8, with the existing density cap of 5. It opens no window,
loads no external art and executes no transport effects.

From the repository root, with `CARGO_BUILD_JOBS=2`:

```text
cargo run --locked -p slither-wgpu-slice --example menu_specimens -- NEW_OUTPUT_DIRECTORY --studio-panel
cargo run --locked -p slither-wgpu-slice --example menu_specimens -- NEW_AFTER_DIRECTORY --studio-panel BEFORE_DIRECTORY
```

Capture the first side before changing the painter, using the same additive
attribution/exporter on the pinned main. Keep both source identities and build
settings. Lossless PNG encodings may differ; comparisons decode the complete
RGBA images. `draws.csv` reconciles each component and both counters;
`pixels.csv` reports the maximum channel delta, its pixel/channel and the number
of changed channels for every paired frame. Any nonzero value is a mismatch,
not pixel identity. The mode saves all images and CSV rows, then exits with an
error if any compared frame differs. Required checks use the normal client build settings;
optional PNG-only dependency optimization can accelerate the capture tool on
both sides without changing the client painter.
