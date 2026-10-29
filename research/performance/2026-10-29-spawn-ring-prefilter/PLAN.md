# Exact spawn head prefilter: frozen plan

Baseline `23eddca8`. Preserve candidate poses and order, all three RNG draws per
attempt, ring construction errors, strict and directional fallback outcomes,
maximin scores/ties, queue ordering and refusal/delay rules. This is one code
PR, independent of the pit minimap cache and marker reuse candidates.

After the first original body initializes the strict safety query, reject a
later head before allocating/trig-building its ring only if strict head
clearance rejects, ring geometry is valid and fallback is ineligible. Keep
the first 256 fallback-eligible candidates on the entire original path. No
search-budget, clearance, retry, score, arena or protocol change.

Local checks run serially, one Cargo job and one Rust test thread: independent
pre-change complete search oracle; bitwise poses/headings, errors and RNG;
strict/fallback age boundaries and exhausted search cases; existing spawn and
host admission tests; a fallback-gate mutation that must fail; full Rust check
group. Compare all 23 deterministic catalog traces and four historical host
fixtures, complete bytes with terminal records, plus stock/protocol-13
Battledome and queued exhausted-search traces. A separate mutation must change
wire bytes in both protocols. Preserve every failure and partial artifact.

Prepare a fixed-work host row with many queued joins and repeated exhausted
4,096-pose searches. Use the real `Simulation::admit_waiting` and retry clocks,
not a replacement queue. Freeze the full host/backend and wire histories and
fork the same starting state per repetition. Include a deliberately synthetic
blocked-band fixture to guarantee repeatable exhaustions; label it as a stress
row, never natural human play or live capacity. Pin admission attempt counts,
RNG, queue size, strict/fallback admissions, world and complete wire outcomes
independently of timing. Also retain an ordinary no-backlog control.

The SSH guest remains unavailable until explicit release. No SSH connection,
build or measurement before that message. Prepare/compile the fixture locally
without selecting local timing results. Once released, run both the minimap
and spawn candidates back to back, each against its own pinned baseline, both
protocols, one discarded AB warmup plus five AB/BA/AB/BA/AB instructions:u
pairs. Require >=99% scheduled counter time and quiet context. Report all-tick
and actual search/exhaustion, food-epoch, minimap/marker and slowest-1% tick
groups separately, CPU p50/p99/max and paired median/range. Include the entire
admission call inside the counted interval. Publish each PR after evidence is
complete; no extrapolated capacity or human-feel claim.
