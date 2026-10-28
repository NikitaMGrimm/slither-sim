# Verified body-block reuse, 2026-10-28

Retaining body-block bounds after checking every body coordinate reduces mature
host instructions by **10.945% with stock clients and 10.648% with protocol 13**,
on top of merged #253's circle rejection. Both complete warm streams and measured
windows are byte-identical. Short guard rows use **0.63–0.95% more instructions**;
the matched short row uses 0.849% more. The production change is recorded in the
PR commit; this study does not duplicate it as a patch.

[Selected results](results.json), [frozen plan](PLAN.md),
[reproduction and linked earlier scripts](REPRODUCE.md).

## Change and selection

The preceding [circle study](../2026-10-28-circle-body-coverage/REPORT.md)
leaves 1.548M diagnostic instructions per mature tick in body-block construction.
Its losing cache prototype saved 9.795% against the pre-circle parent. That is
evidence for testing reuse, not a prediction of its saving after circle
rejection. This study combines that verified reuse with the landed circle test;
collision and further coverage changes remain separate work.

The backend retains each snake's body coordinates and block bounds between
broadcasts. Before reuse it compares length and both coordinate bit patterns at
every position. Any mismatch rebuilds the complete body's blocks. This handles
same-length interior edits, signed zero and NaN-payload changes without relying
on a hash, tolerance, lifecycle identity or tail-relative partition. Anchor
insertion contracts interior points too, so a head/tail-only key is insufficient.
Short/dead bodies clear their entries; shrinking slot counts drops old entries.
Invalid/extreme coordinates clear the blocks and use the existing complete
coverage walk. The current head and head-to-front edge are still tested
separately. Retaining a duplicate body snapshot and each slot's vector capacities
costs memory; this study does not claim an RSS improvement.

The circle/AABB rejection predicates and exact surviving edge operations/order
are unchanged. An interrupted broadcast can discard its scratch cache; the next
broadcast rebuilds it. No encoding/decoding, transport, OS-specific production
code, public API or checkpoint format changes.

## Fixed work and byte identity

Reuse the [full-host fixture](../2026-10-28-mature-covered-bounds/REPORT.md)
without checkpoint restoration. Both runtimes start at tick zero: seed 7,
`Config::official(500)`, explicit Official delivery, all 500 slots remote,
A4-mean core controls every six 7 ms ticks. All recipients and broadcasts run
during warmup, preserving natural food presentation history. Through tick
180,000, 100 slots stay resident and 400 retire/rejoin one every 50 ticks.
Reset only the external admission RNG at 180,000, stop forced churn, and continue
to fixed tick 193,000. Keep the existing living/length screen unchanged.

Count only the following 360 `advance_tick_with` calls. Control generation,
admission, frame draining and trace serialization remain outside the count;
no bot decoding or observations are counted. Matched short work starts at tick
zero and counts 1,080 advances, including construction churn. Length is not the
only difference between those rows. The original four short guards retain their
own definitions. There is no mature 2,281-seat measurement.

One OS thread is asserted before forks/windows. Each child opens a fresh
current-thread PMU counter; the parent stays frozen until its final direct
continuation. Compare complete actions, ordered frame/time bytes, canonical
physical states and host RNG/lifecycle endpoints across two children per arm
and both direct parents. Compare every warm-stream byte live with bounded
buffers. Hashes identify artifacts; complete byte comparisons establish parity.

## Results

| Selected row | Before → after, M instructions/tick | Median reduction | Pair range |
| --- | ---: | ---: | ---: |
| Mature stock | 11.6195 → 10.3477 | 10.944796% | 10.944677–10.944799% |
| Mature protocol 13 | 11.9577 → 10.6844 | 10.648277% | 10.648254–10.648287% |
| Matched short | 6.7040 → 6.7609 | −0.848871% | −0.848913–−0.848693% |

Negative reductions mean more instructions. The original short guards also
regress; these small, repeatable changes exceed their instruction-count spread.
Accepting the cache is a tradeoff for the primary mature workload, not a claim
of a short-body improvement.

| Original short row | Median instruction increase | Pair range |
| --- | ---: | ---: |
| `official500` | 0.846210% | 0.845409–0.848426% |
| `official500-legacy` | 0.789851% | 0.788022–0.793355% |
| `official2281` | 0.949118% | 0.948381–0.949991% |
| `client-official-500` | 0.629072% | 0.628536–0.629252% |

Every row has one discarded AB warmup and five adjacent AB/BA/AB/BA/AB pairs.
Host rows count complete advances; `client-official-500` counts its complete
fixed command. Selected results retain medians, ranges, counts and table inputs;
every raw pair remains local-only.

Both protocols qualify at the same scene: 499 living, mean length 55.3307,
p90 176, p99 247, max 271, 27,610 anchors, 28,897 food and 20 prey. The window
ends with 500 living, mean 55.884, p90 176 and p99 247. Stock compares
5,876,276,676 warm bytes per arm and protocol 13 compares 10,132,210,980.

## Remaining mature budgets

Separate diagnostic builds, million user instructions per tick. They use the same
eligibility-vector instrumentation on both arms; they are not plain savings.
Coverage, encoding/history, recipient food/prey, block construction/verification,
whole-body bounds and grid maintenance nest inside broadcast. Compaction and
collision nest inside physics; do not add child scopes to inclusive parents.

| Scope | Baseline | Verified reuse |
| --- | ---: | ---: |
| complete host advance | 12.9357 | 11.6582 |
| world physics | 4.4916 | 4.4909 |
| food-interest advance | 0.2970 | 0.2970 |
| broadcast inclusive | 8.0992 | 6.8224 |
| snake coverage | 2.3583 | 2.3139 |
| snake encoding/history | 1.2573 | 1.2571 |
| recipient food | 0.4724 | 0.4722 |
| recipient prey | 0.2578 | 0.2578 |
| whole food compaction | 0.1722 | 0.1722 |
| collision bounds/index construction | 0.8480 | 0.8478 |
| collision query/exact tests | 2.1962 | 2.1962 |
| body-block construction / verification and rebuild | 1.5479 | 0.3244 |
| whole-body broadcast bounds | 0.5494 | 0.5493 |
| coverage grid update | 0.0893 | 0.0893 |

Verification and rebuilding remain necessary: reuse removes 1.2235M of the
1.5479M construction budget, not the whole parent. The plain complete-advance
delta is 1.2717M instructions/tick. Eligibility extraction and counter brackets
produce different compiled loops; small changes in other scopes are not an
algorithmic claim or a decomposition of that plain delta. Every complete phase
window matches its plain reference byte for byte.

Ranked remaining parents:

1. **Coverage exact tests and query bookkeeping: 2.314M instructions/tick.**
2. **Collision queries/exact geometry: 2.196M instructions/tick.**
3. **Snake encoding/history: 1.257M instructions/tick.**

These are measured upper budgets, not removable subsets. Collision queries
remain a suitable next investigation alongside further coverage rejection;
encoding needs specific attribution before claiming reusable work. Whole-body
broadcast bounds retain another 0.549M budget that could be inspected together
with the verified body snapshot, in a separate change.

## Correctness and limits

Debug/release tests pass, comparing retained coverage against the frozen original
predicate. The edit oracles include X-only and Y-only changes at every body
position, unchanged reuse, length/lifetime/slot-count transitions, nonfinite and
extreme points, signed zero and NaN payloads. Deliberately faulty length-only,
X-only and numeric-equality reuse keys fail as intended. Existing narrowed AABB
and circle-margin controls also fail; restored source passes.

The full offline Rust group passes. All 23 catalog plus four original host/wire
traces match completely, including terminal records. Stock/protocol-13 output
mutations and warm-stream bit flips are detected. Mature output mutations change
the trace while preserving both canonical physical states. All seven
documentation tests pass with the assembled report and selected results.

Baseline runtime: merged #253 `634b54d835e56fd054d220f7aab13389f93564ff`.
Cache runtime: `b12f70b9c13ff0984648579415deb9131c656b59`. The later merged
research-guide change alters no Rust/build input. All five build arms reconstruct
their complete measured Rust-source hash maps using the linked earlier builder;
all retained measured Rust sources and copied binaries still match their pins.
Compact archive/source/binary identities belong in selected results; full source
maps, binaries, samples, traces, commands and failed attempts remain local-only.

During the run, the guest's access service briefly refused a bounded SSH
status request. No configuration or security setting was changed. The most recent successful
receipt already showed the complete Rust group passing, stock warm-byte parity,
repeated-child window parity and the mature stock output mutation being detected.
Access resumed without restarting the controller; subsequent durable receipts
passed every acceptance gate. The refused status request is not a measurement.

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99, portable release,
thin LTO, one codegen unit, debuginfo 1 and locked offline dependencies.
Builds/checks may overlap unmeasured warmup, then all active jobs wait before
counts. Counted children run sequentially on CPU 0; controllers/inventories use
another CPU. Full process commands at boundaries are retained with the account
redacted before writing. No builds, tests or active warmers may overlap counted
windows. Routine control and bounded SSH status reads are the only other task
activity.

Completed mature warmups consume **1.480 CPU hours**. Available
host/fork/controller receipts total **1.531 CPU hours**, including their windows
and matched short/stream controls; compilation and the original catalog/short
matrix are separate. This is within the approximately six-CPU-hour mature-fixture
allowance. To recover disk space, the bounded Cargo target was removed after
builds and Rust checks, while the controller was parked before its paired-count
phase. Copied binaries and sources remain pinned outside it. Final process checks
confirm no task workers remain and the target is absent. Full receipts remain
local-only.

Repetitions share one warmed world history rather than five independent
histories. Instruction ranges do not provide elapsed-time or socket-capacity
confidence intervals. Synthetic fixed-work parity/performance does not establish
official capture fidelity, stationarity or real-socket qualification. No
independent review is claimed; the maintaining agent reviews before landing.
