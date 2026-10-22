# Simulator rules and evidence limits

Mechanics combine protocol-19 measurements, client constants and explicit models.
The official server is closed source; packet agreement does not establish its
full algorithms.

`Config::default()` uses a 1,000-unit arena. `Config::realistic(n)` scales arena
and ecology using a provisional population model.

## Packaged regression evidence

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

## Provisional defaults

- Kiosk penalty is on. Set `config.kiosk.enabled: false` for motion replays where
  this unverified penalty should not affect trajectories.
- Each 75-unit food cell admits up to 2 ambient, 9 boost and 22 death pellets.
  Rejected boost emission still spends mass; these limits remain inferred.
- Dynamic radius is optional; `radiusUpdates` traces override the population model.
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
