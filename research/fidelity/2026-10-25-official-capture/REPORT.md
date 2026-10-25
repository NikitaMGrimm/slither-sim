# Official capture, 2026-10-25

Six ten-minute official-server batches completed, three per endpoint. All are
timing-contaminated on this shared Windows machine; the frozen collector-stall
mask leaves insufficient clean exposure. This session does not validate a latency change.
The fresh joins also leave the official spawn rejection rule unidentified.
No simulator model, default or bot policy changed.

## Collection and provenance

Base: `421e384c3738e7c4d08e1eb12275733266d7a629`. The original
[plan](plan.json) was committed as `10fa4f6` before any game connection.
It is unchanged. The official list at `https://slither.io/i80124.txt`, fetched
21:35:37 UTC, listed both pinned port-444 endpoints active:
`57.129.37.42:444` and `198.244.231.35:444`. Their fresh batch RTT medians
match the intended low and approximately 25-35 ms route classes.

The first batch failed: both clients exhausted three retries before WebSocket
open, with code 1006 and no received frames. A separately committed ten-second
[Origin diagnostic](handshake-plan.json) succeeded with `http://slither.io`.
The [continuation plan](continuation-plan.json), committed as `44f7f7a`, restored
three ten-minute targets per endpoint with that header. The failed batch,
diagnostic, censored pings and every reconnect remain retained. Both failed
streams have valid gzip/receipts and are quarantined for missing gameplay.
See [deviations](DEVIATIONS.md) and [capture receipts](capture-receipts.json).

Each batch used one unmodified forage collector and one existing 15-second
join/leave collector in one process, pinned to CPUs 4-5 with one libuv worker.
The reconstructed peak is **2 simultaneous
connections**, including connecting sockets; **250 connection
attempts** and **244 own joins** include the failed
batch and diagnostic. No custom endpoint or third monitoring connection was used.

Before each batch the coordinator waited for `host_load` and `slither-sim` to
exit. A strict total-load wait stayed above 30% for more than five minutes;
the parent's explicit contamination option then allowed collection to proceed. The
[load decision](CPU-CONTAMINATION-DECISION.md) precedes continuation acquisition.
Ten-second CPU means across continuation batches ranged 37.8-100.0%.
Full concurrent-process inventories and per-second collector CPU/memory remain
local-only. Machine contamination applies to each entire batch. A later
`slither-sim` process overlapped one recorded sample in second-1 despite the
pre-start gate; that overlap is also retained.

## Network path

The [analysis source](analyze.py) was sealed as `f6b3556` before held-out acquisition.
Development batches are low-2/low-3/second-1/second-2; low-4 and second-3 stayed
uninspected until development analysis completed. All lives/clients in a batch
stay together; frame counts are not independent trials.

The primary metric removes Theil-Sen clock slope from 5-second p1 floors, then
a centered +/-10-second minimum, within each setup epoch after 1-second burn-in.
Epochs need 1,000 frames and 30 seconds. The parent's +/-600-frame-floor metric
is retained separately in [selected results](selected-results.json); it is a
different offset gauge. No parameter was refitted. The model column is the
median of three frozen seeds on the observed server schedules, using the pinned
retained-fit simulator, not a fresh Rust-host calibration.

| Batch | RTT p50 ms | Observed excess p50 / p90 / p99 ms | Model p99 ms | Clean frames |
| --- | ---: | ---: | ---: | ---: |
| low-2 (development) | 14.4 | 2.0 / 5.6 / 57.5 | 13.5 | 0 |
| low-3 (development) | 14.3 | 2.0 / 5.7 / 66.5 | 13.2 | 0 |
| low-4 (heldout) | 14.7 | 2.1 / 6.2 / 29.9 | 13.5 | 0 |
| second-1 (development) | 30.0 | 2.2 / 5.4 / 22.7 | 13.7 | 0 |
| second-2 (development) | 29.4 | 2.0 / 5.0 / 21.2 | 13.6 | 2 |
| second-3 (heldout) | 31.7 | 2.7 / 49.0 / 110.2 | 13.1 | 8 |

Telemetry retains monotonic raw callback/send times, ping sends/replies/censoring,
1-75 ms timer/lateness histograms plus overflow, event-loop delay, receive cost,
GC, queue/backpressure and CPU/memory. The frozen rule masks each telemetry
second with a >5 ms stall indicator, plus 100 ms margins. Ordinary Windows timer
granularity trips this rule across nearly all recorded seconds. The small or
empty clean subset cannot isolate network delay or prove every second was
CPU-stalled. All archive bytes and unfiltered results remain available. No cutoff was loosened after inspection.

### Explicit pause-tail check

The [parent-requested ablations](CPU-AND-TAIL-AMENDMENT.md) were declared before
held-out inspection. These compare fixed alternatives, not selected fixes.
Held-out p99, milliseconds:

| Fixed model variant | low-4 | second-3 |
| --- | ---: | ---: |
| fullModel | 13.5 | 13.1 |
| noPauses | 5.4 | 5.5 |
| halfPauseRate | 7.3 | 7.0 |
| halfPauseLength | 6.4 | 6.4 |

The report also retains contiguous excess >10/20 ms episodes, peaks, observed
durations and surrounding-receipt brackets. Observed duration is a lower bound;
single-frame episodes have duration 0. Threshold-run rates are not the hidden
Poisson pause rate. For excess >10 ms:

| Held-out batch | Observed runs/s | Full-model runs/s, median seed | Longest observed run ms |
| --- | ---: | ---: | ---: |
| low-4 | 2.598 | 0.618 | 390.0 |
| second-3 | 4.831 | 0.571 | 1033.5 |

Neither held-out batch passes the full frozen validation contract: clean
exposure is insufficient and whole-batch CPU contamination blocks adoption. Per-seed
quantile, burst and ACF gates remain in the selected results, including failures.
These contaminated callbacks have heavier tails than the fit. Reducing pauses
moves the model farther from them; this does not confirm or refute the earlier
overprediction on quieter retained recordings. The ablations cannot estimate
a new physical pause rate/length or resolve the parent's loopback attribution.

RTT-minus-downlink-excess and RTT/excess correlations are retained per epoch.
They still include constant delay, uplink and scheduling. A conditional
synthetic sensitivity gives RTT p99 about 34.7 ms with mirrored independent
pauses, versus 25.2 ms with pauses only on downlink, under the chosen 4.974 ms
base and 0-7 ms pong wait. This is not an identified direction split or RTT-time
model. Client data cannot identify absolute one-way delay or direction coupling. No
stock-browser RAF or isolated input-phase intervention was collected.

For context, the parent's existing local `probe-a` reproduces p99=43.6 ms on
the frame-floor metric. The current `probe-b` second connection has 531.7 ms
wall-minus-server progression. These are separately pinned local traces,
not new official observations. They do not establish a unique source of delay.

## Spawn admission

The pinned historical ecology, placement and neighbor audits reconstruct
**243 fresh own joins**, **209 with visible neighbors**, exceeding the planned
150-join visible-neighbor target. Batch leaderboard medians were 465-480 players.
Incidental forage joins count; opponent appearance packets do not. Each radius
uses the last received radius in that connection. Body/corridor/head-nearest
contributors retain separate identity and freshness.

- Minimum reconstructed whole-newborn-corridor clearance: **604.60 units**.
- Accepted corridor <550: **0**; accepted head <600: **0**.
- Predeclared robust/fresh counterexamples (corridor <530 or head <580):
  **0 / 0**.
- Maximum accepted radial fraction: **0.7240290**;
  above 0.8452: **0**.
- Radial-grid tolerance 0.0002: **243 of 243**;
  maximum residual **5.13e-05**.

The sample leaves 550-unit corridor versus 600-unit head-only rejection
unresolved. Accepted observations cannot prove either rejection algorithm;
unseen neighbors and reconstructed body geometry remain limits. Observing no
value above the declared radial boundary does not prove a universal cap. Selected minima,
maximum radial placement and any counterexamples are retained with source hashes.

## Access, reproduction and checks

Full raw bytes, gzip receipts, source snapshots, attempts and analyses are
**local-only** under `C:\Users\Nikita\.t3\scratch\official-capture-20261025`.
[Source pins](source-pins.json) identify those files and the final manifest.
This selected record is not a complete evidence-bundle-v1 archive. Downloaded
official client assets and full recordings are not redistributed; no permission
to redistribute them is established. Hashes establish byte identity, not
scientific validity or server-source authenticity.

Offline reproduction: `python -B analyze.py` for development, then
`python -B analyze.py --validation`, from this study directory's source with the
pinned local paths available. Outputs refuse overwrite; preserve originals in
a separate scratch copy before rerunning. Python 3.13.13, NumPy 2.5.3,
SciPy 1.18.1 and Node 24.15.0 were used; numerical threads are capped at one.
Acquisition drivers contact the frozen official endpoints; do not run them as
offline tests.

Verification: 104 offline collector tests, 9 historical spawn tests, 4 retained
analysis tests, 4 study analysis tests and 7 documentation tests passed. All
continuation gzip/terminal receipts and frozen source/plan hashes were checked.
No Rust changed, so the Rust check group was not run. The optional scheduler
input-phase scenario was not collected; no bot policies were changed.

Remaining work: a genuinely quiet collector or approved kernel-arrival timing
is needed for tail attribution, followed by independent validation. The spawn
rule needs fresh discriminating geometry; repeating threshold-consistent joins
alone will not identify rejected candidates.
