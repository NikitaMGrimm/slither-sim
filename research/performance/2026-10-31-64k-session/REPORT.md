# What the 64,000-bot session taught us

On 2026-10-31 a public SlitherSim session ran 64,000 bots on one c8a.2xlarge in
Ohio, from 17:00 to 18:40 UTC. This report records what we measured getting there and
during the session, and what we keep from it. It is not a 64k capacity claim for
the shipped host: the session ran a private prototype with approximate bot
collisions and few real players.

Raw data, checkpoints, traces and failed attempts are kept outside the
repository. Numbers below give their own window and conditions; results from
different setups are not additive.

## What ran

The final public build was `c91bd130` (binary SHA-256
`202bc2ad6904e4b2c40caa1ec1b0b152ca2771f32fe960c98ac7cc49ab389818`):

- 64,000 bots, radius 32,000, hard boundary, prey off, normal food (no cap).
- Bot heads tested for collisions every third tick (stride 3). Player heads were
  tested every tick.
- Bots spawned with the official position sampler and random headings, then
  steered to the center. A small zone in front of each player stayed free of
  newborn bots.
- An adaptive tick of 80 to 100 ms (see [maxTick](#adaptive-tick)).
- Fat LTO, native CPU targeting, profile-guided optimization (PGO) and
  `GLIBC_TUNABLES=glibc.malloc.hugetlb=1`. World and sockets on separate cores.

Offline timings use two checkpoints captured from the live world with no players
connected: L64-240 (after 240 s, 55,733 pellets) and L64-600 (after 600 s,
186,601 pellets, longest snake 235 segments; SHA-256
`df9966ac93a7130d865bb949570e8042a9b9e804d458dbddc09ae7db97c84b20`). Both run
at a fixed 40 ms tick. D64 is an earlier synthetic 64k world at 7 ms.

## Shortcuts that change the game

Testing bot heads only every Nth tick was the biggest single saving. World-step
time on L64-600, one c8a.xlarge core, three 512-tick runs per row:

| Stride N | p50 / p99 (ms) |
| ---: | ---: |
| 1 (exact) | 68.0 / 69.0 |
| 2 | 51.2 / 52.3 |
| 3 | 44.4 / 45.6 |
| 4 | 42.9 / 46.1 (separate later run) |

The cost is real: snakes can pass through each other. A test checks the straight
line between two tested head positions, not the curved path in between, and a
target can die before the deferred test. Over 64 ticks, 199 snakes that died at
N=1 survived at N=2, and 417 at N=3. The worlds diverge after the first changed
death, so these are outcome differences, not a measured delay. A missed contact
is not bounded to N−1 ticks.

Keeping exact collisions only near players ("LOD") was slower than plain stride,
because the protected bots sit in the dense center. With 50 player-like markers,
it measured 52.4 / 48.4 ms at N=3 / N=4 against 42.9 ms for plain N=4.

**Decision:** stride and LOD stay out of the product. Their code is kept outside
the repository.

## Exact speedups

These keep results identical. Each was measured on its own base, so the savings
don't add up.

| Change | Measurement | Status |
| --- | --- | --- |
| Expanded collision grid | D64, ten 512-tick pairs: p50 81.4 → 59.8 ms (−26%) | Merged [#368](https://github.com/NikitaMGrimm/slither-sim/pull/368) |
| Steering cache | D64, ten pairs: −1.7% | Merged [#369](https://github.com/NikitaMGrimm/slither-sim/pull/369) |
| Compact food index threshold | D64, ten pairs: −1.2% | Merged [#370](https://github.com/NikitaMGrimm/slither-sim/pull/370) |
| Keep collision join order | 64k, ten pairs: −1.85 ms (−2.3%), 9/10 faster | Merged [#376](https://github.com/NikitaMGrimm/slither-sim/pull/376) |
| Dense food lookup after depletion | L64-240: p50 227.4 → 55.6 ms, 10/10 faster; no change on L64-600 | Merged [#377](https://github.com/NikitaMGrimm/slither-sim/pull/377) |
| Cap grid reach for grown snakes | L64, ten pairs: 77.4 → 73.6 ms, 10/10 faster | [#378](https://github.com/NikitaMGrimm/slither-sim/pull/378) |
| Guarded distance test | L64-600, ten pairs: −3.55%, 9/10 faster | [#386](https://github.com/NikitaMGrimm/slither-sim/pull/386) |
| Reuse collision bounds for visibility | Two 15-minute runs: shared presentation setup 18.0 → 6.4 ms (prey also changed) | [#389](https://github.com/NikitaMGrimm/slither-sim/pull/389) |

The food fix matters most for a world that has been eaten down: in the slowest
captured L64-240 tick, food lookups alone took 214 ms.

Some ideas failed and stay recorded in the retained notes. Packed collision data
was faster on L64 but slower on small worlds. Several food-order and movement
variants lost or were inconclusive.

## Build and allocator

Separate replays of L64-600 on one c8a.xlarge core, three counted 512-tick runs
per build, world-step CPU only:

| Build | p50 / p99 (ms) |
| --- | ---: |
| Portable release (thin LTO) | 70.7 / 75.4 |
| Fat LTO, native CPU, panic=abort | 69.1 / 74.3 |
| Same, plus PGO | 63.0 / 68.9 |

Fat LTO + native + PGO was 10.9% faster than the portable release, with
byte-identical output. PGO was trained on D64, not on the measured world. On the
live host the gain looked smaller: an earlier 48k comparison of evolving worlds
measured about 1%.

`GLIBC_TUNABLES=glibc.malloc.hugetlb=1` cut the same replay's p50 from 75.8 to
70.6 ms (−6.9%), with the system's transparent hugepage setting left at
`madvise`. Setting it to `always` instead gave −2.2%. The build rows above
already include this setting, so the two percentages don't add up.

## Worlds get heavier with age

Short runs on a fresh world underestimated the cost. Food builds up and snakes
grow. At a fixed 60 ms tick, stride 3 with prey on averaged 82.6 ms of work over
the last five minutes of a 15-minute run, with 1,366 ticks deferred. The final
80 ms setting averaged 71.8 ms (p99 86.6) over the last five minutes of a
15-minute run with two clients, with no ticks deferred, which is 90% of the
budget.

Falling behind shows up as ping. A 48,000-bot world at 40 ms in Frankfurt fell
behind as it aged: ticks started up to 355 ms late, and the player saw about
350 ms of ping. A separate network sample measured 19 to 89 ms round trips,
so the backlog, not the network, most likely explains the ping.

## Adaptive tick

An adaptive tick runs each tick for the real time since the last one, between
a floor and a ceiling, so an overloaded world stretches its ticks instead of
falling behind.

- **40–100 ms:** in a 15-minute test with 50 clients, the world kept real
  time (game/wall 0.99999), but the owner found the pacing uneven.
- **80–100 ms:** used for the public session. With the floor at the planned
  tick, the tick stays even whenever the work fits.

The host option is [#384](https://github.com/NikitaMGrimm/slither-sim/pull/384)
(`maxTick`). Work beyond the ceiling still loses time; the host reports it as
`lost_seconds`.

## Two worlds on one machine

During the session, a second 45,000-bot world ran on two other cores of the
same eight-core VM. The main world's work per tick:

| Window | Work mean / p99 (ms) | Tick p99 (ms) |
| --- | ---: | ---: |
| Before the second world, 3 min | 66.8 / 80.3 | 80.3 |
| With it, 3 min | 86.3 / 103.4 | 100.0 |
| After stopping it, 3 min | 75.9 / 91.3 | 91.4 |

While both worlds ran, frames arrived about every 97 ms (median, measured from a
client outside AWS), against the planned 80 ms. After the second world stopped,
the median tick was back at 80.1 ms.

The world aged and player counts changed between these windows, so this is not a
clean measurement. The recovery after stopping it still points to shared cache
and memory, not just aging. The longer, uneven ticks also showed on screen:
snakes jiggled back and forth along their paths. Separate cores are not
isolation; run one large world per machine.

## Client compatibility

Vlither players could not join: the host closed every Vlither login with
"Invalid challenge reply". Vlither solves the login challenge with a built-in
formula that steps by 2 instead of the stock client's 4. Our client protocol
notes had listed Vlither as compatible from reading its source. A replay of
Vlither's exact login bytes found the failure. The fix,
[#383](https://github.com/NikitaMGrimm/slither-sim/pull/383), is merged but was
never deployed to the session. [#385](https://github.com/NikitaMGrimm/slither-sim/pull/385)
adds refusal counts to the host log, so this kind of failure is visible.

**Lesson:** replay a client's real login before listing it as supported.

## Per-player work

A design study for moving per-player work to other threads found that the
15–18 ms of "presentation" at 64k was almost all shared setup done once per
tick. The per-recipient loop took 0.6 ms with about two viewers. Reducing that
shared work (the bounds reuse above) comes first. Extra presentation threads
would only help hosts with many real players, and only as an opt-in that costs a
one-core host nothing.
