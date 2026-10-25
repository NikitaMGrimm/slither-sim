# Tail/turn jitter: local host versus official

2026-10-25. **Exploratory, one shared Windows desktop.** This adapts the local
study pinned below and adds owner-reported host-load verification. Original
captures and experimental variants remain local-only. The documentation change
did not rerun captures, jitter analysis or the load measurement. Different bot
counts, routes and experimental builds prevent an isolated performance claim.

## Question and metric

Why did the stock client's tail/head correction look larger on the local host?
On each new anchor the client resets `chl` and smooths `dchl = 1 - chl_at_arrival`
and the head offset over 53 frames. `tailjitter.py` estimates the signed correction
in world units as `v_local × anchor_gap - 42`, using the surrounding ten anchors
(nine gaps) to estimate local speed. It skips windows whose reported speed varies
by more than 15%, splits on appearance/removal, and reports quantiles of the
absolute correction for receipt and server-header clocks. Perfectly timed
constant-speed anchors would give zero. Overlapping windows are correlated;
this metric is not a direct measurement of rendered jitter or user perception.

`delayspread.py` discards the first 50 frames per connection and measures
`receipt_time - server_time` above the minimum in a neighboring window of up to
1,200 frames. This excess delay includes transport, proxy and host scheduling;
it is not absolute one-way latency. Its sliding baseline differs from
`host_load`'s disjoint 1,200-frame chunks, so their quantiles are not identical
estimands.

## Captures and findings

Official comparisons use retained headless capture groups `fidelity-motion-clock`,
`fidelity-paired-coil`, `run5-long-circles` and `visibility-crossing`. The original
report calls these four captures but does not pin whether paired `bot1` entered
the pooled range; both retained paired streams are identified below. The figures
are preserved as reported, without inventing a more precise selection.
Local captures used the same headless client through `keyrelay.py` → port-8798
viewer proxy → host, the browser route minus its cookie relay; `hop-direct`
also tested a direct host hop.

| Capture / condition | Frames/s | Absolute correction p50 (units) | p95 (units) | Excess delay p50 / p90 (ms) |
| --- | ---: | ---: | ---: | --- |
| Official, four capture groups | 58–94 | 0.66–0.78 | 2.4–2.7 | 1.8 / 3.5 |
| Local before, `ours-b6` | 27.6 | 3.59 | 6.75 | 11.8 / 18.4 (`hop-direct`) |
| Local per-tick remote anchors, `anchor-only-150` | 45.8 | 1.53 | 3.47 | 8.9 / 16.5 |
| Local per-tick remote anchors, 120 bots, `hop-direct-120` | 45.4 | 0.97 | 2.36 | 1.3 / 7.3 |

The before row combines correction and direct-hop delay from separate local
captures, as the original report did. It is not a single matched trial.

## Causes and rejected attempt

1. **Batching (supported inference).** The old host held anchors for a six-tick
   42 ms batch. Retained official traffic supports prompt anchors and frames at
   nominal 7 ms opportunities, with motion reporting separately near 42 ms.
   Server-clock correction was 3.36 units locally versus 0.63 officially; sending
   remote anchors per tick reduced the local value to 0.42. The
   [scheduler study](../2026-10-25-packet-scheduler/REPORT.md) retains counterexamples
   and explains why actual official creation ticks remain unobserved.
2. **Host stalls (local profiling).** The single host thread built and decoded
   all 480 background seekers' broadcasts and decisions in the same tick cycle,
   taking about 20 ms. Remote frame lateness depended on tick phase: phase 7
   about 1.2 ms, phase 21 about 20.6 ms. The spread disappeared with 120 bots.
   The original diagnostic ruled out the configured ordered ±1 ms latency model
   as the main cause; its sleep probe measured about 1.5 ms for `sleep(1 ms)`.

Staggering background broadcasts/decisions by slot was tried and reverted.
Median delay fell to 3.5 ms, but per-tick sector rebuilding scanned about 51,000
pellets and sorted listings (about 2.4 ms/tick). Load reached about 88%, and the
host fell roughly one second behind as the world matured. The saved
`simulation.rs.staggered-final` retains this attempt; the original report noted
`STAGGER = false` in its then-live copy. That is historical state, not a claim
about the current repository.

The staggered profile per 30 seconds was sector sync 10.7 s (food loop 6.3,
sort/compare 3.7), seeker decisions 7.2 s, updates 3.1 s, physics 1.9 s and frame
decoding 1.5 s. The interim remote-anchor-only variant with 480 bots recorded no
deferrals over 150 s. The source report names backup
`battledome-live-v2-20261025/anchor-cadence-before-20261025`; its absolute location
and digest were not supplied, so that backup is **unavailable/unverified here**.

## Decision and after evidence

Adopt per-tick emission, separate simulation/socket/bot threads, and independent
packet-family release rules. Replace repeated full food scans with an incremental
food index and reduce per-connection snake checks with a coverage grid. These
performance mechanisms preserve the chosen wire behavior; the scheduler models
remain partial. See [hosting](../../../docs/raw-server.md#embedding-and-load-checks).

The owner ran the release build on this same shared Windows desktop:

```sh
cargo run --release --locked --features networking --example host_load -- --bots 500 --seconds 60
```

| Stage | Simulation-thread load | Deferred ticks in 60 s | Other reported result |
| --- | ---: | ---: | --- |
| Threading only (`50e7edd`, local `c97b89e`) | ≈100% | 2,518 | ≈100 ticks/s instead of ≈143 |
| Family cadences + food index/coverage grid (`ccc4e65`, local `20dd5a4`) | 43% | 0 | Watcher 69 frames/s |

After watcher excess delay: **p50 0.32 ms, p90 0.52 ms, p99 0.73 ms,
maximum 21.59 ms**. This measures receipt-minus-server-time spread above each
1,200-frame chunk's minimum, with the example's fixed 40 ms RTT and no jitter.
Frame frequency depends on nonempty traffic and is not physics tick frequency.
`simulation_seconds()` measures simulation-thread busy wall time, not total
process CPU across bot/socket threads.

The pre-cadence profile per approximately 700 ticks attributed 3.1 s to sector
listing rebuilds, 1.1 s to presentation reconciliation, 1.7 s to per-connection
snake checks and 0.4 s to physics. These timings and the before/after values
were supplied by the parent implementation task; no raw load log, binary digest,
compiler version or isolated repeated-trial series was supplied to this record.
The commits identify source stages, not an independently pinned executable.
The combined change prevents deferrals in the reported run; it does not isolate
each optimization's effect or establish a portable capacity guarantee.
No post-change tail-correction capture was supplied: watcher excess delay is
after evidence for host scheduling, not proof of official-equivalent rendering.

## Reproduction and local-only evidence

The original report, scripts, config, captures and reverted source below are
**local-only**, under `C:\Users\Nikita\.t3\scratch\jitter-study-20261025`.
The scripts use standard-library Python plus their historical local protocol
decoder import; consult the pinned script before running with an archive path.
No complete evidence-bundle-v1 package or frozen confirmatory plan is claimed.
Raw recordings and official assets are not redistributed; permission to
redistribute those external materials is not established here. Hashes identify
bytes and do not validate scientific claims.

| Local-only path below root | SHA-256 |
| --- | --- |
| `REPORT.md` | `37fbc409123863d91f4bcf1d51620602b27c6cb88eecdb6fe699a6336d5d08a7` |
| `tailjitter.py` | `f3a49aa0104faa9903f3ae1e8d2b2c60eb6234926af454936ab5d2574f4974fa` |
| `delayspread.py` | `bad9d9838ea772d3fedc4d2cc7b5e8846e20c90dfb3923f9bbfba884a166f6b7` |
| `keyrelay.py` | `e69af0bc5b8ec9f0bdcdf16b4c1c54f72da0547104734244c9dd7c76648cdfc2` |
| `runtime-config.480.json` | `48d26b856d71de704ba1cb717ba55e4b85cd01153f69d64ba595debde0075e46` |
| `simulation.rs.staggered-final` | `65a7239570f95510eda71b0db4c28b01b4681d71ef953a39a42a2d454d326e75` |
| `ours-b6/bot0.jsonl.gz` | `571120a010e871483d018d546c7811274ced8d4de3735b4bb3e5833e4d87284c` |
| `hop-direct/bot0.jsonl.gz` | `0ee5994bc3e45630df2279f190cc9a202981df88953d4d807e98d98e2098d58e` |
| `anchor-only-150/bot0.jsonl.gz` | `ab145f8404aacf0d7e867c57b78d30ec87159f232f58e957d0345ebb8f33af38` |
| `hop-direct-120/bot0.jsonl.gz` | `26245c7255f5837dca281956a4eeef959724f529d06f7ad6321cc89d5052293a` |
| `stagger-direct/bot0.jsonl.gz` | `c16c37e85343146bf4b0afcde50015d65940b01460d798d00db764b375a10b99` |
| `stagger-proxy/bot0.jsonl.gz` | `2dfd69a7a9cbe8e7b770501cbcc4b17be5792a5c8eaba19ae54e6338979530d6` |

Official source root (**local-only**): `C:\Users\Nikita\Source\slither-arena-rs\recordings\headless`.

| Local-only path below root | SHA-256 |
| --- | --- |
| `fidelity-motion-clock-20260927/bot0.jsonl.gz` | `a9b2477cb29c4534d973818df32730227e183817fb2fe6c32f616482a90d4698` |
| `fidelity-paired-coil-20260927/bot0.jsonl.gz` | `559225fcb6316c565ff5642fb038aa28e8b5bd1c0a5d019471a3c90e8862c719` |
| `fidelity-paired-coil-20260927/bot1.jsonl.gz` | `849beb2f3e9c7c899e834d8252d72027b349d214b6ee68615074411d3ec1ff8d` |
| `run5-long-circles-20260927/bot0.jsonl.gz` | `705571c23aee9f67212afc88e7b9b4469a017a005ca4a9adf2cdd60e5b794cb3` |
| `visibility-crossing-20260928/bot0.jsonl.gz` | `ace9432fa3397481604d1a57a99bf7402f9997429d5efcb60608a5c0e810b571` |

Historical decoder import used by the jitter scripts: `C:\Users\Nikita\.t3\worktrees\slither-sim\perf-workload-matrix\tools\fidelity\slither_protocol.py`. The jitter report did not preserve its run-time hash. The scheduler study's [source pins](../2026-10-25-packet-scheduler/source-pins.json) identify a later scratch decoder and client sources; they do not establish the earlier import's identity. Exact historical decoder/build reproduction remains incomplete.
