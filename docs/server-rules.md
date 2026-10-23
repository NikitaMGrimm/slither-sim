# Simulator rules and evidence limits

Mechanics combine protocol-19 measurements, client constants and explicit models.
The official server is closed source; packet agreement does not establish its
full algorithms.

`Config::default()` is `Config::official(1)`, the best current approximation
including the provisional models below; `Config::sandbox()` is a compact fixed
arena with the same mechanics. See [official and sandbox](configuration.md#official-and-sandbox)
and the [official-defaults report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-official-defaults/REPORT.md).

## Packaged regression evidence

[Client presentation](presentation.md) covers pellet palettes separately from
physical food generation and pickup rules.
[Host fidelity](host-fidelity.md) records coverage, spawn and minimap corrections
and the remaining unmeasured server policies.

Fixtures retain sources, methods and hashes. Original recordings and analysis
outputs are not distributed; packaged tests run without them, but repeating the
measurements may require the original archives.

| Rule | Tests and fixtures | Limit |
| --- | --- | --- |
| Movement/steering | `physics.rs`, `movement_turns.rs`; `movement-turn-runs.json`, `speed-ramp-validation.json` | About 7 ms server loops; protocol time/speed units differ. |
| Growth/cruise speed | `cruise_growth_retention.rs`, `speed_state.rs`; `food-growth.json`, `cruise-growth-retention-20260930.json`, `cruise-retention-pair.json` | Retained cruise measured; transitions modelled. |
| Body anchors | `physics.rs`, `server_body.rs`; `body-updates*.json`, `server-body-snapshots.json` | Client and received-server evidence differ; reconstruction is not universally exact. |
| Collision | `physics.rs`; `official-collision-cases.json` | Some cases describe client geometry, not observed server hitboxes. |
| Pickup | `circle_pickup.rs`, `pickup_events.rs`; `boost-circle-food.json` | Selected behavior, not complete coverage. |
| Death food | `death_food.rs`; `death-food.json`, `death-food-prospective.json`, `death-jitter-fresh.json` | Scatter fits retain count gaps and held-out exceptions. |
| Food density | `ambient_food.rs`, `boost_food.rs`, `death_food.rs` | Received occupancy cannot isolate generation from broadcast filtering. |
| Boundary/radius | `boundary.rs`; `boundary-contact.json`, `arena-radius-updates.json` | Sliding is fitted; radius traces are stronger than the population law. |
| Prey | `prey.rs`, `prey_outward.rs`, `prey_decisions.rs`, `prey_activation.rs`; `prey-*.json` | Speeds constrained; steering, population and departure remain approximate. |
| Spawning | `spawn.rs` | Radial bands do not determine their probability law; ring starts are synthetic. |
| Kiosk penalty | `kiosk.rs` and core unit tests | Historical model; current official applicability unverified. |

Tests are under `tests/`, fixtures under `tests/fixtures/`. Run `cargo test --locked`.

## Movement and geometry

The historical source is `slither-arena-rs` commit
`c2fb7285febb4273034e30699dc880715a334423`. Its research notes
`movement-fidelity.md`, `speed-equations.md`, `circle-eating-investigation.md`,
`official-collisions-20261003.md` and `body-geometry.md` explain the fitted turning,
speed, pickup, collision and contraction choices. The corresponding packaged
fixtures are listed above; full original studies are not distributed here.

The October 23 recheck reproduced all 1,567 retained fixed-size speed ramps from
three hash-matched validation archives. This supports those slopes, not growth
during a ramp. Collision geometry comes from the Android offline client; the
simulator moves all snakes before resolving collisions, while that client moves
and tests each snake in sequence. Online move/test phasing remains unverified.
Three selected near-rim prey removal projections agree with the rim within five
units; recipient removals do not establish global deletion or its activation rule.

For ecology/radius counterexamples and receipt policies, see
[host fidelity](host-fidelity.md). Keep physical world state, reconstructed client
state and renderer prediction distinct when interpreting a regression.

## Provisional defaults

- Kiosk penalty is on. Set `config.kiosk.enabled: false` for motion replays where
  this unverified penalty should not affect trajectories.
- Each 75-unit food cell admits up to 2 ambient, 9 boost and 22 death pellets.
  Rejected boost emission still spends mass; these limits remain inferred.
- Dynamic radius is on by default; `radiusUpdates` traces override the population model.
- The default prey target is a working density assumption, not a measured census.
- Sliding and hard-boundary modes are available.
- Prey behavior includes an outward replay mode whose official trigger is unknown.

The [geometric controller](bot-api.md#observation-boundary) returns immediate
simulation state, including entire visible opponents. The [portable API](client-api.md)
reconstructs delivered messages with timing and receipt history; its visibility
subscriptions and minimap rasterization remain approximate. Neither establishes
playing strength or exact official behavior.

Mechanics changes need an evidence note, a distilled regression and an explanation
of default changes. Keep the 7 ms physics loop separate from protocol conversion
constants.
