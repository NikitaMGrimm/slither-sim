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
Checkpoint format **v2** and rules **v5** remain: there is no new saved field or
physics tick law. Numeric saved configs restore with their original coverage;
arena checkpoints retain the string and reproduce their observations.

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
