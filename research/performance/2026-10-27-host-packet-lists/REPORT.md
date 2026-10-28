# Recipient packet-list reuse, 2026-10-27

Reuse the update packet lists when no pong or minimap precedes them, avoiding
another outer Vec allocation and a move of every packet. The physical and
decorated streams retain their exact prefix, update and HUD order. One function
changes, with no persistent state or new invariant. Arena-vision overlap is
limited to this recipient assembly block in broadcast.rs.

Measured `4532a76f22958ae3abcca801e90edd30ed9f466f` ->
`a2ad4e6d8ff4aaec8de805129561168a65ad6056`; delivery adds research/changelog
only, with the same runtime sources. [Selected results and pins](results.json).
The earlier rejection used a blanket 2% threshold. The owner clarified that
changes of at most 50 lines with no new invariants qualify at a consistent 1%;
the measured candidate is unchanged.

## Matched evidence

WSL2 Linux x86-64, Rust 1.99 locked release/debug=1, default target. Separate
archives/targets and copied, rehashed binaries; builds/Rayon/tests one, at most
two pinned logical CPUs. Synthetic official-500, seed7, zero bots, offline.
One warmup pair then five alternating whole-process instructions:u pairs per
protocol mix; all counters 100% scheduled, no runs excluded.

| Recipients | Instructions reduced | Reduction range |
| --- | ---: | ---: |
| Stock | 1.7272% | 1.7258–1.7293% |
| Half protocol 13 | 1.6298% | 1.6297–1.6328% |
| All protocol 13 | 1.6268% | 1.6255–1.6285% |

Counts include setup, 1080 ticks and drains; controls every six ticks, reconnect
at tick720, minimum499 alive and terminal500. Socket service/kernel work are
excluded. No live CPU, latency or capacity improvement is claimed.

All fifteen complete traces match: stock/half13/all13, U5/U250/U500 and nine
historical protocol/portable, official500, Battledome40/64/80 and retained
step_all/next_frame comparisons. Frame bytes/boundaries/times, complete ordered
state/RNG and terminal records are retained. No expectations were regenerated.
132 focused simulation tests and rustfmt pass. The full Rust group (all 15 commands) also passes.
The first focused command selected an old default compiler; its rejection is
retained, and the explicit Rust1.99 rerun passed unchanged sources.

## Reproduce and limits

Create base.tar/candidate.tar with git archive at the pinned commits. Run
`python reproduce.py MODE ARCHIVES OUT` for build, trace and count
(`--perf PERF` for count). Linux x86-64 user PMU, Python3.12+, patch and Rust1.99
are required; the scripts reject -O. The shared fixture and historical overlays
come from the pinned base archive. Raw logs, traces, binaries and the original
selection history are local-only outside /tmp, with no public download.

The machine was shared; fixed-work instruction counts are primary evidence.
This fixture does not qualify a real one-core server or human-client load.
