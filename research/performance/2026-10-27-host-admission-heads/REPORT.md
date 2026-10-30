# Earlier spawn-head rejection, 2026-10-27

Keep. Main `368abe4` (#196) → measured runtime `dcb7fcb`.
Delivery integrates main `3a530e0` (#197). Counts and socket holds predate it;
four additional stock/legacy integration traces match the same main hashes.
The final branch also includes release-only #198, with no further source change.
Check the existing head-runway predicate before full-body collision tests.
Accepted poses, RNG draws/order, 4096/256 budgets, strict preference, fallback
scores/ties and errors stay unchanged. Body construction remains; scoring still
requires a fresh successful body check. No persistent state or new deadline.

## Matched instructions and parity

WSL2 Linux x86-64, Rust 1.99 locked release/debug=1; build/Rayon/tests=1.
One warmup then five alternating whole-process `instructions:u` pairs,
counter scheduling >=99%, binaries rehashed. Seed 7, 60 world seconds at 64/80
remote slots: hosted queue/protection/retry policy, decoded stock frames driving
frozen A4 every 6 ticks. Includes setup, clients, physics, broadcast and draining;
excludes sockets/kernel. These are synthetic fixtures, not a capacity test.

Arena rows merge unapproved vision `b5ddee5` into both arms for measurement only;
all remote slots receive arena coverage. The production change excludes it.
[Selected results](results.json) pin the two trees, binaries and trace hashes.

| Vision | Seats | Median instructions, B: base → candidate | Paired reduction (range) |
| --- | ---: | ---: | ---: |
| Main | 64 | 95.611 → 69.561 | 27.241% (27.234–27.250%) |
| Main | 80 | 111.832 → 81.462 | 27.160% (27.145–27.167%) |
| Arena branch | 64 | 125.994 → 99.958 | 20.665% (20.665–20.665%) |
| Arena branch | 80 | 173.394 → 143.019 | 17.518% (17.518–17.518%) |

Six stock traces per vision (seeds 7/29/47), plus two modern protocol-13 traces
per vision (seed 7), match complete decompressed bytes: birth poses/RNG,
frames/times, deaths, initial/final checkpoints and terminal summaries.
Legacy output uses #196's M/V; an equal stock-world twin supplies A4 because
the public bot decoder supports only 19. Twins assert equal snakes every tick
and final checkpoints/spawn RNG. No legacy performance claim.

Main: 631 births, 616 complete 3s followups, 15 late alive censored, zero early
deaths. Arena: 634/627/7/0. Every seat admits and all finish alive.
Identical first-wave wait medians: 2.065–2.079s at 64, 2.093–2.163s at 80.
Cold-start p99/max still reaches 9.205/10.017s; roughly 2s describes the median,
not all waits. Rejoins: max 0/0.007s on main, zero on arena. World-time quantiles.

## Admission diagnostics

64/80 stock loopback sockets, no bots; host on one logical CPU, load on one
other CPU, 15s warmup/20s hold. Simulation-thread user instructions include
instrumentation. Different trajectories and partial populations: mean playing
39.4–54.4, waiting 19.9–27.4; load-generator busy <8%. These holds describe
causes and tails; they are not matched speed estimates.

Full-budget search means, millions of instructions: main 64: 79.43→34.09,
main 80: 85.92→36.39; arena 64: 76.69→35.45, arena 80: 84.13→36.19.
Before: strict body checks 67–71%, body construction 21–23%, fallback body
about 5%, query construction <0.4%. Strict body calls per full-budget search
fall from 4096 to 48/57 on main, 66/50 on arena; fallback from 256 to 137–147.
Body construction still costs 17.88M; head tests/loop work remain substantial.

Whole-tick census, millions of instructions, base→candidate:

| Vision/seats | n | p50 | p99 | p99.9 | Max | Ticks >2×median, % |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Main64 | 2458→2858 | 1.339→1.375 | 90.848→37.337 | 92.991→42.174 | 98.279→44.954 | 28.15→32.75 |
| Main80 | 1900→2858 | 1.849→1.765 | 97.153→41.343 | 102.631→47.661 | 103.228→49.842 | 35.53→39.19 |
| Arena64 | 2465→2820 | 2.795→2.175 | 110.102→92.344 | 207.502→159.917 | 214.286→162.544 | 38.66→27.41 |
| Arena80 | 2325→2857 | 3.721→3.696 | 138.510→159.752 | 265.365→222.383 | 273.115→226.411 | 38.92→41.23 |

Admission dominates main's heavy ticks. Arena vision also has large recipient
food tails: the 214.286M tick spends 102.522M there and 74.341M in admission,
with 53,091 single-pellet F encodings and no physical-food births. Arena 80's
p99 rises despite its lower max; different live states preclude a paired tail
claim. The shrinking-pit food retain scans cost at most 0.041M, so they are not
the useful target. Arena code is untouched.

## Validation and reproduction

The retained [legacy overlay](legacy_overlay.py) drains protocol-13 output
while driving an equal stock-world twin and checking its physical state.

Full Rust group and docs pass on Linux, repeated after #197 integration,
including seven safety tests; changing
the inclusive head threshold to strict fails the new ULP boundary test for both
tiers. Windows/macOS await CI. Preparatory connection-setup and unsupported
legacy-decode attempts are retained locally; no counted pair was excluded.
Shared-machine timing is contaminated: no CPU-time, latency or capacity claim.

From a full Linux checkout, run [reproduce.py](reproduce.py) modes `build`,
`trace`, `count` with REPO and NEW_OUTPUT; count takes `--perf PERF`.
For arena add `--vision b5ddee51c45d5e850a8ccff3268a1c4a6edeae39`.
For legacy use separate build/trace output with `--legacy --seeds 7`.
Raw traces, counters, diagnostic overlays and failed attempts remain local.
