# Share encoded food bytes, frozen before measurement

Base: `23e987f37b061ac80307404c24f6b7fecc71a770` (current origin/main).
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
