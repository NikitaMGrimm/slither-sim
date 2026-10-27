# Recipient prey cells, 2026-10-27

Keep shared x-axis cells for recipient prey lookup. Main #219
`aee542100496b8cbc031b8ab19bb58e78bd8af2b` -> measured/tested candidate
`2496288635d5d5fc70ed04ca71e79bf8ef111653`. Only production change:
`src/client/simulation/prey_visibility.rs`. No edits to broadcast, snake updates,
coverage or the socket server; the capacity worker was notified of the boundary.
The constructor/query API is unchanged, limiting arena-vision conflicts.
[Selected identities and results](results.json).

## Narrow profile before selecting the change

Current-thread user-instruction scopes use 853 official-500 steady ticks:
tick >=120, all 500 alive, excluding forced reconnect tick720. A shared-build
scope and 500 recipient scopes bracket lookup/exact tests and bookkeeping/packet
work separately. Scopes add overhead and affect compilation; the nested total
includes their cost. These are diagnostic budgets, not predicted unprofiled
savings. Profiled and control raw traces match for all three protocol variants.
The control is the default-target #218 study executable; #219's opt-in full-map
fixture is outside these three cases, and every output byte still matches.

| Prey phase | Base / candidate, M instructions per steady stock tick |
| --- | ---: |
| Shared bounds/index construction | 0.036 / 0.054 |
| Recipient lookup, exact tests and visible-map insertion | 0.625 / 0.381 |
| Recipient bookkeeping and packets | 0.321 / 0.321 |
| Nested recipient total, including probe overhead | 1.026 / 0.783 |

Lookup/exact work was the largest measured prey component, motivating the index.
Mixed/legacy lookup remains 0.625 / 0.381M and bookkeeping 0.321 / 0.321M;
shared construction ranges 0.036–0.038 / 0.056–0.060M. The extra shared index
maintenance is included in whole-process measurements below. No per-query
allocation or sort is introduced. Packet/correction clocks are unchanged.

## Exact candidate order and numeric coverage

Retain each original `[x0,y0,x1,y1]` interval, including its conservative
`reach=(2*vision).max(1)` slack. Append its world index to every x cell from
`(x0/reach) as i64` through `(x1/reach) as i64`, inclusive. Division by a positive
finite reach and the truncating/saturating cast are monotone: a head inside the
original interval maps between those endpoints. The query selects one cell,
then applies the original bounds comparisons. Thus its filtered candidates
match the exhaustive broad phase in both membership and order.

Every cell receives each prey once, in world order. This preserves the caller's
first-visible duplicate-ID winner and subsequent ID-ordered appearance/removal
passes. The caller's exact `hypot` comparison remains untouched; correction,
eater IDs, removal/appearance order, death handling and RNG use do not change.
The rationale stays beside the index construction and query.

Fewer than 16 prey, non-finite size/x bounds, an overflowing cell span, or a span
of at least 256 cells retains the exhaustive path. Non-finite query x also scans
exhaustively. These select a broad phase, not a gameplay rule. Bounds rebuild
from the current prey slice on every broadcast, so motion, insertion, removal
and reordering need no persistent cache synchronization.

## Full traces and whole-process pairs

All twelve full raw traces are byte-identical: official-500 stock/half/all
protocol-13; historical portable/protocol, official-500, Battledome-64 and both
retained-client interfaces; separate Battledome 40/64/80. Checks include frames,
ordering, timestamps, complete initial/final checkpoints, RNG endpoints and
terminal/population summaries. No tolerance or new normalization is used.

One untallied warmup pair, then five pairs alternating base/candidate and
candidate/base for each protocol. These count the entire 1,080-tick fixture,
including admission/reconnect and all construction/allocation. Same default
x86-64 release target, Rust 1.99.0, thin LTO, one codegen unit, debuginfo=1 and
default features on both sides. Unlike the separate CPU-target study, the
instruction mix target is fixed. No sample is excluded or replaced.

| Clients | Base median instructions | Candidate median | Median paired reduction | Full paired reduction range |
| --- | ---: | ---: | ---: | ---: |
| Stock | 12,083,296,324 | 11,813,012,782 | 2.235% | 2.231–2.256% |
| Half protocol-13 | 12,431,933,826 | 12,169,927,157 | 2.104% | 2.098–2.129% |
| All protocol-13 | 12,781,364,066 | 12,516,768,379 | 2.072% | 2.056–2.081% |

Shared WSL2 on an AMD Ryzen 7 9800X3D, pinned to one logical CPU with one
Cargo/Rayon/test job. User counters only; security settings remain unchanged.
No external game servers, administrator changes or deployment. These fixed
replay instruction counts establish no CPU-time, latency, capacity or native
EPYC transfer claim. Other populations/distributions have no measured speed
claim here.

## Checks and reproduction

The randomized/boundary oracle compares the ordered candidate list against the
original exhaustive bounds scan, then independently compares exact visible-ID
maps and first duplicate winners. It exercises inclusive cell/visibility and
ULP edges, negative/zero cells, huge/saturating coordinates, sparse/small and
non-finite fallbacks, plus rebuilding after motion/removal/reordering/insertion.
Existing original visibility and rebuild regressions remain.

All four focused tests and the full Linux Rust group (15 commands) pass, as do
the seven documentation tests. A scratch mutation changing inclusive cell
insertion `..=` to `..` makes the randomized/boundary oracle fail; the selected
runtime is untouched. Portable profile preparation reproduces all seven measured
overlay files byte-for-byte. All six Python entry points (three study tools and
three historical children) reject `-O`. Only research/changelog integration is
added after the measured/tested production revision.

```sh
STUDY=research/performance/2026-10-27-host-prey-cells
OUT=/tmp/slither-prey-new
python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" trace "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" count "$PWD" "$OUT" --cpu 3 --perf /path/to/perf
python3 "$STUDY/profile.py" aee542100496b8cbc031b8ab19bb58e78bd8af2b /tmp/prey-profile-base --cpu 3 --control "$OUT/native-base/bin/study_protocols"
python3 "$STUDY/profile.py" 2496288635d5d5fc70ed04ca71e79bf8ef111653 /tmp/prey-profile-candidate --cpu 3 --control "$OUT/native-candidate/bin/study_protocols"
cargo test --locked --lib prey_visibility
python3 tools/check.py rust
```

Python 3.12+, Rust 1.99.0, patch and Linux x86-64 user PMU access are required.
Reproducers reuse the pinned recipient-work fixture; all Python entry points,
including historical children, reject optimization mode. Optional task-owned
Cargo caches affect storage only; immutable copied binaries are rehashed.
Raw arrays, archives, executables and full traces stay local-only outside Git
under the task's scratch output; selected summaries/hashes are committed.
