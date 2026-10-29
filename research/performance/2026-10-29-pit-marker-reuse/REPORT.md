# Reuse listings outside moving pit markers

The candidate preserves all 31 complete byte comparisons, including protocol-13
and protocol-19 shrinking-marker rows. Both dropped-hint wire mutations and the
focused coverage mutation are detected. Performance measurement remains
**pending explicit SSH bench release**; there is no speed or capacity claim.

Baseline `23eddca8`, implementation `e51e5bef`. This candidate is independent
of the minimap projection and spawn prefilter branches. [Plan](PLAN.md),
[attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[selected correctness](correctness.json).

A marker update previously revisited decorated listings for every subscribed
sector, including sectors outside every old/new ring. The candidate records
sectors covered by disappearing, current and overlapping generations, and
retains the other decorated listings. First subscriptions and physical-food
edits still refresh normally. The original conservative full-union changed
hints are preserved. Marker trigonometry/points, quantization, styles, ordering,
physical-food priority, growth replacement and 0.5-second overlap/expiry are
unchanged. No marker cadence, fade, arena or gameplay rule changes.

## Correctness

The affected-sector oracle covers growth, shrink, rapid growth, overlap,
disappearing generations and expiry at adjacent floats/exact equality. Every
changed overlay must be covered; unchanged updates clear the hints. Dropping
all hints fails that oracle. After source restoration, the existing full-union
oracle still matches the entire changed-hint inventory and both listing views.
Existing food/listing, pit and protocol-13 regressions pass locally. The full
Rust-group receipt is recorded when its serial run completes.

All 23 catalog and six host traces compare complete bytes and terminal records.
The latter include four historical stock/mixed/protocol-13/2,281-seat fixtures
and two Battledome growth/shrink/reconnect fixtures. Two more shrinking-row
traces preserve the complete worlds/RNG, queue records and every frame/time.

| Protocol | Living decorated viewers | Waiting joins | Marker-radius changes | Baseline/candidate complete trace bytes | Dropped-hint wire mutation |
| --- | ---: | ---: | ---: | ---: | --- |
| 19 | 80 | 0 | 5 | 2729706 | Detected |
| 13 | 80 | 0 | 5 | 5478893 | Detected |

The table reads `shrinkingRow[*].protocol`, `base.markerTicks`, `base.bytes`,
`base.summary.waiting` and `wireMutationDetected` in the selected JSON. The
raw initial/final state gates require 80 live decorated connections; this is
synthetic held-pose fixed work, with real backend food, marker and wire history,
not natural growth, real WebSocket delivery or capacity evidence. Full raw
artifacts are local-only and not publicly downloadable.

The first wire mutation was not detected because direct fixture setup had left
decoration delivery disabled. That attempt failed qualification and was retained.
The corrected fixture explicitly enables stock decorations and checks all 80
connections; the mutation then changes encoded frames in both protocols.
The shared fixture's final routing/phase audit and source manifests are kept
separate from these production-source correctness pins.

## Remaining evidence

Finish the serial full Rust group and final common-fixture qualification. Only
after the maintaining agent releases the SSH guest, run native full-byte gates,
repeated-fork/direct-parent proofs and five alternating instructions:u pairs,
with marker/food/minimap and slowest-CPU-1% groups separate. Report CPU
p50/p99/max and paired instruction median/range. Measure minimap and spawn
candidates back to back first, then this candidate; open each PR only with its
own completed evidence. No SSH guest use or additional AWS cost has occurred.
