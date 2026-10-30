# Crowded wgpu frames

The renderer preserves every eligible world quad and its original paint order.
It serializes directly into an 8,192-quad scratch buffer, uploads fixed-size
vertex chunks, then binds those chunks in one render pass. Only adjacent equal
pipeline/texture/sampler states batch together, including across the same
texture's repeat/clamp states. A run crossing a chunk boundary splits without
changing primitive order. MSAA is cleared and resolved once for the whole frame.
Canvas output and core painter traversal/quality semantics are unchanged.

Each call to `Renderer::render(&self)` owns its GPU buffers. Concurrent calls and
retained retries cannot overwrite each other's vertices. `create_buffer_init`
remains per chunk; cross-frame buffer reuse would require synchronization across
write and submit. There is no persistent shared vertex buffer. Conversion and
serialization happen in one traversal with no per-quad heap allocation and no
full-frame `Quad` vector. All geometry is validated before any draw submission.
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
| Reused CPU vertex scratch, at most one chunk | 983,040 (0.9375 MiB) |
| GPU vertex data across at most 32 chunks | 31,457,280 (30 MiB) |

Chunk/state-vector headers, allocator and driver overhead are additional. Vec
growth can temporarily retain spare Draw capacity below twice the logical
record budget, and painter plans retain bounded per-command records alongside
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
request default limits; no raised device limit is required. Chunks are all
retained through one submission: total vertex storage scales at 120 bytes per
quad, rather than being a constant-size streaming ring.

The existing **64 MiB active-pixel**, texture-count, queued-pixel and attachment
limits are unchanged. Text producer limits also remain: HUD 128, minimap 12 and
each panel 96. These bound the producers themselves; they no longer reserve
space or reduce world capacity. There is no visibility-dependent announcement
reserve. A future menu's 512-draw bound should likewise be a producer quota,
with its own 64-slot / 4 MiB cache; append it in paint order and apply the final
picture memory/pixel checks without subtracting 512 from the world allowance.

## Recoverable oversized pictures

Exceeding the logical or queued-Draw allowance returns the explicit
`DRAW_MEMORY_LIMIT` error. Production `LocalGame` handles this error at Setup,
ordinary preparation and resize: **skip the whole picture, leave the previous
image on screen, and keep the session**. It commits packet/model/HUD/quality
bookkeeping once, without counting a successful present. No oversized picture
is issued for retry; later packets and smaller frames continue normally.
The native host discards its acquired surface image and uses its existing
100 ms no-picture retry; the browser continues its existing RAF schedule.
`LocalGame::draw_diagnostics` records `skipped_frames` and
`last_skipped_identity`, separately from session failure and bot observations.
The counter saturates; it stores no unbounded diagnostic history.

Other errors, including invalid geometry, missing resources and pixel-budget
failures, retain their existing failure behavior. Direct renderer callers get
the explicit error before draw submission. This PR does not trim geometry or
claim that arbitrarily large frames fit; farthest-first degradation remains a
separate safety-net change.

The regression suite uses authored packets through `prepare_gameplay`, including
former-overflow frames, immutable retries, resize, memory skips and later ingress
and rendering. Synthetic native byte identity and headless browser evidence do
not establish Canvas/official perceptual parity or gameplay FPS.
