# Live snake capacity

The shared client admits **512 live wire IDs**, including the player. This covers
a simulator Battledome with 500 bots and one player. It does not change server
subscriptions: zoom cannot reveal a snake the server has not sent. Official
server population/visibility policies do not set this client limit.

Protocols 13 and 19 use the same state admission and presentation policy. The
existing **4,096 anchors per received snake** is a client retention policy, not
a protocol length field or an official-game rule. The aggregate received quota
is now `512 * 4096 = 2,097,152` anchors. Received positions, IDs, lifecycle and
food/prey addressing are never decimated.

A new live ID beyond 512 returns `StateError::Limit`. The complete packet rolls
back, including earlier commands in its envelope. Replacing an existing ID at
the cap remains legal. Native and WebGPU checked-error handling calls `fail`,
queues Close and stops that session; the Canvas session propagates the checked
ingress error. This clean hard-limit disconnect remains intentional.

## Presentation pressure

Every formerly admissible session keeps the original numeric/draw path. Pressure
latches only when a received world exceeds the former 256-ID/65,536-anchor
envelope, physical presentation anchors would exceed 131,072, or retained
appearance objects would exceed 1,024. Reset clears the latch. Merely moving the
camera does not enable it.

Under pressure:

- Keep the original own object, all its anchors, corrections, curve and painter.
- Bound visual anchors to **32,768 across the entire retained collection**.
  Reduce small/distant rivals first, ordered by `scale / (1 + camera distance)`;
  appearance order breaks ties. Repeated reductions keep endpoints, the three
  newest anchors and surviving correction offsets, folding removed intervals
  into their retained neighbour. The logical count stays independent. Reductions
  are nested and are not reversed during that appearance, preventing oscillation
  as the camera crosses a boundary. Newly arriving head anchors remain detailed.
- Share **49,152 curve samples** among visible rivals, with an eight-sample floor.
  Extra samples are proportional to `scale / (256 + camera distance)`. This
  avoids swapping whole quotas when two rivals exchange distance order. Sampling
  retries are private; eye/spacing transitions commit once. The own curve keeps
  its existing independent 65,536-sample bound.
- Paint reduced rivals as connected base-image spans with covered joints, at
  most two image commands per sample. Omit their outline, shadow, boost, death
  and fade decoration. Native reduces translucent eye-outline tessellation and
  rival antenna strokes; bulbs, numeric antenna updates and RNG cadence remain.
  Canvas uses the same connected-body planner. The legacy painter is unchanged.
- Native reserves the own body/antenna and overlay capacity first. Remaining
  rival body quotas can reduce the connected curve further, to a head marker or
  no optional body commands when exhausted. Optional rival faces, bulbs and names
  cannot exhaust that reservation. Fresh maximum-length full-map fixtures retain
  a body for every snake; this last resort protects unusually expensive own draws.
- Select bounded suffixes of retained food/prey for painting only. Their complete
  received and animated state, including eater tokens, remains retained.
- At the retained-object ceiling, retire the oldest unaddressed rival appearance
  and only its already-consumed food/prey flights. Keep every addressable live
  snake and the original own object. This prevents appearance churn from turning
  a visual-object quota into another session limit.

Unused visual-vector capacity is reclaimed when its aggregate exceeds the
corresponding anchor budget. This does not change any retained numeric record.
Allocation is lazy; none of these ceilings preallocates an entire maximum world.

## Quota audit

Values are per world/picture unless a transaction or cache is specified.

| Owner / bound | Before | Now | Protection / pressure behavior |
| --- | ---: | ---: | --- |
| `state::MAX_SNAKES` | 256 | 512 | Atomic live-ID admission, shared by 13/19 and both adapters |
| Received anchors per snake | 4,096 | 4,096 | Independent retained-body policy; over-limit packet rejects atomically |
| Received total anchors | 65,536 | 2,097,152 | Derived from live IDs × per-snake anchors |
| Presentation appearance objects | 512 | 1,024 | `2 * MAX_SNAKES`; retired visuals trim at pressure |
| Physical anchors per presentation body | 8,192 | 8,192 | Living plus dying anchors; own remains complete |
| Aggregate visual anchors | 131,072 | 131,072 legacy / 32,768 pressure | Fixed memory envelope, not received-anchor × population growth |
| Aggregate allocated anchor slots | No aggregate capacity guard | Same as the applicable visual-anchor quota | Reclaim spare vector storage without changing geometry |
| Single curve / shared ordinal flags | 65,536 | 65,536 | Per-snake sampling work and retained Float32 paint flags |
| Aggregate numeric draw samples | 262,144 | 262,144 | Own plus pressured rival quota fits this bound |
| Pressure rival samples | — | 49,152 | `96 * MAX_SNAKES`, own excluded |
| Single standard-body image plan | 524,304 | 524,304 | `8 * MAX_CURVE_SAMPLES + 16`; exact preflight before allocation |
| Canvas transaction image pool | 524,304 | Same legacy / 688,144 pressure | Own hard maximum + reduced rivals + bounded entities and face/bulb allowance |
| Ordinary antenna nodes | 11 | 11 | Topology is 8, 9 or 11 nodes, independent of population |
| Antenna bulbs / paths / strokes per transaction | 512 / 1,024 / 2,048 | 1,024 / 2,048 / 4,096 | Derived from retained appearances; actual paths are charged |
| Antenna vertices / scalar RNG units per transaction | 11,264 / 10,240 | 22,528 / 20,480 | `2 * nodes * retained` / `2 * (nodes-1) * retained` |
| Native/WebGPU `MAX_DRAWS` | 262,144 | 262,144 | **64 MiB** CPU records at 256 bytes each; pressured rival detail trims |
| GPU vertex chunk | 8,192 quads | 8,192 quads | 983,040 bytes/chunk, within downlevel limits; at most 32 chunks |
| CPU/GPU serialized vertices / CPU draw states | 30 / 6 MiB | 30 / 6 MiB | At most 120 / 24 bytes per admitted quad; no per-snake GPU buffer |
| Active picture pixels | 64 MiB | 64 MiB | Shared image residency, independent of snake count |
| Cosmetic pixels / encoded input | 16 / 4 MiB | 16 / 4 MiB | Independent artwork admission; no per-snake image copies |
| Canvas recipe asset uses | 262,144 | 524,288 | `512 * MAX_RETAINED_SNAKES`; repeated recipes share image sources |
| Canvas aggregate recipe catalog entries | 4,096 | 524,288 | Same derived metadata ceiling; each recipe still has at most 4,096 IDs |
| Canvas distinct body sources | 4,096 | 4,096 | Shared DOM source identities, not one source set per snake |
| Food / prey retained instances | 65,536 / 4,096 | Unchanged | Received food keys and presentation multiplicity have separate bounds |
| Food / prey painter commands | 458,752 / 24,576 | Unchanged hard maxima | Seven / six commands per retained instance |
| Pressure food / prey paint allowance | — | At most 32,768 / 16,384 | `64 / 32 * MAX_SNAKES`; native can select fewer to protect own/UI |
| Food/prey family source entries | 4,096 each | Unchanged | Bounded creation metadata, not proportional to eater count |
| Combined Canvas food/prey catalog / snapshots | 262,144 / 524,288 | Unchanged | Shared ingress registry work; no new per-snake catalog |
| Native names submitted / nickname cache | 128 / 216 runs | Unchanged | Visible prefix trims; subpixel names exit before rasterization |
| Native total text caches | 386 runs / 31 MiB | Unchanged | HUD/minimap/world, panel and menu groups have separate producer quotas |
| HUD text spans / native HUD draws | 40 / 128 | Unchanged | Bounded screen-space HUD, independent of population |
| Leaderboard ingress rows / bytes | 149,795 / 1 MiB | Unchanged | Framing-derived cumulative copy budget; population and IDs are u16 |
| Own score-table domain | Advertised `N + 2048` index | Unchanged | Scoring contract, independent of physical anchors; maximum-body session fixtures advertise a compatible N |
| Minimap axis / layers / native draws | 512 / 2 / 12 | Unchanged | At most 2 MiB RGBA core images; raster data is not a snake array |
| Arena tessellation | 4,096 segments | Unchanged | View-dependent edge geometry, not per-snake work |
| Native Setup pictures / aggregate records and pixels | 16 / 64 MiB / 64 MiB | Unchanged | Immutable queued ingress pictures; separate from live-ID admission |
| Canvas target axis | 4,096 | Unchanged | Screen attachment dimensions, independent of whole-map zoom |
| Canvas Setup count / background work | 64 / 67,108,864 target pixels | Unchanged | Shared transaction work; antenna/sample/image pools are cumulative |
| Canvas submitted zoom | 0.05–8 | Finite, positive, at most 8 | Whole-map views can cross the old floor; target pixels and command quotas still bound work |
| Frame bytes / commands / bundled-command bytes | 1 MiB / 4,096 / 8,191 | Unchanged | Maximum-length appearances use bare frames; no framing relaxation |
| Native/WebGPU deferred ingress | 64 envelopes / 1 MiB | Unchanged | Supersede eligible prepared work and drain state at pressure; issued pictures and I/O barriers remain protected |
| Native transport poll / message bytes | 64 messages / 1 MiB each | Unchanged | Yield after a bounded receive batch; larger worlds arrive across batches |
| Native outbound pending threshold | 4,096 bytes before a send | Unchanged | Transport backpressure, independent of received population |
| World attachment extent / bytes | 8,192 / 128 MiB | Unchanged | Viewport/device policy; Native MSAA has its separate 64 MiB allowance |

The packet, malformed-source, attachment and device-error guards remain separate
contracts. The pressure policy does not change a pre-existing unrenderable own
recipe, unsupported own score-table index or invalid arithmetic into a different
own appearance. Maximum-anchor full-session fixtures advertise a compatible
score-table domain; received-anchor admission itself is independent of it.

## Memory accounting

The single Rust/Wasm-owned presentation working envelope is **1,536 MiB
(1,610,612,736 bytes)**. The executable accounting assertion includes transaction
copies, visual anchors, prepared curves, image-command plans, borrowed recipe
metadata, complete received-anchor copies, food/prey state copies and fixed
renderer/adapter allowances. It takes the larger of the legacy and pressure
storage branches, rather than multiplying full pressure raw state by legacy
visual detail. It is an
upper bound on requested buffer payloads, not an allocator/RSS measurement.
The supported native-layout accounting sums to **1,574,112,512 bytes**, including
sample-bounded spare command slots, within that single envelope.
Caller-owned DOM images/fonts and device memory have their independent admission
contracts above; they are not Wasm linear-memory copies per snake.

On the supported native layout, a received `Point` is 16 bytes and a corrected
visual `BodyPoint` is 960 bytes:

| Buffer payload | Before | After |
| --- | ---: | ---: |
| Received anchors, one world | 1,048,576 bytes | 33,554,432 bytes |
| Visual anchor records, one world | 125,829,120 bytes | Same legacy ceiling; **31,457,280 bytes** under pressure |
| Allocated visual-anchor slots | No aggregate guard; a loose per-object Vec-growth upper bound was 8,053,063,680 bytes | **125,829,120 bytes** legacy / **31,457,280 bytes** pressure, plus bounded single-body transaction scratch |
| Aggregate presentation working envelope | No single asserted envelope | **1,610,612,736 bytes**, lazily used |

The old loose capacity bound is `512 retained objects * 2 * 8192 anchors * 960`;
it is not a measured or claimed simultaneously reachable resident heap. The new
sum-of-capacities check removes that accumulation. Simply scaling the old visual
anchor quota with the new received quota would instead permit 3,840 MiB of
visual records before transaction copies, exceeding Wasm's useful memory range.

Tests cover the exact 512/513 live boundary, maximum received anchor product,
allocated visual slots, own-body preservation, connected reduced spans, draw and
pixel limits, and real Canvas Wasm admission. Headless workload measurements and
their precise limits are in the [capacity study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-01-snake-capacity/REPORT.md).
