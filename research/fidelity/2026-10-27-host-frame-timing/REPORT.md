# Host frame timing, 2026-10-27

The socket thread added avoidable delay after a frame had already been encoded.
It slept until its next socket-input poll, without a notification from the
simulation thread. The change wakes it on publication, while retaining the
existing wait for a frame's future latency-model deadline. Windows uses an
auto-reset event and a high-resolution waitable timer; Linux uses the existing
delivery channel. No spin, global timer-resolution change, physics, encoder,
elapsed-header, packet-order, or latency-model change is made.

This identifies and removes one measured source of delivery jitter. It does
**not** establish that this accounts for the owner's perceived browser stutter,
or that the stock renderer is now smooth. Spawn-admission work and OS scheduling
still delay some frames before handoff.

## Method and provenance

Baseline: `20e5064378ae8cabc7ca50cceacdbe339e6846b4`. See [plan and retained
amendments](PLAN.md), [selected results](results.json), and [source pins](sources.json).
All experiments are exploratory, on one shared Windows 11 desktop and its
Ubuntu 24.04 WSL2 guest, with Rust 1.99.0 release builds, locked dependencies,
thin LTO/one codegen unit. Windows receiver: Node 24.15.0; WSL receiver: Node
22.23.1. Builds used at most two jobs; study host, bot workers, recorder and
helpers were restricted to logical CPUs 0 and 1. No owner service was touched,
no external game server was contacted, and no new official recordings were made.
Only isolated `127.0.0.1:8790+` study endpoints were used.

The world is seed-7 Battledome, 64 background A4 bots, two bot workers,
responsive delivery and zero modeled player latency. The matched series has
four reserved player slots and one recording client; initial diagnostics had
one reserved slot. The natural collector is the repository stock protocol-19
headless forager, without the physical-food extension. Deaths, sparse views and
rejoins remain in the data. A separate synthetic probe sends its next single
outstanding game ping immediately upon each pong, including after death, so
otherwise silent ticks can be distinguished from delayed delivery. Its workload
is deliberately different from official play and is never used as a parity gate.

Five alternating baseline/candidate synthetic pairs per platform use 20-second
captures and fresh ports. Initial natural diagnostics use 40 seconds. CPU is
host process user+kernel CPU divided by its whole lifetime, including startup,
shutdown and temporary trace writing; 100% means one logical CPU. No spin is used.

Receipt timestamps are taken at the recorder's incoming WebSocket callback.
Elapsed headers are decoded with the existing protocol-19 decoder. Each receipt
interval must lie wholly after setup+5 seconds, in the same connection/setup
epoch. Nothing is removed by measured gap value. Quantiles use nearest rank;
the series table reports medians of five per-run quantiles and the largest
observed maximum. Rates pool their actual eligible denominators. A burst is two
originally adjacent receipt gaps `<1 ms` then `>14 ms`; the latter also defines
the two-nominal-tick exceedance, with a 7 ms nominal tick. Sparse views legitimately
skip emissions, so a long gap alone is not a delivery error.

Temporary instrumentation buffers tick wake/deadline, command/admission work,
world advance including encoding, early socket handoff, socket release and poll
duration, then writes at orderly host shutdown. Stage summaries start at host
wall 6 seconds and continue until shutdown; they are not the same window as the
receiver summary. Handoff-to-release matches frames by modeled due time in order.
This includes channel publication/consumption and release preparation, excludes
the kernel/receiver after release, and is not absolute one-way network latency.
Production code retains none of these traces or metrics.

## Measurements

Synthetic one-outstanding-ping probe, five pairs per platform. Times are ms;
quantiles are medians of run quantiles, max is the largest gap across runs.

| Platform / arm | Gaps | Receipt p50 | p90 | p99 | Max | >14 ms | Burst pairs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Windows baseline | 10,192 | 7.098 | 7.838 | 13.414 | 17.944 | 0.667% | 0.0393% |
| Windows fixed | 10,239 | 7.073 | 7.365 | 13.039 | 15.762 | 0.449% | 0.0293% |
| WSL2 baseline | 10,183 | 7.356 | 7.602 | 13.837 | 35.748 | 0.717% | 0.0197% |
| WSL2 fixed | 10,273 | 6.999 | 7.145 | 12.747 | 27.573 | 0.409% | 0.0292% |

Median header p50/p90/p99 is **7/7/14** for Windows before and after and WSL2
before; **7/7/7** for WSL2 after. Neither a fixed-rate emission assumption nor
an isolated OS deadline guarantee follows from these workload-dependent headers.

| Stage / cost | Windows baseline -> fixed | WSL2 baseline -> fixed |
| --- | --- | --- |
| Handoff-to-release p99, median (run range), ms | 1.516 (1.496–1.517) -> 0.030 (0.026–0.054) | 1.645 (1.231–2.424) -> 0.435 (0.341–0.645) |
| Tick-wake error p99, median (run range), ms | 0.988 (0.931–1.081) -> 0.982 (0.923–1.180) | 1.677 (0.631–8.783) -> 0.792 (0.597–1.801) |
| Advance including encode p99, median, ms | 0.585 -> 0.556 | 0.816 -> 0.666 |
| Total release lateness p99, median, ms | 6.577 -> 5.885 | 6.773 -> 5.962 |
| Absolute receipt-gap minus header-gap p99, median, ms | 5.423 -> 5.497 | 5.941 -> 5.846 |
| Host CPU, % of one core, median (run range) | 15.96 (14.57–19.47) -> 16.08 (15.76–16.24) | 17.47 (16.70–19.87) -> 16.87 (16.29–18.28) |

All scheduled paired runs complete with zero deferred ticks and successful host
and collector exits. Windows input responsiveness is preserved in the final
candidate: its synthetic pong header p50 remains 7 ms. The post-encode stage
improvement is repeatable across every run; end-to-end p99 and CPU differences
overlap run variation and are not claimed as statistically established wins.
The burst rate does not consistently improve on WSL2. These results rule out
coarse 15.6 ms simulation batching as the main Windows baseline behavior here,
but do not rule out occasional OS scheduling stalls.

The largest component of early-handoff delay above 4 ms was command/admission
work in **256/263 Windows baseline**, **258/264 Windows fixed**, **235/270 WSL2
baseline**, and **265/293 WSL2 fixed** frame witnesses. The rest is mostly wake
error; three WSL2 fixed witnesses were dominated by advance before early handoff.
These witnesses are correlated frames, not independent stalls. The timing fix
does not change admission work or its observable game behavior.

Separate fixed-host profiles, one per platform (3,143 ticks each after wall 6 s):
Windows broadcast/encoding p50/p90/p99/max **0.162/0.266/0.434/3.894 ms**, admission
**0.0001/0.0001/5.231/6.185 ms**. WSL2 broadcast/encoding
**0.184/0.357/0.764/5.017 ms**, admission **0.00005/0.00013/5.601/7.848 ms**.
Broadcast timing includes shared report preparation and per-recipient encoding;
world advance additionally includes physics. These diagnostic runs do not enter
the paired performance or CPU estimates.

Natural-forager checks, one 20-second capture per arm/platform; descriptive,
with different wall-clock-driven worlds. All burst percentages are zero except
WSL2 fixed, 0.0864% (one pair). Header tails closely track receipt tails.

| Platform / arm | Gaps | Receipt p50 / p90 / p99 / max, ms | Header p50 / p90 / p99 / max, ms | >14 ms |
| --- | ---: | --- | --- | ---: |
| Windows baseline | 801 | 8.042 / 27.612 / 63.300 / 97.632 | 7 / 28 / 63 / 98 | 33.71% |
| Windows fixed | 903 | 7.223 / 21.812 / 62.812 / 97.978 | 7 / 21 / 63 / 98 | 29.01% |
| WSL2 baseline | 854 | 7.540 / 27.715 / 63.055 / 97.938 | 7 / 28 / 63 / 98 | 26.46% |
| WSL2 fixed | 1,159 | 7.038 / 21.040 / 63.026 / 91.133 | 7 / 21 / 63 / 91 | 23.47% |

## Retained official comparison

All six completed near-server #111 archives selected by the timing-reference
audit were rehashed against `transport-official-near-server.json`, decoded offline
and measured with the same receipt/header formulas. The #177 protocol catalog's
single successful stock protocol-19 archive was separately rehashed and its raw
`receive` events normalized without changing their timestamps or payloads.
Its failed login and legacy protocol-13 sessions are not silently pooled into
the protocol-19 sample; no `encode13` source was touched.

The [sparse-view study](../2026-10-26-sparse-view-cadence/REPORT.md) (#146) already
shows official median header gaps of 14 ms with 1–2 visible peers and 7 ms with
6–10 peers; older zero-peer captures have a 47 ms median. The
[network-path study](../2026-10-25-network-path/REPORT.md) is a home-route/collector
fit with acknowledged timing contamination, not a reason to inject more jitter.
Official populations, views, route, activity, receiver and duration differ from
Battledome and the synthetic pong probe. These rows describe retained data,
not a workload-matched claim of official equivalence.

Same five-second per-setup burn-in, without telemetry masking or value trimming:

| Retained official archive | Gaps | Receipt p50 / p90 / p99 / max, ms | Header p50 / p90 / p99 / max, ms | >14 ms | Burst pairs |
| --- | ---: | --- | --- | ---: | ---: |
| Near low-1 | 66,587 | 7.124 / 13.809 / 22.253 / 62.684 | 7 / 14 / 22 / 63 | 9.27% | 0.00150% |
| Near second-1 | 59,971 | 7.177 / 14.252 / 32.572 / 188.173 | 7 / 14 / 32 / 188 | 12.57% | 0% |
| Near low-2 | 58,169 | 7.436 / 14.409 / 34.299 / 119.589 | 8 / 14 / 34 / 119 | 14.17% | 0% |
| Near second-2 | 62,636 | 7.091 / 14.196 / 30.555 / 139.385 | 7 / 14 / 30 / 139 | 11.85% | 0.00160% |
| Near low-3 | 62,996 | 7.315 / 14.332 / 28.250 / 114.395 | 7 / 14 / 28 / 114 | 11.40% | 0.00317% |
| Near second-3 | 64,247 | 7.048 / 14.158 / 28.831 / 128.022 | 7 / 14 / 29 / 128 | 11.22% | 0.00778% |
| #177 catalog stock-6, home receiver | 12,495 | 7.618 / 16.906 / 32.165 / 160.997 | 8 / 15 / 29 / 44 | 18.70% | 0.6643% |

The near-server data include rare receipt bursts and large header gaps; the
home catalog has much larger receipt/header disagreement. These observations
support separating emission cadence from route/receiver delay. They do not
identify which remote process, network queue, or receiver caused a given tail.

## Rejected attempts and limits

The first channel-timeout candidate is rejected. It reduced outgoing handoff
delay but Windows socket input was then polled at coarse timeout intervals:
median synthetic pong headers doubled to 14 ms and p90 reached 21 ms. Those raw
runs and their summaries remain selected. Windows now uses a notification event
plus a per-host high-resolution timer instead of the channel timeout. The
first rejected run's socket poll interval p90/p99/max was **14.775/15.783/16.981 ms**,
versus **1.545/1.756/4.429 ms** in its preceding baseline; the first final candidate
returned to **1.528/1.731/2.509 ms**. This directly locates the rejected regression
in waiting/input-poll cadence, rather than attributing it from pong counts alone. The
[Microsoft API contract](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createwaitabletimerexw)
specifies the high-resolution flag from Windows 10 1803. Only rejection of that
flag (`ERROR_INVALID_PARAMETER`) retries an ordinary timer to preserve startup
on older Windows; that compatibility path is not measured here.

The initial synthetic invocation failed its bind after cross-platform port reuse
and also exposed an incorrect new collector import. A later summary helper named
`select.py` shadowed Python's standard library during metadata finalization; its
completed raw baseline is retained with unavailable CPU, and the remaining four
pairs use a fresh output directory. A refused resume never connected. See the
plan for exact errors and selection. No failure is counted as a successful pair.

WSL2 is a Linux guest on this desktop, not a native production Linux host.
Scheduling noise, timer behavior on other Windows builds, virtualized loopback,
browser event handling and rendering, subjective smoothness, long-duration
tails, all 64 bots staying alive, and nonzero modeled latency are not qualified
by the short paired series. Physics/encoding source is unchanged; wall-clock
driven live controls mean independent network recordings are not byte-identical
world trajectories. Exact byte/order regressions are a separate check.
The paired measurements use buffered instrumentation on both sides; there is no
uninstrumented paired latency/CPU confirmation. Do not extrapolate the absolute
CPU estimates to a production service.

## Validation

- Windows: `python tools/check.py rust` passes in full: formatting, all Clippy
  feature combinations, default/core/bot/networking tests, doctests, C ABI,
  Python crate build check and the basic example. The 42 focused host tests
  include notification retention, timer rearming, prompt shutdown, queue bounds,
  framed bytes/sequence wrap, and the new future-deadline/elapsed-byte/order check.
- WSL2: 41 focused host tests, 38 protocol tests and 20 loopback integration
  tests pass with Rust 1.99.0 and at most two test threads.
- Seven documentation tests and the synthetic study-analysis regression pass.
  The latter checks burn-in, connection/setup resets, burst adjacency and actual
  eligible denominators. Source/binary pins and selected summary hashes verify.
- `src/client/simulation*`, physics and `src/client/encode13*` have no changes.
  No fresh official data, API/physics constant, timing tolerance, or baseline
  fixture is changed. Hosted CI and independent maintainer review remain separate.

Full validation logs are local-only beside the raw runs; their digests are in
the source pins. No independent agent reviewer or browser acceptance is claimed.

## Reproduction and access

Full compressed raw frames, failed attempts, temporary CSVs and binaries are
local-only under this worktree's ignored `outputs/frame-timing/`; they are not
public downloads. Hashes in the selected files establish consistency, not server
identity or scientific acceptance. [summarize.py](summarize.py) selects bounded
summaries, including failed and rejected runs, without retaining gameplay bytes
in Git. The following commands are loopback-only; use new output paths and ports.

For each baseline/candidate source, copy [host_timing.rs](host_timing.rs) to
`examples/host_timing.rs`, apply [instrument.py](instrument.py), and build the
example in a separate release target directory with at most two CPUs/jobs.
The instrumentation is temporary: restore production sources afterward and
remove that example. [profile_encode.py](profile_encode.py) optionally measures
broadcast/encoding separately, outside the matched performance series.

```sh
cargo build --release --locked --example host_timing -j2
python research/fidelity/2026-10-27-host-frame-timing/series.py BASE_BINARY FIXED_BINARY NEW_OUTPUT --port 8820 --seconds 20
python research/fidelity/2026-10-27-host-frame-timing/run.py FIXED_BINARY NEW_OUTPUT --port 8830 --seconds 20
python research/fidelity/2026-10-27-host-frame-timing/analyze.py RUN_DIRECTORY --out NEW_SUMMARY.json
```

On Linux prefix the runner/build with `taskset -c 0,1`; select Rust 1.99.0.
The Python runner enforces process affinity before launching either child.
There is no independent reviewer conversation or browser acceptance in this work.
