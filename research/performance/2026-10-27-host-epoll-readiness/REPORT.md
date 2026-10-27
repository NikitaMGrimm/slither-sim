# Persistent Linux readiness, 2026-10-27

Three native pairs use less total host CPU with persistent epoll registrations.
Keep the small CPU candidate for review, with an unresolved latency failure:
**one candidate hold deferred 39 ticks and had 470 ms frame-excess p99.**
No latency, capacity or c7a improvement is established. No hold was excluded.

Measured `4532a76` -> `e10a20a`; [full pins/results](results.json).
Delivery merges documentation-only main changes; runtime sources still match
the measured candidate. Two runtime files change, about 100 net lines plus
tests, with no dependency, thread, wait, delivery or flush-policy change.

## Native pairs

One core of a current EPYC server, native Linux x86-64, Rust1.99 locked default
release/debug=1. The [frozen plan](PLAN.md) specifies AB, BA, AB, 15 s warmup and
120 s holds after an eight-player smoke per arm. Official500, seed7, stock
protocol19 sockets, no boost/bots/TLS/added latency. Every arm passes traffic,
affinity and generator-headroom gates; host[0], one frozen load shard[1], jobs1.
All builds/holds take the original shared benchmark lock. The run.sh bytes are
unchanged; a private runner copy changes only owned paths, port18934, shard
count/affinity and smoke duration. No parent runner, system or security changes.

| Pair, order | Base CPU ms/tick | Candidate | Reduction | Excess p99 base/candidate ms | Deferred base/candidate |
| --- | ---: | ---: | ---: | --- | --- |
| 1, AB | 4.4785 | 4.3665 | 2.5006% | 10.171 / 470.346 | 0 / 39 |
| 2, BA | 4.0594 | 3.8467 | 5.2395% | 7.799 / 6.409 | 0 / 0 |
| 3, AB | 4.4368 | 4.2189 | 4.9116% | 8.596 / 8.186 | 0 / 0 |

Paired median reduction 4.9116%, range 2.5006–5.2395%. Process system CPU also
decreases in each pair; changing simulation cost contributes to total variation.
Live worlds are not instruction-matched replays. Mean playing499.976–500,
final500; generator CPU67.5–73.6% of one core. Native legacy performance is
unmeasured. All six arm values, two smokes and tick tails remain in JSON.

The bad hold saturates its core during seconds114.1–120: about98% process CPU,
57% simulation and40% sockets, with no sampled CPU0 steal. Maximum tick-start
gap is under16ms; this is accumulated backlog, not evidence of one long pause.
The cause is unresolved. One-second telemetry supplies no usable after-window.
The older anomaly helper rejects that empty window; isolated analysis marks it
unavailable without changing raw data. No favorable rerun replaces this hold.

## Code and checks

Each Open starts unregistered, including after descriptor reuse. ADD registers
once; MOD refreshes the event index after session compaction. Level-triggered
epoll_wait uses zero timeout at the old snapshot point; flags feed the original
session-order service/flush loops. Creation/registration/query failures retain
the original poll snapshot. A read-only check 0.2 s after the final timed window
found500 watches in that host, outside the measurement interval.

Poll-oracle tests cover writes, partial drains, compaction, hangup, descriptor
replacement and failure. Skipping MOD fails the oracle; restored tests pass.
Focused server/loopback/Clippy and all15 Rust-group commands pass. These WSL2
checks completed before the owner's no-heavy-local-compute limit. All fifteen
complete stock/half13/all13/U and historical wire/state traces are byte-identical;
selected hashes are in JSON. CI and independent maintainer review remain separate.

## Reproduce and access

Use replay.py build/trace with base.tar/candidate.tar from the full pinned Git
commits. For native holds, first install the retained [native bench](../2026-10-27-native-zen5-bench/REPORT.md).
Copy setup-native.py, native-series.py, PLAN.md and both archives as
base.tar.gz/candidate.tar.gz into a fresh ~/slither-bench/capacity/RUN directory.
Run setup-native.py, then bench.py build FULL_SHA for each arm, followed by
native-series.py smoke and pairs. The setup uses the same shared lock; scripts
reject -O. Keep completed progress instead of overwriting it. Raw logs, complete
traces, telemetry, source snapshots and binaries are local-only outside /tmp,
not publicly downloadable. Selected source/tool/binary hashes identify them.
