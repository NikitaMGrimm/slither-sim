# View-circle body block rejection, 2026-10-28

The mature fixture selects tighter circle-to-block rejection over verified
bounds reuse. **The selected change reduces host instructions by 10.628% with
stock clients and 10.356% with protocol 13.** All complete stock/protocol-13
warm streams and measured windows match byte for byte. Short rows remain
essentially unchanged (0.033–0.058% fewer instructions); their small changes are
regression guard results, not a useful short-body acceleration claim.

[Selected results](results.json), [frozen plan and amendment](PLAN.md),
[attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md).

## Why this candidate

The merged #252 mature parents leave 3.661M user instructions/tick in coverage,
2.196M in collision queries and 1.548M in body-block construction. These are
nested diagnostic budgets, not predicted savings. The parent study is
[here](../2026-10-28-mature-covered-bounds/REPORT.md).

The existing block test expands an axis-aligned box by `vision + 1`. This keeps
corner blocks whose whole box is outside the view circle. For query `p` and
block bounds, compute `dx = max(min_x - p.x, p.x - max_x, 0)` and similarly
`dy`. Reject only when `dx*dx + dy*dy > (vision + 1)*(vision + 1)`. Every edge
lies inside its block's box, so the nearest box distance bounds every edge
distance from below. The existing finite-coordinate/radius domain (1e7) and
one-unit roundoff margin stay in force; extreme inputs use the original complete
walk. All surviving exact edge predicates, their arithmetic and traversal order
remain. The production change adds seven lines in `covered`, with no new stored
state.

The other lead was a complete-coordinate-verified bounds cache. Tail-relative
identity alone cannot prove unchanged interiors: `append_anchor` contracts body
points from index 3 through the tail. The cache compares all coordinates before
reuse and rebuilds all blocks after any edit, including external same-time
poses. It passes independent edit/coverage oracles, narrowed-margin and
length-only-key mutations, and the full offline Rust group.
[cache.patch](cache.patch) retains its complete production/test changes; its
archive and binaries remain local. Operation-aware incremental envelopes could
still be investigated separately.

| Exploratory candidate | M instructions/tick | Reduction vs same parent |
| --- | ---: | ---: |
| Baseline | 13.0009 | — |
| Circle rejection | 11.6191 | 10.629087% |
| Verified bounds reuse | 11.7275 | 9.795292% |

These are single exploratory stock windows from the identical mature scene, not
the final five-pair result. Both prototypes' complete window bytes and two
child/direct-parent references match. The cache cohort also compares every warm
byte; circle's exploratory warm stream is untraced. The larger measured lead is
selected, then receives fresh fully traced stock/protocol-13 cohorts and five
new pairs. No pilot count enters those pairs and no cache change is combined
with the selected production change.

## Mature and short work

Reuse the preceding full-host fixture without checkpoint restoration: seed 7,
`Config::official(500)`, explicit Official delivery, all 500 slots remote,
A4-mean core controls every six 7 ms ticks. All recipients and broadcasts run
from tick zero so food birth/pickup/palette history exists naturally. Through
180,000 ticks, 100 slots stay resident and 400 retire/rejoin one every 50 ticks.
Reset only the external admission RNG at 180,000, stop forced churn, continue to
fixed tick 193,000. The unchanged living/length screen qualifies at 499 living,
mean 55.3307, p90 176, p99 247, max 271, 27,610 anchors, 28,897 food and 20
prey. The 360-tick window ends with 500 living, mean 55.884, p90 176, p99 247.

Count only `advance_tick_with`: control generation, admission, frame draining
and trace serialization stay outside. No bot decoding/observations are counted.
Both runtimes use identical driver work. The matched short row uses ticks
0–1080, including forced construction churn; the mature window has no forced
churn. Length is not their only difference. Original circle/boost and client
catalog rows retain their own definitions. No mature 2,281-seat world is
measured.

One OS thread is asserted before forks/windows. Every child opens a fresh
current-thread PMU counter. The parent stays frozen until its final direct
continuation. Complete actions, ordered frames/times, canonical physical states
and host RNG/lifecycle endpoints match across repeated children and both direct
parents. Hashes identify artifacts; full byte comparisons establish acceptance.
Warm streams are compared live using bounded buffers, not stored on the small
disk. Stock compares 5,876,276,676 bytes/arm and protocol 13 compares
10,132,210,980. Full window traces and mismatch witnesses remain local.

| Selected row | Before → after, M instructions/tick | Median reduction | Pair range |
| --- | ---: | ---: | ---: |
| Mature stock | 12.9999 → 11.6183 | 10.628302% | 10.628277–10.628342% |
| Mature protocol 13 | 13.3385 → 11.9571 | 10.356156% | 10.356152–10.356161% |
| Matched short | 6.7069 → 6.7044 | 0.036584% | 0.036557–0.036680% |

| Original short row | Median reduction | Pair range |
| --- | ---: | ---: |
| `official500` | 0.048199% | 0.046618–0.049479% |
| `official500-legacy` | 0.037345% | 0.032922–0.037462% |
| `official2281` | 0.037470% | 0.036312–0.039337% |
| `client-official-500` | 0.057735% | 0.057197–0.058120% |

Each row has one discarded AB warmup plus five adjacent AB/BA/AB/BA/AB pairs.
Host rows count complete advances; `client-official-500` counts its complete
fixed command. All pairs and work counts are retained. These repetitions share
one warmed world history rather than five independent histories. Ranges describe
instruction counts, not elapsed-time or socket-capacity confidence intervals.

## Remaining mature budgets

Separate diagnostic builds, million user instructions per tick. The phase builds
have identical eligibility-vector instrumentation on both arms; they are not
plain savings. Coverage, encoding, recipient food/prey, block construction,
whole-body bounds and grid maintenance nest inside broadcast. Compaction and
collision nest inside physics; do not sum children into their inclusive parent.
Every diagnostic window matches its plain reference completely.

| Scope | Baseline | Circle rejection |
| --- | ---: | ---: |
| complete host advance | 14.2376 | 12.9347 |
| world physics | 4.4912 | 4.4912 |
| food-interest advance | 0.2970 | 0.2970 |
| broadcast inclusive | 9.4017 | 8.0988 |
| snake coverage | 3.6613 | 2.3583 |
| snake encoding/history | 1.2571 | 1.2571 |
| recipient food | 0.4723 | 0.4723 |
| recipient prey | 0.2579 | 0.2579 |
| whole food compaction | 0.1722 | 0.1722 |
| collision bounds/index construction | 0.8477 | 0.8477 |
| collision query/exact tests | 2.1962 | 2.1962 |
| body-block construction | 1.5478 | 1.5478 |
| whole-body broadcast bounds | 0.5493 | 0.5493 |
| coverage grid update | 0.0893 | 0.0893 |

The diagnostic coverage delta is 1.303M instructions/tick; the plain complete
advance delta is 1.382M. Eligibility extraction and counter brackets produce a
different compiled loop, so these are not a plain-delta decomposition.

1. **Coverage exact tests and query bookkeeping: 2.358M instructions/tick.**
2. **Collision queries/exact geometry: 2.196M instructions/tick.**
3. **Body-block construction: 1.548M instructions/tick.**

These parents are upper bounds for future changes, not removable subsets. The
cache pilot's 9.795% belongs to the pre-circle baseline; it is not a measured
saving on this new baseline. Encoding/history and remaining broad-phase work
also need specific attribution before claiming a memoizable subset.

## Supplemental instruction-IP diagnosis

A user-only perf ring samples only host-advance scope 2 on the retained #252
phase runtime. Its runtime sources/manifests/lockfile/build script match merged
main, and every sampled window matches the plain reference. A 512 KiB ring uses
predeclared periods 1,000,003 and 2,000,003. Two fork children/direct parent at
the first period and a separate direct parent at the second provide 18,025
samples. Events reject loss, throttling, overflow, incomplete running coverage
and a sampled-weight discrepancy exceeding 2%. Inline source frames use the PIE
image base. Raw maps/addresses stay private; no unknown IP is assigned a caller.

| Selected sampled bucket | Share range across windows |
| --- | ---: |
| body bounds construction | 11.15–11.65% |
| coverage projection/exact edge tests | 11.61–13.17% |
| coverage bounds/query bookkeeping | 4.23–5.22% |
| collision exact tests | 8.50–9.59% |
| Unmapped IP, unassigned | 4.21–5.18% |

These coarse shares include sampling/instrumentation overhead and skid. Listed
code buckets are selected source groups, not complete phase parents; other
executable/library weight remains explicit in results.json. They support
inspection, while exact phase counters and plain pairs determine the result. The
first 64 MiB ring mmap fails before any accepted window; no errno was retained.
The smaller ring passes a short probe before repeating warmup. All original
failure/source records remain, with no system or limit changes.

## Correctness, pins and limits

Debug/release frozen-original coverage oracle passes, including over 160,000
random/adjacent-radius queries and new 3-4-5 endpoint cases at box corners and
the last partial block. Nonfinite/extreme, degenerate, short, signed-zero and
lifetime cases retain the previous regressions. Narrowed AABB and new
circle-radius mutations fail; restored source passes. The complete offline Rust
group passes. All 23 catalog plus four original host/wire traces match complete
decompressed bytes and terminal records. Stock/protocol-13 output perturbations
and warm-stream bit flips are detected. Both mature radius mutations change
output while both canonical physical states remain identical.

Base `85e745096e533143dedfb6c1948b697781e697e9`; selected runtime
`5904e1f941facb5d994dbb520cb8527c94c19af6`. The cache prototype is retained
locally at `0b071fb2` and reconstructible from its patch against the baseline.
Archive, manifest, lockfile, generated Rust and binary hashes are selected in
results.json. All five selected build arms reconstruct exactly; later
documentation commits do not relabel another runtime. The canonical LF fixture
reconstructs the frozen terminal CRLF payload explicitly. Original wrapper
hashes identify historical Python sources separately from the published
equivalent recipe.

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99, portable release, thin
LTO, one codegen unit, debuginfo 1, locked offline dependencies. Builds/checks
may share unmeasured warmup, then all active jobs wait before counts. Counted
children run sequentially on CPU 0; controllers/inventories use another CPU.
Full process commands at boundaries are retained with the account redacted
before writing. No builds, tests or active warmers run during counted windows.
Routine control and bounded SSH status reads are the only other task activity.

Completed mature warmups, including the failed sampler and both exploratory
candidates, consume **2.702 CPU hours**. Available host/fork/controller receipts
total **2.772 CPU hours**, covering measured windows and short controls as well.
This remains inside the approximately six-CPU-hour fixture allowance;
compilation is separate. Full per-cohort receipts are retained.

The one bounded Cargo target was removed after verification; copied binaries,
sources, complete windows, process inventories and failed attempts remain local.
Synthetic fixed-work parity/performance does not establish official capture
fidelity, stationarity, elapsed-time capacity or real-socket qualification. No
independent review is claimed; the maintaining agent reviews before landing.
