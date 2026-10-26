# Shared prey visibility candidates

Frozen before performance measurements, 2026-10-26. Synthetic loopback only;
no cloud compute or outside game server. Do not alter Battledome admission.

The retained #168 candidate profile assigns 11.86% of sampled user CPU to
`hypot` at `broadcast.rs`'s per-recipient prey visibility loop. The host resolves
the official prey target by arena area; every viewer currently scans all prey.
Build one conservative spatial index per broadcast, retain original world
order and the exact original distance comparison. Keep the linear scan below
16 prey or 16 set-up recipients to avoid building nine memberships per prey
for small batches. This is an engineering work selector, not a visibility law.

Starting baseline: main `249cdb217d83a234895bc7eee4d98c3636c1cd76`. #168 is
accepted but not yet merged at this source pin; do not add its speedup to this
study's results. Pin any baseline update before builds and keep failed attempts.

Use Rust 1.99.0, locked release, thin LTO, one codegen unit, debug level 1;
independent baseline/candidate builds copied outside targets. At most four
logical CPUs in total; two build jobs, two test/Rayon threads. All host threads
share CPU 0; unchanged generators use CPUs 2/4/6.

Require focused visibility tests and the full Rust group, plus byte equality
for all six #168 traces and its Battledome-40/64/80 catalog extension before
live timing. Preserve complete decompressed comparisons, not hashes alone.
The original portable/protocol cases use 32 operations; official-500 uses one
decision after warmup; Battledome traces use four; retained cases 384 decisions.

Screen fixed official-500 instruction counts before investing in live timing.
Then use the same #168 method on official-500 and boosting Battledome-40/64/80:
15 s warmup, one discarded 15 s hold per arm, five alternating adjacent pairs
of 45 s measured holds. Report process user+system CPU/executed tick, all paired
ratios and ranges, alive/waiting populations, frame excess p50/p99, deferrals
and completed/censored spawn-wait summaries. Report all completed pairs; no
native capacity claim. Preserve admission-limited denominators.

Separate profiles use `cpu-clock:u`, 199 Hz and DWARF/16384-byte stacks.
Keep kernel work explicitly outside the sample shares. Repeat the fixed-fixture
instruction diagnostic on all four workloads: one discarded pair and three
alternating pairs of 100 decisions, construction/warmup included. Keep records
small: report under 8 KB, selected JSON summaries; raw data stays in scratch.

## Rejected grid and second candidate, before second-candidate timing

Grid candidate `3f54b249c6b7c741580b10b1c571f95346720a33` passed all traces
and checks but official-500's five CPU ratios were 0.85345, 1.03448, 1.07758,
1.14174 and 0.93487 (median 1.03448). A single 25-decision screen retired
2.8% fewer user instructions. This does not establish a live CPU benefit.
Retain its complete official pairs, separate profiles, code patch and binaries
in scratch. Stop the planned grid series (an untallied Battledome warmup was
interrupted); no partial run is a completed result and no grid change ships.

Replace the grid with a single world-ordered array of per-prey axis bounds,
prepared once per broadcast. The half-width is `max(1, 2*vision)` for rounding
slack. Scan these cheap bounds and retain the original hypot comparison for
survivors. No hash maps, cell memberships, small-batch threshold or persistent
state. This second candidate uses the same pinned baseline and starts a fresh
full parity/check/measurement series with the same four workloads/protocol.

The second candidate's single 25-decision screen retired 8,628,984,992 user
instructions versus a fresh baseline's 8,969,555,316 (100% scheduled). This
screen is not a repeated result or CPU-speed claim. Candidate runtime pin:
`fde812bb4c5c6dea52ebb32a8341d98fe45ea76b` (source unchanged during timing).

## Supplemental profile after main integration

After the frozen series, main advanced through #168 and v0.6.1. Integrate main
`c012a723` without rewriting the measured runtime pin. Profile combined runtime
`bf5e29c1928d6edb69399ab18e7d3dd242e8b6c9` separately on all four workloads,
using the same probe, baseline-built generators, CPU affinity, warmup, hold and
sampling settings. This updates the remaining-cost breakdown; it is not a paired
CPU comparison and cannot establish an additive gain. Complete checks first.
