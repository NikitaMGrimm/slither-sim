# Exact pit minimap projection cache

Implementation and local correctness checks are complete for the stated scope;
the required fixed-work performance evidence is **pending bench availability**.
No instruction, slow-tick, capacity or human-play improvement is claimed yet.
No code PR is ready for adoption on this evidence alone.

The backend caches only the pit's outside/edge cell indices, leaving each
snake raster, protocol-13/19 packet, recipient baseline and delivery cadence
live. Reuse is allowed only within the exact interval defined by the original
per-cell nearest/farthest comparisons. Equality remains strict for outside and
interior cells and inclusive for edge cells. This avoids quantizing a radius
whose floating representation can move during proportional arena growth.
The public uncached rasterizer remains an independent reference. The cache
belongs to one backend and is rebuilt for a different side or threshold range.

Baseline `cf67b80e`, implementation `af420c84`; later study edits add only
correctness tooling and notes. [Plan](PLAN.md), [attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md), [selected correctness](correctness.json).

## Current evidence

- All 23 deterministic catalog rows and four historical host traces compare
  complete bytes and terminal records exactly. This includes stock, mixed and
  protocol-13 500-seat fixtures and the 2,281-seat fixture.
- Two supplementary eight-seat Battledome traces preserve full bytes through
  growth, shrink, deaths and reconnects, one per protocol. The original final
  world checkpoints, spawn/latency RNG and every outgoing frame/time agree.
- A projected-radius multiplier of 0.98 changes wire-frame bytes in both
  supplementary protocols. A focused strict-to-inclusive threshold mutation
  fails the independent cell oracle; restoring source passes all 398 active
  library tests, with one pre-existing ignored test.
- Cell oracles cover odd/even sides, all cell thresholds for selected small
  grids, adjacent floats, nonfinite radii, changing side, real world growth and
  shrink, ordinary worlds and invalid-side errors. The test also verifies that
  adjacent radii inside one exact interval retain the mask allocations.
- Existing Battledome and protocol-13 tests and strict default all-target
  Clippy pass locally. All 15 required full Rust-group commands pass; see
  [verification](verification.json) and the attempt notes.

These are untimed local Windows debug-build checks with one Cargo job and one
Rust test thread, not the Linux performance experiment. The earlier 16-seat
supplementary initialization failure and failed helper copy are retained,
including original sources/binaries/logs. No incomplete trace qualifies.
Full recordings are local-only; selected hashes identify them without making
them publicly retrievable.

## Remaining gate

Wait for the maintaining agent to release the SSH guest. Then run the frozen
Battledome full-backend fixed-work experiment, both protocols, five alternating
`instructions:u` pairs, mutation checks and separate slow-tick results. Keep
all baseline/candidate windows and quiet-context evidence. Performance must be
measured directly rather than inferred from eliminated `hypot` calls.

After this PR, spawn-search acceleration precedes marker reuse. Its fixed-work
row must reproduce many waiting joins and exhausted 4,096-pose searches while
preserving candidate poses, draw/order/RNG, admitted poses, refusal/delay rules
and complete wire bytes. No marker change has been implemented.
