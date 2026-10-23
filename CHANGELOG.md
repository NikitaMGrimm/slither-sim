# Changelog

## Unreleased

- Run declared paired-prey routes through the managed evidence collector and
  verify route settings against the plan; retain process outcomes across audit errors.

- Move crossing acquisition's near waypoint inward and hold achieved endpoints
  for observation; retain commanded hold offsets and release timing in metadata.

- Require fresh reference geometry before crossing endpoints advance; separate
  the initial approach from completed legs and retain exact endpoint witnesses.
  Record the paired visibility recheck's model failures without retuning physics.

- Hash and decode the same replay-source snapshot; retain protocol and tool
  identities and distinguish replay-window checks from a full evidence audit.

- Compare verified evidence batches with bounded permutation tests and declared
  exposure/effect gates; add manual, read-only CI intake for pinned bundles.
  Keep measured calibration separate from synthetic prey/radius behavior tests.

- Keep empty food subscriptions internal and send per-connection minimap XOR
  deltas when shorter. Add fixed diagonal collection routes and clarify the
  source/evidence boundary, ecology models and offline collision rationale.

- Match the default prey turn setup to physics; separate leaderboard deadlines
  and bounded prey reporting from gameplay broadcasts. Add an offline packet
  census, source-pinned cadence fixture and generic localhost capture workflow;
  keep recording ping timing through post-death observation and capture shutdown.
- Keep correcting predicted prey positions even when authoritative grid/motion
  is unchanged, preventing drifting ghosts and later snap-backs.
- Use shared finite 1,500-unit snake/prey coverage and retained food-sector interest;
  restore 136-cell minimaps with an independent clock. Require safe whole-body
  admission for hosted startup, joins and respawns, with no cramped fallback.
- Make synthetic world-run records/victory entry opt-in (`localRecords`, default false).

- Restore source-linked pellet palettes, persistent ambient colors, birth types
  and explicit pickup attribution from the earlier client presentation layer.

- Keep answering post-death pings so the stock browser's death fade and victory
  entry do not trigger lag slowdown; document a local endpoint that survives Play Again.
- Continue opponent, food and prey updates through the stock death animation,
  preserving broadcast cadence while suppressing repeated death and minimap reports.

- Keep configured prey sizes positive and within their bounds, ambient food
  inside tiny arenas, and finite extreme respawn poses numerically usable.
- Disambiguate initial protocol timing headers and bound retained client bodies
  and food with atomic frame rejection.
- Require the original v1 evidence source minimum and report replay targets
  left unprocessed after early simulator death.

## 0.1.0 (2026-10-22)

- Deterministic headless Rust simulator with configurable worlds, geometric
  observations and mechanics regression fixtures.
- Core-only library builds, selected-slot observations, supplied-body respawn
  and runtime ambient refill-rate control.
- Portable bot frames (`slither-client-v2`) reconstructed by a shared
  protocol-19 decoder, with simulator and WebSocket backends, receipt ages,
  measured ping and simulated latency.
- JSON-lines interfaces for portable bots (`slither-client-json-v1`) and direct
  world control (`slither-sim-json-v1`), with Rust, Python and JavaScript examples.
- Bounded shared-world WebSocket host with profiles, leaderboards, kill reports,
  record messages and optional connection identifiers.
- Evidence tools for explicit-endpoint captures, decoding, auditing, recorded-input
  replay and bundles containing frozen plans, attempt records and source snapshots.
- Offline worked study, documented fidelity limits, extraction provenance and
  manual checks on Windows and Linux.
- Automatic pull-request checks, shared local/CI check commands, contributor
  task guidance and protocol-labelled regression fixtures.

Interfaces remain experimental.
