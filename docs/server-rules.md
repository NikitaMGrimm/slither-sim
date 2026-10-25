# Mechanics and delivery limits

SlitherSim combines protocol-19 measurements, client constants and explicit
models. The official server is closed source; packet agreement does not identify
its full algorithms. This page owns the current behavior and its evidence limits;
[research records](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md)
retain dated measurements, failed fits and counterexamples.

## Defaults

`Config::default()` is `Config::official(1)`. `Config::sandbox()` uses the same
mechanics in a compact fixed arena. Neither is an exact official replica. See
[configuration](configuration.md#official-and-sandbox) for all defaults and overrides.

The radius controller, food admission limits, prey population, kiosk penalty and
rim sliding are provisional. In particular:

- Dynamic radius is enabled; use recorded `radiusUpdates` with
  `dynamicRadius: false` for measured replay.
- A 75-unit cell admits a new ambient, boost or death pellet only while it holds
  fewer than 2, 9 or 22 pellets of any kind, respectively. Rejected boost
  emission still spends mass.
- The prey target is a working density assumption, not a measured global census.
- Kiosk is enabled. Set `kiosk.enabled: false` for motion replays where it should
  not affect trajectories. The historical [WORM Labs study](https://www.reddit.com/r/Slitherio/comments/1dagyjq/worm_labs_inc_study_on_new_kiosk_penalty/)
  motivates the model; current official onset and recovery remain unverified.

## Packaged regression evidence

Tests are under `tests/`, fixtures under `tests/fixtures/`; the published crate
omits both, apart from fixtures its unit tests read. Fixtures identify their
sources, methods and hashes. Tests run without the full recordings, but repeating
measurements may require those local-only archives.

| Rule | Tests and fixtures | Evidence limit |
| --- | --- | --- |
| Movement/steering | `physics.rs`, `movement_turns.rs`; `movement-turn-runs.json`, `speed-ramp-validation.json` | About 7 ms server loops; protocol conversion units differ. |
| Growth/cruise | `cruise_growth_retention.rs`, `speed_state.rs`; `food-growth.json`, `cruise-growth-retention-20260930.json`, `cruise-retention-pair.json` | Retained cruise measured; transitions modeled. Fixed-size ramps do not test growth during a ramp. |
| Body anchors | `physics.rs`, `server_body.rs`; `body-updates*.json`, `server-body-snapshots.json` | Client and received-server evidence differ; reconstruction is not universally exact. |
| Collision | `physics.rs`; `official-collision-cases.json` | Android offline-client geometry, not measured server hitboxes. Online move/test phasing is unverified. |
| Pickup | `circle_pickup.rs`, `pickup_events.rs`; `boost-circle-food.json` | Selected behavior, not complete coverage. |
| Death food | `death_food.rs`; `death-food.json`, `death-food-prospective.json`, `death-jitter-fresh.json` | Scatter fits retain count gaps and held-out exceptions. |
| Food density | `ambient_food.rs`, `boost_food.rs`, `death_food.rs` | Received occupancy cannot isolate generation from delivery filtering. |
| Boundary/radius | `boundary.rs`; `boundary-contact.json`, `arena-radius-updates.json` | Sliding is fitted; recorded radius traces are stronger than the population law. |
| Prey | `prey.rs`, `prey_outward.rs`, `prey_decisions.rs`, `prey_activation.rs`; `prey-*.json` | Speeds constrained; steering, population and outward departure trigger remain approximate. |
| Kiosk | `kiosk.rs` and core unit tests | Historical model; current official applicability unverified. |

## Client delivery and spawning

The [`serve` controller](bot-api.md#frames) exposes immediate simulation state,
including the entire stored record of a visible opponent. The [portable client
API](client-api.md) reconstructs delivered messages with timing and receipt ages.
A receipt, a reconstructed pose and renderer prediction are different states.

| Area | Implemented behavior | Evidence and regression |
| --- | --- | --- |
| Food interest | 45 sectors: 7×7 without four corners, 300 units per sector, center led 100 units along physical heading. Retain last-touched sectors for 2.6 s; cleanup on nominal 504 ms opportunities. | `client-food-interest.json` pins selected receipts. Later food-model counterexamples remain; scheduler and size scaling are provisional. |
| Snakes | Complete centerline intersects `world.vision`, default 1,500. | `src/client/visibility.rs` tests whole-body coverage. Paired receipts reject 500 and whole-world streaming; 1,500 is an engineering choice, not an identified boundary. |
| Prey coverage | Points checked independently at the same configurable distance. | `src/client/simulation.rs` tests delivery and actual eater IDs. A shared numeric default does not establish a shared official subscription algorithm. |
| Hosted spawning | Radial-lattice candidates; complete newborn body clears living heads/body edges by 600 units. Crowded admission waits or fails without a cramped fallback. | `spawn.rs` and host tests. Observed own joins include exceptions below 600; this is conservative local admission. Initial geometric test rings are synthetic. |
| Minimap | Global living centerlines, 136×136 by default, server-wide 2.504 s clock; stop after death and reset on reconnect. | `client-reporting-cadence.json` and client tests. Resolution/cadence are measured; exact raster filtering and official encoding selection are unknown. |
| Pellet presentation | A separate seeded RNG gives persistent colors to admitted births; primary skins select boost/death colors, with retained stock/custom recipes. | `client-presentation.json` retains recipes and pooled ambient counts. Starting phase, rejected-birth advancement and some custom boost behavior remain unverified. Unknown skins use an approximate modulo-9 fallback. |

The [visibility study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md)
preserves food counterexamples and unsuccessful crossing acquisitions. No follow-up
validated a replacement snake/prey subscription rule. The current collector's
bounded-staleness pause has offline coverage; live route completion remains to be
shown before fitting a rule.

### Food sector delivery

The host keeps empty subscriptions internal. Nonempty entry listings use `F`,
subsequent births/pickups use deltas, and `w` clears recipient food on exit without
deleting physical food. The first birth in an already empty subscription uses a
birth delta; this is a local choice. The food fixture retains the full-archive
announcement census as well as selected entry witnesses. Decoder support for
other packet forms does not require the host to emit them.

### Minimap encoding

Each connection retains its last queued bitmap. First/reconnected/resized maps
use full `M`; later maps use XOR `V` only when strictly shorter. The base advances
after queuing that recipient's packet. Both forms reconstruct the bitmap; this
shortest-encoding policy is local and differs from some retained official choices.

### Packet timing

Physics ticks default to 7 ms. Gameplay updates are encoded every tick, omitting
empty frames. The
[scheduler study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-packet-scheduler/REPORT.md)
supports separate release paths and shared entity/maintenance phases. The
following gates are adopted inferences, not an exact recovered implementation:

| Family | Simulator release rule |
| --- | --- |
| Motion | Changed quantized heading/wanted-heading/speed tuple, at least six ticks apart (42 ms at default `dt`); one gate per snake shared across recipients. A stable tuple sends no heartbeat. |
| Standalone mass `h` | Changed mass, at least 15 ticks apart (105 ms at default `dt`), shared per snake. Tail removals and growth anchors can carry mass outside this gate. |
| Body | Prompt anchors and tail changes. Incremental per-snake order is tail/standalone mass, then motion, then anchors; growth anchors carry their own mass. |
| Food | Ambient births become wire-visible when `floor(t / 0.504)` advances, with sector cleanup on the shared maintenance epoch. Boost/death `b` and pickups are prompt; `F` listings remain an interest-entry path. Physical ambient generation is unchanged. |
| Prey `j` | One correction clock per prey shared across viewers, nominal one third second. Exact-11 pulse transitions use a 210 ms guard; ordinary turning alone does not accelerate reporting. Quantized position need not change. Viewer entry does not restart the clock. |

The corpus includes sub-35 ms motion gaps, sub-100 ms mass gaps, 27 reverse
mass/motion pairs and a 2,694 ms same-prey correction gap with other traffic
continuing. The simulator's strict gates, common ordering and bounded prey
corrections do not reproduce every exception. Exact dirty comparators, rearm
rules, event-to-send deadlines and visibility triggers remain unknown.
An incremental food index and snake coverage grid reduce work while preserving
wire selection and ordering; they are performance mechanisms, not official rules.

Minimaps and leaderboards follow two independent server-wide clocks. Officially
both are shared by all recipients, reconnects join the running phase, and mean
periods are 2503.994/3008.062 ms rather than 2500/3000. The simulator services
absolute periods of 2.504/3.008 s at the first tick in each, so a first map or
board arrives 0–2.5/3 s after entry, and puts the map first in its frame. Why
official periods exceed 2.5/3 s is unknown; nominal thresholds rearmed after
service would fit, but the simulator only reproduces the mean.

Setup advertises the active prey turn rate in reference-frame units (default
`mamu2=0.029`), as checked by `client-hud-leaderboard.json`. The cadence fixture
pins a selected epoch with source/tool hashes; it is not a universal timing bound.
Use the [packet census](research-workflow.md#inspect-analyze-and-reproduce) for new
captures and [configuration](configuration.md#timing-and-ping) for local settings.

## Ecology models

Population-scaled radius, food target/refill, radial food profile and prey density
are models. Recipient arrivals and occupancy cannot identify rejected generation,
global counts or startup seeding. A later matched-sector check rejected the earlier
ambient-replacement association, so it supplies no refill retuning.

The radius controller is known to differ from retained observations. In
`run4-radius-long-20260927/bot0.jsonl.gz` (SHA-256
`3180b8201f51d5f4fe9732b6c69e85bab8ce49448742a2a72cfd228ef60613ef`),
connection 4, server time 292,116→292,243 ms, radius grows 13,940→13,947 faster than
the model's expansion cap. Its replacement remains unidentified; replay recorded
`radiusUpdates` rather than installing the fastest observed slope as a new cap.

## Movement and geometry

Historical source: `slither-arena-rs` commit
`c2fb7285febb4273034e30699dc880715a334423`. Its `movement-fidelity.md`,
`speed-equations.md`, `circle-eating-investigation.md`,
`official-collisions-20261003.md` and `body-geometry.md` retain the fitted turning,
speed, pickup, collision and contraction choices. The corresponding selected
fixtures are packaged here; the full original studies are not.

The retained 1,567 fixed-size speed ramps support those slopes, not growth during
a ramp. The simulator moves all snakes before resolving collisions, whereas the
Android offline client moves and tests each in sequence. Online phasing remains
unverified. Selected near-rim prey removals constrain received motion but do not
establish global deletion or its activation rule.

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
