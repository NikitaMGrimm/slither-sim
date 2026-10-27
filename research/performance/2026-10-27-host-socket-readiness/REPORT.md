# Skip empty Linux socket reads, 2026-10-27

Keep one nonblocking poll snapshot per gateway pass. After a real transport
WouldBlock, wait for readability; parser-buffered messages, fairness yields,
handshakes, due output, flushes and close/login deadlines still progress.
Errors/hangups are readable; query failure services everything. Other platforms
keep their read path. Simulation/broadcast still run on their existing thread.

Measured runtime `46e00fa` → `5b46f0b`; delivery head adds this record and
main's research notes, with identical production source. [Selected results](results.json).
WSL2 Linux x86-64, Rust 1.99 locked release/debug=1, thin LTO/one codegen unit.
At most two logical CPUs: host on one, load/sampler on the other;
jobs/Rayon/tests one. Shared-machine timing is contaminated.
**Kernel instructions unavailable: perf_event_paranoid=2, unchanged.**

## Fixed work and full host

Primary fixed-work diagnostic: 500 idle TCP sockets, 4,000 actual gateway passes.
One untallied pair, five alternating whole-process instructions:u pairs,
>=99% scheduled. Median 1,166,409,199 → 443,644,495: **61.965% fewer**
(range 61.96487–61.96498%). Exactly 2,000,000 empty reads → 0; candidate 4,000
readiness queries. Startup/teardown included. No simulation, output or capacity
claim follows from this isolated read-path result.

Real loopback official 500: seed 7, no bots, shared open-loop steering/boosts,
10 s warmup +12 s hold, one untallied pair and five alternating pairs per protocol.
Attach user counters separately to main/simulation/socket; sum for whole host.
All >=99% scheduled. Read/write/frame counters use the same diagnostic overlay.
This evolving workload is descriptive; paired world trajectories are not exact.
Counters are included in the instruction totals; removing reads also removes
their diagnostic counter cost. These are instrumented comparisons.

| Official500 | Socket M user instructions/tick, base → candidate | Whole-host paired reduction (range) | Socket kernel CPU seconds, base → candidate |
| --- | ---: | ---: | ---: |
| Stock | 2.184 → 1.718 | 3.294% (2.239–4.299%) | 5.45 → 5.14 |
| Half13 | 2.258 → 1.846 | 2.948% (0.013–4.256%) | 5.42 → 5.11 |
| All13 | 2.417 → 1.977 | 3.854% (1.891–4.520%) | 5.54 → 5.18 |

Kernel columns are socket-thread **CPU seconds per 12 s hold**, from
/proc/PID/task/TID/stat, CLK_TCK=100. They are not instructions. Per-thread
user/system splits need not sum exactly to the process splits. Pair ranges and
populations remain in selected results; no native CPU-time/capacity gain claimed.
Stock kernel paired ratios span 0.921–1.017, including a worse pair. All timing
results remain descriptive. Mean playing populations span 498.927–499.484;
final counts 496–500, with normal reconnects. All measured holds have zero deferred
ticks; generators used 57–66% of their separate core. No 500-alive capacity gate.

## Joint ranking before implementation

1. Empty reads: baseline 145k–166k/s in the earlier 20 s census, receive-wrapper
   ceiling 0.52–0.60M user instructions/tick. The entire socket kernel budget was
   9.11–9.43 CPU seconds/20 s, including necessary writes; it is not removable.
2. Indexed Gateway::deliver: 1.06–1.07M user instructions/tick, 44–48% of socket
   user work. This is an inclusive lookup/queue ceiling; queue work remains.
3. Food pickup broad phase: 4.760M user instructions/tick, 28% of simulation work
   in the matched 500-alive fixture. Whole-phase ceiling; achievable saving unknown.

These scopes/denominators differ. The native one-core recheck remains the
[total-host reference](../2026-10-27-epyc-host-recheck/REPORT.md).
Write batching ranks lower: a 16 KiB write buffer already combines messages.
Earlier census all13 offered 126k frames/s through 54k writes/s; stock 56k/56k.
Own-child strace -c -f succeeded: 187,084 recvfrom, 108,140 sendto, 1,233 accept4,
including startup/teardown. Its 8 s hold averaged 455 alive, final 482: intrusive,
unqualified for 500, and unusable as an untraced kernel-cost comparison.

## Permanent protocol13 cost

Matched physical worlds: 853 steady all 500-alive ticks, same as the
[candidate-union study](https://github.com/NikitaMGrimm/slither-sim/blob/49e3c8e02f888f28e47de9f767d06ec8978e9416/research/performance/2026-10-27-host-snake-candidates/REPORT.md).
Extra sim user instructions per legacy recipient/tick: half13 **1,605**
(1,601–1,617); all13 **1,573** (1,571–1,584). All13 conversion 0.302M/tick,
framing excluding conversion 0.936M vs stock 0.470M; U rebuild/decode zero.
Conversion is 1.7% of diagnostic sim work, below the ranked candidates.

Real-socket baseline stock/half13/all13 medians: socket user instructions
2.184M/2.258M/2.417M per tick; user CPU 0.67/0.65/0.64 seconds per 12 s hold.
The extra socket user work is roughly 294/465 instructions per legacy recipient
per tick (half/all), with different live trajectories, so not a causal estimate.
Received frames/s are about 54k/87k/117k through 54k/53k/54k transport writes/s.

Per-client bytes per nominal world second, stock → all13; 5.971 world seconds,
short snakes. Derived unmasked WebSocket headers; no TCP/IP/TLS/handshakes:

| Family | Stock | All13 |
| --- | ---: | ---: |
| Anchors | 103.54 | 112.93 |
| Food | 201.15 | 220.62 |
| Appearances | 23.95 | 23.95 |
| Motion | 844.87 | 913.97 |
| Minimap | 472.79 | 472.79 |
| Elapsed / bundle prefixes | 165.86 /451.28 | 517.81 /0 |
| WebSocket headers | 168.00 | 519.49 |
| Game / total WebSocket bytes | 2482.22 /2650.22 | 2480.85 /3000.34 |
| Frames/s | 82.93 | 258.91 |

Prey 26.93, removals 2.30, other 189.55 unchanged. Legacy gives 3.12x frames and
13.21% more WebSocket bytes here; rates are not achieved wall throughput or a
general bandwidth estimate. M/V bytes are shared in both protocols.

## Validation and reproduction

Full Linux Rust group passes, 44 focused server tests. Removing the buffered-input
bypass fails the 33-message regression. Tests also cover fragmented/pipelined
upgrade budgets, timeouts, due output, FD input/drain/hangup/error and fallback.
Twelve full traces match byte for byte: stock/half/all13 raw frames with timestamps,
checkpoints/RNG endpoints, and nine historical portable/protocol/official/
Battledome/retained traces. Raw traces cover simulation, not readiness scheduling;
the socket regressions cover that invariant. Other platform execution is CI-only.

Python 3.12+, patch, Rust 1.99 and Linux user PMU access:
`python3 reproduce.py build REPO OUTPUT`, then `idle REPO OUTPUT --perf PERF`;
`python3 measure.py OUTPUT` repeats live pairs. Windows archives can be supplied
with `--archives DIRECTORY` (base.tar/candidate.tar, full SHA checked).
For all 12 traces use the pinned candidate-union reproducer linked above with
`--base 46e00fa --candidate 5b46f0b` and its build/trace modes.
Helpers build diagnostics only; no sysctl/security changes. Raw arrays, binaries,
stacks, failed harness attempts and intrusive captures remain local-only outside
Git. Initial TCP FIN test timing assumption was removed using UnixStream::pair;
real TCP integration coverage remains. Fresh portable-helper builds and five idle
pairs reproduce the rounded 61.965% reduction. These results do not qualify capacity.
