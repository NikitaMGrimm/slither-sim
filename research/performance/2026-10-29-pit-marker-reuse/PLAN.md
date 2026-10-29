# Exact marker-sector reuse: frozen plan

Baseline `23eddca8`, separate from the minimap and spawn candidates. Keep marker
generation, trigonometry, quantization, styles, physical-food priority, ordering,
growth replacement and 0.5-second overlap/expiry rules unchanged. Track only the
union of old/new current/previous ring sectors during an actual marker change.
Reuse unaffected decorated listings; physical edits and first subscriptions
still rebuild normally. Preserve the original conservative full-union change
hints so private hint inventory and wire outputs remain identical.

Run focused sector-coverage and existing full-union/listing/marker regressions,
both protocols, a dropped-hint mutation and restored checks, then the entire
Rust check group, serially with one Cargo job and one test thread. Compare all
23 catalog/four historical host traces, both Battledome traces, and the
shrinking 80-viewer control, complete bytes and terminal records. Require a
wire mutation detected in protocol 13 and 19. Retain all failures.

Reuse the prepared spawn study's fixed-work harness by reference, with its
`markers` row: 80 live synthetic viewers, actual marker changes and food/wire
histories, held poses outside the counted interval and explicit radius replay.
Qualify at least three actual marker-change ticks per counted 360-tick window
and zero waiting admissions. This row is synthetic fixed work, not naturally
grown bodies or a capacity test. The minimap/no-backlog and exhausted-search
rows remain separate controls for the other candidates.

No SSH guest connection, build or measurement until explicit maintaining-agent
release. Prepare/hash/compile locally first. Then use the shared Linux native
builder, full-byte gates, repeated-fork/direct-parent proof traces and five
alternating instructions:u pairs for both protocols, with marker-change, food
epoch, minimap and slowest CPU 1% groups separate. Retain every pair and quiet
process inventory; report CPU p50/p99/max, paired median/range and limitations.
No performance, latency or human-feel claim before those measurements. Open a
separate PR after evidence is complete; do not merge it.
