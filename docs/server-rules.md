# Mechanics and delivery limits

SlitherSim combines protocol-19 measurements, client constants and models.
The official server is closed source; packet agreement cannot identify its full
algorithms. This guide owns current behavior and evidence limits;
[research records](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md)
retain dated measurements, failed fits and counterexamples.

## Defaults

See [configuration](configuration.md#official-and-sandbox) for official and
sandbox defaults and overrides. Neither is an exact official replica.

[`Config::battledome()`](configuration.md#battledome) owns its custom pit,
size-floor, free-boost and food-budget rules and synthetic regressions. These
are design choices, not official-server measurements.
Battledome enables `foodOnlyInsidePit`: ambient supply scales by pit area;
ordinary full-body death food includes floor-size and pit deaths. Scattered
death and boost pellets are discarded outside the strict live pit edge.
Moving edges remove stranded food. Above-floor boost mass spent outside the pit
is lost; floor boosting stays free without pellets. The total-food refill target
pauses ambient generation but does not cap death/boost food. Long-run supply
balance for this owner-directed version has not yet been measured.

The seat-following radius, food admission limits, prey population, kiosk penalty and rim
sliding are provisional. In particular:

- For measured radius replay, use [recorded samples](configuration.md#world-config).
- Default official and host worlds follow occupied seats (living snakes and
  those waiting to respawn), outside a 3% dead zone. The arena grows at once
  when players join and shrinks at 23.4375 units/s when they leave; food and
  prey targets follow its area. This is our design, not an identified official
  law, and it does not reproduce official multi-minute waves; see the
  [decision](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-arena-radius/SEAT-FOLLOWING.md).
- [Food admission limits](configuration.md#food-admission-limits) count all origins
  toward occupancy, with origin-specific caps.
- The [prey target](configuration.md#official-and-sandbox) is a density assumption,
  not a measured global census.
- The historical [WORM Labs study](https://www.reddit.com/r/Slitherio/comments/1dagyjq/worm_labs_inc_study_on_new_kiosk_penalty/)
  motivates [kiosk](configuration.md#kiosk-penalty); current official onset and
  recovery remain unverified.

## Packaged regression evidence

`tests/` and `tests/fixtures/` are excluded from the crate except unit-test
fixtures. Fixtures name sources, methods and hashes. Tests need no full recordings;
repeating measurements may require those local-only archives.

| Rule | Tests and fixtures | Evidence limit |
| --- | --- | --- |
| Movement/steering | `physics.rs`, `movement_turns.rs`; `movement-turn-runs.json`, `speed-ramp-validation.json` | About 7 ms server loops; protocol conversion units differ. |
| Growth/cruise | `cruise_growth_retention.rs`, `speed_state.rs`; `food-growth.json`, `cruise-growth-retention-20260930.json`, `cruise-retention-pair.json` | Retained cruise measured; transitions modeled. Fixed-size ramps do not test growth during a ramp. |
| Body anchors | `physics.rs`, `server_body.rs`; `body-updates*.json`, `server-body-snapshots.json` | Client and received-server evidence differ; reconstruction is not universally exact. |
| Collision | `physics.rs`; `official-collision-cases.json` | Android offline-client geometry, not measured server hitboxes. Online move/test phasing is unverified. |
| Pickup | `circle_pickup.rs`, `pickup_events.rs`; `boost-circle-food.json` | Selected behavior, not complete coverage. |
| Death food | `death_food.rs`; `death-food.json`, `death-food-prospective.json`, `death-jitter-fresh.json` | Scatter fits retain count gaps and held-out exceptions. |
| Food density | `ambient_food.rs`, `boost_food.rs`, `death_food.rs` | Received occupancy cannot isolate generation from delivery filtering. |
| Boundary/radius | `boundary.rs`, `seat_radius.rs`; `boundary-contact.json`, `arena-radius-updates.json`, `arena-seat-following-v2.json` | Sliding is fitted; seat-following is a synthetic product model. Recorded traces remain unchanged. |
| Prey | `prey.rs`, `prey_outward.rs`, `prey_decisions.rs`, `prey_activation.rs`; `prey-*.json` | Speeds constrained; steering, population and outward departure trigger remain approximate. |
| Kiosk | `kiosk.rs` and core unit tests | Historical model; current official applicability unverified. |

## Client delivery and spawning

The [`serve` controller](bot-api.md#frames) exposes immediate simulation state,
including visible opponents' entire stored records. The [portable API](client-api.md)
reconstructs delivered messages with timing and receipt ages. Receipts,
reconstructed poses and renderer predictions are distinct states.

| Area | Implemented behavior | Evidence and regression |
| --- | --- | --- |
| Food interest | 45 sectors: 7×7 without four corners, 300 units per sector, center led 100 units along physical heading. Retain last-touched sectors for 2.6 s; cleanup on nominal 504 ms opportunities. | `client-food-interest.json` pins selected receipts. Later food-model counterexamples remain; scheduler and size scaling are provisional. |
| Snakes | Complete centerline intersects `world.vision`, default 1,500. | `src/client/visibility.rs` tests whole-body coverage. Paired receipts reject 500 and whole-world streaming; 1,500 is an engineering choice, not an identified boundary. |
| Prey coverage | Points checked independently at the same configurable distance. | `src/client/simulation.rs` tests delivery and actual eater IDs. A shared numeric default does not establish a shared official subscription algorithm. |
| Hosted spawning | Without `spawnRadius`, radial-lattice candidates prefer 600-unit body clearance, then the most open positive-contact-margin pose of 64 draws; a failed search refuses. A refusal means the sampled candidates failed, not that no legal pose exists. Circular worlds require body, head-path and departure clearance and queue joins; see [configuration](configuration.md#battledome). | `spawn.rs` and host tests. Official joins were never seen refused, and three of 181 came within 550–600 units of visible bodies; see the [spawn admission record](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-spawn-admission/REPORT.md). The preference, fallback and custom circular admission are local policies. Initial geometric test rings are synthetic. |
| Minimap | Global living heads and body centerlines, 136×136 by default, server-wide 2.504 s model clock; stop after death and reset on reconnect. | `client-reporting-cadence-near-server.json` and client tests; the original home summary remains. Resolution/cadence are measured; exact raster filtering and official encoding selection are unknown. |
| Pellet presentation | A separate seeded RNG gives persistent colors to admitted births; primary skins 0–8 select matching boost/death colors, with retained stock/custom recipes. | `client-presentation.json` retains recipes, pooled ambient counts and weights. Starting phase, rejected-birth advancement and some custom boost behavior remain unverified. Unknown skins use an approximate modulo-9 fallback. |

The [visibility study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md)
retains food counterexamples and unsuccessful crossings; no follow-up validated
a replacement snake/prey subscription rule. The collector's bounded-staleness
pause has offline coverage; live route completion is still required before fitting.

### Food sector delivery

Empty subscriptions send neither `W` nor empty `F`. Nonempty entries use `F`,
births/pickups use deltas, and `w` clears recipient food on exit without deleting
physical food. The first birth in an empty subscription uses a birth delta, a
local choice. The food fixture retains the full-archive announcement census and
selected entry witnesses. Decoding other packet forms does not require emitting them.

### Minimap encoding

Each connection retains its last queued bitmap. First/reconnected/resized maps
use full `M`; later maps use XOR `V` only when strictly shorter. Queuing advances
that recipient's base. Both reconstruct the bitmap; this local shortest-encoding
policy differs from some retained official choices.

### Packet timing

Physics ticks default to 7 ms, encoding gameplay every tick without empty frames.
The
[scheduler study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-packet-scheduler/REPORT.md)
supports separate release paths and shared entity/maintenance phases.
`delivery: "official"` uses these adopted inferences, not an exact recovered
implementation:

| Family | Simulator release rule |
| --- | --- |
| Motion | Changed quantized heading/wanted-heading/speed tuple, at least six ticks apart (42 ms at default `dt`); one gate per snake shared across recipients. A stable tuple sends no heartbeat. |
| Standalone mass `h` | Changed mass, at least 15 ticks apart (105 ms at default `dt`), shared per snake. Tail removals and growth anchors can carry mass outside this gate. |
| Body | Prompt anchors and tail changes. Incremental per-snake order is tail/standalone mass, then motion, then anchors; growth anchors carry their own mass. |
| Food | Ambient births become wire-visible when `floor(t / 0.504)` advances, with sector cleanup on the shared maintenance epoch. Boost/death `b` and pickups are prompt; `F` listings remain an interest-entry path. Physical ambient generation is unchanged. |
| Prey `j` | One correction clock per prey shared across viewers, with a nominal one-third-second interval. When the current or last reported speed encodes as exactly 11, the interval is 210 ms; ordinary turning alone does not accelerate reporting. Quantized position need not change. Viewer entry does not restart the clock. |

The default `delivery: "responsive"` is our design for players, not official behavior.
Changed wanted-heading/speed bytes and standalone mass bypass their gates in
the next tick's frame. Heading-only changes keep six-tick spacing; other
families keep official pacing. It uses the same packets and stock client.
See the [comparison](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-delivery-profiles/REPORT.md).

The corpus includes sub-35 ms motion gaps, sub-100 ms mass gaps, 27 reverse
mass/motion pairs and a 2,694 ms same-prey correction gap with other traffic
continuing. Strict gates, common ordering and bounded prey corrections miss
these exceptions. Exact dirty comparators, rearm rules, event-to-send deadlines
and visibility triggers remain unknown. Incremental food indexing and a snake
coverage grid preserve wire selection/order as performance mechanisms, not rules.
The later [near-server study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-vps-timing/REPORT.md)
retains clean held-out 24 ms motion, 84 ms mass, same-frame motion-before-mass
and 3,044 ms repeat-prey witnesses. Decoder tests accept those exceptions;
the fixed simulator release model is unchanged. The
[movement study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-movement-scheduler/REPORT.md)
found 451 motion gaps under 35 ms and 107 standalone `h` gaps under 100 ms in
its three held-out runs, so neither gap is a hard floor. It selects no
replacement scheduler: clients never see when state changes or inputs apply.
Own wanted-heading and boost changes wait for the same six-tick gate. In a
[follow-up check](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-wanted-gate/REPORT.md),
549 of 581 held steering commands sent while the gate was armed were reflected
41–44 ms after the previous own motion report; 78% of held commands found it
already open, which keeps median reflection short. After confound screens,
3 of 575 steering and 1 of 86 boost commands were reflected sooner. The server
clock bounds the send time, not arrival or the physics tick.

Each tick reads all inputs and pings received during it; pongs lead its frame.
Official input reflection takes about one ping round trip (median 13.2 vs
13.4 ms on one route); pings return in as little as 4.9 ms. Neither waits an extra
tick, but the receive-to-send deadline is unknown. On a near-server route,
the [movement study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-movement-scheduler/REPORT.md#input-reflection-bounds)
bounds the time from sending a held steering command to the server frame that
first reflects it. In every run the median lies between about 4 and 7.7 ms and
the p95 between about 19 and 25.5 ms; the earlier route's bounds were
5.7–11.6 ms and 25.9–31.8 ms. Neither isolates when the server applies an
input. Pongs lead 81,830 of 81,843 official pong frames.

Minimaps and leaderboards have independent server-wide clocks. Official periods
average 2503.994/3008.062 ms rather than 2500/3000; all recipients share each
clock, and reconnects join its running phase. The simulator services absolute
2.504/3.008 s periods at their first tick. First maps/boards wait for the next
opportunity after entry, plus tick and link delay. Maps follow pongs and precede
gameplay. The excess over 2.5/3 s is unexplained; rearming nominal thresholds
after service would fit, but the simulator reproduces only the mean.

Setup advertises prey turn rate in reference-frame units (default `mamu2=0.029`);
`client-hud-leaderboard.json` checks it. The cadence fixture pins one epoch with
source/tool hashes, not a universal timing bound. See the
[packet census](research-workflow.md#inspect-analyze-and-reproduce) for new captures
and [configuration](configuration.md#timing-and-ping) for local settings.

## Ecology models

Population-scaled radius, food target/refill, radial food profile and prey density
are models. Recipient arrivals/occupancy cannot identify rejected generation,
global counts or startup seeding. A matched-sector check rejected the earlier
ambient-replacement association; it supplies no refill retuning.

The radius model's shrink rate is not an official bound. In
`run4-radius-long-20260927/bot0.jsonl.gz` (SHA-256
`3180b8201f51d5f4fe9732b6c69e85bab8ce49448742a2a72cfd228ef60613ef`),
connection 4, server time 292,116→292,243 ms, radius grows 13,940→13,947 faster than
the former model's expansion cap. The official controller is unidentified;
replay recorded
`radiusUpdates` with `dynamicRadius: false` rather than adopting the fastest
observed slope as a cap. The retrospective, exploratory
[radius study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-arena-radius/REPORT.md)
finds sustained official waves (12,926–14,611 units over about 400 s at 447–505
reported players), versus frequent death/respawn reversals in the historical
living-count controller. Forecast-origin holding (the first usable-count radius, not the
setup radius) beats the reported-count controller in
94/114 connection forecasts (mean MAE 211 versus 520 units), but loses in 20 and
suppresses real waves. No tested population, EMA, clock or load proxy identifies
the official controller. Default worlds now follow occupied seats as a
[product decision](configuration.md#population-sizing); the old living-count
controller is removed. The study's historical results do not validate this model.

## Movement and geometry

Historical source: `slither-arena-rs` commit
`c2fb7285febb4273034e30699dc880715a334423`. Its `movement-fidelity.md`,
`speed-equations.md`, `circle-eating-investigation.md`,
`official-collisions-20261003.md` and `body-geometry.md` retain the fitted turning,
speed, pickup, collision and contraction choices. Selected fixtures are packaged
here; full original studies are not.

The 1,567 fixed-size speed ramps support their slopes, not growth during a ramp.
The simulator moves all snakes before resolving collisions; the Android offline
client moves/tests each in sequence. Online phasing is unverified. Selected
near-rim prey removals constrain received motion, not global deletion or its trigger.

## Open contributions

Choose a discriminating case that resolves one limit above:

- Growth inside a speed ramp with competing predictions beyond timing/quantization error.
- A controlled collision near miss separating contact boundaries or move/test phasing.
- Paired food recipients or controlled consumption separating generation from filtering.
- A competing radius-controller prediction against a recorded window.
- A named prey activation/departure explanation against a retained transition.
- Completed live visibility-route legs before paired crossings and held-out validation.
- A shareable sub-600-unit spawn exception with fresh neighbor/corridor attribution.
- A bounded kiosk trial with stationary/moving controls separating onset and recovery.

Mechanics changes need supporting evidence, a distilled regression and an
explanation of changed defaults; see [contributing](../CONTRIBUTING.md#evidence-proportional-to-the-change).
