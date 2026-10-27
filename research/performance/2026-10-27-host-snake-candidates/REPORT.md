# Merge recipient snake candidates, 2026-10-27

Keep: merge two ordered slot streams into one broadcast-owned scratch vector,
instead of collecting, sorting and allocating for every recipient. Preserve own
snake first, ascending peers, duplicate removal and the copied membership used
while recipient history mutates. No persistent cache or release-gate changes.

Measured runtime: `ca7ac2f` -> `7f766d7`; delivery head also contains this record.
[Selected pins, ratios and traces](results.json). Synthetic offline fixtures;
WSL2 Linux x86-64, Rust 1.99 locked release/debug=1, thin LTO, one codegen unit.
Jobs/Rayon/tests one, at most two logical CPUs; counted child pinned to one.
Shared-machine live timing is contaminated. No CPU-time or capacity claim.

## Instruction pairs and parity

One untallied warmup pair, five alternating whole-process `instructions:u`
pairs, counters >=99% scheduled. Includes construction, all 1,080 ticks and
client drains; excludes socket/kernel work. No pair excluded.

| Official 500 | Base / candidate median, billion instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 18.979 / 18.376 | 3.175% (3.172–3.180%) |
| Half protocol 13 | 19.513 / 18.912 | 3.085% (3.063–3.092%) |
| All protocol 13 | 20.042 / 19.436 | 3.025% (3.009–3.039%) |

All three complete raw traces are byte-identical, including packet/frame order,
delivery timestamps, checkpoints and spawn/latency RNG endpoints. Their physical
worlds also match across protocols. Each starts/ends with 500 alive, minimum 499,
three rejoins, zero refusals and 1,500 maps. Nine historical full traces also
match: portable/protocol, official500, Battledome40/64/80 and retained clients.

Linux Rust check group and 16 catalog example tests per arm pass. The candidate
oracle exhausts 65,536 membership combinations and checks allocation reuse.
Changing `<` to `<=` in the merge fails it by duplicating members. Raw traces
exercise simulation delivery; actual socket scheduling is outside this change.

## Current-main profile and permanent protocol 13

Diagnostic user-instruction brackets include overhead. Same 853 steady ticks:
tick >=120, all 500 alive, forced reconnect tick720 excluded. Millions/tick,
stock / half13 / all13:

| Phase | Stock | Half13 | All13 |
| --- | ---: | ---: | ---: |
| Physics | 7.240 | 7.240 | 7.240 |
| Shared coverage | 1.376 | 1.374 | 1.386 |
| Candidate union | 0.815 | 0.816 | 0.816 |
| Snake updates | 1.984 | 1.966 | 1.959 |
| Shared / recipient food | 0.857 / 1.598 | 0.857 / 1.595 | 0.857 / 1.593 |
| Recipient prey | 0.923 | 0.922 | 0.922 |
| Framing / legacy conversion | 0.470 / 0 | 0.709 / 0.160 | 0.936 / 0.302 |
| Reports / HUD, maps, boards | 0.220 / 0.046 | 0.220 / 0.046 | 0.220 / 0.046 |
| Other | 1.399 | 1.429 | 1.457 |
| Total | 16.928 | 17.335 | 17.734 |

Pickup alone is 4.760M within physics (whole-phase ceiling, not a predicted win).
The union's 0.815M ceiling motivated this change; delta caches already share peer
bytes for equal history, and food listings/maps are shared. Legacy conversion
is only 1.7% here; default13 passes M/V through, with zero U rebuild/decode.

Separate unprofiled alternating protocol triplets give 1,605 extra sim user
instructions per legacy recipient/tick in half13 (range1,601–1,617), and 1,573
in all13 (1,571–1,584). Rates over the same 5.971 world seconds:

| Per client / nominal world second | Stock | All13 |
| --- | ---: | ---: |
| Frames | 82.93 | 258.91 |
| Game bytes | 2,482.22 | 2,480.85 |
| Including derived WebSocket headers | 2,650.22 | 3,000.34 |

3.12x frames and 13.21% more WebSocket bytes; excludes TCP/IP/TLS/handshakes.
This short-snake fixture is not a general bandwidth estimate. Socket/kernel
cost needs separate real-socket measurement; frame count does not equal syscalls.

## Reproduce and limits

`python3 reproduce.py build REPO OUTPUT`, then `trace` and
`count REPO OUTPUT --perf PATH_TO_PERF`. Python3.12+, patch and Linux user PMU
access required; separate arm targets and immutable copied executables. For
legacy tick brackets run the base binary for each case with
`STUDY_TICK_COUNTS=FILE`; select the 853 ticks above, average column1, subtract
stock and divide by 250/500. Columns: tick, instructions, enabled, running, alive.

The scripts reproduce the parity/count claim; coarse diagnostic hooks and full
raw captures are local-only, outside Git. Counter/profiler scopes, native fixture
and evolving real sockets have different denominators. No transfer of their
percentages to native one-core host capacity is justified. Failed diagnostic
captures, loss-bearing samples and full arrays remain local; none entered pairs.
