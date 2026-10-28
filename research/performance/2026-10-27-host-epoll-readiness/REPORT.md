# Persistent Linux readiness, 2026-10-27

**Keep #235 held.** Seven native pairs have mixed CPU results; added post-window
telemetry catches a shorter candidate-only CPU/backlog burst. The original
39-deferral stall remains unexplained. No latency/capacity improvement is claimed.
No hold was replaced or excluded. [Selected results](results.json), [plan](PLAN.md).

Measured runtime: `4532a76f22958ae3abcca801e90edd30ed9f466f` ->
`e10a20abb4f2b2fc072560787e20d918954a31ba`. The branch merges main through
`f9154e92`, including #230/#231: those changes are **outside this frozen
comparison**. Gateway/readiness sources remain the measured candidate.

## Seven pairs

Native Linux x86-64, one core of a current EPYC server, Rust 1.99 locked default
release/debug=1. Official-500, seed 7, stock protocol-19 sockets, no boost/bots/TLS/added
latency. Host CPU 0, one load shard on CPU 1, one build job. Original run.sh bytes and shared
benchmark lock, owned adapter, port 18934. All fourteen arms pass traffic,
affinity and generator-headroom gates.

| Pair, order | Base CPU ms/tick | Candidate | Excess p 99 base/candidate ms | Deferred base/candidate |
| --- | ---: | ---: | --- | --- |
| 1, AB | 4.4785 | 4.3665 | 10.171 / 470.346 | 0 / 39 |
| 2, BA | 4.0594 | 3.8467 | 7.799 / 6.409 | 0 / 0 |
| 3, AB | 4.4368 | 4.2189 | 8.596 / 8.186 | 0 / 0 |
| 4, BA | 4.2910 | 3.9731 | 8.254 / 6.708 | 0 / 0 |
| 5, AB | 4.0891 | 4.2700 | 7.292 / 7.480 | 0 / 0 |
| 6, BA | 3.8704 | 3.9243 | 6.968 / 6.893 | 0 / 0 |
| 7, AB | 3.9311 | 3.9427 | 6.660 / 7.894 | 0 / 0 |

Original pairs 1–3: median CPU reduction 4.91%, range 2.50–5.24%. Added pairs 4–7:
median **increase 0.84%**, ranging from a 7.41% reduction to a 4.42% increase;
three of four favor base. All seven: median reduction 2.50%, same wider range.
Live worlds are not instruction-matched replays. A consistent CPU improvement
is not reproduced.

Pairs 1–3 retain their original binaries, 15 s warmup/120 s hold. Before pairs 4–7,
commit `3a2e3e7d` froze BA/AB/BA/AB, symmetric diagnostic rebuilds, the same
warmup/primary window and 30 s of continued host/load controls afterward. Primary
CPU boundaries/frame records remain clipped to 120 s. Original and diagnostic
binary/load/probe hashes are separate. Every added arm retains 500 living snakes
through both windows, with zero births/deaths inside either; generator CPU
67.9–72.3%. All post-windows have zero deferrals.

## Retained-data diagnosis

All six originals retain per-second completed ticks, /proc process/thread CPU,
aggregate/CPU 0 busy and steal, and client lifecycle bins. World segments/mass/food
and all-process inventories were **not recorded** and cannot be reconstructed
from timings or sampled controls. Full second-by-second reductions remain local;
JSON keeps selected comparisons.

Simulation CPU/tick rises **before** backlog/deferrals: about 2.59 ms at 100.17 s,
2.97 at 102.20 s and 3.13 at 104.22 s, while relative lag is within about one tick.
At 106.25 s it is 3.59 with about 3 ticks lag, then 4.16–4.34 at 107–108 s with 9–17 ticks
lag. Completion rate falls from about 143/s to 135–136/s. A brief catch-up is
followed by renewed saturation. Deferrals begin around 114 s; the final window
uses about 98% host CPU,57% simulation and 40% sockets. Extra simulation CPU/tick
therefore precedes catch-up; these counters cannot distinguish more instructions
from slower execution.

The other five holds span 2.05–2.52 simulation CPU ms/tick early and 2.39–2.78 in
the final sampled window, against 2.24 ->4.06 in the bad hold. Only that hold has
primary rejoins/deaths (10/11), following the initial cost rise, and finishes 499
playing. Client bins omit the incomplete terminal interval; hold totals are
authoritative. Host player counts measure connections, not exact living snakes.

Throughout **all six** originals, aggregate accounting shows about two cores
of unattributed guest CPU beyond host+load (202–204% of one core). CPU 0 residual
falls rather than spikes at onset. Its sampled steal is zero during final
saturation, and maximum tick-start gap is under 16 ms: accumulated backlog rather
than one long pause. Historical node/cloudflared/other-process activity is
unavailable; surviving lifetime counters cannot locate that activity in time.

## Follow-up diagnosis

The frozen threshold is a new deferral, or two adjacent approximately 1 s samples
with hostCPU>=95%, backlog>=4 in both and increasing backlog. No added primary
or post-window meets it. **A marginal recurrence remains:** pair 6 candidate has
a shorter post-window burst at primary seconds 136–138:

| Sample end, s | Executed ticks | Sim CPU ms/tick | Socket CPU ms/tick | Host core % | Backlog |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 135.303 | 146 | 2.397 | 1.301 | 53.0 | 0 |
| 136.320 | 136 | 3.456 | 2.132 | 74.8 | 15 |
| 137.340 | 127 | 4.724 | 3.228 | 100.0 | 34 |
| 138.357 | 173 | 2.601 | 2.081 | 79.7 | 0 |

Intervals are about 1.02 s; separately timed world snapshots are within 0.5 s.
Backlog 34 is about 238 ms of scheduled ticks; it clears without deferral. Post
maximum start gap 14.158 ms, busy 14.156 ms. This resembles the original cost/catch-up
shape at shorter duration; it does not establish read starvation or a mechanism.

Aligned food counts 43165 ->43319 ->43491 ->43675 and logical segments 10977
->11022 ->11072 ->11161 grow smoothly. Live 500, mass/radius and births/deaths
show no discrete jump. There is no competing-process CPU or sampled CPU 0 steal
spike. Counts omit spatial clustering, allocation events and execution conditions
outside the guest: the trigger remains unclassified.

The inventory does catch concurrent, non-owned work despite the correctly shared
lock: rustc averages 54.5%/89.8% of a core in pair 4 base's primary/post windows,
and 77.5%/32.5% in pair 7 candidate. A simulation study child averages 5.8% in pair 5
base. Short-lived processes can be missed, so these are sampled lower bounds.
Node/cloudflared are typically below 1% each. All affected holds remain included.
Different live radii, food and mass further limit attribution; per-arm ranges
are selected in JSON.

The diagnostic death field initially summed an already cumulative history. The
driver was paused while the running hold continued; recovery was validated before
resuming without changing binaries/order or replacing holds. Analysis uses births
minus alive before shutdown: empty initial world, death-only reconnects, no
errors, monotone counts and load-counter agreement. Erroneous integrated history
is retained raw. /proc CPU counters are 10 ms-quantized; tick busy is wall time.

Recommendation: retain the hold. The exact threshold episode did not recur,
but a candidate-only marginal burst and mixed CPU results prevent a clean
environmental/world-driven attribution or demonstrated steady CPU win. The
owner decides landing; no further outcome-based reruns are added.

## Checks and reproduction

Runtime patch unchanged: per-open ADD, compaction MOD, level-triggered zero-timeout
query at the old snapshot point, original session-order loops and poll fallback.
Fifteen complete stock/half 13/all 13/U and historical wire/state traces match at
the measured refs. Original full Rust group:15 commands pass. Removing MOD fails
the compaction oracle; restored tests pass. After merging main, native single-thread
checks pass:46 server/readiness,21 loopback and 7 documentation tests. Fresh-head
CI/review remain separate.

Original builds/traces: replay.py and setup-native.py with both archives and the
retained native bench. Diagnostic run: put PLAN.md, followup-probes.py,
setup-followup.py, followup-series.py and analyze-followup.py in a fresh sibling
of the retained epoll-20261027 directory under ~/slither-bench/capacity. Run
setup-followup.py, then followup-series.py smoke and pairs, then analyze-followup.py.
Builds/holds take the same lock; scripts reject -O. The diagnostic probe remains
frozen/reproducible; analysis contains the disclosed death correction. Full
seconds, logs, binaries and sources stay local-only outside /tmp; no public raw
download or machine identity.
