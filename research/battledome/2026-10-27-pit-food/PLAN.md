# Pit-food supply sanity check

Owner-directed local design check, not a performance benchmark or an equilibrium
study. Frozen before collection: main `5eb0736ee3650687adff5b5c431c33994bf75bb9`
and pit-food merge `010b2d48a2b8387745b232b1faee8b4984f78692`.

Use seed 7 and 300 world seconds at 32 and 64 bot seats, plus one empty reserved
socket seat. Run main-32, pit-food-32, pit-food-64, main-64 sequentially. One
two-second main-32 pilot checks instrumentation before the four selected runs;
retain its output separately. Do not replace or omit failures silently.

The same fixture drives host Simulation/Seat admission, physical-food delivery,
shared default A4, rejoins, zero added latency and 42 ms decisions over 7 ms
physics. It opens no socket. Authoritative food counters are diagnostics, never
policy inputs. Builds use one Cargo job; the runner and inherited children are
restricted to two available CPUs, with one test/Rayon thread.

Record exact per-tick food-count minima/maxima and one-second samples of food
inside/outside the live pit, nutrition, target, wall/pit geometry, living seats,
scores, credited kills and deaths with food. Report startup, peak, endpoint,
last-minute count range and late mean alongside main. A finite window and one
seed can show an observed bounded inventory, not prove long-run boundedness.
If inventory grows concerningly, report it and propose a minimal follow-up;
do not change the approved mechanics to improve these results.

The fixture is injected only into archived source copies, adding one read-only
world accessor and an ignored test. The runner records source commits, fixture,
instrumented Rust sources and binary digests. Full builds/logs remain local;
selected numeric results and reproduction sources belong beside this plan.
