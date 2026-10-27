# Review diagnostics plan

Authorized after the original qualification. Preserve all original attempt
objects and qualification table rows exactly. New measurements are labeled
`rerun`, remain separate from qualification, and do not replace failed results.
No regression claim is planned in either direction.

## Frozen schedule

Same seed 7, fresh worlds, shared frozen generator executable, eight shards on
CPUs 1-3, all host threads on CPU 0, no bots/TLS/added latency. Every measured
row has 15 s warmup followed by a 120 s hold. Execute once in this order:

1. Official-500: A=v0.7.0 `4c2471b`, B=#211 `65ff713`, C=pinned main `9960f4f`.
2. Official-500: C, B, A.
3. Current-main diagnostic `4cd6ce542b45e385d542d9423089d884bc314778`:
   official-500, official-375, Battledome-80.

Build only the new commit, from a local `git archive`, using the original
locked release profile and a fresh target, then remove the target after
copying/hashing the binary. Reuse all three original host binaries unchanged.
One eight-client, 3 s warmup / 8 s smoke of the new host is labeled `smoke`.
No result-based repeats or exclusions. Retain infrastructure failures.

## Added observations

Use an additive Python-worker overlay, leaving Rust instrumentation and all
binaries unchanged: retain `/proc/PID/stat` user/system counters separately,
CPU-0 `/proc/stat` counters and CPU-0 `/proc/cpuinfo` MHz at each existing
resource sample. Compute user/system ms per executed tick from the exact same
resource boundaries as total CPU. Report the clock as the guest-observed MHz,
not a measurement of physical turbo frequency. No root changes or package
installation. Keep data/targets under `~/slither-bench/`, preserving old files.

From the original #211 official-500 raw counters, publish every sampled
deferral increment and a before/during/after correlation with tick busy tails,
aggregate steal, generator CPU, host total/thread CPU and RSS. Original user/
system split, CPU-0-specific steal and clock were not retained; mark them
unavailable instead of inferring them. Inspect published #201/#211 JSON for
matching user/system fields before making a user-CPU comparison.

Report all nine diagnostic holds separately, per-row CPU user/system/total,
clock, delivery metrics, occupancy and generator headroom. Summarize the two
new repetitions per original commit with ranges and original-row context;
three holds per commit are descriptive, not a causal regression test.

## Retrospective deferral-window extension

After the second #211 repeat also deferred ticks, apply the same before/during/
after reduction to **every** new `rerun` with positive deferrals. This uses
already collected data, retains all affected holds and does not alter the
schedule or qualification. New samples permit user/system and CPU-0 clock/
steal correlations for those new episodes, without reconstructing missing
observations from the original episode.
