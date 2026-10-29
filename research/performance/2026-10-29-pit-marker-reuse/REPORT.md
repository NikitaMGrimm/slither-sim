# Reuse listings outside moving pit markers

The retained-vector candidate passes all **31 complete byte comparisons** and
both protocol-13/19 wire mutation controls against #264. All 15 Rust checks
pass offline with at most two Cargo jobs and one test thread. There is **no
CPU or instruction result**: the guest belongs to another worker and has not
been contacted during this continuation.

The local operation census suggests a **marginal candidate, not a PR to open
without a measured shrinking-tail win**. It removes 399 empty-overlay calls
over five marker updates, adds 585 coverage queries and processes 944 coverage
keys, while all 178 nonempty overlays still run. The fixed-pit control removes
no target work. Different operation costs prevent a CPU-saving estimate from
these counts.

Current baseline `cc1bb1e9`; production candidate `46823eea`. The original
tree-based implementation `e51e5bef` and its [correctness](correctness.json) /
[verification](verification.json) remain historical receipts. The original
[plan](PLAN.md), [local amendment](PLAN-LOCAL.md) and [attempts](ATTEMPTS.md)
distinguish them from the revised source. Full raw sources, binaries, manifests
and traces are **local-only**, not public downloads.

## Implementation and unchanged behavior

A marker transition used to revisit the decorated listing of every subscribed
sector. This candidate tracks the union of disappearing/current/overlapping
ring sectors and reuses other listings. First subscriptions and physical-food
edits refresh normally. Original conservative full-union change hints remain
identical, including sectors whose listing values do not change.

The affected union is a sorted, deduplicated vector queried by binary search.
Its scratch capacity survives clears and ring replacement, avoiding tree-node
allocation and destruction at each transition. This revision changes private
membership storage only. Marker trigonometry/points, quantization, styles,
physical-food priority, ordering, growth replacement and the 0.5-second
overlap/expiry rules are unchanged. It neither caches nonempty overlay builds
nor eliminates marker generation, and still visits the full subscription union
when markers change.

## Local correctness

The coverage oracle compares before/after overlays through growth, shrink,
rapid replacement, overlapping/disappearing generations and exact/adjacent
expiry times. Making the binary-search membership always false fails it;
restored source passes. Existing complete changed-hint and listing-value
oracles also pass, along with food, pit and protocol-13 regressions.

[Current qualification](local-current.json) compares 23 catalog, six host
(stock/mixed/legacy/2,281 seats plus two Battledome cases) and two shrinking-row
traces in full, including terminal world/RNG/queue state and every frame/time.
The dropped-hint mutation must alter encoded wire bytes in both protocols.

| Protocol | Live decorated viewers | Waiting joins | Actual marker-update ticks | Complete trace bytes | Wire mutation |
| --- | ---: | ---: | ---: | ---: | --- |
| 19 | 80 | 0 | 5 | 2729706 | Detected |
| 13 | 80 | 0 | 5 | 5478893 | Detected |

Both initial/final records require 80 live decorated viewers and nonzero routing
generations. These are synthetic held poses with an explicit radius replay,
real food/marker/wire histories and zero queued admissions, not naturally grown
snakes, real sockets or capacity evidence. [Local checks](local-checks.json)
record all focused commands, the expected mutation failure, source restoration
and the complete Rust group. Native evidence for earlier independent minimap
and spawn studies does not qualify this candidate's performance.

## Untimed operation census

[count_work.py](count_work.py) overlays only counters in fresh copies of the
qualified source. It runs 360 ticks of shrinking and fixed-pit controls in
both protocols, with clocks/PMU disabled, verifies distinct executable hashes
and compares all four baseline/candidate traces completely. The diagnostic
binaries are separate from any future performance build.

| Protocol | Row / group | Ticks | Empty overlay calls, base to candidate | Nonempty overlays, base to candidate | Added coverage queries | Coverage input / unique keys |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 19 | shrinking / marker ticks | 5 | 407 to 8 | 178 to 178 | 585 | 944 / 350 |
| 19 | fixed pit / all ticks | 360 | 81 to 81 | 0 to 0 | 0 | 0 / 0 |
| 13 | shrinking / marker ticks | 5 | 407 to 8 | 178 to 178 | 585 | 944 / 350 |
| 13 | fixed pit / all ticks | 360 | 81 to 81 | 0 to 0 | 0 | 0 / 0 |

Counts come from [work-counts.json](work-counts.json); the table is recomputed
by [work_table.py](work_table.py). Input/unique keys are sums across updates,
not globally distinct keys. The shrinking group contains the five actual
marker ticks; the fixed-pit control shows all 360 ticks and has zero marker
changes. Both protocols have identical counts.

About 80 empty-overlay calls disappear per marker update, in exchange for
about 117 coverage lookups and 189 collected/sorted keys (70 unique per update).
The 585 sector visits and 178 nonempty overlays remain. The census counts
operations, not instructions, allocations or CPU; no weighting model or speed
percentage is inferred. All fixed-pit operation counts remain identical.

## Estimate and decision

The target is population-drop stutter, not steady 80-seat boosting. The
[earlier descending-80 diagnostic](../2026-10-29-battledome-experience/REPORT.md)
attributed 28.6% of old tail CPU to shared sector preparation, including 12.0%
to marker update/generation. The latter work still runs. Eliminating *all*
other shared preparation would give a loose **16.6% ceiling on CPU-time saving
in that historical tail**; this candidate removes only a small part of it and
adds tracking work. That ceiling is not a prediction or a bound for a new
server build, nor a 16.6% capacity/speed claim. Actual saving can be zero or
negative.

The [constant-boost ascending study](../2026-10-29-battledome-boost/REPORT.md)
had no marker changes in its slow tail. Here the fixed-pit census likewise
removes zero target work. There is no basis for promising a steady-world CPU
gain. The local estimate is therefore marginal/uncertain: do not publish a
separate optimization PR unless a released-guest comparison shows a useful
whole-window or shrinking-tail win. Preserve this exact candidate and await
the maintaining agent's explicit release before any guest access or timing.

Future measurement must retain five alternating instruction pairs per protocol,
full-byte/fork controls and CPU p50/p99/max for marker/food/map/own slowest-1%
groups, including post-marker ticks so cleanup cost cannot be hidden. No rule
or feel change is proposed. [Reproduction](REPRODUCE.md) links the common tools.

## Evidence size and resources

Instrumented sources are pinned by one private-manifest digest per arm;
full per-file maps stay outside Git. Shared preparation hashes appear once.
The operation census references the qualification file's runtime pins rather
than repeating them; future native evidence should do the same when unchanged.
All raw logs, failed/mutated artifacts and the original tree prototype remain
private with source/binary hashes. No SSH guest, AWS resources, service or
performance timing was used for this local continuation.
