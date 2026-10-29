# Exact lazy head edge blocks in Battledome spawn searches

Keep the Safety-owned lazy table. Across five alternating pairs per protocol,
the exhausted row uses **63.18% fewer user instructions** than main, retaining
almost all of the eager version's saving. Median per-window CPU p99 falls
**1.466 → 0.645 ms (protocol 19)** and **1.462 → 0.643 ms (13)**, with separated
five-window ranges. First-use work remains inside these counters.

The requested successful-search CPU bar is met on the 80/160 controls: all
four mean-CPU ranges overlap main and every paired ratio range crosses 1.
Median paired CPU changes are −1.04%, +0.79%, −0.34% and −0.53% for
80/19, 80/13, 160/19 and 160/13. Search instructions change from −0.003% to
+0.026%, instead of the eager version's +4.5–6%. Construction of the unqueried
Safety pair returns to about **28 µs at 80** and **56 µs at 160**, against
eager's 51/105 µs. These are bounded synthetic results, not live capacity.

The initial [eager result](EAGER.md) and its selected JSON remain unchanged
at the study root. This revision compares main `e860ec23`, reviewed eager
head `78f7d1ec` and lazy candidate `3d2965f5` in one session. The source proof
and final results for this revision are under [lazy/](lazy/verification.json).

## Change and exactness

The [portable baseline profile](profile.json) selected head edge scans:
74.9% of samples were in the predicate, including 62.5% in projection/
comparison. Eight-edge bounds then saved about 63% of exhausted-row
instructions, but building them for every obstacle made successful searches
more expensive. The maintaining agent requested the lazy revision before
landing; [revision plan](lazy/PLAN.md), [attempts](lazy/ATTEMPTS.md).

Safety now holds a lazy table of unsynchronized `OnceCell<Vec<Bounds>>`
entries, indexed by its immutable obstacle order. Construction leaves the
whole table absent. Only a supported query reached after the original outer
`near` check creates the table; only the visited entry initializes bounds.
Later calls reuse them. Unsupported head/width queries retain the linear
path without building. The obstacle definition is identical to main's;
cache state lives outside that array. Bounds contain immutable geometry,
independent of head, width and prior verdict. Body/departure predicates are
unchanged.

The original conservative guard is unchanged: finite endpoints/query/width
in the `1e7` domain, blocks of eight edges (including the partial last block),
and per-axis separation with `head_width + 1`. The closest projected point
lies inside the edge/block box apart from roundoff far below one unit in
this domain. A skipped block therefore cannot satisfy the inclusive exact
distance test. Borderline boxes test their edges, and extreme/non-finite
data retains the original loop. Edge order, poses, random draws, admission,
refusals and retry timing stay identical. Cache mutation changes no verdict.

All seven focused tests pass: the independent raw-geometry oracle retains
1,223,404 head verdicts, 50,159 direct block verdicts, 409,996 segment and
19,992 swept comparisons, plus explicit eager/lazy equivalence, untouched-cache
and reuse checks. The wrong-skip mutation fails; exact source restoration
passes. All 15 Rust commands pass. The 23 catalog, six host and two queued
traces retain complete bytes/terminal records; both wire controls differ in
actual frames. All measured three-arm traces and direct-parent/repeated-fork
proofs match. Every counted tick has a 1.0 PMU scheduled fraction.

## Portable fixed-work results

| Row | Protocol | Instructions change median [range] | CPU p50 base → candidate ms | CPU p99 base → candidate ms | CPU max base → candidate ms |
| --- | --- | ---: | ---: | ---: | ---: |
| spawn | 19 | -63.18% [-63.18, -63.18] | 1.392 → 0.614 | 1.466 → 0.645 | 1.583 → 0.814 |
| spawn | 13 | -63.18% [-63.18, -63.18] | 1.388 → 0.615 | 1.462 → 0.643 | 1.569 → 0.824 |
| seats80 | 19 | -0.01% [-0.01, -0.01] | 0.148 → 0.147 | 0.349 → 0.349 | 0.825 → 0.830 |
| seats80 | 13 | +0.01% [+0.01, +0.01] | 0.147 → 0.146 | 0.397 → 0.402 | 0.837 → 0.836 |
| seats160 | 19 | +0.00% [+0.00, +0.00] | 0.451 → 0.448 | 0.877 → 0.877 | 1.376 → 1.370 |
| seats160 | 13 | -0.00% [-0.00, -0.00] | 0.452 → 0.450 | 1.050 → 1.064 | 1.423 → 1.394 |

| Row / protocol | Group | Instructions change median [range] | CPU p99 base → candidate ms |
| --- | --- | ---: | ---: |
| spawn / 19 | search | -64.23% [-64.23, -64.23] | 1.564 → 0.756 |
| spawn / 19 | exhausted | -64.23% [-64.23, -64.23] | 1.564 → 0.756 |
| spawn / 19 | other | -0.01% [-0.01, -0.01] | 0.045 → 0.041 |
| spawn / 19 | slowestCpu1Percent | -60.64% [-60.74, -60.37] | 1.583 → 0.814 |
| spawn / 13 | search | -64.23% [-64.23, -64.23] | 1.551 → 0.765 |
| spawn / 13 | exhausted | -64.23% [-64.23, -64.23] | 1.551 → 0.765 |
| spawn / 13 | other | -0.00% [-0.00, -0.00] | 0.044 → 0.043 |
| spawn / 13 | slowestCpu1Percent | -60.35% [-60.43, -60.10] | 1.569 → 0.824 |
| seats80 / 19 | search | -0.00% [-0.00, -0.00] | 0.334 → 0.338 |
| seats80 / 19 | other | -0.01% [-0.01, -0.01] | 0.341 → 0.342 |
| seats80 / 19 | slowestCpu1Percent | -0.23% [-0.23, +7.24] | 0.825 → 0.830 |
| seats80 / 13 | search | +0.03% [+0.03, +0.03] | 0.300 → 0.310 |
| seats80 / 13 | other | +0.01% [+0.01, +0.01] | 0.397 → 0.402 |
| seats80 / 13 | slowestCpu1Percent | +0.00% [+0.00, +0.00] | 0.837 → 0.836 |
| seats160 / 19 | search | -0.00% [-0.00, -0.00] | 0.702 → 0.703 |
| seats160 / 19 | other | +0.00% [+0.00, +0.00] | 0.877 → 0.877 |
| seats160 / 19 | slowestCpu1Percent | +0.06% [+0.06, +0.06] | 1.376 → 1.370 |
| seats160 / 13 | search | +0.01% [+0.01, +0.01] | 0.748 → 0.749 |
| seats160 / 13 | other | -0.00% [-0.00, -0.00] | 1.050 → 1.064 |
| seats160 / 13 | slowestCpu1Percent | -0.02% [-0.02, -0.02] | 1.423 → 1.394 |

| Row / protocol | CPU p50 range base / candidate ms | CPU p99 range base / candidate ms |
| --- | ---: | ---: |
| spawn / 19 | 1.385–1.394 / 0.612–0.615 | 1.462–1.467 / 0.642–0.658 |
| spawn / 13 | 1.386–1.392 / 0.612–0.615 | 1.459–1.464 / 0.641–0.648 |
| seats80 / 19 | 0.147–0.150 / 0.147–0.147 | 0.340–0.384 / 0.338–0.359 |
| seats80 / 13 | 0.147–0.147 / 0.146–0.147 | 0.393–0.409 / 0.399–0.412 |
| seats160 / 19 | 0.451–0.451 / 0.448–0.449 | 0.872–0.881 / 0.870–0.893 |
| seats160 / 13 | 0.451–0.453 / 0.449–0.451 | 1.045–1.055 / 1.055–1.071 |

| Row / protocol | Whole two-Safety CPU median base → candidate µs | Paired CPU ratio median [range] | Candidate blocks alone / one Safety µs |
| --- | ---: | ---: | ---: |
| spawn / 19 | 6.992 → 6.918 | 0.988 [0.895, 1.116] | 3.077 |
| spawn / 13 | 7.017 → 6.928 | 0.989 [0.887, 1.001] | 3.071 |
| seats80 / 19 | 27.567 → 27.866 | 1.005 [1.001, 1.047] | 9.106 |
| seats80 / 13 | 27.281 → 26.673 | 0.969 [0.928, 1.030] | 9.330 |
| seats160 / 19 | 56.335 → 56.412 | 1.002 [0.987, 1.016] | 18.370 |
| seats160 / 13 | 55.895 → 55.735 | 0.997 [0.996, 1.051] | 18.918 |

| Successful row / protocol | Main / eager / lazy mean CPU/search µs, median [window ranges] | Lazy/main CPU ratio median [range] | Lazy/main search instructions change |
| --- | ---: | ---: | ---: |
| seats80 / 19 | 276.139 [270.474, 281.409] / 288.794 [283.400, 292.286] / 272.236 [270.765, 279.587] | 0.9896 [0.9805, 1.0315] | -0.003% |
| seats80 / 13 | 269.095 [267.352, 270.461] / 299.477 [298.603, 308.178] / 269.773 [266.074, 271.231] | 1.0079 [0.9838, 1.0101] | +0.026% |
| seats160 / 19 | 621.274 [619.221, 625.367] / 688.599 [683.286, 690.148] / 621.309 [618.724, 622.825] | 0.9966 [0.9958, 1.0031] | -0.000% |
| seats160 / 13 | 671.121 [665.551, 671.764] / 702.084 [694.852, 709.535] / 667.478 [662.556, 669.161] | 0.9947 [0.9872, 1.0029] | +0.007% |

| Row / protocol | Eager → lazy total instructions | Eager → lazy CPU p99 ms | Whole two-Safety build CPU main / eager / lazy µs |
| --- | ---: | ---: | ---: |
| spawn / 19 | +0.663% | 0.648 → 0.645 | 6.992 / 13.900 / 6.918 |
| spawn / 13 | +0.663% | 0.648 → 0.643 | 7.017 / 13.909 / 6.928 |
| seats80 / 19 | -0.110% | 0.354 → 0.349 | 27.567 / 50.925 / 27.866 |
| seats80 / 13 | -0.092% | 0.413 → 0.402 | 27.281 / 50.796 / 26.673 |
| seats160 / 19 | -0.100% | 0.893 → 0.877 | 56.335 / 104.511 / 56.412 |
| seats160 / 13 | -0.073% | 1.066 → 1.064 | 55.895 / 104.713 / 55.735 |

CPU quantile cells are medians of five per-window quantiles, including maxima,
not pooled quantiles. Successful-search CPU means are first computed over the
five successful-search ticks in each control window, then summarized across
the five windows. All individual pairs and warmups remain in
[main/lazy measurements](lazy/measurements.json),
[eager/lazy measurements](lazy/eager-measurements.json) and
[construction](lazy/construction.json). Each arm's slowest CPU 1% is selected
independently; its tick indices may differ. Phase quantiles are not added.

The first per-obstacle lazy-cell attempt removed the instruction tax but
failed CPU acceptance: successful-search means rose 8.3% at 80/13 and 5.0%
at 160/19, with separated ranges. Its [tables](lazy/first/TABLES.md), selected
JSON, [retrieval receipt](lazy/first/retrieval.json) and private verified
archives are retained. Moving cells into the lazy table preserves main's
obstacle definition and meets the specified mean-CPU bar in the second run.
This does not establish the first shift's microarchitectural cause.

No CPU speedup is claimed for successful searches or ordinary ticks. Small
instruction differences remain measurable. Whole-window 160/13 p99 also
rises from 1.050 to 1.064 ms; its ranges overlap narrowly (main 1.045465–
1.055195, lazy 1.054830–1.070862 ms). Other control p99 ranges overlap too.
These five-window comparisons do not prove universal equivalence of timing
or absence of a live regression; the byte/rule exactness claim is separate.

## Method and limits

All three arms use the identical #272 synthetic fixed-host fixture. The
exhausted row holds seven rings, an observer and 72 waiting requests: warm
300 ticks, count 360 ticks, 180 exhausted searches of 4,096 poses and 72
waiters remaining. The separated 80/160-viewer controls prime the arena via
normal seat growth, then perform five successful reconnects/poses with no
backlog and all viewers alive. No naturally grown bots or real sockets run.
Held-pose resets, reconnect insertion and trace serialization stay outside
per-tick counters; queue/admission, world/broadcast, delivery hand-over and
all actual first-use cache construction stay inside them.

One discarded three-arm warmup precedes five alternating sequences:
main/eager/lazy, lazy/eager/main, repeated. Both main/lazy and eager/lazy
comparisons therefore alternate AB/BA five times. Every arm's repeated-fork
and direct-parent traces match, and every measured full-state/byte window
matches across all three. The complete catalog/host/queued matrix and wire
controls compare main against lazy. Eager's additional evidence is the full
fixed-row comparison, not a newly repeated independent Rust check group.

Separate construction windows repeatedly create/drop preferred and departure-
floor Safety objects without querying them. For lazy these measure unbuilt
caches; first use remains charged in the main rows. Sixteen construction
pairs per sample and 64 samples per window amortize clocks/counters, with
five alternating three-arm windows after warmup. The candidate-only raw
blocks diagnostic separately builds/drops bounds over an existing obstacle
edge set, one Safety, 16 repeats × 64 samples. It is one descriptive window,
not a timing subtraction or a claim that initialization is free. World state
must remain unchanged during construction diagnostics.

Default portable release profile, Rust 1.99, locked/offline builds, no native
CPU or other Rust flags. One pinned virtualized Intel core runs children on
a Frankfurt Spot builder; another assigned physical core runs the controller,
with SMT disabled at launch. No concurrent builds, tests, probes or reduction
run during counting. Hardware `instructions:u` requires >=99% scheduled time
per tick, without kernel/security changes. The SSH guest remains untouched;
no local native work or heavy reduction runs. No live capacity, network latency
or 320/480-seat claim follows. No independent agent review was performed.

The raw-geometry oracle, byte controls and numerical guard establish the stated
exactness evidence; synthetic performance rows do not establish product
capacity. Complete logs, sources, binaries, traces and raw samples remain
local-only; archive digests do not provide public access. Lean selected data
recomputes with [lazy/tables.py](lazy/tables.py), reusing the original harnesses
by reference; [revision reproduction](lazy/REPRODUCE.md).

## Retrieval, cost and cleanup

Both lazy attempts together have a **$0.40 conservative bound**: 45 rounded
launch-to-final-scan minutes at the $0.20/h bid cap plus $0.25 for storage,
transfer and IPv4. This is a bound, not an invoice. The prelaunch full-timer
bound was $0.657, below the $1 cap. The original eager study's separate $0.35
bound remains unchanged; their sum is $0.75. [Revision cost](lazy/cost.json).

The 120-minute in-guest poweroff timer, terminate shutdown behavior and root
DeleteOnTermination were verified. Spot service-terminated the builder after
complete proof/timing archive retrieval, before its timer. No source/result was
lost: an optional later table transfer failed, but the table text was already
inside the verified final delta. The task key/group were deleted and protected
resources left untouched. At 18:58:39 UTC all 17 enabled regions scanned clean:
zero nonterminated instances, open/active Spot requests, volumes (including
available), Elastic IPs and project/task keys/groups. [Cleanup](lazy/cleanup.json).

Every archive was closed, tar/gzip-read and fully member-hash-verified remotely
before readiness. Downloads were size/SHA-verified, and selected JSON and tables
bound to the final manifest. Proof sources were retrieved before timing; the
final delta references that immutable archive. All first-attempt data survives
with its own receipt/pins. No local native work or guest use occurred.
