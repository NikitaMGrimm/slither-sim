# Arena vision playtest plan — 2026-10-27

Synthetic, offline loopback only. This is a custom-mode design/cost check, not
an official visibility inference, capacity qualification or fitted gameplay claim.
Base: origin/main `20e5064`. One logical CPU for each process and its children;
sequential runs, build jobs 1, Rayon 1, test threads 1. No outside game servers.

Implement numeric-or-`"arena"` world vision, whole-arena socket snake/prey/food
coverage following the live radius, and a host `bots.vision` cap defaulting to
1500 with the existing local food stencil. Other presets remain numeric 1500.
Keep a finite numeric observation radius (live diameter for arena vision).
Geometric JSON v6 identifies the extended config/observation contract. Portable
client v2 and fixed numeric C ABI v3 remain; physics rules v5 and checkpoint
format v2 remain (no new state or tick law, numeric checkpoints still restore).

Before measurement, pin the candidate source and release binary. Four matched
75-second host runs: 64 then 80 background bots, seed 7, numeric 1500 then arena
for socket players, bot cap 1500, two bot threads on the same one-CPU affinity.
Record 10-second stats, host process user+kernel CPU, ticks and deferred ticks.
This no-socket pair isolates the bot cap and shared host overhead.

Then test real loopback socket delivery at 64/80 seats if the retained load
fixture supports it, otherwise the allowed bot-plus-up-to-three-stock-client
workload. Measure per-recipient steady downlink, join payload and payload+64
bytes/frame cost against the 2 MiB queued-output policy. Protocol 19 required;
protocol 13 and `[99,4]` admission included if the retained harness is usable.
Exercise retained unmodified stock game code with ~2–3k pellets; check distant
snake/food receipt, decoder/frame handling and errors. Retain failed attempts.

Run affected Rust check group and docs checks. Compare finite non-Battledome
stock-wire traces and direct observation/frame payloads against the base,
keeping JSON envelope contract changes separate. Do not regenerate historical
fixtures. No PR until the owner approves the playable branch.
