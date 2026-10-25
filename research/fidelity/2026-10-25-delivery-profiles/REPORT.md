# Responsive delivery profiles

2026-10-25. Exploratory local-simulator measurements on a shared Windows
desktop; no outside game servers. Base: `9eaecc3` (main). Source changes are
this PR; temporary measurement sources, executables and captures are pinned in
[source-pins.json](source-pins.json). Full distributions: [results.json](results.json).
The change was later rebased onto `d790cdc` (pit markers); the measurements
retain their original pre-marker source pin. Integrated checks/parity use that
new main; this is not a fresh measurement of its host cost or marker bandwidth.

## Decision

Keep `official` unchanged. Adopt `responsive`: changed wanted-heading or speed
bytes bypass the motion gate; changed standalone mass bypasses its gate.
Heading-only updates retain six-tick spacing. Other families remain unchanged.
The naive one-tick motion/mass gates stay outside production code.

Responsive matched naive input-report latency with 62% less generated bandwidth
in the 500-seat sample. The stock client's `h` handler calls `snl`, smoothing
body length and refreshing the own-score display; mass therefore benefits from
prompt delivery. Growth/tail packets already carry it promptly. This is our
design, not a recovered official scheduler.

## Method

Frozen local plan: `PLAN.md` under
`C:\Users\Nikita\.t3\scratch\delivery-profiles-20261025-805feb91`.
Everything there is **local-only**, not downloadable through these paths.
Rust 1.99.0, release/thin-LTO, Windows MSVC, Ryzen 7 9800X3D. Process trees used
affinity `0x30` (two logical CPUs); Cargo jobs, test/Rayon threads at most two;
host `botThreads: 1`. Runs were sequential; each host ran 40 seconds.

Variants: official gates 6/15 ticks; naive gates 1/1; responsive as above.
Seed 7, zero added latency. The two-snake raw-server probe varied input phase
across 96 scenes per event/profile, applied commands halfway into a tick and
checked the first reported wanted/speed byte against the applied change.
Mass used controlled fractional gains without segment growth. Both recipients
decoded actual wire frames; the server's recipient caches identified report
ticks. No commands were superseded. Every snake remained alive.

Live hosts reserved one remote seat plus 499 official-world bots, or 63
Battledome bots. They used unmodified world presets. A headless forager captured
35 seconds at `127.0.0.1:18766`, origin `http://127.0.0.1:8780`.
Order: official/naive/responsive for 500 seats, reversed for Battledome.
Simulation busy wall time and generated payload bytes were counted over host
seconds 5–40. Bytes include the protocol header and framing, exclude WebSocket/
TCP/IP overhead, and include native bot clients. Means divide by configured
seats, including inactive ones. A temporary relaxed atomic counter per recipient
was present in all variants; it is removed from production.

## Input-to-report distributions

Delay is **report tick minus application tick**: zero means the same tick's
frame, the next tick after the queued command. This isolates release delay;
it excludes uplink, downlink, browser input throttling and rendering/smoothing.
Own and opponent distributions matched exactly in all 1,152 event scenes.
Quantiles use sorted index `n//2` for p50 and `floor(p*(n-1))` otherwise.

| Event, 96 samples/profile | Official mean / p50 / p95 / max (ms) | Naive | Responsive |
| --- | --- | --- | --- |
| Wanted heading | 17.5 / 21 / 35 / 35 | All zero | All zero |
| Boost on, first changed speed | 17.5 / 21 / 35 / 35 | All zero | All zero |
| Boost off, first changed speed | 9.04 / 0 / 35 / 35 | All zero | All zero |
| Fractional mass | 50.31 / 49 / 98 / 98 | All zero | All zero |

Official heading/boost-on had 16 samples at each delay 0–5 ticks. Boost-off
had counts `[63,4,4,4,5,16]`; mass had 6–7 samples per delay 0–14 ticks.
Responsive and naive each had 96 zero-delay samples for every event.
Boost-off phase includes both ramping and saturated boost, explaining its
different distribution; this is not a player-input frequency distribution.

## Bandwidth and simulation cost

| Host | Profile | Mean / median generated B/s/client | Busy ms/tick | Deferred ticks |
| --- | --- | ---: | ---: | ---: |
| Official, 500 seats | Official | 2,065 / 2,064 | 3.960 | 53 |
| Official, 500 seats | Naive | 5,746 / 5,702 | 4.004 | 0 |
| Official, 500 seats | Responsive | 2,156 / 2,145 | 3.187 | 0 |
| Battledome, 64 seats, overloaded | Official | 75 / 0 | 40.693 | 4,124 |
| Battledome, 64 seats, overloaded | Naive | 215 / 0 | 40.521 | 4,141 |
| Battledome, 64 seats, overloaded | Responsive | 92 / 0 | 38.007 | 4,096 |

The 500-seat responsive sample used 4.4% more bandwidth than official, with
zero deferrals and mean simulation cost below the 7 ms budget. This is one
short run, not a capacity guarantee or evidence of a CPU speedup. Controllers
observe delivered packets, so trajectories and workload diverge between profiles.
OS stalls affected the official sample despite mean cost below budget.

**The default 64-seat Battledome experiment failed as an active-64 comparison.**
Only 29/28/29 seats generated any bytes in the measured official/naive/responsive
windows; these are not simultaneous living counts. The watcher never received
gameplay, with three unsuccessful connections each; close code 1008 said
`No spawn position is clear of other snakes`. The tiny per-seat bandwidth
means include empty seats and slow world progress; they are not useful capacity
or profile-efficiency estimates. Admission/retry work in the crowded fixed spawn
ring is a plausible cause, not isolated by this experiment. All three variants
show the failure. No arena, admission rule or size floor was changed to hide it.

## Stock-client correction and held turning

Ran the earlier `tailjitter.py` and `delayspread.py` unchanged. Correction is
`v_local * anchor_gap - 42`, estimating speed over nine neighboring gaps,
excluding >15% speed variation and splitting appearances/removals. Units below
are world units, absolute-error p50/p95; overlapping windows are correlated.
Excess delay is receipt minus header time above a sliding local minimum.

| 500-seat profile | Windows | Receipt correction RMS / p50 / p95 | Header correction RMS / p50 / p95 | Excess delay p50 / p90 / p99 (ms) |
| --- | ---: | --- | --- | --- |
| Official | 612 | 3.49 / 0.66 / 5.31 | 0.68 / 0.28 / 1.38 | 2.1 / 372.6 / 534.2 |
| Naive | 818 | 4.78 / 0.66 / 10.23 | 0.64 / 0.28 / 1.37 | 1.5 / 13.3 / 126.2 |
| Responsive | 527 | 0.89 / 0.45 / 1.73 | 0.71 / 0.42 / 1.38 | 1.3 / 3.4 / 12.7 |

No window had 100+ body points. Battledome correction/delay is **unavailable**:
no gameplay reached the watcher. The old scripts raised `StatisticsError` and
`IndexError` on their empty samples; errors and raw archives are retained.
Receipt differences include host/OS scheduling and changed traffic. Header
correction is similar across profiles, consistent with unchanged prompt anchors;
these samples do not establish perceived smoothness or statistical superiority.

The pinned stock client turns toward a held wanted angle itself. A conservative
zero-link-delay calculation across the measured 2–293-segment range bounds
42 ms of drift from its 106 scale denominator versus physics' 111 denominator
at 0.00344 rad. Adding a full angle-byte uncertainty gives <0.028 rad and
<0.515 world units of boost-position error, below our 1-unit design tolerance.
This analytical bound assumes fixed size and correct transmitted turn direction;
it is not a rendered-client measurement. We found no reason to add extra steady
turn reports or an error-trigger mechanism in this change.

## Reproduction, failed attempts and limits

Local `NOTES.md` retains the initial overlapping-body probe failure and corrected
parallel lanes, the scratch argument-wrapper failure, and empty-stream analyzer
errors. No failed attempt was dropped. `delivery_study.rs`, `delivery_load.rs`,
`host-instrumentation.patch`, `snakes-naive.rs`, `live.py`, `analyze.py` and copied
executables preserve the measurement implementation. `run.ps1` pins affinity;
`select_results.py` produces the checked-in selected JSON. The original jitter
scripts and their imported decoder, stock JS and collector are hash-pinned.

The first parity attempt differed only in the new default config metadata;
wire/state records matched. Default `official` is now omitted during config
serialization to preserve those bytes. Measurement builds preceded this
serialization-only correction; release scheduling did not change afterward.
The final `uv run --locked python tools/perf_compare.py --base origin/main
--repeats 1 --no-profiles --output <fresh-directory>` exited 0: all ten default
workload traces and definitions equal main, including wire records.
The same comparison against integrated main `d790cdc` also exited 0, all ten
equal. `uv run --locked python tools/check.py rust` and
`python -B -m unittest discover -s tools -p test_documentation.py` passed after
integration, including existing packet-timing tests unchanged in their assertions.

This study changes a delivery design, not physics or fitted official rules.
There is no official-server collection or held-out fitted claim. Human rendering,
nonzero-network latency, long mature worlds, successful 64-active Battledome
capacity and the cause of its admission/load failure remain unverified.
