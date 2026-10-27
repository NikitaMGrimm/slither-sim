# Eighty-socket fan-out follow-up — frozen 2026-10-27

Offline loopback only, source `3c6fb18`. This is a synthetic throughput/stress
measurement, not an official rule, natural-player workload or admission study.
`tools/perf_transport.py` is the geometric JSON-pipe tool. Reuse the actual PR
socket workload's `examples/host_load.rs::socket_driver`, via a study wrapper.
Keep its binary WebSocket login, nonblocking drain, circle commands and death
reconnect lifecycle. Add traffic counters and a synchronized reconnect cue.

One instrumented release executable serves both vision settings and the two
client shards. Host pinned to logical CPU 4; two 40-client load processes pinned
to CPUs 5 and 6. Builds/tests use only CPU 4, jobs/Rayon/test threads 1. No ports
8766/8780 or processes in the owner's play directory. Study port 19766.

Compare numeric vision 1500 and arena on identical fixed seed 7 and resolved
80-seat constant-density geometry (wall 3584*sqrt(80/32), pit and spawn band
scaled in proportion). Zero bots. Fixing this geometry removes startup/shrink
controller variation from a fan-out comparison; all other mode rules remain.

Use explicitly labeled replay geometry: slots 0..15 have 128-segment bodies on
separate 300-unit circular routes inside the pit; 64 floor-size snakes use three
separated circular lanes outside it. The study fixture imports each admitted
life and overrides steering after ordinary command ingest. This is controlled
motion, not a new gameplay policy; strict admission itself is unchanged.
Once all 80 sockets are setup and living, reset their replay poses/masses and
seed 6000 deterministic, distinct, quantized ambient pellets strictly inside
the pit. Food target/refill are zero so this fixed supply can only be eaten.

Permit up to 120 wall seconds to reach full setup (record failure otherwise).
Warm the complete fixture 10 s; measure 120 s with all 80 living recipients.
Then cue simultaneous reconnects for the 16 floor clients holding slots 64..79,
keeping the 16 large incumbents, and measure another 60 s. Retain queued joins,
closures, deaths, censored waits and actual request spread. A steady run is not
qualified at 80 unless living/setup counters show the full population throughout.

Four sequential runs, fixed order: 1500, arena, arena, 1500. No significance or
speedup claim from two descriptive pairs. Finish compilation before trials.
Retain failed/aborted attempts, do not silently substitute them into the table.

Counters: individual simulation-tick elapsed time including command/admission,
physics/broadcast and handover; mean busy/wall load, p99/max tick; host process
user+kernel CPU; deferred ticks; per-client payload rates in complete 1 s bins
and client-window totals; initial and reconnect request/admission timestamps;
first-250ms per-life burst bytes including frame allowance; exact backend and
gateway budgeted-queue peaks, plus unbudgeted inter-thread transit separately.
The budget counts payload plus 64 bytes/frame (2 MiB per budgeted queue).
Exclude OS/TCP/tungstenite buffers from those queue measurements and say so.

Buffer diagnostics in memory and write them on shutdown, except tiny setup,
phase/cue and completion files. Instrumentation/controller overhead is included
in both arms; report it as instrumented cost, not an unmodified release claim.
Pin original source, study overlay, harness, configurations and binary before
running. Full outputs stay local; add selected summaries and limits to REPORT.
