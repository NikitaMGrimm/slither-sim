# Index socket deliveries, 2026-10-27

Keep a slot-to-session vector refreshed in the existing compaction pass.
Delivery uses one lookup and retains the phase/generation guards, including
closes earlier in the same batch. No packet, queue-order or admission changes.

Measured `3423d76` → `e5403de`; runtime files match main after #205 (`af38bf3`)
and candidate `89df272`: src, Cargo manifests/lock, build.rs and examples match.
Delivery also rebases onto independent #207; measurements predate it and do not
quantify that combination. Gateway source remains identical.
[Selected pins, medians, ratios and traces](results.json).

WSL2 Linux x86-64, Rust 1.99 locked release/debug=1, thin LTO/one codegen unit.
Jobs/Rayon/tests one; at most two logical CPUs, host/fixture on one and load/checks
on the other. Shared-machine CPU/wall timing is contaminated. Kernel instructions
are unavailable at unchanged perf_event_paranoid=2; no security setting changed.

## Fixed world and gateway replay

One untallied pair then five alternating whole-process instructions:u pairs,
>=99% scheduled. Seed 7, official 500, 1,080 ticks, three native life rejoins,
zero refusals; min 499/end 500 alive. All 500 TCP sockets stay admitted for replay;
the native fixture's life resets reuse them. Payloads/FIFO are checked against
every actual WebSocket receive. Release times become zero only in this replay.
Counts include construction, expected-payload clones, client drain and teardown;
exclude kernel. Real-host holds cover actual handshakes, reconnects and scheduling.

| Protocol mix | Base / candidate median, billion instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 20.074 /19.292 | 3.899% (3.884–3.914%) |
| Half13 | 21.081 /20.296 | 3.721% (3.710–3.729%) |
| All13 | 22.093 /21.312 | 3.541% (3.533–3.550%) |

About 0.72M saved instructions per fixture tick, including setup. This is below
the earlier 1.07M live inclusive ceiling: queue work and index refresh remain,
and fixture traffic differs. The whole-fixture reduction is clear enough to keep.

## Real host

Five alternating pairs after one untallied pair per mix; 10 s warmup +12 s hold,
500 requested sockets, no bots, common steering/boosts. Sum main/sim/socket user
PMUs; the load runs separately. Same cheap transport counters on both arms.
Live worlds evolve differently; these are descriptive instruction comparisons.

| Mix | Socket M user instructions/tick, base → candidate | Whole-host paired reduction (range) |
| --- | ---: | ---: |
| Stock | 1.683 → 0.925 | 5.139% (3.531–7.436%) |
| Half13 | 1.837 → 1.060 | 4.692% (4.253–5.426%) |
| All13 | 2.022 → 1.185 | 6.122% (3.781–7.496%) |

Mean playing 498.984–499.521; final 497–500; zero deferred ticks.
Generator busy 53.8–64.2% on its separate core; no load errors or excluded pairs.
socketCpuSeconds=[user,system] medians per 12 s; kernel time is not instructions.
JSON arrays: instructions=[sim,socket,host], I/O/s=[deliveries,writes,frames,polls].
No native CPU-time, latency or capacity claim; native one-core holds remain the reference.

## Validation and rerun

Full final Linux Rust group, 45 focused server tests; expected-failure controls
for generation guard, compaction index and reversed frame order. Twelve measured
byte-identical traces: stock/half/all13 frames/timestamps/checkpoints/RNG, plus
nine historical portable/protocol/official/Battledome/retained cases. Replay
compares every socket payload/order too. Other platforms are CI-only locally.

In a fresh checkout at af38bf3, archive base.tar; restore only src/client/server.rs
from this PR, commit locally, archive candidate.tar. This reconstructs pre-#207.
Python 3.12+, Rust 1.99, patch and Linux user PMU access:
`python3 reproduce.py build ARCHIVES OUTPUT`, then modes `trace`, `gateway`,
`count --perf PERF`, `pairs` with the same paths. Reuses the guarded #204/#205
helpers frozen from the base archive. Use fresh output; -O and log overwrites
are refused. Fixed summaries are under trace/instructions; live per-hold
selected.json files provide arm medians and five paired candidate/base ratios.

Raw data, binaries and failed setup attempts are local-only outside Git.
