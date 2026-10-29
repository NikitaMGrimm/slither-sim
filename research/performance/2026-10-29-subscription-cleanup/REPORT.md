# Skip subscription cleanup until a sector can expire

The exact oldest-sector hint preserves byte-identical output and reduces mature
maintenance user instructions by **3.74% with stock clients and 3.52% with
protocol 13**. Other ticks use about 0.15% more instructions, leaving whole-window
work about 0.03% higher. CPU means/p99s fall descriptively, but timing ranges
overlap. No live world-tail or capacity improvement is established.

[Selected results](results.json), [table calculation](tables.py),
[frozen plan](PLAN.md), [reproduction](REPRODUCE.md).

## Exact skip

The previous [500-socket diagnostic](../2026-10-29-food-diffs/REPORT.md#remaining-measured-work)
attributes 1.000 CPU ms per maintenance tick to subscription advancement plus
processing flags. It is an out-of-class explanatory window, not a qualified
matched baseline. This change targets the retained-sector sweep inside that
bracket; no whole-bracket saving is assumed.

Each connection now retains the exact minimum stored `last_seen` outside its
current stencil, with a sector witness. For fixed finite `now`, floating-point
subtraction is monotone in the subtrahend. If the minimum cannot satisfy
`now - oldest > 2.6`, no other non-current sector can expire. Current sectors
remain protected by the original stencil predicate. Cleanup epochs advance
even when the sweep is skipped.

On a stencil change, the previous stencil is still rewritten at its previous
`touched` time before the new stencil is touched at `now`. Existing non-current
times cannot exceed that previous touch under the caller's monotonic-time
contract. Thus a surviving oldest witness remains a minimum. If its sector
re-enters the current stencil, recompute the minimum after the rewrites.
A real retain sweep also rebuilds the minimum. Reconnect creates the same fresh
connection state; clones retain their own matching hint. Large time jumps use
the original single bounded epoch sweep.

The predicate keeps subtraction and strict expiry. For example, with
`seen = 0.119` and `now = seen + 2.6`, rounded subtraction produces
`2.6000000000000005`: an added future deadline would incorrectly retain that
sector at equality. This is a synthetic arithmetic regression, not a newly
fitted retention rule. The existing provisional 2.6-second model is unchanged.

## Fixed work and byte identity

Base is main after #260, `23e987f37b061ac80307404c24f6b7fecc71a770`;
measured implementation is `7a384183d0816ed903402e4b4062f410d584b0f2`.
Later production-source edits add only the report-link comment. Builds use
Rust 1.99, locked offline dependencies, release/thin LTO/one codegen unit and
native CPU targeting on a 4-vCPU Zen 5 EPYC KVM guest.

The unchanged [full-backend fixture](../2026-10-29-food-diffs/REPRODUCE.md)
warms each arm once, at full speed with all 500 recipients, from tick zero
to 193,000. The validated physics checkpoint omits hidden births, presentation
lifetimes, held listings and sector interest, so restoring it alone cannot
continue this path. Full-backend forks preserve that state for every pair.
The mature census is 499 living, mean length 55.3307, p90 176 and p99 247;
count the next 360 advances. The short guard counts 1,080 advances from zero.
Controls, admission, draining and trace serialization are outside each advance.

Every warm byte is compared live. Two repeated proof continuations per arm
and both direct-parent continuations must agree in full, including ordered
frame/time bytes, states, actions, RNG and lifecycle endpoints. One discarded
AB warmup precedes five AB/BA/AB/BA/AB pairs. Actual food-epoch transitions
identify maintenance ticks. CPU intervals include identical clock/PMU wrappers;
no overhead is subtracted. User instructions exclude kernel work.

| Fixture | Ticks | M instructions/tick, before -> after | Paired change %, median (five-pair range) |
| --- | --- | ---: | ---: |
| Mature 19 | Maintenance | 18.3277 -> 17.6421 | -3.740560 (-3.740565 to -3.740546) |
| Mature 19 | Other | 8.3143 -> 8.3266 | +0.146814 (+0.146790 to +0.146825) |
| Mature 19 | All | 8.4534 -> 8.4559 | +0.029757 (+0.029733 to +0.029768) |
| Mature 13 | Maintenance | 19.5356 -> 18.8473 | -3.523150 (-3.523167 to -3.523142) |
| Mature 13 | Other | 8.6249 -> 8.6373 | +0.142932 (+0.142924 to +0.142941) |
| Mature 13 | All | 8.7765 -> 8.7791 | +0.029594 (+0.029586 to +0.029602) |
| Short 19 | Maintenance | 14.6766 -> 13.8732 | -5.474092 (-5.474109 to -5.474072) |
| Short 19 | Other | 5.7910 -> 5.8025 | +0.199598 (+0.199587 to +0.199609) |
| Short 19 | All | 5.9144 -> 5.9146 | +0.004051 (+0.004041 to +0.004062) |

Negative means less work. Maintenance instructions fall 3.74%/3.52% in the
mature rows, but other ticks rise 0.147%/0.143%; whole-window work rises about
0.030%. The short row trades a 5.47% maintenance reduction for 0.200% more on
other ticks and 0.004% more overall. This is a tail-work tradeoff, not an
amortized throughput improvement.

Each mature window has five maintenance and 355 other ticks; short has 15 and
1,065. CPU values below are medians of five run means or per-run p99s, in ms.

| Fixture | Maintenance CPU mean, before -> after | Other CPU mean | All-tick CPU p99 |
| --- | ---: | ---: | ---: |
| Mature 19 | 4.431 -> 4.009 | 1.719 -> 1.693 | 4.402 -> 3.953 |
| Mature 13 | 5.355 -> 4.415 | 2.076 -> 2.009 | 5.017 -> 4.477 |
| Short 19 | 3.731 -> 3.496 | 1.733 -> 1.712 | 3.829 -> 3.463 |

Ranges overlap. The following are min–max across the same five runs, not
confidence intervals:

| Fixture | Maintenance mean range, before -> after (ms) | All-tick p99 range (ms) |
| --- | ---: | ---: |
| Mature 19 | 4.415–4.692 -> 3.998–5.382 | 4.308–4.856 -> 3.864–5.228 |
| Mature 13 | 4.788–5.614 -> 4.291–5.351 | 4.782–5.646 -> 4.410–5.199 |
| Short 19 | 3.663–3.977 -> 3.477–3.718 | 3.651–3.875 -> 3.452–3.688 |

All 27 catalog/host traces match complete decompressed bytes with terminal
checks. Stock compares 5,876,276,676 warm bytes per arm; protocol 13
compares 10,132,210,980. Every repeated proof and direct-parent
continuation agrees. Both deliberate wire mutations are detected even after
selecting only encoded frame payloads (first changed frames 21 and 567).
All source and copied-binary identities are rechecked after measurement.

These are descriptive fixed-scene results. Five forks measure counter/timing
repeatability, not production-world variation. There is no new live socket
confirmation or 7 ms capacity qualification.

## Checks, failures and retained material

The complete offline Rust group passes on the measured candidate, including
feature combinations, loopback contracts, C ABI, Python crate check and docs.
The new frozen full-sweep oracle compares membership, stored times, center,
touch time, cleanup epoch, exact minimum and revision-change behavior over
48,000 synthetic random-walk advances. Directed cases cover clipped stencils,
re-entry, resets, cloning, large jumps and the rounded strict boundary. Deliberate
wrong outgoing timestamps, missing re-entry refresh and added future deadlines
each fail at runtime; restored source passes. Existing reconnect and handoff
wire regressions remain in the full group. Documentation checks pass locally.
No independent review is claimed; the maintaining agent reviews and lands this PR.

The first directed large-time rounding example had a false premise and failed
its own assertion before measurement. Its archive/log remain outside Git; the
corrected vector above and all three mutation controls pass their intended gates.
A host restart detached the check runner's stdout after it durably recorded the
full Rust group's exit zero, preventing the final aggregate receipt. Every checked
source file was verified against the candidate archive before reconstructing that
receipt. No failed check was relabeled as passing.

Full sources, binaries, complete traces, raw tick samples, command inventories
and failed attempts are local-only. Completed cohorts are copied externally,
SHA-256 verified, and every removed duplicate trace is checked against its archive
before pruning. Selected aggregates and enough code to recompute these tables
are retained here. Archive hashes establish identity, not scientific or delivery
acceptance. The round reuses one bounded Cargo target; no guest configuration,
package, outside-server, local play-server or cloud resource changes are made.
