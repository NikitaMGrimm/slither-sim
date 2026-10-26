# Host broadcast and frame ownership, 2026-10-26

The combined change reduces host CPU per executed tick by 4.8% at 250 socket
clients and 4.7% at 500 in matched native AWS holds. All six deterministic
client traces match byte for byte. The 250-client candidate narrowly passes
the 10 ms excess-p99 grade in one 300 s hold; 500 clients still fall behind.

## Question and scope

Reduce one-core host CPU without changing protocol-19 bytes, connection order,
virtual delivery times or packet-family cadence. This is synthetic simulator
traffic. No outside game server was contacted, and no policy or mechanics code
was changed. The [plan](PLAN.md) was written before timing.

The [native capacity profile](../2026-10-26-host-capacity/REPORT.md) measured
`6bcb6f0`. The final comparison starts at main-at-launch `473e188`, including #113's
spawn queue. The first run on `9480608` finished while #113 merged and is
retained separately in [initial results](INITIAL-RESULTS.md); those samples
are not pooled with the final comparison. See the [rebase plan](REBASE-PLAN.md).

## Candidate ranking

This is a risk-adjusted implementation ranking. Profile percentages are bounds
on the containing work, not predictions of cycles saved; the categories include
work that every candidate must still do. No individual contribution has an
isolated speed measurement in this comparison.

| Rank | Repeated work in current source | Expected saving and evidence | Risk / decision |
| --- | --- | --- | --- |
| 1 | `food_updates` takes and rebuilds each recipient's retained sector vector, merges unchanged subscriptions and clones every sector Arc every tick. | Removes this work on stable subscriptions, including empty sectors; plausibly a material part of the 9.94% food-updates self cost at 500 clients. Shared `FoodIndex` listings and dirty-sector rebuilding already exist, so rebuilding the entire world-food index would duplicate existing work. | Low: retain sector/offset order, preserve pickup processing even for equal Arc pointers, use the original merge when membership changes. Implemented. |
| 2 | `changes` repeats anchor recognition, geometry/trigonometry and delta encoding for viewers with identical prior `KnownSnake` state. | Reuses completed deltas within the 24.77% recipient-encoding category, while still copying small packet buffers into each recipient's output. Benefit depends on shared visibility and histories; misses have a cache cost. | Moderate: cache the complete before/after state for only this snake and broadcast; own packets bypass it. One entry bounds divergent-history storage. Implemented. |
| 3 | `Open::send` clones an already-owned delivery Vec, replaces/slices its two-byte header, then transfers it into `Message::Binary`. | Removes one payload-buffer copy/allocation in that path. It cannot remove tungstenite's subsequent `Frame::format_into_buf` payload copy, WebSocket headers or kernel writes. Expected smaller than the whole 23.15% framing/flush category. | Low: consume the popped queue entry, overwrite sequence bytes and slice bare Bytes; preserve the same write/flush/error paths. Implemented without a dependency. |
| 4 | `Gateway::deliver` linearly searches sessions for each slot/generation delivery. | A generation-checked slot-to-session index could remove an O(sessions × deliveries) lookup; the profile does not isolate its cycle share, so no numerical saving is predicted. | Moderate: sessions compact after closes, upgrades are pending separately, and stale generations must stay rejected. Deferred to avoid crossing the admission/lifecycle changes during this bounded comparison. |
| 5 | Changed/new sectors are diffed or encoded separately for recipients holding the same listing. | Could share sector-delta or initial `F` encodings; existing unchanged-Arc skipping already avoids pellet scans on most sectors. Remaining opportunity is a subset of food cost, especially maintenance, pickups and joins. | Moderate: previous listing, current physical/decorated listing and this broadcast's eater set must all agree. Deferred pending residual profiling. |
| 6 | Every gateway pass attempts reads on idle sessions, scans queued delivery times and flushes every open socket. | Readiness could remove futile receives from the 15.16% socket-service category at 500 clients (35.11% at 100). Flushing is already after servicing all sockets, with a 1 ms idle ceiling; changing its policy could alter release latency. | Higher timing/lifecycle risk. The older WSL epoll ABBA experiment established no reliable gain. No new reactor/dependency, flush policy, sleep policy or handwritten framing is introduced. |

Leaderboard rows, minimap construction, snake coverage candidates, prey report
clocks and world food listings are already shared. Per-recipient histories,
visibility membership, elapsed headers, session IDs and latency draws cannot
simply be shared. This change leaves their owners and scheduling intact.

## Output identity

Before any AWS timing, both independently built release binaries produced equal
complete decompressed traces. Comparisons streamed every byte independently of
the hashes. Every child exited successfully with a terminal report. Selected
hashes and sizes are in `identity.json`; full JSONL/gzip files remain local-only.

| Trace | Decisions / catalog window | Equal uncompressed bytes per revision |
| --- | --- | ---: |
| portable | catalog | 17,204,000 |
| protocol | catalog | 8,248,574 |
| client-official-500 | 1 decision after warmup | 82,184,175 |
| client-battledome-64 | 4 decisions after warmup | 14,279,992 |
| retained step_all | 384 decisions | 1,771,005,662 |
| retained next_frame | 384 decisions | 1,745,484,172 |

The client traces retain ordered wire records, actions, frames and world/event
state. Retained modes include the manual reconnect at decision 192, persistent
independent decoder replay and all eight slots. This proves identity for these
finite traces; the retained public harness does not expose private backend RNG,
queues or subscription state. It is not a universal proof over arbitrary input.

Focused regressions compare cached deltas to the uncached encoder for divergent
histories, own/peer packet forms and full-resend fallback; preserve food storage
while changing listings and equal-sized subscription sets; and compare actual
WebSocket bytes in elapsed, bare and sequence modes, including sequence wrap
and short/16-bit/64-bit payload lengths. Existing pickup, marker, report-clock,
latency-order, reconnect, backpressure and fragmented-input tests remain in use.

The initial complete CONTRIBUTING Rust group passed on Windows with
`CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=4`, `RAYON_NUM_THREADS=4`, including all
feature Clippy builds, default/core-only tests, doctests, C ABI tests and Python
crate checking. The focused client protocol/server/contract/runner/transport
tests and perf-baseline tests also passed. The rebased candidate's entire Rust
group passed on native Linux AWS before identity and timing, including the
merged spawn-queue tests and the new optimization regressions. No local timing
sample is used; new heavy local commands honor the owner's quiet-window lock.
The final documentation suite also passed on AWS after the holds, using an
overlay of the report and changelog. The large final generator archive was
hashed on AWS; the small server archive was hashed after download. Pins label
the digest origin; the large local download was not rehashed during the owner's
quiet window.

## Measurement method

Base `473e188` and candidate `c63eef6` were built on the dedicated AWS generator
with Rust 1.99.0, locked dependencies, release thin LTO, one codegen unit and
line-table debuginfo. Binaries were copied out of mutable targets and hashed.
The earlier comparison used base `9480608` and candidate `08b1fad`. Rebase over
#113 changed runtime/admission and the Battledome harness, so it triggered a
complete follow-up comparison rather than relabeling the first measurements.

`tools/perf_compare.py --base 473e188 --cases client-official-500
client-battledome-64 official-100-step step-default --repeats 5 --no-profiles`
ran on one pinned core of the otherwise quiet c7a.4xlarge, before any capacity
load. The runner independently checks full-state parity, binds binaries to source
and harness identities, warms up and times adjacent alternating pairs. The two
physics rows are unaffected controls. Linux process CPU is unavailable in that
runner; those results are operation wall times, separately from capacity CPU.

Capacity uses #118's unchanged `host_probe.rs`, `host_capacity_load.rs`,
`run_trial.py` and `analyze.py`, against the same c7a.medium/c7a.4xlarge pair in
one zone. All host threads share the medium's single physical core. Eight
generator shards run on separate generator cores. Each trial starts a fresh
seed-7 default official world with N socket seats, zero bots, a 7 ms tick and
15 s warmup. Base/candidate confirm-250 holds last 300 s; candidate/base
sweep-500 holds last 120 s. Population, generator errors, CPU, deferred ticks
and pooled nearest-rank excess p99 are retained. CPU/tick divides the host's
sampled user+system CPU seconds by executed ticks; simulation busy-wall load
includes preemption and is a different metric.

Excess is receipt time minus the server's cumulative elapsed header and that
connection's minimum within the hold. Fixed offsets cancel; world slowdown
does not. This is not an official-server fidelity measurement. The original
≤3 ms and ≤10 ms/no-deferral grades are unchanged.

## Paired operation results

| Workload | Measured operations per child | Median base → candidate ms/operation | Median paired wall ratio | Five-pair range |
| --- | ---: | ---: | ---: | ---: |
| client-official-500 | 25 decisions | 42.627 → 40.038 | 0.9385 | 0.9362–0.9427 |
| client-battledome-64 | 40 decisions | 11.618 → 11.269 | 0.9701 | 0.9666–0.9721 |
| official-100-step | 1,000 ticks | 0.16659 → 0.16723 | 1.0053 | 0.9973–1.0129 |
| step-default | 100,000 ticks | 0.0017333 → 0.0017501 | 1.0055 | 0.9941–1.0217 |

The synthetic operations take about **6.2% less wall time** at official-500 and
**3.0% less** at Battledome-64 in these five pairs. The unaffected controls have
mixed pair ranges around 1; their small median shifts support no speed claim.
These are descriptive paired ratios, not significance tests or isolated CPU
savings. The comparison's four trace rows are also equal. Full raw per-child
readings and the untallied warmup remain under the local `perf-combined/` result
tree. The earlier pre-#113 Battledome result was under 1% and inconclusive; it
is preserved with its different admission workload in the initial round.

## Capacity results

| Hold | Revision | CPU ms/executed tick | Ticks/s | New deferred ticks | Excess p99 ms | Playing |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| confirm-250, 300 s | base | 3.710 | 142.850 | 0 | 11.696 | 250.000 |
| confirm-250, 300 s | candidate | 3.534 | 142.880 | 0 | 9.938 | 250.000 |
| sweep-500, 120 s | base | 7.023 | 138.500 | 520 | 3,782.955 | 499.796 |
| sweep-500, 120 s | candidate | 6.690 | 142.400 | 76 | 740.637 | 499.903 |

Selected reductions are in `capacity-results.json`; original receipt and
resource samples remain local. One matched hold per variant does not establish
a stable capacity cap or a statistical guarantee on the tail.

At 250 clients, host CPU per executed tick falls **4.8%**. The candidate narrowly
passes the ≤10 ms/no-deferral grade in this hold; the base fails the tail grade
while keeping world time. Both keep all 250 clients playing and report no
generator error. A p99 just below 10 ms is not comfortable long-run headroom.

At 500 clients CPU per tick falls **4.7%**, new deferred ticks fall from 520 to
76, and excess p99 falls from 3.783 s to 0.741 s. Both variants still fall
behind and fail the delivery grade. The result supports lower CPU cost, not
500-client real-time capacity. Neither sweep reports a generator error.
The host reports zero CPU steal in all final holds. The generator uses under
0.90 of one core in aggregate across its eight shards on 16 cores; its sampled
steal is below 0.001% of total capacity. Selected memory/resource figures remain
in the JSON; lower CPU is not inferred from simulation busy-wall load.

## Cost and cleanup

[aws-summary.json](aws-summary.json) records the launch-quote lifetime estimate,
ancillary allowance, before/after credit amounts and sanitized regional cleanup
counts. This is an estimate, not an invoice; reported credit balances can lag.
The initial round and zero-instance quota failure are included in the total.
Both provisioned rounds use one-time Spot, 17 concurrent vCPUs, root-volume
delete-on-termination and a verified 120-minute shutdown timer. Instance pairs
never overlap. Keys and cloud-identifying metadata stay outside the repository.
The total estimate is **$0.93**, including both rounds and $0.40 of ancillary
allowances; the zero-instance quota failure adds no compute cost. Reported
credits are **$118.77 before and after**. Final cleanup at **04:14:59 UTC**
verified zero nonterminated instances, volumes, addresses, snapshots and
open/active Spot requests, with the key pair and security groups deleted.

## Limits and evidence access

Five timing pairs are descriptive, and one capacity hold per revision/size is
not a confidence interval or long-run tail guarantee. The three changes are
measured together; no isolated gain is attributed to any one commit. This does
not establish capacity for long snakes, clustering, boosting, slow readers,
TLS, public Internet paths, mixed 500-bot/socket loads or macOS. The source is
portable and Windows tests passed; Linux is the measured host platform.
After all measurements, main advanced to `77f23d4` (#121, typed core errors).
The PR's merge check is conflict-free; that later runtime is outside these
performance pins. Cross-platform CI on the PR's merged checkout remains a
separate integration check, not a new timing sample.

Raw samples, complete traces, build/test logs, immutable binary copies, source
bundles and cloud metadata are local-only under
`C:\Users\Nikita\.t3\scratch\host-opt-aws\`. Identifying cloud data and keys
are excluded from Git. Selected results and hashes identify retained material;
they do not make it publicly downloadable or establish universal performance.

## Reproduction and retained attempts

Use the source and harness pins in `source-pins.json`, independent release
targets, and the trace commands from the [performance guide](../../../docs/performance.md).
On one quiet dedicated AWS core, run the command above from the final candidate
checkout. For capacity, use the unchanged tools in the adjacent
[capacity study](../2026-10-26-host-capacity/REPORT.md), with `run_trial.py host`
and `load`, a shared epoch, eight load shards, 15 s warmup and the stated hold
duration. Bind a fresh world for every hold and preserve full receipts and
telemetry before reduction with `analyze.py`.

The first local lint run found a redundant Bytes conversion; it was removed
before the candidate was measured, and the full Rust group passed on rerun.
The first complete AWS round was superseded by #113's merge, not discarded;
its traces, samples, pins, cost and cleanup are retained in
[initial results](INITIAL-RESULTS.md). An immediate follow-up launch failed
with `MaxSpotInstanceCountExceeded`, launched zero instances and cleaned up
all temporary key/groups before inspection. A fresh bounded attempt after
the quota-accounting delay succeeded. No timing result was selected or removed
because of its speed, and no incomplete attempt supports a speed claim.
