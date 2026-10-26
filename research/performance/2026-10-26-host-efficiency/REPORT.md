# Host broadcast and frame ownership, 2026-10-26

Three changes cut one-core host CPU per executed tick by 4.8% at 250 socket
clients and 4.7% at 500 in matched native AWS holds, with byte-identical output
in all six deterministic traces. The 250-client candidate narrowly passes the
10 ms excess-p99 grade in one 300 s hold; 500 clients still fall behind.

The traffic is synthetic simulator traffic. No outside game server was
contacted, and no policy or mechanics code changed. The [plan](PLAN.md),
including its follow-up section, was written before each round of timing.

## Changes

The [capacity study](../2026-10-26-host-capacity/REPORT.md) profiled 500
clients at `6bcb6f0`: recipient encoding 24.77%, framing and flush 23.15%, food
sectors 20.06% and socket service 15.16%. Those are bounds on the containing
work, not predicted savings. The three changes are measured together; no gain is
attributed to any one of them.

1. **Stable food subscriptions.** `food_updates` used to take and rebuild each
   recipient's sector vector and clone every sector `Arc` every tick. When a
   recipient's sectors are unchanged it now keeps them in place. Pickups are
   still processed for equal `Arc` pointers, and changed memberships still use
   the original ordered merge.
2. **Shared peer deltas.** Within one broadcast, recipients with identical prior
   `KnownSnake` state for a peer snake now reuse one encoded delta instead of
   repeating anchor recognition, trigonometry and encoding. The cache holds one
   entry per snake, so divergent histories cost one miss each and storage stays
   bounded. Own-snake packets bypass it. Every other input to the encoder is per
   snake and fixed for the broadcast.
3. **Owned frames.** `Open::send` cloned an already-owned delivery buffer before
   handing it to tungstenite. It now overwrites the sequence header in place and
   slices bare frames from the owned bytes. Tungstenite's own payload copy,
   WebSocket headers and kernel writes remain.

Deferred, with the reason:

- A generation-checked slot-to-session index for `Gateway::deliver`. The profile
  does not isolate its share, and it would cross the admission changes.
- Shared sector-delta or initial `F` encodings for recipients holding the same
  listing. Unchanged-`Arc` skipping already avoids most pellet scans.
- Readiness-driven socket service. An earlier WSL epoll comparison found no
  reliable gain, and changing flush policy could alter release latency.

Leaderboard rows, minimap, coverage candidates, report clocks and world food
listings were already shared. Per-recipient histories, visibility, elapsed
headers, session IDs and latency draws cannot be.

## Output identity

Independently built base and candidate release binaries produced equal complete
decompressed traces before any timing. Comparisons streamed every byte; selected
hashes and sizes are in [identity.json](identity.json).

| Trace | Window | Equal uncompressed bytes per revision |
| --- | --- | ---: |
| portable | catalog | 17,204,000 |
| protocol | catalog | 8,248,574 |
| client-official-500 | 1 decision after warmup | 82,184,175 |
| client-battledome-64 | 4 decisions after warmup | 14,279,992 |
| retained step_all | 384 decisions | 1,771,005,662 |
| retained next_frame | 384 decisions | 1,745,484,172 |

Client traces hold ordered wire records, actions, frames and world and event
state. The retained runs include a manual reconnect at decision 192, an
independent decoder replay and all eight slots. They prove identity for these
finite traces only, and do not expose private backend RNG, queues or
subscription state.

New regressions compare cached deltas with the uncached encoder across divergent
histories, own and peer packet forms and full-resend fallback; keep food storage
across changed listings and equal-sized subscription sets; and compare actual
WebSocket bytes in elapsed, bare and sequence framing, including sequence wrap
and all three payload-length forms. The full CONTRIBUTING Rust group passed on
Windows before the rebase and on the AWS generator after it.

## Method

Base `473e188` and candidate `c63eef6` were built on the AWS generator with Rust
1.99.0, locked dependencies, thin LTO, one codegen unit and line-table
debuginfo, then copied out of the target directory and hashed
([source pins](source-pins.json)).

Operation timing ran on one pinned core of the otherwise idle c7a.4xlarge
before any capacity load:

```text
tools/perf_compare.py --base 473e188 --cases client-official-500
  client-battledome-64 official-100-step step-default --repeats 5 --no-profiles
```

The runner checks full-state parity, warms up and times five adjacent
alternating pairs. The two physics rows are unaffected controls. These are wall
times; the runner does not report Linux process CPU.

Capacity reused the capacity study's unchanged probe, load generator, trial
runner and analysis on the same c7a.medium server and c7a.4xlarge generator in
one zone. All host threads share the server's single core; eight load shards run
on separate generator cores. Each hold starts a fresh seed-7 default official
world with N socket seats, no bots, a 7 ms tick and 15 s warmup. 250-client
holds last 300 s (base first); 500-client holds last 120 s (candidate first).
CPU per tick divides sampled host user and system CPU by executed ticks. Excess
is receipt time minus the server's elapsed header and that connection's minimum
within the hold; it tracks host delay, not official-server fidelity. The ≤3 ms
and ≤10 ms/no-deferral grades are unchanged from the capacity study.

## Results

| Workload | Operations per child | Median base → candidate ms/op | Median paired ratio | Five-pair range |
| --- | ---: | ---: | ---: | ---: |
| client-official-500 | 25 decisions | 42.627 → 40.038 | 0.9385 | 0.9362–0.9427 |
| client-battledome-64 | 40 decisions | 11.618 → 11.269 | 0.9701 | 0.9666–0.9721 |
| official-100-step | 1,000 ticks | 0.16659 → 0.16723 | 1.0053 | 0.9973–1.0129 |
| step-default | 100,000 ticks | 0.0017333 → 0.0017501 | 1.0055 | 0.9941–1.0217 |

Client operations take about 6.2% less wall time at official-500 and 3.0% less
at Battledome-64. The controls' pair ranges straddle 1 and support no speed
claim. These are descriptive paired ratios, not significance tests
([perf-results.json](perf-results.json)).

| Hold | Revision | CPU ms/executed tick | Ticks/s | New deferred ticks | Excess p99 ms | Playing |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 250, 300 s | base | 3.710 | 142.850 | 0 | 11.696 | 250.000 |
| 250, 300 s | candidate | 3.534 | 142.880 | 0 | 9.938 | 250.000 |
| 500, 120 s | base | 7.023 | 138.500 | 520 | 3,782.955 | 499.796 |
| 500, 120 s | candidate | 6.690 | 142.400 | 76 | 740.637 | 499.903 |

At 250 clients both revisions keep world time and every client playing; the
candidate passes the ≤10 ms grade and the base misses it on the tail. A p99
just under 10 ms is not comfortable headroom. At 500 clients deferred ticks fall
from 520 to 76 and excess p99 from 3.78 s to 0.74 s, but both revisions fall
behind. This supports lower CPU cost, not 500-client real-time capacity.

No hold recorded a generator error or host CPU steal. The generator used under
0.9 of one core across its eight shards. Selected reductions, including memory
and resource figures, are in [capacity-results.json](capacity-results.json).

## First round before #113

The first complete round compared base `9480608` with candidate `08b1fad` on the
same instance types. #113 merged during it and changed host admission and the
Battledome harness, so the follow-up above repeated everything on the new main
instead of relabeling these results. Its [identity](initial/identity.json),
[operation](initial/perf-results.json), [capacity](initial/capacity-results.json),
[pin](initial/source-pins.json) and [cost](initial/aws-summary.json) selections
are kept separately and not pooled with the final comparison.

All six traces matched there too. Official-500 operations had a median paired
ratio of 0.9404; Battledome-64, under the old admission workload, 0.9928. At
250 clients CPU per tick fell from 3.733 to 3.588 ms and excess p99 from 10.840
to 9.619 ms. At 500 clients CPU per tick fell 6.9% and deferred ticks from 936 to
68, and both revisions still fell behind.

## Cost, cleanup and attempts

Both rounds used one-time Spot instances, 17 concurrent vCPUs, root volumes
deleted on termination and a verified 120-minute shutdown timer; instance pairs
never overlapped. The estimated total is $0.93, including $0.40 of ancillary
allowance; it is an estimate from launch quotes, not an invoice
([aws-summary.json](aws-summary.json)). Final cleanup at 04:14:59 UTC verified
no remaining instances, volumes, addresses, snapshots or Spot requests, and
deleted the key pair and security groups.

One launch between the rounds failed with `MaxSpotInstanceCountExceeded`,
started no instances and was cleaned up; a later launch succeeded. The first
lint run found a redundant `Bytes` conversion, removed before measurement. No
timing result was selected or dropped for its speed.

## Limits

Five timing pairs and one capacity hold per revision and size are descriptive;
they give no confidence interval or long-run tail guarantee. The holds do not
cover long snakes, clustering, boosting, slow readers, TLS, Internet paths,
mixed bot and socket loads, or macOS. Linux is the only measured platform.
Main advanced to `77f23d4` (#121, typed core errors) after the measurements;
that runtime is outside these pins.

Raw samples, complete traces, logs, binaries and cloud metadata remain
local-only. Keys and identifying cloud data are excluded.

## Reproduction

Check out the pinned sources, build independent release targets and run the
trace commands from the [performance guide](../../../docs/performance.md). Run
the operation command above on one idle dedicated core. For capacity, use the
capacity study's unchanged tools with `run_trial.py host` and `load`, a shared
epoch, eight load shards, 15 s warmup, the stated hold and a fresh world per
hold, then reduce full receipts and telemetry with `analyze.py`.
