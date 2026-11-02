# Studio panel draw records

The skin studio now draws with egui (see [skin studio](skin-studio.md)). This
note describes the retained old-toolkit studio painter, which no longer draws
the studio; its `--studio-panel` exporter was retired with that change.

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
