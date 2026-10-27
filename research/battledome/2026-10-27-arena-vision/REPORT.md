# Whole-arena Battledome vision — 2026-10-27

Playtest candidate, not an official visibility rule. Players in Battledome know
all living snakes/prey and every arena food sector from any position. Host
bots keep A4's tuned local radius and food stencil. No PR or owner vision-playtest approval
yet. The chosen arena, pit, admission, food and scoring rules are unchanged.

## Decision and contracts

`Config.vision` is `Vision::Radius(f64)` or `Vision::Arena`; JSON accepts the
existing positive finite number or `"arena"`. Battledome defaults to arena;
other presets remain 1500, and explicit numbers override presets. Snake/prey
arena paths bypass spatial-range arithmetic. Food interest covers the live
wall's disc, including the wire-rounding strip at sector edges. One shared
sector set changes identity only when membership changes; radius intervals
avoid rebuilding it on every shrink tick. Reconnects get the current set.

The subsequent food split adds `Config.food_view` / JSON `foodView`, with
`"local"` (normal food stencil) and `"arena"` (all live-wall sectors), independent
of snake/prey coverage on the wire. Battledome defaults both to arena; other
presets keep local food. `vision: 1500, foodView: "arena"` supports nearby snakes
with all food. Host bot seats always keep local food. Geometric local food
retains its numeric vision circle, or 1500 when snake vision is arena; arena
food includes every physical pellet independently. The
[NTL study](../../fidelity/2026-10-27-ntl-vision/REPORT.md) motivates this option;
its small-scene controls do not qualify dynamic-1500 correction-tail parity.
The historical cost tables below predate the split and are not new timings for it.

Host `bots.vision` is a positive finite number, default 1500. Each bot uses the
smaller numeric world radius/cap, or that cap under arena vision, and retains
the normal local food stencil. Socket slots use world vision. Standalone
simulation and geometric sessions use world vision without the host cap.

Geometric JSON is **slither-sim-json-v6**, identifying the extended config and
Battledome observation coverage. The Rust Config API breaks. Observation/frame
radii stay finite and numeric: the live wall diameter for arena mode. Portable
**slither-client-v2**, its JSON v2 envelope, and **C ABI v3** keep their layouts,
units and nullability. Wire observations still report unknown vision (`null`).
Python dictionaries accept the new config value; `Frame.vision` remains float.
Checkpoint format **v2** and rules **v5** remain: there is no new physics tick
law or saved physics state. Optional config `foodView` is omitted for local;
older numeric checkpoints retain their exact shape and original coverage.
Arena-food checkpoints retain the explicit setting and reproduce observations.
Both unshipped view changes share geometric JSON **v6**; no second bump.

## Method and source identity

Followed the frozen [plan](PLAN.md). Offline loopback, seed 7, sequential runs,
release build, one logical CPU inherited by all children, jobs/Rayon/test
threads 1. Two host bot workers shared that same CPU. Base `20e5064` includes
`fa92c47`. [Source pins](source-pins.json) identify the measured release binary,
complete local source manifest, retained stock game and harnesses. Cost runs precede a final conservative `1e-8` sector-boundary
rounding guard: adding the wire origin can round an inside pellet into the
neighboring sector. The new regression fails without the guard. A complete
eight-decision Battledome-64 trace matches the measured release byte for byte
after it; final-build timing was not repeated. Other subsequent changes were
tests/comments, examples and documentation. Full logs, browser samples, scripts and failed outputs are local-only
under `outputs/arena-vision`; selected [rows](results.json) contain the tables.

Four bot-only runs lasted 75 wall seconds with 64/80 bots and zero sockets.
CPU is Windows process user+kernel time for all host threads, including startup.
Steady simulation load averages the last five 10-second stats intervals.

| Bots | Player world vision | Simulation load | Process CPU s / 75 s | Deferred ticks |
| ---: | --- | ---: | ---: | ---: |
| 64 | 1500 | 4.66% | 10.86 | 0 |
| 64 | arena | 4.76% | 10.88 | 0 |
| 80 | 1500 | 5.52% | 12.53 | 0 |
| 80 | arena | 5.78% | 13.84 | 0 |

The 80-bot arena arm used 1.31 more CPU seconds in this one pair; 64 was nearly
equal. Asynchronous scheduling varies; these short runs establish no speedup or zero-cost claim.

Four stock protocol-19 runs used **N−1 bots plus one real stock browser socket**,
10 s bot warmup and about 40 s of browser sampling. The separately retained,
unmodified stock game ran headlessly with a 20 Hz render scheduler to respect
the shared CPU budget. HTTP/WebSocket allowlists, a deny proxy and DNS mapping
prevented outside connections. Some local image requests were intentionally
blocked; HTTP teardown logged aborted requests. There were zero game page
exceptions. Per-recipient downlink is application payload, excluding WebSocket,
TCP and TLS overhead; steady sampling excludes the first five sampling seconds.

| Seats | Vision | Host process CPU | Simulation load | Payload kB/s | Join budget kB | Max food entries |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 64 | 1500 | 17.6% | 7.63% | 3.85 | 1.12 | 456 |
| 64 | arena | 15.7% | 14.47% | 13.51 | 16.14 | 2034 |
| 80 | 1500 | 18.8% | 7.60% | 2.32 | 1.12 | 250 |
| 80 | arena | 17.6% | 7.97% | 18.20 | 19.32 | 2280 |

Simulation load above averages the last three stats intervals. All processes,
including Chromium and its renderer, competed on one CPU. Socket cases recorded
151/211 deferred ticks at 64 (numeric/arena), and 169/95 at 80. Preemption
contributes to this wall-busy metric; these browser runs do not qualify one-core
host capacity. Process CPU covers the host alone over 53.6–54.8 s. Neither the
lower process CPU in the arena arms nor unequal deferred counts proves a policy
speedup. There is no repeated-sample or cross-machine claim.

Arena readers saw all 64/80 living snakes, distant snakes and distant food, with
maxima of 2034/2280 food entries (including decorative pit pellets). The game
kept processing and rendering; this is not normal-refresh-rate or NTL rendering
qualification. Synthetic caller tests separately verify distant physical food,
prey and snakes, growth, shrink/culls, cached subscription unions and reconnect.

Join budget sums payload plus the host's 64-byte/frame allowance from connection
start through 250 ms after confirmed play. It includes all initial bodies and
food plus early updates, a conservative burst observation rather than an actual
queue high-water mark. The largest arena-19 burst was **19,323 bytes (0.92% of
2 MiB)**. Large future snakes, slow readers and simultaneous human joins can
cost more; the existing per-recipient close-on-overflow policy remains.

Two additional wire-only protocol-13 runs used N−1 bots and one socket for 30 s
after 10 s warmup. Explicit bare framing plus `[99,4]`, the shared 24-byte reply
and selector 12 produced setup 13. Both had zero deaths or socket errors.

| Seats | Protocol | Payload kB/s | Join budget bytes | Budget share |
| ---: | ---: | ---: | ---: | ---: |
| 64 | 13 | 9.71 | 49321 | 2.35% |
| 80 | 13 | 11.27 | 59502 | 2.84% |

Bare wire omits two timing bytes, so the budget calculation adds 66 bytes/frame
back to its received size. The largest frame was 1603 bytes. This verifies
legacy login and delivery, not protocol-13 stock rendering or a relative
protocol bandwidth advantage: the steering/life histories differ from the
browser runs.

## Failed attempts retained

The initial 64-socket numeric hold completed but 63 connections were rejected
because the custom driver tried same-socket login after death. Reconnecting
fixed that harness error. Its replacement completed one numeric 64-socket hold
(64 joined, 58 alive at end, no unexpected closes); the arena driver then exited
before a complete 60 s record. That cohort provides no paired fan-out/capacity
result and is excluded above. Full numeric and partial candidate outputs remain.

Early two-page stock attempts timed out on the legacy challenge/framing adapter
or were stopped when the harness did not finish. A one-page 12 s pilot had zero
game exceptions but failed teardown because its WebSocket callback's frame list
had been removed too early. Final runs retain that list until close and always
stop their host. Only the final one-page protocol-19 runs qualify stock handling;
protocol 13 is wire-only. A separate raw legacy attempt failed to request bare
framing; it never joined. The final explicit `[1]` then `[99,4]` runs passed.

The offline package resolver requested an uncached Playwright wheel. No download
was made; the final harness reused installed Playwright 1.64.1 and retained
Chromium. Failed cohort manifests and their SHA-256 digests are in the source
pins; full files stay outside Git. No failed case was substituted into a pass.

## Verification and limits

The CONTRIBUTING Rust feature matrix passed, including default/core/bot/network
builds and tests, doctests, C ABI (2 unit + 9 integration tests) and Python crate
checks. After the final rounding guard, default and networking-only Clippy with
`-D warnings` and library suites passed again: 342/338 tests, each with one
existing ignore. The regression's negative control was 0 passed / 1 failed;
restored code passed. Numeric-stencil regressions explicitly keep numeric vision.
Historical v5 JSON fixtures retain their labels; callers check the v6 envelope
separately while comparing the frozen error/radius payloads.

Documentation checks (7), fixture-version checks (14) and transport-tool checks
(10) passed. The built abi3 Python wheel ran 36 tests: 35 passed and one optional
NumPy case skipped, including the new arena-config/finite-frame-radius case.
A fresh release built successfully.

Four complete canonical traces against archived main `20e5064` matched byte for
byte: portable and protocol replay (32 operations each), geometric observations
(32 operations), and stock official-500 (one decision). The retained arena trace
above checks the final rounding change separately. Hashes and byte counts are
in source pins. These finite fixtures establish their own output parity;
the geometric JSON envelope intentionally advances to v6.
These measurements do not establish 80-human capacity, normal-frame-rate client
performance, NTL zoom feel or large-body join limits.

## Eighty-socket fan-out follow-up

This follow-up measures real socket delivery rather than bot decoding or one
browser recipient. The [frozen follow-up plan](FANOUT-PLAN.md) uses source
`3c6fb18`, seed 7, zero bots and 80 protocol-19 WebSockets. The same instrumented
release executable runs four sequential cases: 1500, arena, arena, 1500.
The host's simulation, gateway and other threads share one logical CPU; two
40-client driver processes use one additional logical CPU each. Workload roles
pin themselves before binding/connecting; the light controller was pinned to
the host CPU after launch, early in the first case, and retained that affinity.
No outside servers or owner play processes are involved.

`tools/perf_transport.py` measures geometric JSON pipes, not sockets. This study
extends the actual PR socket workload, `examples/host_load.rs::socket_driver`,
retaining binary login, nonblocking draining, commands and reconnect-on-death.
The [source pins](fanout-source-pins.json) identify the measured binary, local-only
`overlay.patch`, `run.py` and `analyze.py`. Those full diagnostic sources and logs
remain under `outputs/arena-vision-fanout`, outside Git. They do not change the
delivered runtime. Per-run selected rows and full diagnostic file hashes are in
[fanout-results.json](fanout-results.json); warmup rows are omitted.

The deliberately synthetic stress fixture fixes the 80-seat wall at 5666.802,
pit at 2820.752 and spawn band at 4243.777–5375.872. It imports 16 admitted
snakes at 128 segments on separate inner circular routes, plus 64 floor-size
snakes on three outer routes. Admission still runs normally, but the fixture
overrides steering after command ingestion to keep recipients separated.
At full setup it resets all poses and seeds 6000 distinct quantized physical
pellets inside the pit. Refill is disabled; food can only be eaten. These are
stress controls, not natural play or a proposed bot/admission policy.

After 10 s warmup at full population, each run measures 120 s steady state,
then reconnects the clients holding slots 64–79 together and measures 60 s
recovery. The 16 large incumbents remain connected. Initial 80-client requests
also arrive together, but precede dense-food seeding; only the reconnect wave
tests joins into an already dense, large-body world.

Load uses the public host busy/wall counter, like `--stats`. Tick times cover
command/admission, physics/broadcast and handover. They are elapsed times,
including preemption; the mean load also includes buffered diagnostic counters.
Process CPU is user+kernel time for the whole host, excluding load clients.
Downlink percentiles pool complete one-second, one-client application-payload
bins, including zero bins; framing/TCP overhead is excluded. Percentiles use
nearest rank. Client-window mean-rate distributions are retained separately.

Actual per-recipient high-water counters cover both 2 MiB budgeted queues:
backend downlink and gateway output. They count payload plus 64 bytes/frame.
The unbounded delivery handoff is measured separately, including deliveries
being assembled before channel send. OS/TCP/tungstenite buffers are excluded.
The counters run before overflow checks; with no overflow, peaks are accepted
queue sizes. A first-250-ms join total is a burst observation, not queue size.

### Fan-out results

All four 120 s holds retained **80 setup/living sockets, 16 × 128-segment
snakes and 3712 total segments** on every sampled world state; per-tick minima
also stayed at 80 setup/living and 16 large snakes. Sampled physical food was
5060–5063 during steady state, then 5060 during reconnect recovery. All 384
requested lives were admitted (4 × (80 initial + 16 reconnect)); zero deaths,
refusals, censored joins, unexpected closures or deferred ticks. Each reconnect
window briefly had 64 living sockets and recovered all 80.

Each steady row has 9520 complete client-second bins; each recovery row has
4720. KiB = 1024 bytes. CPU seconds and load cover that row’s wall interval.

| Run / vision | Window | Sim load | Tick p99 / max ms | Host CPU s / % | Downlink p50 / p99 KiB/s | Deferred |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| numeric-1 / 1500.0 | 120 s steady | 3.19% | 0.89 / 29.39 | 12.70 / 10.59% | 0.92 / 3.18 | 0 |
| numeric-1 / 1500.0 | 60 s reconnect | 5.53% | 8.65 / 42.53 | 7.33 / 12.21% | 0.90 / 3.14 | 0 |
| arena-1 / arena | 120 s steady | 4.91% | 1.21 / 20.50 | 24.08 / 20.06% | 6.56 / 7.31 | 0 |
| arena-1 / arena | 60 s reconnect | 7.16% | 7.49 / 11.20 | 13.30 / 22.16% | 6.45 / 8.39 | 0 |
| arena-2 / arena | 120 s steady | 4.24% | 0.73 / 7.29 | 23.05 / 19.20% | 6.56 / 7.33 | 0 |
| arena-2 / arena | 60 s reconnect | 7.18% | 8.07 / 13.13 | 13.80 / 22.99% | 6.47 / 8.39 | 0 |
| numeric-2 / 1500.0 | 120 s steady | 3.64% | 1.02 / 10.40 | 12.95 / 10.79% | 0.91 / 3.17 | 0 |
| numeric-2 / 1500.0 | 60 s reconnect | 5.36% | 7.78 / 13.07 | 7.36 / 12.26% | 0.91 / 3.16 | 0 |

In this fixture arena vision cost about 8.4–9.5 additional host CPU percentage
points during the steady holds. Mean simulation load rose from 3.19–3.64% to
4.24–4.91%. Two descriptive repetitions and one seed do not establish a general
cost ratio or significance. Occasional ticks exceeded 7 ms, including a 42.53 ms
numeric reconnect tick; zero deferred ticks does not mean every tick met 7 ms.

The distribution across 80 individual steady-window mean rates was numeric
p50 0.67 / p99 2.77 KiB/s, versus arena p50/p99 both 6.54 KiB/s. This is
distinct from the pooled client-second percentiles above.

| Run | Full-run backend / gateway peak bytes | Share of 2 MiB (higher queue) | Unbudgeted handoff peak bytes/recipient | Reconnect gateway peak bytes |
| --- | ---: | ---: | ---: | ---: |
| numeric-1 | 15382 / 15483 | 0.74% | 15483 | 1469 |
| arena-1 | 64700 / 65375 | 3.12% | 65375 | 37984 |
| arena-2 | 64700 / 65375 | 3.12% | 65375 | 38709 |
| numeric-2 | 15380 / 15481 | 0.74% | 15481 | 1396 |

The largest budgeted peak was **65,375 bytes (3.12% of 2 MiB)** during the
initial dense-food publication. Steady queue maxima were 1569/1544 bytes for
numeric and 1170/858 for arena. Fast loopback readers are the only readers tested.

| Run | Join cohort | Requests spread ms | Max admission wait s | First-250-ms max payload / budget bytes | Sum of cohort budgets bytes |
| --- | --- | ---: | ---: | ---: | ---: |
| numeric-1 | 80 initial | 62.9 | 2.529 | 3640 / 6136 | 203032 |
| numeric-1 | 16 reconnect | 63.6 | 2.111 | 824 / 1515 | 21154 |
| arena-1 | 80 initial | 65.3 | 2.566 | 43577 / 46457 | 1486794 |
| arena-1 | 16 reconnect | 47.2 | 2.133 | 40522 / 43082 | 673717 |
| arena-2 | 80 initial | 72.3 | 2.562 | 43577 / 46457 | 1484761 |
| arena-2 | 16 reconnect | 46.0 | 2.145 | 41024 / 43904 | 677585 |
| numeric-2 | 80 initial | 63.4 | 2.531 | 3625 / 6057 | 201664 |
| numeric-2 | 16 reconnect | 55.3 | 2.111 | 797 / 1494 | 21204 |

All 16-client waves rejoined within 2.15 s of their respective requests. At
reconnect all 16 large incumbents and 5060 physical pellets remained present. The
largest reconnect frame was 37,551 bytes and the largest 250 ms budget was
43,904 bytes. The largest initial frame was 41,143 bytes. Reconnect client-second
maxima were 45,110/45,167 payload bytes/s for arena, versus 3896/3787 numeric;
the pooled p99 alone misses these rare join peaks.

### Follow-up qualification, reproduction and limits

Three pilots are retained in the source pins. Pilot 1 stopped on the first
food pickup because fixture replacement had bypassed presentation provenance.
Registering presentation/index state before stepping corrected the fixture.
Pilot 2 completed but selected only 15 reconnects due to a zero-based wire-ID
mistake. Pilot 3 verified 80 living sockets, 16 large bodies, dense food and
exactly 16 reconnects with short 8 s holds. Only the four frozen final runs
enter the tables; the same unchanged binary served both final arms.

With the retained harness, reproduce on Windows by extracting
`git archive 3c6fb18` to an outside-Git study directory’s `source`, applying
`overlay.patch` there, and keeping `run.py` and `analyze.py` beside `source`. Build the
`arena_fanout` example in release/default features with `--offline --locked`,
`CARGO_BUILD_JOBS=1`, `RAYON_NUM_THREADS=1` and `CARGO_TARGET_DIR` set to the
study directory’s `target`. Pin builds and the controller to one logical CPU.
The wrapper pins all host threads to that CPU and each 40-client shard to one
of two additional CPUs. Run `python run.py final`, then `python analyze.py`.
The retained harness uses only loopback port 19766. Frozen archive plus overlay
was reapplied and checked byte-for-byte against the measured source.
The release build, reused driver's bundled-life regression (1), documentation
check (7), and selected-row exposure/lifecycle/hash assertions passed. The
checked-in follow-up changes only research records, so production Rust checks
were not repeated.

This is synthetic protocol-19 delivery qualification for the stated bodies,
food and finite intervals. It does not establish natural 80-human capacity,
larger-than-128-segment stress, slow-reader headroom, sustained death/boost/refill
food churn, moving-boundary cost, legacy fan-out, rendering, WAN behavior or
NTL feel. Fixed routes avoid deaths and mass growth; seeded sizes satisfy the
large-body exposure instead. The instrumentation/controller overhead is
included and has not been subtracted; these are not unmodified-release timings.
The load driver splits frames and tracks its own life; it does not render or
fully decode every distant body/food entry. The earlier stock-client checks
remain the semantic/client-health evidence.
No production source, policy, contracts or configuration defaults changed.

## Split-view integration checks

The subsequent split rebases onto `0434834` (including ordered snake merging
#204, gateway readiness #205 and NTL evidence #206). The ascending/unique arena
candidate list is covered through its merge with known dead slots. Caller
regressions decode all food with 1500-range snakes/prey, and local food with
arena snakes; host bots retain local food under every tested snake cap.
Geometric batch/single observations, preset/config round-trips and checkpoint
continuations cover both food modes. Two historical marker viewport fixtures
now explicitly select local food instead of relying on numeric vision to do so.

The full CONTRIBUTING Rust group passed (all 15 commands); documentation 7/7,
fixture versions 14/14, and Python semantic tests 37/37 passed. Python used a
fresh cargo-built abi3 extension because maturin was unavailable offline;
wheel packaging was not tested. Four canonical traces against `0434834` match
byte for byte: portable/protocol/geometric at 32 operations and stock official
500 at one operation. The geometric JSON envelope intentionally remains v6;
these payload traces do not contain that envelope.

[Verification pins](split-verification.json) identify the distinct final
executables, source and trace digests. An initial shared-target comparison
reused one Cargo executable for both arms and is excluded; its manifest and
full outputs remain local-only alongside the final, separately built traces.
These checks add no timing, NTL rendering, natural-player capacity or playtest
qualification for the split setting.
