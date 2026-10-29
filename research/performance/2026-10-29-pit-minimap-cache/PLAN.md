# Pit projection cache: frozen evidence plan

Baseline `cf67b80e`; preserve every world, RNG, protocol and delivery outcome.
Cache only the pit projection mask. Its reuse interval comes from the original
nearest/farthest cell distances with the original strict/inclusive comparisons.
Rebuild for a changed side or any threshold crossing; snake rasterization and
recipient encodings/history remain live. Public `Minimap::from_world` stays the
uncached oracle. No radius rounding, epsilon, cadence or rule change.

Locally run boundary/next-float and growth/shrink tests, existing Battledome and
protocol-13 tests, a deliberately inclusive-outside-threshold mutation (must
fail), restored library checks, and strict Clippy. Use one Cargo job and one
test thread. Preserve failed attempts. No timing claim from these checks.

Use the earlier [full trace matrix](../2026-10-28-broadcast-distance/reproduce.py)
unchanged: 23 deterministic catalog rows plus four host fixtures, complete
decompressed bytes and terminal records. Add Battledome stock/protocol-13 traces
that cross growth/shrink, minimap periods, deaths, queued admissions and
reconnects. Require original/candidate world checkpoint and RNG equality plus
all frames/timestamps, and a minimap mutation that changes wire bytes in both
protocols. Keep full traces and binaries outside Git.

The SSH timing guest is occupied. No connection, build or measurement there
until the maintaining agent explicitly releases it. Then reuse the linked
[fixed full-backend fork workflow](../2026-10-29-food-diffs/REPRODUCE.md), adapting
only the fixture to shipped responsive Battledome. Warm naturally from tick zero
and freeze the full backend, not a physical checkpoint. Every pair starts from
that same full state; no invented food or wire history. Preserve shared policy
steering and high-duty harness boost. Retain seats, living population, body
length, food, admission and boost census. Include a stable row and a shrinking
population transition so every changed path actually runs.

For protocol 19 and 13, discard one AB warmup then count five alternating
AB/BA/AB/BA/AB `instructions:u` pairs on one core, checking >=99% counter
scheduling and quiet process inventories before/after. Builds/tests finish
before counting. Report all-tick and minimap-due, marker-change, food-epoch and
admission-search tick groups separately, plus slowest 1% and maximum CPU ticks.
Keep CPU clock/PMU overhead included on both sides. Measure only after complete
byte parity and mutation controls pass. Report median and range per pair with
no capacity or human-feel claim. Publish one exact-change PR with selected
results and table recomputation, after its required evidence is complete.
