# Owned protocol-13 frames, 2026-10-27

Keep: consume converted legacy packets as frames. Reuse buffers with header
room; otherwise allocate the final size once, avoiding reallocate+shift copies.
Stock bundles retain their encoder. No persistent conversion cache or changes
to report gates, frame boundaries, elapsed allocation, or physical latency draws.
Measured runtime: `587b597` -> `b104038` (before #212). Delivery integrates
`32d4cbc` with identical framing files; four raw traces and the full Rust group
also pass at `47485d2`. Performance counts remain pinned to the measured arms.
[Selected pins, paired summaries and thirteen trace hashes](results.json).

## Profile

WSL2 Linux x86-64, Rust1.99 locked release/debug=1, thin LTO/one codegen unit.
Jobs/Rayon/tests one; <=2 pinned CPUs. Offline official-500, no bots. Diagnostic
user-instruction brackets include overhead; same 853 steady ticks: tick>=120,
all 500 alive, reconnect720 excluded, >=99% PMU scheduled. Diagnostic/control
trace bytes match. Mean million instructions/tick; nested U work excluded here:

| Emission phase | Stock | Half13 | All13 |
| --- | ---: | ---: | ---: |
| Conversion | 0 | 0.167 | 0.315 |
| Group preparation | 0.084 | 0.071 | 0.053 |
| Physical arrival scheduling | 0.020 | 0.087 | 0.154 |
| Game framing and enqueue | 0.371 | 0.460 | 0.545 |
| Other, including destruction | 0.073 | 0.156 | 0.236 |
| Total emission | 0.548 | 0.941 | 1.304 |

Fine probes put anchor/food conversion at 0.062M/0.046M, including their own
probe overhead. Sharing them still needs recipient front/spacing/history gates
and output ownership. Framing removes more repeated allocation work without
cache state. Socket WebSocket headers are outside these sim scopes; the socket
already transfers owned Bytes to tungstenite. Frames and bandwidth stay equal.

Default [99,4] performs zero U rebuilds. All-[99,3] is secondary: U work averages
1.191M/tick; three map-due ticks rebuild 500 maps each for roughly 333–342M instructions,
with a 356M worst diagnostic tick. This change does not target that optional U burst.

## Matched evidence

One untallied warmup pair then five alternating whole-process instructions:u
pairs, >=99% scheduled, no exclusions. Includes setup,1080 ticks and client
drains; excludes sockets/kernel. Changes below are candidate/base:

| Official500 | Base/candidate billion | Instruction change (range) |
| --- | ---: | ---: |
| Stock | 13.844/13.858 | +0.105% (+0.086–+0.128%) |
| Half13 | 14.371/14.206 | -1.157% (-1.219–-1.128%) |
| All13 | 14.892/14.552 | -2.284% (-2.310–-2.282%) |
| All [99,3] | 15.911/15.571 | -2.147% (-2.159–-2.120%) |

Five alternating steady-tick triplets after warmup, same 853-tick gate, exclude
construction/drains. Median within-triplet extra sim instructions per legacy recipient/tick:
half13 1,562->958, all13 1,540->938, about 39% less. Ranges are in results.json.
The small stock instruction increase is retained as a tradeoff for majority13.

Live whole-host user-instruction pairs use the same plan, 10s warmup/12s hold,
host on one CPU and load/sampler on another. Stock changes +0.17% (range -1.49
to +3.03%); half13 falls 0.82% (0.59–3.86%); all13 falls 2.89% (1.73–4.68%).
Zero load errors, PMU>=99%; selected socket instructions, frames/write attempts
and living populations are recorded. Evolving worlds make these descriptive.
Write attempts are instrumentation events, not independently counted syscalls.

Thirteen complete traces match every frame byte/boundary, release time,
checkpoint and RNG endpoint: stock/half13/all13/full-map plus nine historical
cases. Raw fixtures start/end 500 alive, minimum 499, three rejoins, no refusals.
Full Rust group passes. Tests cover printable/prefixed/empty/jumbo packets,
nonuniform overlapping copies, capacity reuse, elapsed saturation, queue
preflight and physical RNG/link progression. Shifting by one byte fails the
oracle; restored tests pass. Stock/legacy socket framing tests also pass.

## Reproduce and limits

Export base.tar/candidate.tar at results.json's full pins. Linux user PMU,
Python3.12+, Rust1.99 and patch required. Run `reproduce.py MODE ARCHIVES OUT`
serially: build, trace, count (--perf PERF), sockets, pairs. Then `tick-cost.py
OUT`; `profile.py ARCHIVE PROFILE_OUT --control BINARY --fixture OUT/harness/protocol_fixture.rs`
rebuilds diagnostic hooks over the frozen #210 profiler; --fine adds inner
probes. Separate arm targets/copied binaries. Python-O is refused.

Raw arrays, traces, earlier build-only candidates and logs remain local-only.
Phase, steady-tick, whole-fixture and evolving socket denominators differ.
Shared-machine timing is contaminated; kernel instructions unavailable with
perf_event_paranoid unchanged at 2. No CPU-time, latency or capacity claim.
No physics/pickup or broadcast/snakes changes; no arena-vision conflict expected.
