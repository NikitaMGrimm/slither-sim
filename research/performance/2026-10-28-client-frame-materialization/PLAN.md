# Exact client-frame materialization

Synthetic offline study. Baseline: `0aba6f97070a06c73b604869761d4c5c14965a78`.
Use the bot-split harness and brackets from population-scaling commit
`ccc87eddd9439d375eedab62e66e642313f0bb5a`, adapting only current hook locations,
population selection and untimed trace output. No authoritative observation path.

First profile official 500, seed 7, 1,000 warmup decisions and 200 counted
decisions, unchanged A4-mean background policy. Nested metadata/HUD brackets
identify materialization work. Pick a simple exact change after that profile.
The initial candidate was a shared scan of the two nearest speed histories.
The first profile found all metadata only 6.7k instructions/decision, versus
43.1k for the frame; that candidate was set aside without a runtime change.
The selected candidate folds completed head-motion intervals once, in original
addition order. A finer diagnostic checks head prediction before measurement.
Its first diagnostic saves only 0.8% of the bot-stage sum (about 1.5k instructions
per decision), so it does not independently qualify. A second, independent
candidate lazily caches the ordered food materialization and invalidates it at
each food mutation; every frame still owns a separate copy. Measure that larger
materialization budget and keep the pair only if total exact savings qualify.
The snapshot pilot with element-wise cloning did not qualify; the final arm
makes the value-only `Food` type `Copy` to permit bulk snapshot copies. This
adds mechanical clone-to-copy caller/test edits, without policy changes.
Do not change policy, delivery, simulation or admission geometry.

Acceptance: focused oracle/regression tests, byte-identical full world/action
traces in the bot-split continuation and the deterministic workload matrix,
and a mutation rejected by the oracle. Compare one untallied warmup pair and
five alternating instruction pairs, all retained, on one pinned logical CPU.
Count current-thread `instructions:u`, require >=99% PMU scheduling in every
counted bracket. Report actual decision and population denominators, stage
and end-to-end costs. Instrumented phase totals include hook overhead; make
no wall-time, socket capacity or real-server claim on this shared guest.
Add official 2,000 with the same recipe if practical. Keep raw logs, traces,
sources and binaries outside Git; publish selected pins and summaries. Remove
task-owned compiler targets after checks and copied-binary verification.

The development guest has four vCPUs; another instruction-count worker shares
it. This task pins its processes to one vCPU, uses one Cargo job and one Rayon
thread, and leaves system settings unchanged. No game-server traffic.

Scope amendment before final pairs: omit 2,000. Persistent free space fell below
1 GiB while retaining the 500-seat trace and full workload matrix. A larger
matching trace pair was not attempted under that remaining budget. The optional
larger population is not qualified or extrapolated from the 500-seat result.
