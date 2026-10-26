# Rebase follow-up plan, 2026-10-26

Written before follow-up timing. #113 merged during the first capacity series,
changing host admission and the Battledome harness; #119 changed source evidence
and test references, and #120 clarified the workload guide. The implementation
rebased cleanly, without edits to the admission changes.

The first series remains pre-#113 evidence, pinned to base `9480608` and
candidate `08b1fad`; keep all samples, trace hashes and outcomes. Do not present
it as a comparison of the rebased PR against current main.

Repeat the same frozen comparison on main
`473e1882932356906cdb2f338b39bce0796667af` and combined candidate
`c63eef61360d6291a7733bfaeeb3f77ac8e12f3b`. The rebased commits are `2a6731c`
(food subscriptions), `beb5714` (peer deltas) and `c63eef6` (owned frames).

Finish the first series, download its raw evidence and verify complete resource
cleanup before launching another one-time Spot pair. Use the same instance
types/topology, region, caps, timer, tags and source/build identity requirements.
The first run's short lifetime leaves enough of the total $3 target and $5
ceiling for the follow-up. Do not overlap instance pairs or exceed 17 vCPUs.

Run the entire CONTRIBUTING Rust group on the dedicated AWS generator before
trace checks and timing. This avoids new heavy desktop work; honor
`C:\Users\Nikita\.t3\scratch\QUIET-WINDOW.lock` before any new heavy local
command. Do not create or remove that coordination lock.

Repeat all six untimed complete-byte trace comparisons, then the same five
adjacent alternating performance pairs and unaffected controls. Use the new
main's identical harness on both revisions. Repeat base/candidate confirm-250
(15 s warmup + 300 s hold) and candidate/base sweep-500 (15 + 120 s), keeping
the original grades. Preserve both rounds and label their different source
pins; base/candidate within each round share the same dedicated instances.

Publish final runtime claims from the follow-up. Treat the initial results as
historical supporting evidence, not additional matched pairs for the new base.
