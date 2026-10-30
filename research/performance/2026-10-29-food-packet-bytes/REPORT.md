# Keep unchanged held food listings through sector retirement

Moving unchanged held listing handles preserves byte-identical output and cuts
mature maintenance user instructions by **0.227% with stock clients and 0.116%
with protocol 13**. Other ticks use 0.012%/0.0066% fewer instructions; whole-window
work falls 0.018%/0.010%. This is a small instruction reduction. Maintenance CPU
ranges overlap; stock fixed-scene p99 falls, while legacy/short p99 ranges
overlap. No live CPU/tail or capacity improvement is established. Both encoded-byte
sharing prototypes increase instructions and are rejected.

[Selected results](results.json), [tables](tables.py), [frozen plan and amendments](PLAN.md),
[reproduction](REPRODUCE.md), [socket-write analysis](SOCKETS.md).

## Measured lead and ownership change

The earlier [maintenance diagnostic](../2026-10-29-food-diffs/REPORT.md#remaining-measured-work)
brackets recipient food at 1.741 CPU ms per maintenance interval. It includes
bookkeeping and encoding as well as copying, has clock overhead and fails the
mature body-class gate. It is not an isolated measurement of packet-copy CPU.

The retained mature-stock base proof contains **2.9328 incremental food packets,
20.5296 encoded bytes per recipient per maintenance tick**. Across five such
ticks it emits 7,332 incremental packets and 5,457 sector clears. These exact
wire counts do not measure allocation CPU. Together with two losing byte-sharing
prototypes below, they motivate a smaller ownership change on the same path.
[Census and provenance](food-packet-census.json), [calculation](count_food_packets.py).

A subscription membership edit takes the full held-listing walk. Previously,
even a retained listing identical to the current listing was dropped and cloned
again into the rebuilt vector. The change moves that existing `Arc` back into
the held vector. New/changed listings clone the current handle as before.
Pickups still emit removals/additions with unchanged listing identities.

This removes repeated reference-count operations on retained sectors without
changing packet production or adding a packet representation/cache. Existing
previous/current cache identities, older-view fallback, physical/decorated food,
ordered clears/removes/adds, frame boundaries, elapsed headers and latency draws
retain their behavior. No physics, protocol or retention-rule change is made.

## Current-main fixed work and byte identity

Primary runtime base is `23eddca80a2ae970753dc40717fa74297ff281fc`, including
the landed #261 cleanup skip. Implementation is
`a15bfc7a4ed4ee07d5d3d772033a869c4bb65024`. Both arms and the wire mutant are
freshly built with the same harness, Rust 1.99, locked offline dependencies,
native CPU targeting, release/thin LTO and one codegen unit on a 4-vCPU Zen 5
EPYC KVM guest. Earlier prototype numbers use their own older base and remain
separate. No obsolete-base timing is substituted into the primary comparison.

The unchanged [full-backend fixture](../2026-10-29-food-diffs/REPRODUCE.md)
warms once per arm/protocol from tick zero to 193,000 at full speed, without
sockets or wall pacing, keeping all recipients and wire history active. The
validated physics checkpoint omits hidden births, presentation lifetimes, held
listings and sector interest, so restoring it alone cannot continue this path.
Every pair forks the frozen full backend instead.

Mature rows start at 499 living, mean length 55.3307, p90 176 and p99 247,
then count 360 complete advances. Short counts 1,080 from zero. Controls,
admission, draining and trace serialization are outside each advance. One
discarded AB warmup precedes five AB/BA/AB/BA/AB pairs. Actual food-epoch
transitions identify maintenance ticks. CPU includes identical clock/PMU wrappers
without overhead subtraction; user instructions exclude kernel work.

| Fixture | Ticks | M instructions/tick, before -> after | Paired change %, median (five-pair range) |
| --- | --- | ---: | ---: |
| Mature 19 | Maintenance | 17.6333 -> 17.5932 | -0.227191 (-0.227200 to -0.227029) |
| Mature 19 | Other | 8.3257 -> 8.3247 | -0.011884 (-0.011896 to -0.011819) |
| Mature 19 | All | 8.4549 -> 8.4534 | -0.018120 (-0.018133 to -0.018053) |
| Mature 13 | Maintenance | 18.8560 -> 18.8341 | -0.115931 (-0.115940 to -0.115908) |
| Mature 13 | Other | 8.6375 -> 8.6370 | -0.006601 (-0.006658 to -0.006545) |
| Mature 13 | All | 8.7795 -> 8.7786 | -0.009862 (-0.009918 to -0.009808) |
| Short 19 | Maintenance | 13.8834 -> 13.8531 | -0.218154 (-0.218157 to -0.218150) |
| Short 19 | Other | 5.8033 -> 5.8027 | -0.010372 (-0.010381 to -0.010347) |
| Short 19 | All | 5.9155 -> 5.9145 | -0.017145 (-0.017153 to -0.017121) |

Negative changes mean fewer instructions. Each mature window contains five
maintenance and 355 other ticks; short contains 15 and 1,065. CPU values
below are medians of five run means or per-run p99s, in ms.

| Fixture | Maintenance CPU mean, before -> after | Other CPU mean | All-tick CPU p99 |
| --- | ---: | ---: | ---: |
| Mature 19 | 4.120 -> 3.852 | 1.773 -> 1.720 | 4.200 -> 3.769 |
| Mature 13 | 4.358 -> 4.340 | 2.043 -> 2.023 | 4.502 -> 4.354 |
| Short 19 | 3.066 -> 2.896 | 1.452 -> 1.501 | 3.247 -> 3.112 |

The following min-max ranges cover the same five runs; they are not
confidence intervals. Timing differences within this spread do not establish
a CPU or tail improvement.

| Fixture | Maintenance mean range, before -> after (ms) | Other mean range (ms) | All-tick p99 range (ms) |
| --- | ---: | ---: | ---: |
| Mature 19 | 3.964-4.325 -> 3.550-4.228 | 1.736-1.872 -> 1.703-1.749 | 3.979-4.364 -> 3.681-3.945 |
| Mature 13 | 4.336-4.663 -> 4.043-4.463 | 1.978-2.057 -> 1.892-2.052 | 4.428-4.919 -> 4.078-4.598 |
| Short 19 | 2.976-3.134 -> 2.783-3.272 | 1.432-1.492 -> 1.388-1.626 | 3.216-3.327 -> 2.930-3.472 |

Maintenance mean ranges overlap in all rows. Mature-stock all-tick p99 ranges
do not overlap in this sample; protocol-13 and short p99 ranges do. The stock
fixed-scene result does not establish variation across independently warmed
worlds or a live host tail improvement.

All 27 catalog/host traces match complete decompressed bytes with terminal
checks. Stock compares 5,876,276,676 warm bytes per arm; protocol 13
compares 10,132,210,980. Every repeated proof and direct-parent
continuation agrees. The stock mature proof also matches the earlier census
trace byte for byte. Both deliberate wire mutations are detected in encoded
frame payloads alone. Sources and copied binaries are rehashed after counting.

Five forks measure repeatability on one fixed scene, not production-world
variation. No new live CPU/tail confirmation or 7 ms capacity qualification runs.
The eight-socket syscall witness is a separate short-world count diagnostic.

## Rejected byte-sharing prototypes

Both are exact but slower in the measured guards. Neither packet representation
lands. Their selected pairs/build identities remain separate from the primary
comparison, and their full source/binary/trace archives are externally retained.

| Prototype / fixture | Maintenance instruction change | Other | All |
| --- | ---: | ---: | ---: |
| Per-packet enum, short 19 | +0.361% | +0.344% | +0.344% |
| Per-packet enum, mature 19 | +0.206% | +0.217% | +0.216% |
| Sector groups, short 19 | +0.354% | +0.650% | +0.640% |

The first (`be09f69b`) widens ordinary packet storage to owned/shared variants,
packs sector bytes and retains per-packet ranges/reference counts. The second
(`e6ba88f8`) shares one complete sector packet group and borrows a slice list
when food is emitted, leaving ordinary vectors unchanged. Both pass the full
Rust group, all 27 trace comparisons and stock/protocol-13 payload mutation
controls. The first also passes its complete mature-stock warm/proof comparison.
No mature protocol-13 timing is run for either loser; the sector-group version
has no mature timing row. Their short regressions are retained, not hidden.

CPU means/p99s do not rescue those instruction regressions. Per-packet short
maintenance/other means are 3.323 -> 3.288 / 1.514 -> 1.554 ms, all p99
3.458 -> 3.372. Mature stock is 5.077 -> 5.153 / 2.001 -> 2.039, p99
5.029 -> 5.049. Sector-group short is 3.230 -> 3.279 / 1.526 -> 1.544,
p99 3.414 -> 3.388. These are descriptive five-run medians with timing variation.

Starting from base `23e987f3`, the [per-packet patch](rejected-per-packet.patch)
and [group patch](rejected-groups.patch) reconstruct the respective losing
runtime sources. Their check runners are [check_packets.py](check_packets.py)
and [check_groups.py](check_groups.py). The selected change is in its source
commit, without a production patch file.

## Checks, incidents and retained material

The [socket table helper](socket_tables.py) recomputes both the earlier
500-player counts and this study's eight-client syscall witness from their
respective selected results.

Focused food tests pass, including retirement plus unchanged-listing pickup
and fresh-state listing delivery. Each of three separate source mutations fails
at runtime as intended: lost unchanged-handle pickup, ignored listing change,
and ignored held-listing identity. Source restoration passes the same suite;
`python3 tools/check.py rust` passes in full. The exact executed receipt producer
is [check_held.py](check_held.py); its six command/exit rows and aggregate receipt
are retained unchanged in `fixed.checks`. Documentation/link checks pass after
assembling this report. Public socket scripts match their executed hashes; both
rejected patches reconstruct their measured runtime sources. All 172 current Rust
files match the measured candidate archive. These contributor checks do not
replace maintainer review.

Main landed #261 before primary retained-listing timing. The obsolete-base
`b657c97f` preparation was stopped and retained separately before collecting any
timing row. Both primary arms, all checks and byte controls are refreshed against
the current base above. The food ownership implementation itself is unchanged.

The first prototype's short row completed before SSH closed during archiving.
After reachability returned, all 106 files in its complete archive matched the
session before use. No incomplete timing was selected. Local prototype iterations
fixed container test types and a redundant test allocation before guest checks.

Complete sources, copied binaries, raw ticks, proof traces, warm-stream receipts,
command inventories and failed preparations are local-only. Warm streams are
compared byte for byte online; their bytes are not stored. Completed/rejected
cohorts are copied externally and SHA-256 verified; every removed duplicate is compared with its
archive before pruning. Only cited aggregates and their table/reproduction code
belong here. Hashes establish identity, not scientific or delivery acceptance.
One bounded guest Cargo target is removed after builds/parity, before counting.
No guest configuration/package change, outside game-server contact, local
play-server change or cloud action is used.
