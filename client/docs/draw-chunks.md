# Crowded wgpu frames

The renderer preserves every eligible world quad and its original paint order.
It serializes directly into an 8,192-quad scratch buffer, uploads fixed-size
vertex chunks, then binds those chunks in one render pass. Only adjacent equal
pipeline/texture/sampler states batch together, including across the same
texture's repeat/clamp states. A run crossing a chunk boundary splits without
changing primitive order. MSAA is cleared and resolved once for the whole frame.
Canvas output and core painter traversal/quality semantics are unchanged.

`Renderer::render(&mut self)` retains growable vertex chunks and writes them with
`queue.write_buffer`. Exclusive calls finish submission before the next call's
writes, so queue order protects earlier in-flight vertices, including the world
and overlay sharing buffers. The MSAA view is retained until extent/sample count
changes. Conversion and serialization happen in one traversal with no per-quad
heap allocation or full-frame `Quad` vector. All geometry is validated before
GPU writes or draw submission. CPU byte/state storage is reused across frames.
Preparation validates the final composed picture without allocating quads;
resource materialization no longer repeats that geometry traversal. The public
renderer validates again because its public `Frame` input remains mutable.

## Memory bound

`frame::DRAW_RECORD_BUDGET` is 64 MiB and `MAX_DRAWS` is that budget divided by
`size_of::<Draw>()`: **262,144 quads**, at **256 bytes per Draw** on both x64
Windows and wasm32. This is a memory guard, independent of vertex-buffer size.
It admits the authored 150,000-quad stress case (38,400,000 Draw bytes).
Existing core entity/sample bounds remain in force.

At the absolute bound, per-picture storage is:

| Storage | Bytes |
| --- | ---: |
| Logical CPU Draw records | 67,108,864 (64 MiB) |
| CPU resolved states, 24 bytes per quad | 6,291,456 (6 MiB) |
| Retained CPU vertex bytes, at most 32 chunks | 31,457,280 (30 MiB) |
| GPU vertex data across at most 32 chunks | 31,457,280 (30 MiB) |

Chunk/state-vector headers, allocator and driver overhead are additional.
Compositor appends and splices check the count before growing Draw storage and
cap its geometric spare capacity at `MAX_DRAWS`, including faces, arena, text and
late overlays. A full picture rejects the next eligible draw before a Vec can
double beyond 64 MiB. Painter plans retain bounded per-command records alongside
composition. The food/prey planners are limited by the remaining picture
allowance; each body planner also retains its existing per-snake sample bound.
These figures describe draw submission storage, not total process memory or a
driver-allocation guarantee. Queued Setup pictures share the same aggregate
64 MiB logical Draw allowance, including space for their bounded late HUDs;
they retain the existing 16-picture and 64 MiB aggregate pixel checks.

The [WebGPU default limits](https://www.w3.org/TR/2026/CRD-webgpu-20260915/#limits)
allow a 268,435,456-byte `maxBufferSize`, a 2,048-byte
`maxVertexBufferArrayStride`, 8 vertex buffers and 16 attributes. Each chunk uses
983,040 bytes at most, a 20-byte stride, one active vertex-buffer slot and three
attributes. Its draw indices cover at most 49,152 vertices. Native and browser
request WebGL2 downlevel validation limits with the adapter's resolution;
no raised buffer limit is required. Chunks are all
retained through one submission: total vertex storage scales at 120 bytes per
quad, rather than being a constant-size streaming ring.

The existing **64 MiB active-pixel**, texture-count, queued-pixel and attachment
limits are unchanged. Text producer limits also remain: HUD 128, minimap 12 and
each panel 96. These bound the producers themselves; they no longer reserve
space or reduce world capacity. There is no visibility-dependent announcement
reserve. The opt-in menu's 512-draw bound is likewise a producer quota,
with its own [64-slot / 4 MiB cache](menu-resources.md); append it in paint order and apply the final
picture memory/pixel checks without subtracting 512 from the world allowance.

## Recoverable bad pictures

Native release builds skip pictures that fail preparation, composition, resource
or geometry validation, retaining the session and the previous presented image.
`LocalGame::draw_diagnostics` records a saturating skip count and last identity,
and logs the error text at most once every five seconds. Debug/test builds return
the errors; `strict_validation` lets tests exercise both policies explicitly.
The browser keeps its existing draw-memory skip policy.

Skipping an issued picture retires model/HUD/control bookkeeping once without
counting a successful present. No invalid picture is retained for retry, and
later ingress and smaller frames continue. The native host discards any acquired
surface image and uses its existing 100 ms retry. Device loss and surface failures
remain host errors and keep their existing recovery/shutdown handling.

Direct renderer callers receive typed `RenderError::Frame` validation errors and
`RenderError::Host` device failures. Geometry is never trimmed to fit a budget.
The [offscreen workload](frame-performance.md) measures this submission path;
passing it does not establish Canvas/official parity or gameplay FPS.

The regression suite uses authored packets through `prepare_gameplay`, including
former-overflow frames, immutable retries, resize, memory skips and later ingress
and rendering. Synthetic native byte identity and headless browser evidence do
not establish Canvas/official perceptual parity or gameplay FPS.
