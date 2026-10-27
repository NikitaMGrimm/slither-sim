# One-core tick/socket handoff, 2026-10-27

**Neither serialized schedule nor the ENA control qualified official-500.** No fix PR, runtime default or
hosting-capacity update. The original and rebased prototypes remain local Git commits and a local-only patch.
Runtime baseline is main-at-start `40ce9fa`; prototype `c41e409` rebased cleanly onto `6769d71` as `587d03e`.
The scheduler files are byte-identical across rebase; #215 framing and #216 food edits elsewhere are preserved.
The measured binaries predate those edits; rebase is not a capacity qualification of later main.
See the frozen [plan](PLAN.md), [harness recovery](RECOVERY.md), and [selected results](selected-results.json).

## c7a.medium official-500, one 120 s hold per arm

Same #201 delivery gate: excess p99 <=10 ms and no new deferred ticks, with valid traffic/occupancy.
All rows miss, with zero new deferrals and 500 clients playing mean/final. Zero server bots; no outside server.

| Arm | Excess p99/max ms | CPU ms/tick | Core CPU % | Tick busy p99/max ms | Peak RSS MiB |
| --- | --- | ---: | ---: | --- | ---: |
| Default | 62.907 / 178.651 | 4.713 | 67.3 | 9.446 / 19.311 | 51.27 |
| Turn-taking | 36.378 / 221.161 | 4.495 | 64.1 | 4.769 / 11.075 | 51.18 |
| Inline | 49.427 / 180.046 | 4.467 | 63.8 | 4.829 / 10.385 | 51.43 |
| Default + ENA RX 64 us | 25.627 / 144.392 | 4.787 | 68.4 | 9.099 / 20.270 | 52.59 |

| Arm | Enqueue-to-ingest p50/p95/p99/max ms | Read-to-ingest p99 ms | Enqueue reaches next started tick % | Read reaches next started tick % |
| --- | --- | ---: | ---: | ---: |
| Default | 4.413 / 9.705 / 62.184 / 170.561 | 61.475 | 84.97 | 95.16 |
| Turn-taking | 8.523 / 12.165 / 58.332 / 224.227 | 58.132 | 29.34 | 94.90 |
| Inline | 8.520 / 12.009 / 55.708 / 186.712 | 52.450 | 29.64 | 95.15 |
| Default + ENA RX 64 us | 4.426 / 8.545 / 26.660 / 142.998 | 22.144 | 85.60 | 95.58 |

Each row matched all 9,600 sampled commands, with no missing read/ingest records. Enqueue is after the client
WebSocket write accepts bytes; it is not a NIC transmit time. Server read is command forwarding, and ingest is
the backend consuming that command before physics. Next started tick uses the actual sampled tick starts;
pending future virtual timestamps during catch-up can postpone consumption beyond the next executed tick.
The once-per-tick read pass avoids mid-tick socket competition but adds input waiting. Even eight-client smokes
showed enqueue-to-ingest p99 7.55 ms default versus 13.87/13.95 ms turns/inline; all 80 samples matched per smoke.
Thus ~95% next-tick ingestion after a read is not proof that inputs enqueued during a tick reach the next tick.

## What changed, and what did not pay

- Explicit experimental `SLITHERSIM_HOST_SCHEDULE=turns`: simulation holds a mutex through the entire tick and
  disables early per-batch delivery. A condition-variable baton signals tick completion, then waits for the
  socket read/send pass acknowledgement. Idle socket waits use the tick signal or next due frame, not 1 ms
  polling inside physics. Future due-frame passes use the same exclusion gate. Default concurrency stays unchanged.
- `inline` removes the socket thread; the simulation runs a nonblocking gateway pass after every tick and services
  modeled due frames between ticks. Input commands still use the existing arrival/ingestion rules; none are backdated.
- Simulation involuntary switches fell from 66,531 to 36,949 with the baton, yet delivery/input tails remained.
  Inline combined physics and socket work on one thread and still missed. Timing serialization alone did not qualify.
- ENA readback was adaptive RX on, RX 20 us /TX 64 us initially. All requests succeeded; RX became 64 us with
  adaptive RX still on, TX remained 64 us. Its lower excess p99 is one descriptive hold, not demonstrated causal gain.
  Network IRQ/s default/turns/inline/ENA was 23,057/21,687/22,285/23,634. ENA socket kernel CPU was 31.0%, versus
  31.5% default: no observed interrupt-rate/kernel-cost relief. No allowance exceedance; no operator setting adopted.
- No candidate passed, so conditional c8a one-/two-core confirmation was not run. Multi-core behavior is not qualified.

## Method and checks

Same #201 frozen load source and steering/decoder/control timings, adding buffered samples only for IDs divisible
by 64. Eight pinned shards on a separate c7a.4xlarge; seed 7, 15 s warmup, no boost/TLS/added latency, fresh worlds.
All host threads were verified on CPU [0]. Input instrumentation and counters were equal across arms; no new packets.
Cross-host enqueue timing uses UTC application samples. Launch chrony snapshots had normal leap status and combined
reported offset + root-delay/2 + dispersion ~0.27 ms; this is not a throughout-hold clock-error bound. Read-to-ingest
is within the host. [analyze_inputs.py](analyze_inputs.py) FIFO-matches ordered steering bytes and reports unmatched
samples: `python analyze_inputs.py HOST LOAD OUT`. Packet bytes are unchanged; probes are not wire timestamps.
Rust 1.99 locked release, thin LTO, one codegen unit, line-table debug, fresh separate targets, jobs=1. Local work
used <=2 logical CPUs. [instrument.py](instrument.py) applies #201 counters and input traces to isolated snapshots.
Tick busy ends at handover before post-tick I/O/acknowledgement; baton acquisition may be included. Operator
simulation_load includes schedule-specific inline service or acknowledgement wait, so its meaning differs by arm.
Total CPU/tick remains process user+system /executed ticks. RSS includes diagnostics, startup and warmup.

Protocol parity: 500-seat fixed-input traces for 19, mixed 13/19 and 13, including reconnect, minimap and leaderboard
periods, matched every recipient payload/timing and final checkpoint across all schedules. Linux native host unit
checks: 49 passed; loopback: 21 per mode, 63 total. Full Rust group passed all 15 commands before and after rebase;
the rebased group reran protocol parity after #215/#216. Native shutdown tests initially lacked #201 tick-output
environment; preserved log and test-environment-only correction are in raw evidence. No runtime binary changed.
Three input-reducer regressions and documentation checks pass. All four holds re-reduced locally; no generator errors.

First pair ran no qualification hold: trace filenames collided with the legacy shard JSON census at smoke reduction.
Both instances terminated; corrected packaging re-reduced the retained smoke, and the one allowed recovery reused
identical tested binaries. Preserve that smoke, failure and all costs. No performance-based repeat or exclusion.
One seed/hold, evolving world, timestamp bounds and input sampling limit conclusions. No browser/TLS, retail EPYC,
long-uptime or later-main capacity claim. Runtime sources, commits, patch and logs remain local-only, not downloadable.

| Local-only archive | SHA-256 |
| --- | --- |
| Recovery c7a.medium | `ce1314723654a4af075810dc22ae1019ee4567f5028f0396dc3c00a0f2a9c541` |
| Recovery c7a.4xlarge | `669b2e99c344c01d4daab1ee8d0e106d7b819d83dd0c9c1850f97a977842d635` |
| First c7a.medium | `7fe48553b877145d7983241d44f8aed45b035225c61b9ef0c7d6573f4a4b1abe` |
| First c7a.4xlarge | `17c725b20595048296826599beb37f34e369ed99e1f364299bf7e5a17d0e2983` |
| Session record | `3e129c5d0c84e6fa90f890d7ea862673218fe391d53c86068e7898e5103f5f04` |

## Cost and cleanup

Estimated **$0.347**, below $3, including both pairs; not an invoice. Compute $0.243,
IPv4 $0.005, storage $0.004, egress $0.085, minor-transfer reserve $0.010.
Launch quotes times observed lifetimes; $0.12/GB-month disks and $0.12/GB downloads, no credits. Root disks delete
on termination; shutdown behavior terminate and hard timers verified (first 120 min, recovery 90 min). No regions
or credentials enabled/obtained. Final scan 2026-10-27T16:01:17.966192+00:00: all 34 regions attempted;
all 17 enabled regions verified zero non-terminated instances, volumes, open/active Spot requests, EIPs and created
groups/keys. The other 17 disabled/API-inaccessible regions remain unverified. Temporary setup resources also deleted.
