# Share encoded food bytes, frozen before measurement

Base: `23e987f37b061ac80307404c24f6b7fecc71a770` (origin/main at branch start).

## Initial per-packet prototype, later rejected
Candidate: record the implementation commit before measuring.

Reuse each broadcast-local food diff's encoded bytes through final frame assembly.
Keep one contiguous sector payload with explicit packet ranges. Packet views stay
inside one simulation-thread broadcast, so local reference counting is sufficient;
final owned frames alone cross to the socket task. Ordinary packet production
keeps owned buffers. The existing transition identities, pickup set, old-view
fallback and physical/decorated separation remain unchanged. Legacy conversion
continues to emit each original command as its own message with unchanged header
and physical latency draws.

The previous out-of-class diagnostic's recipient-food bracket is 1.741 CPU ms per
maintenance tick. It includes held-listing bookkeeping and encoding as well as
copies; do not equate it with the removable allocation cost. Hypothesis: shared
byte storage reduces maintenance instructions and CPU without an unacceptable
other-tick cost. Report all three tick classes and every regression.

Reuse the food-diff/cleanup fixed rows unchanged: short stock 0/1080, mature stock
and protocol 13 193000/360. Warm full backends once per arm/protocol, compare every
warm byte, fork each measured pair, and require complete proof/direct-parent
identity. One discarded AB warmup then five AB/BA/AB/BA/AB user-instruction pairs.
Report per-advance CPU maintenance/other means and p99s with ranges. Keep the
guest otherwise idle during counting and retain redacted full process commands
at boundaries. No new live capacity confirmation is planned.

Require all 27 catalog/host traces, stock/protocol-13 payload-only mutation
controls, existing eaten/decorated/older-view/handoff/reconnect oracles, new shared
storage/lifetime and framing/header/latency regressions, a deliberate range
mutation caught at runtime, complete offline Rust group and documentation checks.
Use one bounded guest target, native release/thin-LTO/one-codegen-unit builds,
locked offline dependencies. Archive/verify/prune between cohorts. Keep only
selected results and reproduction deltas in Git, linking earlier scripts.

Socket writes are analysis only: report the earlier 500-recipient diagnostics as
TCP Write calls, verify actual kernel syscall mapping with a bounded own-child
ptrace witness, and keep its scheduling-distorted counts separate from timing.
No socket implementation, changed message boundaries, outside game server,
guest system/package change, or merge.

## Whole-sector-group alternative, frozen before its measurements

The initial `be09f69b` prototype is rejected after both short and mature-stock
instruction pairs regress: short maintenance/other/all +0.361/+0.344/+0.344%,
mature stock +0.206/+0.217/+0.216%. No mature protocol-13 timing is run for that
losing implementation. Preserve its complete successful byte checks, controls,
raw data and selected results; do not relabel them as the replacement's evidence.

The replacement retains each sector's encoded `Vec<Vec<u8>>` behind one local
reference-counted group, sharing that group once per recipient. It avoids the
initial prototype's buffer packing, per-packet reference counts, and ordinary
packet representation change. Ordinary packet vectors stay unchanged; when food
is present, final framing borrows an ordered list of slices across ordinary
prefix, food groups and ordinary suffix. Physical/decorated food keeps separate
groups while borrowing the same unchanged ordinary packet bytes.

Hypothesis: group ownership removes repeated food-byte copies cheaply enough to
reduce maintenance instructions. Run the same three complete fixed rows, full
Rust group, 27 traces and both payload-only controls. New runtime controls must
catch lost packet boundaries and recipient copies, while existing older-view,
pickup, pit, handoff and reconnect regressions remain active. Also verify food
insertion positions, byte aliasing, cache lifetime, frame errors, elapsed headers
and physical latency draws. Retain all outcomes and timing overlap. No new live
capacity hold or socket implementation is added.

## Retained-listing alternative, frozen before its measurements

The group variant also regresses its short guard: maintenance +0.354%, other
+0.650%, all +0.640% instructions. It is rejected; mature and protocol-13 timing
are not run for that losing version. Preserve all 27 exact traces, mutation
controls, full checks, short pairs and its source separately.

The retained mature-stock baseline proof contains only 2.9328 incremental food
packets / 20.5296 bytes per recipient per maintenance tick. This is a byte census,
not allocation CPU attribution. The 1.741-ms food bracket includes bookkeeping;
it cannot be treated as pure byte-copy cost. Before more packet-carrier plumbing,
test the smaller retained-listing ownership operation on the same path.

Membership changes currently drop/reclone every retained listing handle even
when it already equals the current listing. Reuse that unchanged handle by move;
changed and new listings still clone the current handle. Keep pickup emission
with unchanged handles, all cache keys/encoding, membership order and wire timing.
No packet representation, frame assembly or socket implementation changes land.

Run the same full three-row comparison, all 27 traces and stock/protocol-13
payload controls. Run focused retirement+pickup+fresh-state regression plus all
existing older-view/pit/handoff/reconnect oracles, source-changing mutation
controls and the complete offline Rust group. Report all tick classes and timing
variation. Reuse the exact pinned baseline after rehash, with fresh candidate and
wire-mutant builds; keep only one bounded target. No live capacity hold is added.
