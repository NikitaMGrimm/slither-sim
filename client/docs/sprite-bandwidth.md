# Food sprite bandwidth

The renderer keeps shared CPU pictures, protocol decisions, controls, projection
and source rasters unchanged. It reduces raster/blend work in soft food layers
and the amount of repeated geometry sent to the GPU. It uses ordinary WebGPU
texture arrays, indexed triangles and single-sample color attachments; there are
no adapter-specific production paths or optional device features.

## Soft layers

`Bank` marks only its authored padded food outlines and food glow sources.
Unmarked resources keep the ordinary ordered path. Food body rasters retain
the [4× sources and filtered mips](pellet-quality.md) and the selected world
sample count.

The early outline barrier can use a full-sized single-sample target when it is
contiguous and any preceding unmarked geometry covers the viewport completely.
Its padded raster already contains edge coverage. The result initializes the
world attachment before bodies and snakes are painted. Other orders fall back
to the ordinary world path.

A contiguous additive food glow barrier is divided by its projected shortest
edge and effective opacity, without changing culling or the CPU draw commands:

| Glow | Attachment extent | Reconstruction |
| --- | --- | --- |
| Edge below 16 physical attachment pixels | Existing world target/sample count | Existing texture sampling |
| Edge at least 32 pixels and opacity at most 0.05 | Ceil(world extent / 8) | Five-tap cubic approximation |
| Other glows with edge at least 16 pixels | Ceil(world extent / 4) | Linear |

The cubic approximation folds the four small corner weights of a separable
Catmull-Rom reconstruction into the center. Its weights sum to one, so constant
light is preserved. It avoids making a broad gradient softer solely because its
attachment is smaller. The smallest glows keep their existing sample coverage
and mip filtering. The authored offscreen tests bound isolated layer differences;
the dated study retains whole-picture readback differences, including maxima.

Only this explicitly additive glow barrier may regroup GPU quads by resolution
and texture binding. Nonnegative additive contributions commute before output
quantization; bodies, text and source-over geometry keep their order. Glow
attachments use the same encoded RGBA/BGRA8 blend domain as the world. The final
composite precedes the remaining world geometry and the unchanged HUD overlay.

## Bindings and geometry

Resources with equal **physical raster dimensions and mip count** share an array
binding. Every original mip is uploaded to its own layer; images are neither
resized nor packed against another image's filtering border. Actual mip bytes
and allocated array layers count against the existing resident pixel budget.
Cloning a wgpu handle retains its allocation using the backend's ownership
model, including the browser's single-threaded handles.
Unchanged resource sets reuse their arrays, bindings and uploads.

Each GPU quad uploads four 28-byte corner records and uses one shared 16-bit
triangle index buffer. The original six-vertex diagnostic iterator still
describes the CPU picture. Two adjacent additive quads with identical asset,
geometry and UVs can share those corners. Both opacities remain separate shader
inputs: quantize each contribution before adding, rather than summing opacity
and brightening contributions that were individually below one output byte.
The pair path is limited to encoded UNORM8 attachments. The two original
rounding steps can differ by at most two output bytes in the authored sweep;
this is not a byte-identical raster claim.

All quads validate before buffer uploads. Queue writes occur after prior calls
submit; wgpu keeps in-flight buffer, texture-array and attachment references
alive. Resize replaces targets without a CPU/GPU idle wait. World sample-count
and resolution policy remain owned by `WorldTargetPlan`.

## Memory and checks

In addition to the existing MSAA/backing allocations, an active outline layer
uses `4 * width * height` bytes. The fine and coarse glow targets use
`4 * ceil(width / factor) * ceil(height / factor)` bytes, for factors four and
eight. Single-sample worlds need no outline intermediate. Targets retire when
the world no longer uses the corresponding outline/glow path. Reports count retained
layers and old/replacement coexistence; these are logical sizes, not driver
residency measurements.

The ignored `sprite_layers` checks exercise AMD/DX12 single-sample and MSAA4
additions, small glows, unchanged body sources, isolated soft-layer error and
resize-back/in-flight reuse. `texture_arrays` compares independent bindings
with arrays, including ordering, crops, repeat, black blending, multiple layer
groups and restoration of old revisions. `pellet_quality` retains its original
round-rim and subpixel mip-energy checks.

The [dated bandwidth study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-02-sprite-bandwidth/REPORT.md)
owns the diagnostic variants, matched run summaries, adapter, readback limits
and recompute script. GPU pass time is not gameplay FPS, physical latency or
owner perceptual acceptance.
