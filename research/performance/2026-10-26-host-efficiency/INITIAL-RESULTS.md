# Initial results before #113, 2026-10-26

Historical round 1: base `9480608`, candidate `08b1fad`. The final PR is rebased
over #113 and uses a separate comparison; see [follow-up plan](REBASE-PLAN.md).
The selections below are retained in `initial/`, with source pins and hashes.

## Question and scope

Reduce one-core host CPU without changing protocol-19 bytes, connection order,
virtual delivery times or packet-family cadence. This is synthetic simulator
traffic. No outside game server was contacted, and no policy or mechanics code
was changed. The [plan](PLAN.md) was written before timing.

The [native capacity profile](../2026-10-26-host-capacity/REPORT.md) measured
`6bcb6f0`. This comparison starts at `9480608`, including the subsequent
construction/admission and prey fixes. The earlier capacity numbers are
motivation, not a same-source control.

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
| 4 | Changed/new sectors are diffed or encoded separately for recipients holding the same listing. | Could share sector-delta or initial `F` encodings; existing unchanged-Arc skipping already avoids pellet scans on most sectors. Remaining opportunity is a subset of food cost, especially maintenance, pickups and joins. | Moderate: previous listing, current physical/decorated listing and this broadcast's eater set must all agree. Deferred pending residual profiling. |
| 5 | Every gateway pass attempts reads on idle sessions, scans queued delivery times and flushes every open socket. | Readiness could remove futile receives from the 15.16% socket-service category at 500 clients (35.11% at 100). Flushing is already after servicing all sockets, with a 1 ms idle ceiling; changing its policy could alter release latency. | Higher timing/lifecycle risk. The older WSL epoll ABBA experiment established no reliable gain. No new reactor/dependency, flush policy, sleep policy or handwritten framing is introduced. |

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
| client-battledome-64 | 4 decisions after warmup | 22,504,185 |
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

The complete CONTRIBUTING Rust group passed on Windows with
`CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=4`, `RAYON_NUM_THREADS=4`, including all
feature Clippy builds, default/core-only tests, doctests, C ABI tests and Python
crate checking. The focused client protocol/server/contract/runner/transport
tests and perf-baseline tests also passed. No local timing sample is used.

## Measurement method

Base `9480608` and candidate `08b1fad` were built on the dedicated AWS generator
with Rust 1.99.0, locked dependencies, release thin LTO, one codegen unit and
line-table debuginfo. Binaries were copied out of mutable targets and hashed.
The delivery branch was rebased over #118's research-only merge; source and
Cargo-file comparison confirms that its runtime matches the measured candidate.

`tools/perf_compare.py --base 9480608 --cases client-official-500
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
| client-official-500 | 25 decisions | 42.178 → 39.707 | 0.9404 | 0.9323–0.9486 |
| client-battledome-64 | 40 decisions | 48.353 → 48.003 | 0.9928 | 0.9917–0.9939 |
| official-100-step | 1,000 ticks | 0.16815 → 0.16850 | 1.0025 | 0.9993–1.0098 |
| step-default | 100,000 ticks | 0.0017303 → 0.0017326 | 1.0005 | 0.9980–1.0065 |

The 500-client synthetic operation takes about **6.0% less wall time** in these
five pairs. Battledome changes by less than 1%; no meaningful Battledome speedup
is claimed. The unaffected controls remain near 1. These are descriptive paired
ratios, not significance tests or isolated CPU savings. The comparison's four
trace rows are also equal. Full raw per-child readings and the untallied warmup
remain under the local `perf-combined/` result tree.

## Capacity results

| Hold | Revision | CPU ms/executed tick | Ticks/s | New deferred ticks | Excess p99 ms | Playing |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| confirm-250, 300 s | base | 3.733 | 142.857 | 0 | 10.840 | 250.000 |
| confirm-250, 300 s | candidate | 3.588 | 142.857 | 0 | 9.619 | 250.000 |
| sweep-500, 120 s | base | 7.197 | 135.051 | 936 | 6,582.182 | 499.759 |
| sweep-500, 120 s | candidate | 6.700 | 142.285 | 68 | 505.189 | 499.880 |

At 250 clients the candidate uses **3.9% less host CPU per executed tick**.
It passes the ≤10 ms/no-deferral grade in this hold, while the base narrowly
misses it. This is one matched hold per variant, not a new stable capacity cap
or a statistical guarantee on the tail. Neither 250-client hold reported a
generator error or lost playing population. Selected reductions are in
`capacity-results.json`; original receipt and resource samples remain local.

At 500 clients CPU per tick falls 6.9% in this pair, but both worlds defer ticks
and fail the delivery grade. No 500-client real-time capacity claim is supported.
All four holds reported no generator errors. This round's estimated cost is
$0.48, including a $0.20 ancillary allowance, not an invoice. Credits were
$118.77 before and after (reporting can lag). At 03:38:02 UTC the regional
inventories had zero nonterminated instances, volumes, addresses, snapshots and
open/active Spot requests; the key pair and both security groups were deleted.

Round-specific selections: [identity](initial/identity.json),
[operation results](initial/perf-results.json),
[capacity](initial/capacity-results.json), [source pins](initial/source-pins.json),
[cost and cleanup](initial/aws-summary.json). Original local-only raw archives
are `host-opt-aws/attempt-1/generator-results.tar.gz` and
`host-opt-aws/attempt-1/server-results.tar.gz`; their hashes are in the pins.

## Limits and evidence access

Five timing pairs are descriptive, and one capacity hold per revision/size is
not a confidence interval or long-run tail guarantee. The three changes are
measured together; no isolated gain is attributed to any one commit. This does
not establish capacity for long snakes, clustering, boosting, slow readers,
TLS, public Internet paths, mixed 500-bot/socket loads or macOS. The source is
portable and Windows tests passed; Linux is the measured host platform.

Raw samples, complete traces, build/test logs, immutable binary copies, source
bundles and cloud metadata are local-only under
`C:\Users\Nikita\.t3\scratch\host-opt-aws\`. Identifying cloud data and keys
are excluded from Git. Selected results and hashes identify retained material;
they do not make it publicly downloadable or establish universal performance.
