# Mature simulation cache and memory diagnostic

**Sparse recipient snake history is the strongest portable layout lead.** At
500 recipients it reserves **20.38 MB** for **176.7 kB of occupied histories**.
Snake visibility/history paths account for 28% of ordinary-tick cycle samples
and 44% of dTLB-miss samples with stock clients. The evidence supports reducing
that working set, but does **not establish that the whole thread is primarily
limited by memory latency or bandwidth**: direct L2/LLC-load events and backend
pipeline metrics are unavailable, and samples lack data addresses.

No production change, speedup or capacity claim is delivered. All MB/kB below
are decimal; cache capacities use KiB/MiB.

[Counter results](counter-results.json), [profiles](profile-results.json),
[footprints](footprint-results.json), [event controls](events.json),
[verification](verification.json), [table calculation](tables.py),
[frozen plan](PLAN.md), [sampling correction](SAMPLING.md),
[reproduction](REPRODUCE.md).

## Scope and counters

Current-main runtime `539b37f3be969827b5eac67c2978e604f9e814e8`, Rust 1.99,
portable release/thin LTO, one codegen unit, debug symbols, no affinity, on
a 4-vCPU Zen 5 EPYC KVM guest. All builds, tests and reductions ran there.
The unchanged full-backend fixture warms at full speed to tick 193,000 and
forks the entire backend for every repetition. This preserves hidden births,
presentation lifetimes, held listings and sector interest; a physics checkpoint
alone cannot continue this path. Both protocols have 500 remote recipients,
without sockets, in-process decoding or wall-clock pacing.

Three 1,080-tick repetitions per event/protocol each contain **15 maintenance
and 1,065 ordinary ticks**, identified by actual food-epoch transitions. Only
`advance_tick_with` is gated; policy, respawn/admission preparation, frame
draining and serialization are outside. No measured repetition was excluded.
Counter-only results below are medians of the three run means. CPU includes
clock/PMU wrappers and is descriptive. Profiles are separate runs.

| Protocol / tick | M instructions | M cycles | IPC | L1d misses / k instructions | dTLB misses / k instructions | Generic cache misses / k instructions | Branch misses / k instructions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 19 ordinary | 10.173 | 7.775 | 1.308 | 28.935 | 1.110 | 9.226 | 4.586 |
| 19 maintenance | 19.785 | 16.844 | 1.175 | 19.625 | 1.013 | 8.528 | 4.622 |
| 13 ordinary | 10.496 | 8.394 | 1.250 | 28.252 | 1.156 | 9.647 | 4.572 |
| 13 maintenance | 21.124 | 17.621 | 1.199 | 18.711 | 1.032 | 8.689 | 4.455 |

Stock ordinary/maintenance CPU means span **1.762–1.816 / 3.818–3.897 ms**;
legacy spans **1.856–1.935 / 4.005–4.028 ms**. These are instrumented, no-socket
rows, not live-world tick budgets. Stock L1d misses rise from **294,349** per
ordinary tick to **388,277** per maintenance tick, and dTLB misses from
**11,295** to **20,034**. Misses per instruction decrease on maintenance while
total work rises. That does not identify the extra work as a memory stall.

All counter-only enabled/running fractions are one. Instructions, cycles,
branch misses, L1d read misses, dTLB read misses, generic cache misses and
frontend stalled cycles open successfully. The exact Linux selectors and
positive controls are retained in `events.json`. The generic LLC-read-miss
selector fails with `ENOENT`; no L2 alias is exposed. The available generic
`cache-misses` event is **not relabeled as LLC misses**, and a dTLB miss is
not equated with a page walk. `perf` is absent, including the checked existing
alternate locations, so no `perf stat -M` top-down metrics were available.

Eight million dependent loads over 4 KiB versus 64 MiB produce about 86 versus
8.21M L1d misses, 2 versus 7.98M dTLB misses, and 40.0M versus 478.1M cycles,
with almost identical 24.0M instructions. This establishes responsive counters,
not their complete architectural semantics under KVM.

The [capacity checkpoint](../2026-10-28-capacity-checkpoint/REPORT.md) measured
different runtime revisions, native code generation and asynchronous real
sockets. Its 0.93 IPC is not reproduced by this 1.17–1.31 IPC row. This study
cannot causally explain the historical 58%-instruction/31%-CPU discrepancy.

## Attribution and its limits

Cycle and dTLB profiles have no throttle or lost records. Initial L1d/generic
cache profiles were throttled despite enabled/running fractions of one. They
remain recorded; their location shares are not primary attribution. A committed
[correction](SAMPLING.md) repeats only those profiles at period 10,007 rather
than 1,009, with fresh identical mature backends. No counter result is replaced.
The lower-rate profiles are unthrottled and lossless. Snake paths lead ordinary
L1d samples at **35.81% / 36.19%** for stock/legacy and generic-cache samples at
**41.64% / 37.76%**. Stock maintenance has **28.62%** of L1d samples in snake
paths and **15.84%** in food, versus **27.57% / 26.84%** for the generic event.
The stock lower-rate profiles contain 31,501 ordinary/587 maintenance L1d
samples and 10,758/272 generic-cache samples. This corroborates the history/
visibility lead without turning generic misses into a measured cache level.

| Stock sample owner | Ordinary cycles | Maintenance cycles | Ordinary dTLB | Maintenance dTLB |
| --- | ---: | ---: | ---: | ---: |
| Snake visibility/history/bounds | 27.92% | 9.80% | 43.73% | 25.74% |
| Recipient/shared food | 8.29% | 29.80% | 8.81% | 27.06% |
| Subscription interest | 0.90% | 14.29% | 0.60% | 6.60% |

Denominators include unresolved and non-crate samples. Ordinary cycle/dTLB
profiles contain 7,816/12,099 samples; maintenance has only 245/303, so its
percentages are coarse diagnostics. Legacy shows the same broad pattern:
snake paths hold 25.63% of ordinary cycle and 41.99% of dTLB samples; food and
subscription interest together hold 39.16% of maintenance cycle samples.

The hottest stock ordinary dTLB source sites are `snakes.rs:180` (2,302
samples, the no-op path assigning the front anchor) and `snakes.rs:304` (802,
`KnownSnakes::get_mut`). The former involves both a body-front read and a
history write: without sampled data addresses it cannot all be charged to
either allocation. These two sites alone account for 25.65% of delivered dTLB
samples. Food samples include held-sector binary search, `Arc` revision/handle
updates and maintenance membership changes; interest samples include the
retained-sector sweep.

Overflow IPs are imprecise (`max_precise=0`), without call chains. Static inline
stacks identify source owners; available dynamic symbols identify some library
functions. Unresolved library/internal symbols remain in the denominator.
Sampling skew and instruction skid prevent interpreting an IP share as exact
stall time or a structure's share of all memory traffic. The process is
unpinned, so migration/cache-state effects are not isolated. Fork copy-on-write
startup remains included; there is no new production-distribution claim.

## Retained data and touched-set model

Both protocols have the same measured snapshot for these structures. Counts
and capacities are measured; allocator and tree-node overhead are excluded.
The slot array includes connection headers and is not additive with them.

| Data | Observed shape / access | Payload / reserved vector storage |
| --- | --- | ---: |
| Authoritative bodies | 27,618 `Point`s, 16 bytes each; contiguous within 500 separately owned bodies | 0.442 / 0.613 MB |
| Body-bound witnesses | 26,734 copied points, coordinate-bit verification; changed bodies rebuild | 0.428 / 0.895 MB |
| Body blocks | 3,464 bounds, 32 bytes each, queried repeatedly by visibility | 0.111 / 0.220 MB |
| Known snakes | 243,903 logical `Option<KnownSnake>` slots; 363,951 capacity; 3,681 occupied | 13.659 / 20.381 MB; occupied records 0.177 MB |
| Shared food listings | 8,490 unique `Arc` allocations, 28,307 entries; held/current identities deduplicated | 0.906 / 1.357 MB |
| Held sector handles | 30,199 sorted key/`Arc` pairs, 16 bytes each; sparse binary search or full membership walk | 0.483 MB |
| Food index | 28,897 entries in 7,195 sector vectors; 56 bytes per entry, 86,384 total capacity | 1.618 / 4.838 MB |
| Subscription history | 30,199 timestamps plus duplicate set membership keys | at least 0.362 MB, before tree nodes |
| Per-recipient headers | `Slot` 1,944 bytes, `Connection` 664 within it; 500 slots | 0.972 MB slot array |

`KnownSnake` is 48 bytes; `Option<KnownSnake>` is 56. The current known-slot
list uses a further 63.6 kB of vector capacity. Physical food itself has at
least 1.387 MB of payload. The shared 65,536-entry listing table reserves
0.524 MB and subscription counts 0.262 MB; both are indexed sparsely rather
than scanned in full each tick. Legacy adapter heap state, presentation maps,
transient frames/packets and tree/allocator overhead are outside this subtotal.

Authoritative bodies, witness points, block bounds and occupied history total
**1,157,168 bytes** of nominal active payload: **4.41 / 2.21 / 1.10 / 0.55**
times 256 KiB / 512 KiB / 1 MiB / 2 MiB. Source loops stream bodies for physics
and broadcast bounds, verify retained witnesses, and revisit body blocks and
occupied histories per recipient. This is a payload model, not a measured
unique-address working set: equality checks can short-circuit, padding/cache
lines add traffic, and not every record field is read.

Food maintenance additionally touches a changing subset of the 1.618 MB live
food-index entries, listing payloads and recipient trees/handles. No exact
unique-byte total was measured. Conversely, the 20.38 MB history reservation is
**not** scanned per tick: the occupied-slot list avoids that. Sparse addresses
can still spread a small useful payload over many pages/cache lines. Total
retained bytes, RSS, capacity and touched bytes are distinct quantities.

## Portable candidates

1. **Compact occupied snake histories behind a small slot index.** This has the
   strongest combination of lookup/update samples and measured waste. A u32
   slot-index payload plus the current occupied records models **1.152 MB**,
   versus **13.659 MB** of logical optional slots: about **12.506 MB** less
   logical storage, before vector growth, headers and ordering metadata. The
   current reservation is larger still. Keep ascending known-slot enumeration,
   independent recipient history and exact values. An extra index read and
   insertion/removal maintenance can offset the locality gain; measure it.
2. **Contiguous retained-sector timestamps/keys.** Food and subscription paths
   dominate maintenance cycle samples. A sorted key/time representation could
   remove duplicated membership keys and tree-node pointer chasing while
   retaining the oldest-sector skip and exact strict float expiry. Splitting
   compact keys/timestamps avoids tuple padding; it removes at least 60.4 kB of
   duplicate key payload and replaces tree allocations with bounded vectors.
   Insertion shifts and separate held-listing histories remain costs; the
   existing random-walk/full-sweep oracle is essential. Node savings and CPU
   benefit have not been measured.
3. **Compact food records and sector capacity.** A combined listing record can
   place the 2-byte key alongside the point/style fields: a 24-byte layout is
   plausible versus the measured 32-byte tuple, saving **226 kB live / 339 kB
   at current capacity** if verified. The food index also reserves three times
   its live payload; pooled/contiguous sector storage could reduce slack and
   thousands of small allocations while preserving per-sector lifetime order.
   Miss attribution does not isolate either saving, so these rank below the
   history path and require separate experiments.

Body vectors are already contiguous per snake; blindly interleaving their
bounds and points could enlarge box-only query traffic. A small shared front/
peer-metadata snapshot is another hypothesis for the hot update path, but IP
skid does not distinguish its loads from scattered history stores. Neither
justifies a production rewrite from this diagnostic alone. No CPU-specific
code, SIMD intrinsics, affinity fix, precision reduction or cache-size tuning
is proposed.

## Verification and access

All same-protocol fixed-work counter summaries match. The packaged builder
regenerates all 180 Rust files byte-for-byte against the measured instrumented
sources; the collector matches after newline normalization. Original failures
and throttled profiles remain in the local-only archive. Selected aggregates
retain every counter repetition; no per-file source-hash maps or raw IP dumps
are committed. See `verification.json` for whole-artifact pins and checks.
