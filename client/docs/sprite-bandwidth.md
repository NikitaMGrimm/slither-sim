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

Each frame measures clipped quad coverage, including overlapping draws. Extra
passes are admitted only when their estimated saved work pays for the full-screen
operations. In attachment screen areas, outlines save `(samples - 1) * area`
against fixed work `samples + 2`. Each glow bin saves
`area * (1 + samples - 2 / factor²)` against `samples + taps + 2 / factor²`,
where reconstruction uses one or five taps; selected bins retain a conservative
`2 * samples` reserve from the original split-pass admission policy. Reduced
targets now render first because they read only immutable source rasters. The
world then paints its original ordered geometry and composites in one MSAA pass,
avoiding an intermediate world store/load without changing the selected bins.
This is a portable work estimate, not a GPU timer.
Each bin must pay independently. Entry requires 125% of fixed work; an active
path remains selected above 100%. History resets on attachment/sample changes.
Sparse pictures therefore keep ordinary drawing, including Low-quality outlines.

A contiguous additive food glow barrier is divided by its projected shortest
edge and effective opacity, without changing culling or the CPU draw commands:

| Glow | Attachment extent | Reconstruction |
| --- | --- | --- |
| Eligible at factor 8 | Ceil(world extent / 8) | Five-tap cubic approximation |
| Eligible at factor 4 | Ceil(world extent / 4) | Linear |
| Eligible at factor 2 | Ceil(world extent / 2) | Linear |
| Other glows | Existing world target/sample count | Existing texture sampling |

Choose the coarsest eligible factor. The shortest edge must span at least four
reduced pixels, and `max(opacity, paired opacity) * factor / edge <= 0.004`.
The latter bounds the opacity gradient per reduced pixel; it is an authored
quality guard, verified by the source-size/opacity/fractional-position sweep.

The cubic approximation folds the four small corner weights of a separable
Catmull-Rom reconstruction into the center. Its weights sum to one, so constant
light is preserved. It avoids making a broad gradient softer solely because its
attachment is smaller. The smallest glows keep their existing sample coverage
and mip filtering. The authored offscreen tests bound isolated layer differences;
the dated study retains whole-picture readback differences, including maxima.

Reduced passes bias the source mip selection by minus `log2(factor)`, retaining
the original world-sized source footprint. Implicit derivatives otherwise apply
extra mip reduction before reconstruction and change overlapping glow energy.
This keeps every #427 source mip available, including ordinary minified glows;
the correction applies only while painting the smaller attachments.

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

Within an already paired UNORM8 draw, both opacities below `0.49 / 255` make
every sampled RGBA contribution round to zero. Those pairs still validate but
issue no draw. The margin below half a byte avoids the rounding boundary. In
the additive food barrier only, paired draws group by zero/nonzero contribution
and source array layer to keep active runs contiguous and improve source locality.
Their quantized nonnegative additions commute in the encoded byte domain; the
dated follow-up retains actual readback differences rather than assuming hardware
arithmetic is byte-identical. Non-paired groups keep the existing ordering.

Initial black source-over quads over the opaque black world clear also issue no
draw. If they cover the entire admitted outline prefix, no outline target/pass
is needed. Logical outline-admission history is retained so the next picture's
hysteresis choice stays unchanged. This rule never applies to loaded overlays,
colored prefixes, later shadows, sRGB or floating-point diagnostic attachments.

All quads validate before buffer uploads. Queue writes occur after prior calls
submit; wgpu keeps in-flight buffer, texture-array and attachment references
alive. Resize replaces targets without a CPU/GPU idle wait. World sample-count
and resolution policy remain owned by `WorldTargetPlan`.

## Memory and checks

In addition to the existing MSAA/backing allocations, an active outline layer
uses `4 * width * height` bytes. Glow targets use
`4 * ceil(width / factor) * ceil(height / factor)` bytes, for factors two, four
and eight. Single-sample worlds need no outline intermediate. Outlines retire
when their path is inactive. Unused glow bins remain cached while another
amortized glow bin is active; all glow targets retire when that path is absent.
Reports count these cached targets as retained storage, as well as
layers and old/replacement coexistence; these are logical sizes, not driver
residency measurements.

The dated study compares caching with per-bin retirement during factor flicker:
at Native 4K, retirement raised median calling-thread preparation/submission
from 0.1863 to 0.2218 ms without a material GPU-time improvement. Caching retained
up to 3.955 MiB of inactive targets in that resize/toggle sequence. This measured
CPU/memory tradeoff motivates keeping the cache; the sparse cost guard still
retires all glow targets when no bin pays for an extra path.

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

The [overlapping-sprite follow-up](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-02-overlapping-sprites/REPORT.md)
profiles the remaining authored crowded cost and records current-main comparisons
and exact RGBA controls for the no-op and pass-order changes above.
