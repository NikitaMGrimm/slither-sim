# Native Linux protocol-19 host capacity, 2026-10-26

On a **c7a.medium: one full AMD EPYC 9R14 core, no SMT, 2 GiB RAM**, the
120-second socket sweep found **100 clients at excess p99 2.323 ms** and
**250 at 7.937 ms with zero deferred ticks**. At **500**, world time fell
behind: **140.058 ticks/s, 341 newly deferred ticks**, and excess p99
**2,349.462 ms**. At 750, throughput fell to 70.760 ticks/s.

**The longer hold matters:** the 300-second confirmation at 250 had no
deferrals but excess p99 **11.393 ms**, so its short-run criterion-(b) pass
did not survive confirmation. **100 clients did pass a 300-second hold:**
excess p99 **2.428 ms**, **142.857 ticks/s**, all 100 playing and zero
deferrals. These are finite workload results, not a universal player cap.

**500 configured background bots passed the separate target scenario:** the
simulation thread used **50.87% of one CPU** (51.10% busy-wall load), with
**zero deferred ticks** and watcher excess **0.502 / 1.007 / 2.160 ms**
(p50 / p99 / max). Bots ran on other cores of the generator machine.

The three largest exclusive categories at 500 clients were **per-recipient
delta encoding 24.77%**, **framing/flush 23.15%**, and **visibility/food-sector
work 20.06%** of sampled cycles. The leading application self functions were
`SimulatorBackend::updates` **10.49%**, `food_updates` **9.94%**, and
`Simulator::advance::<false>` **6.92%**. Kernel interrupt entry was a larger
individual self symbol at **12.76%**. These are descriptive profiles from a
separate frame-pointer build, with no speedup claim.

## Question, thresholds and scope

This is a current native-Linux capacity/profile baseline, with **no host or
simulator optimization**. It supersedes the older study's workload/profile
for this question, not its preserved historical results. All game traffic
went between the two owned instances' private addresses or over loopback;
no outside game server was contacted.

| Grade | Definition | Largest passing or first failing tested N |
| --- | --- | --- |
| (a) Official-like delivery | Per-connection min-normalized frame excess p99 ≤3 ms | **100**, including the 300-second hold at **2.428 ms** |
| (b) Real-time delivery | Excess p99 ≤10 ms and zero newly deferred ticks in the hold | **250 for 120 s**, but **100 confirmed for 300 s**; 250 failed the long delivery criterion |
| (c) World time falls behind | Executed ticks and newly deferred ticks show slowed world time | **500**, the first failing sweep count; transition bracketed above 250 and at/below 500 |

The three definitions were fixed before collection. No threshold was relaxed.
The [official timing reference](../../fidelity/2026-10-26-vps-timing/REPORT.md)
reported clean held-out excess p99 about 2.3–2.4 ms. That reference used clean
eligible windows; this study applies **no stall masking**, so the ≤3 ms grade
is an operational comparison, not an identical acquisition or a fidelity test.

The requested 1,000-client hold was conditional on 750 meeting (b). It was
therefore **not attempted**. A failed long confirmation is retained rather
than replaced with the earlier passing short hold. The supplemental hold at
the next lower predeclared count was frozen before seeing its data.

## Source, build and machine

- Measured source: `6bcb6f0b1057810bc7b31889c080ce9e3e57d6ae`, freshly fetched
  `origin/main` at launch. Both #111 and #112 were merged. This includes
  threaded/per-tick hosting, early socket handoff, bounded spawn admission,
  and bounded socket reads.
  Later main commits #116/#117 landed while this study was running; **they
  were not measured**. Rebasing this documentation PR does not change the
  measured source pin or establish capacity for that later code.
- Ubuntu 24.04 x86-64, native EC2, same `eu-central-1` availability zone.
  Server: c7a.medium; generator/bot scenario: c7a.4xlarge, 16 full cores,
  32 GiB. Actual kernel/toolchain and binary identities are pinned in
  [source-pins.json](source-pins.json). No owner-PC performance is used.
- Release, checked-in lockfile, Rust 1.99.0, thin LTO, one codegen unit,
  `CARGO_PROFILE_RELEASE_DEBUG=1` (line-table debuginfo). `.debug_line` and
  `.debug_info` were verified. One native build served both machines and
  its binary hashes matched after copying. Profiles use a separate release
  build with `RUSTFLAGS='-C force-frame-pointers=yes'`; the capacity binary
  remains unchanged. Both binary manifests are pinned.
- [`host_probe.rs`](host_probe.rs) calls the same public `Host` runtime as
  `slither-sim host`; it adds counter sampling and bounded shutdown around
  it. No internal host instrumentation, behavior patch or CLI replacement
  implementation is used. The wrapper's CPU/observer cost is included.
- Per-process nofile soft/hard limits were 4,096 / 1,048,576 for the admitted
  trials. No administrator/kernel/socket tuning was performed. Actual CPU,
  RSS, NIC, system-busy and steal covariates are in the selected results.

## Workload and timing definitions

Every socket hold starts a fresh world, seed 7, `maxClients=N`, `bots=0`,
`botThreads=0`, and otherwise default `HostConfig`. This is the host's default
official approximation, including seat-sized initial radius/food, dynamic
radius and `dt=0.007`: target **142.857 ticks/s**. Per-family gates remain
default: frames can be emitted every tick; own motion opportunities are
normally six ticks apart, with immediate anchors and separately gated food,
mass, prey, minimap and leaderboard reports. There is no artificial link delay
in the socket sweep.

Seat sizing is the host's existing default adjustment, not a replacement
`Config::official(N)`: it expands the default 670-unit world to
`670*sqrt(N)` and scales its 109-pellet initial target and ambient rate by
area. In particular, the default **one prey remains one**. Dynamic evolution
is not frozen. These are configuration/model facts; actual global food,
body-length and living-bot distributions were not independently sampled.

The generator adapts the 2026-10-24 `host_study_load.rs`, retaining its real
challenge/login, `ClientState` decoding of **every** frame, steering/pinging,
death detection, and 250 ms death/rejoin delay. It uses eight processes,
disjoint client IDs, three pending logins/process, ≥4 ms between new starts,
and a roughly 1 ms nonblocking service pass. Steering every 100 ms avoids
nearby snakes/rim, seeks food and keeps turning; boost is always false.
Failed admissions back off and are counted, never promoted to players.

All workers share an absolute start epoch, followed by **15 s warmup** and
the specified **120 or 300 s hold**. Per-frame records, own-update records
and per-second lifecycle/resource samples are buffered and written after
the hold. CPU and NIC use live process/counter snapshots at both boundaries;
their actual boundary interval is retained. Host counters are sampled at
the requested boundaries; their small scheduling overruns remain visible.

- **Requested/admitted/playing:** requested is the configured client count;
  admitted is time-weighted setup+playing+dead, playing excludes dead and
  disconnected/retry time. Integration uses service-pass lifecycle counts.
  Counts are not endpoint snapshots after synchronized closure.
- **Own-update gaps:** both receipts must be inside the hold and the same
  connection, for own motion/anchor-bearing frames. Pongs/HUD/lifecycle-only
  frames are excluded. Turning usually changes motion bytes, but a missing
  change can legitimately lengthen a gap. The old generator's 42 ms absolute
  deviation fields are not used to invent a per-tick nominal own cadence.
- **Frame excess:** all post-setup frame receipts inside the hold, minus
  cumulative server elapsed-header milliseconds; subtract the minimum
  separately for each connection **within that hold**. The setup epoch and
  any fixed link offset cancel. A reconnect gets a new baseline. This uses
  the whole connection's hold, not `host_load`'s rolling 1,200-frame chunks.
  **It includes accumulated world-time slowdown**: a slow world cannot pass
  merely because its stale frames arrive regularly. It is not absolute
  one-way latency or kernel arrival time.
- **Quantiles:** exact nearest rank over the retained rounded timings,
  pooled by receipt/update, not by giving each player equal weight. Per-client
  p50/p99/max are also retained in [selected-results.json](selected-results.json).
  Headers have 1 ms resolution; receipt sampling includes generator polling,
  scheduling and decode delays. Samples are correlated; p99/max are not
  long-run guarantees or confidence bounds.
- **World service:** ticks/s and newly deferred ticks use public counters.
  Start/end cumulative deferrals are retained. Relative world lag uses
  elapsed time since the wrapper's post-start timestamp minus `ticks*7 ms`;
  its constant epoch offset is small but unmeasured. Its **change**, executed
  ticks and deferrals establish slowing without assuming that offset is zero.
- **CPU:** process/thread CPU is Linux user+system jiffies, percentage of one
  core. `simulation_seconds()` is wall time inside simulation work, including
  preemption: it is **not** simulation CPU. RSS includes the measured process
  and its wrapper. System busy/steal are separate `/proc/stat` covariates.
- **Traffic:** payload is received binary protocol bytes, including elapsed
  headers, excluding WebSocket/TCP/IP. NIC TX/RX are `/proc/net/dev` counters,
  including TCP overhead and small SSH/management traffic; virtual-NIC/offload
  accounting is not a billed or physical-wire byte claim. Normalize by
  time-weighted playing-client seconds. No isolated uplink-capacity claim.

For 500 bots, the host runs on the generator with `bots=500`, one watcher,
seven bot threads and fixed 40 ms RTT without jitter. Simulation is pinned
to CPU 0, socket/wrapper to CPU 1, bot threads to CPUs 2–15, and the circular
watcher to CPU 1. Affinities were independently read back. This is 500
**configured** background seekers; their living population was not separately
instrumented. The watcher decodes, pings and rejoins after death. It had one
death/rejoin, so its time-weighted playing population is 0.998.

## Selected measurements and profiles

| Run | Requested / admitted / playing | Ticks/s | Deferred in hold | CPU, one core | Peak RSS MiB | Own gap p50 / p99 / max ms | Excess p50 / p99 / max ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| sweep-100 | 100 / 100.000 / 100.000 | 142.859 | 0 | 21.19% | 14.25 | 41.331 / 43.415 / 51.997 | 0.932 / 2.323 / 10.527 |
| sweep-250 | 250 / 250.000 / 250.000 | 142.862 | 0 | 49.96% | 30.64 | 41.354 / 45.572 / 70.975 | 1.439 / 7.937 / 30.103 |
| sweep-500 | 500 / 499.856 / 499.856 | 140.058 | 341 | 96.59% | 62.68 | 40.148 / 83.602 / 289.023 | 1,171.786 / 2,349.462 / 2,409.430 |
| sweep-750 | 750 / 749.978 / 749.978 | 70.760 | 8,650 | 96.97% | 98.79 | 79.469 / 154.419 / 387.472 | 28,670.241 / 59,930.691 / 60,558.785 |
| confirm-250 | 250 / 250.000 / 250.000 | 142.858 | 0 | 53.21% | 31.62 | 41.350 / 47.615 / 77.561 | 1.685 / 11.393 / 43.151 |
| bots-500 | 1 / 0.998 / 0.998 | 142.858 | 0 | 94.94% | 121.38 | 42.310 / 63.524 / 63.681 | 0.502 / 1.007 / 2.160 |
| confirm-100 | 100 / 100.000 / 100.000 | 142.857 | 0 | 21.96% | 15.60 | 41.328 / 43.424 / 52.961 | 0.991 / 2.428 / 11.997 |

| Run | Payload B / playing-client / s | NIC TX / RX B / playing-client / s | Generator cores / 16 | Busiest generator process, one core | Deaths / rejoins / refusals |
| --- | ---: | ---: | ---: | ---: | ---: |
| sweep-100 | 2073.74 | 9690.29 / 3554.47 | 0.202 | 2.67% | 0 / 0 / 0 |
| sweep-250 | 2134.33 | 9886.88 / 3604.36 | 0.446 | 5.89% | 0 / 0 / 0 |
| sweep-500 | 2108.79 | 9899.20 / 3616.34 | 0.886 | 11.66% | 62 / 61 / 0 |
| sweep-750 | 1132.71 | 4938.08 / 2086.25 | 1.154 | 15.23% | 9 / 9 / 0 |
| confirm-250 | 2299.03 | 10242.02 / 3678.18 | 0.454 | 6.07% | 0 / 0 / 0 |
| bots-500 | 2180.75 | loopback / in-process | 0.008 | 0.77% | 1 / 1 / 0 |
| confirm-100 | 2112.98 | 9775.58 / 3571.39 | 0.203 | 2.73% | 0 / 0 / 0 |

| Run | Deferrals at start / end | Relative world lag at start / end ms | Simulation busy-wall load | Machine busy / steal |
| --- | ---: | ---: | ---: | ---: |
| sweep-100 | 0 / 0 | 2.04 / 0.51 | 8.44% | 20.32% / 0.000% |
| sweep-250 | 0 / 0 | 11.84 / 8.10 | 27.08% | 50.79% / 0.000% |
| sweep-500 | 0 / 341 | 245.75 / 2597.05 | 98.11% | 99.38% / 0.000% |
| sweep-750 | 1359 / 10009 | 9794.34 / 70355.28 | 99.91% | 100.00% / 0.000% |
| confirm-250 | 0 / 0 | 8.32 / 7.25 | 31.79% | 58.24% / 0.000% |
| bots-500 | 0 / 0 | 4.08 / 3.61 | 51.10% | 6.12% / 0.000% |
| confirm-100 | 0 / 0 | 4.28 / 5.29 | 9.18% | 20.57% / 0.000% |

### profile-fp-500

5,841 retained samples; weighting: hardware cycles. Percentages use all sampled host threads.

| Exclusive category | Share of sampled CPU |
| --- | ---: |
| per-recipient delta encoding | 24.77% |
| framing/flush | 23.15% |
| visibility/food-sector work | 20.06% |
| socket service | 15.16% |
| physics | 11.83% |
| allocation | 2.59% |
| other/unknown | 2.44% |

| Self function | Share |
| --- | ---: |
| `__irqentry_text_start` | 12.76% |
| `<slither_sim::client::simulation::SimulatorBackend>::updates` | 10.49% |
| `slither_sim::client::simulation::food::food_updates` | 9.94% |
| `<slither_sim::simulator::Simulator>::advance::<false>` | 6.92% |
| `[unknown]` | 5.80% |
| `<slither_sim::client::simulation::SimulatorBackend>::broadcast_with` | 4.27% |
| `recv` | 2.70% |
| `hypot` | 2.47% |
| `<slither_sim::client::server::Gateway>::run` | 2.30% |
| `asm_sysvec_apic_timer_interrupt` | 2.06% |
| `entry_SYSCALL_64_after_hwframe` | 1.31% |
| `do_syscall_64` | 0.98% |

| Application inclusive (overlapping) function | Share |
| --- | ---: |
| `<slither_sim::client::server::Simulation>::run` | 58.94% |
| `<slither_sim::client::simulation::SimulatorBackend>::broadcast_with` | 45.43% |
| `<slither_sim::client::server::Gateway>::run` | 38.79% |
| `<slither_sim::client::simulation::SimulatorBackend>::updates` | 31.33% |
| `slither_sim::client::simulation::food::food_updates` | 14.42% |
| `<slither_sim::simulator::Simulator>::advance::<false>` | 11.47% |
| `<slither_sim::client::simulation::SimulatorBackend>::emit_presented` | 0.90% |
| `<slither_sim::food_grid::FoodCache>::sync` | 0.89% |
| `<slither_sim::client::simulation::SimulatorBackend>::minimap` | 0.85% |
| `<slither_sim::client::visibility::SectorInterest>::advance` | 0.82% |
| `slither_sim::client::encode::motion` | 0.61% |
| `slither_sim::client::encode::minimap_delta` | 0.60% |

### profile-fp-100

1,217 retained samples; weighting: hardware cycles. Percentages use all sampled host threads.

| Exclusive category | Share of sampled CPU |
| --- | ---: |
| socket service | 35.11% |
| framing/flush | 19.97% |
| per-recipient delta encoding | 15.37% |
| visibility/food-sector work | 11.64% |
| physics | 10.08% |
| other/unknown | 5.58% |
| allocation | 2.25% |

| Self function | Share |
| --- | ---: |
| `__irqentry_text_start` | 8.63% |
| `[unknown]` | 8.34% |
| `slither_sim::client::simulation::food::food_updates` | 8.28% |
| `recv` | 6.28% |
| `<slither_sim::simulator::Simulator>::advance::<false>` | 5.86% |
| `hypot` | 4.50% |
| `do_syscall_64` | 4.23% |
| `<slither_sim::client::simulation::SimulatorBackend>::updates` | 3.96% |
| `<slither_sim::client::simulation::SimulatorBackend>::broadcast_with` | 2.77% |
| `entry_SYSCALL_64_after_hwframe` | 2.26% |
| `asm_sysvec_apic_timer_interrupt` | 1.62% |
| `release_sock` | 1.61% |

| Application inclusive (overlapping) function | Share |
| --- | ---: |
| `<slither_sim::client::server::Gateway>::run` | 55.64% |
| `<slither_sim::client::server::Simulation>::run` | 41.55% |
| `<slither_sim::client::simulation::SimulatorBackend>::broadcast_with` | 27.87% |
| `<slither_sim::client::simulation::SimulatorBackend>::updates` | 15.93% |
| `<slither_sim::simulator::Simulator>::advance::<false>` | 9.70% |
| `slither_sim::client::simulation::food::food_updates` | 8.61% |
| `<slither_sim::client::server::Clock>::sleep_until` | 4.21% |
| `<slither_sim::food_grid::FoodCache>::sync` | 1.39% |
| `<slither_sim::client::simulation::SimulatorBackend>::minimap` | 1.05% |
| `<slither_sim::client::simulation::SimulatorBackend>::emit_presented` | 0.97% |
| `<slither_sim::client::visibility::SectorInterest>::advance` | 0.96% |
| `slither_sim::client::encode::minimap_delta` | 0.77% |

The bot row's host CPU aggregates simulation, socket and bot threads across
the 16-core machine. Its generator CPU is just the watcher. Its NIC columns
are management traffic: watcher traffic is loopback and bot traffic is
in-process. Those bandwidth values are not comparable to the remote sweep.

## Profiling interpretation and next work

Profiles are **separate diagnostic holds**, excluded from capacity thresholds.
Hardware cycles are available without changing `perf_event_paranoid`; primary
profiles use `perf record -e cycles -F 99 --call-graph fp`, frame pointers and
release line tables. Period-weighted cycle shares approximate CPU-time shares; they
are not instrumented elapsed phase timers. Inclusive function percentages
overlap and must not be added.

The initial DWARF profiles are diagnostic attempts, excluded from selected
shares: the 500-client export had 5,840 hardware event headers but 2,561
blank user-space call chains. Self instruction-pointer reports remained
available. `--merge-callchains` did not recover those stacks. No missing
samples were renormalized away. A separate late 250-client cycle attachment
also ended with SIGTERM near target teardown; its data/log are retained.
The corrected primary profiles at **500 and the long-confirmed 100** use
the separately pinned frame-pointer build and end before teardown.
The frame-pointer 500-client diagnostic ran at 129.742 ticks/s versus
140.058 in the unprofiled capacity hold: code generation and profiling
perturb execution, so profile proportions are descriptive rather than
unprofiled phase timers. Kernel interrupt-entry attribution is retained;
its large self share is not an identified game-function optimization.
Only samples inside each hold are admitted using a recorded perf-monotonic
to Unix clock bridge. No profiled row is substituted for an unprofiled SLO.

[`profile.py`](profile.py) owns the explicit, mutually exclusive category
rules. Allocation leaves take precedence, then socket service/framing, specific
visibility/food-sector work, physics/world-step calls, and remaining recipient
encoding. Unresolved/uncategorized stacks stay `other/unknown`. Inline frames,
unwinding and function boundaries limit that attribution; categories are not
an exact accounting of all callers' elapsed wall time. Full top self/inclusive
lists and sample weights remain selected JSON. Local SVG flamegraphs are
kept outside Git, with upstream renderer revision/digests pinned.

For the next optimization experiment, prioritize **recipient state/diff and
food-sector work** (44.83% combined exclusive share at 500). The self functions
above and retained inline stacks identify known-snake iteration and sector
diff work to isolate before choosing a change. Second, separate useful output
work from repeated receive/flush service: framing/flush plus socket service
are **38.31%**. At 100, those socket categories are **55.08%**, so their relative
importance changes with scale. Physics is **11.83%** at 500 and follows those
two priorities. Allocation's directly sampled leaf share is **2.59%**, not
the cost of all allocation callers. No projected gain, optimization or
networking rewrite is validated by these shares alone.

## Failed attempts, verification and limits

Verification: nine study-tool tests pass on Linux; seven pass with two
Linux-only skips on Windows. All seven repository documentation tests pass,
the Rust harness formatting check passes, and native release builds/smokes
are verified. No production Rust source changed. CI/review status belongs
to the PR rather than this frozen measurement record.

1. Local smoke: 50/50 playing, no decode errors, zero deferrals, at most four
   logical CPUs; builds used two jobs. A native 50-client end-to-end sampler
   smoke also passed before the admitted sweep.
2. First AWS attempt completed client/host-counter acquisition at 100, but
   `/proc` fd permissions changed while a thread exited and the external
   sampler failed before serialization. That attempt is **excluded** from
   capacity/profile claims; partial archives and the failure remain local.
   A Linux regression covers the exit race.
3. Immediate relaunch hit `MaxSpotInstanceCountExceeded` with no live
   instances. No instance launched. The quota was independently read as 32;
   a later bounded idempotent retry succeeded.
4. The admitted attempt's native smoke/100 run initially read group CPU via
   `/proc/TID/stat` for each thread. Those **thread** CPU values are discarded;
   process CPU, timing and NIC are valid. Before 250, the sampler changed to
   `/proc/PID/task/TID/stat`, with a focused regression. All bot thread CPU
   values use the corrected path. No host binary changed.
5. The failed 250 long hold, supplemental-hold decision, profile discovery
   symlink correction, incomplete DWARF exports, partial hardware attachment
   and controller continuation
   are retained in scratch notes/source pins. None was silently replaced by a
   passing result. The original plan remains unchanged.

This is one source/seed and one admitted instance pair, with ascending short
holds and explicit longer follow-up. Admission and OS scheduling are not
deterministic wire replays. No mature large-body, clustered, boosting,
slow-reader, public-Internet, TLS, many-hour, multi-AZ or multi-instance
generalization is established. No mechanics model was fitted, so held-out
mechanics validation and behavior fixtures are inapplicable. The experiment
locates useful capacity brackets and current CPU costs, not an exact maximum.

## Reproduce and access evidence

The scripts require a checkout, Rust 1.99, Python 3.12+, OpenSSH and an already
authenticated AWS CLI. They never upload cloud credentials; account metadata,
SSH keys, addresses and all raw output stay in a separately supplied scratch
directory. A new run refuses an existing output directory.

```sh
python3 prepare.py /path/to/new/build --ref origin/main --build
python3 smoke.py /path/to/new/build/target/release/examples /path/to/new/smoke
python3 -m unittest discover -s . -p 'test_*.py'
python3 aws_run.py /path/to/new/aws --repo /path/to/checkout --aws aws
python3 select_results.py /path/to/completed/aws .
python3 write_report.py selected-results.json /path/to/table-fragment.md
```

Run the first two commands from this study directory or supply their script
paths. `prepare.py` uses Git's repository root; `aws_run.py` takes it explicitly.
`run_trial.py` is the native worker, `analyze.py` the exact receipt/resource
reducer, and `host_capacity_load.rs` the adapted generator. Compile examples
only in the isolated source snapshot as `prepare.py` does. `--resume` is a
bounded recovery path for this attempt's recorded follow-up, not a new launch.

`selected-results.json`, `source-pins.json`, the frozen [plan](plan.json),
scripts and this report are the bounded Git selection. Raw timing CSV/JSON,
perf data/stacks, failed attempts, toolchains/binaries and flamegraph SVGs are
**local-only**, outside Git. Archive basenames and SHA-256 in source pins
identify those retained objects; they do not provide downloadable access or
prove representativeness. No raw recording, profile or official-server
capture is uploaded by this PR.

## AWS cost and cleanup

Estimated total **$0.50**, including the failed first compute attempt:
**$0.4033 Spot compute** from launch quotes and conservative instance lifetimes,
plus a **$0.10 allowance** for EBS, public IPv4 and roughly 0.502 GiB of returned
archives. This is an estimate, not a settled invoice. The no-instance quota
failure incurred no instance compute. See [aws-summary.json](aws-summary.json).

Both pairs used one-time Spot requests capped at $0.65/hour for the generator
and $0.06/hour for the server, 24 GiB gp3 roots deleted on termination, study
tags and verified `shutdown -P +150` timers with instance-initiated shutdown
set to terminate. Planned duration was 120 minutes plus 30 minutes safety.
The worst-case compute/storage/IP/transfer reservation stayed below the
authorized **$5** cap. Only SSH from the operator's current public /32 and
game TCP from the generator security group were admitted on the server.

After termination, explicit Spot cancellation, key deletion and security-group
deletion, a fresh account-wide regional inventory at **2026-10-26 02:11:23 UTC**
verified:

```json
{"nonterminatedInstances":0,"volumes":0,"addresses":0,"snapshots":0,"openOrActiveSpotRequests":0,"keyPairDeleted":true,"securityGroupsDeleted":true}
```

EC2's unfiltered Spot listing briefly lagged successful ID-filtered cancellation
confirmation; that stale view is retained, followed by the all-zero inventory.
No resource was left running while writing the report or handling PR review.

Credit-plan state was saved before and after every attempt with
`aws freetier get-account-plan-state --region us-east-1`. Credit reporting
lags, and the account balance change is not an isolated invoice for this study.

Agent assistance: GPT 6.1 Sol via Codex — capacity tooling/runs, profiling,
verification and PR writing. No independent agent review is claimed.
