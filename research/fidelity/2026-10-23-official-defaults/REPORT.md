# Official baseline selection, 2026-10-23

Status: configuration selection and API change. No new capture, fit, benchmark,
training run or claim of exact official-server equivalence.

## Selected behavior

`Config::official(n)` selects the simulator's best-current model of ordinary
official gameplay. `Config::default()` and omitted JSON world settings select
`official(1)`. The initial one-snake assumption keeps an unconfigured world small;
it does not represent a typical official lobby or create players. Callers choose
the initial population argument or explicit world settings for their scenario.

The selected baseline includes population-driven radius, ambient replenishment,
prey, kiosk penalties, food admission limits, retained boost progress and sliding
rim contact. Existing mechanics and their coefficients are unchanged. The
maintained [configuration reference](../../../docs/configuration.md#official-and-sandbox)
owns the constructors, defaults and override instructions.

`Config::sandbox()` derives the same mechanics from `official(1)` and selects the
previous compact scene: fixed radius, fixed ambient target/refill, and zero
initial/replenished prey. The zero count is an explicit inexpensive scenario
choice; prey behavior remains available through `prey.count`. Tests and historical
performance workloads that require that scene now request it explicitly.

## Evidence and modeling choices

- **Radius and food:** retain the pre-change `realistic(n)` construction formulas
  from SlitherSim `0640b50e5cda92ea8363c1f105cf9e611c287ea2`:
  `670 * sqrt(max(n, 1))` radius, seven startup ambient placement attempts per
  300-by-300 sector of arena area, and 0.1024 replenishment attempts per sector
  per second. The dynamic controller is the existing heuristic. It changes
  radius using living snakes; initial food and prey targets do not automatically
  rescale afterwards. Global food generation and the radius controller are
  unresolved. The existing controller's expansion cap has recorded
  counterexamples; see [host fidelity](../../../docs/host-fidelity.md#ecology-models).
- **Prey population:** choose `max(1, round(area / 10_000_000))` as a coarse
  working initial target. This is a new explicit model choice, with one prey
  in the smallest world and 14 in `official(100)`. The imported research note
  `slither-arena-rs/docs/research/prey-state-audit.md`, section "Visible prey
  count", records about 0.04-0.18 recipient-visible prey per million square units
  of loaded food-sector area. Only 50-92% of visible prey lay in those sectors.
  This proxy motivates a nonzero population and supplies a rough scale; it does
  not identify a global population law, spawning rate or visibility area. The
  selected 0.1 density and one-prey minimum remain assumptions, without a fitted
  error or validation claim. Existing prey motion, nutrition, threat and
  replenishment parameters are retained; their packaged regressions are listed
  in [server rules](../../../docs/server-rules.md#packaged-regression-evidence).
- **Kiosk:** keep the existing enabled historical model. Its source is the
  [WORM Labs June 2024 study](https://www.reddit.com/r/Slitherio/comments/1dagyjq/worm_labs_inc_study_on_new_kiosk_penalty/).
  The stationary-area detector, rates and recovery remain approximations, and
  current official onset has not been established by a controlled long trial.
- **Other mechanics:** retain the current enabled admission models, retained
  speed, sliding boundary, protocol constants and coverage/reporting estimates.
  Their measured, client-derived and modeled portions remain distinguished in
  [server rules](../../../docs/server-rules.md) and
  [host fidelity](../../../docs/host-fidelity.md).

## Compatibility and reproducibility

The `realistic` constructor is removed without an alias. This is a breaking Rust
API change, requiring the next compatible versioning decision to use a minor
bump within `0.x`. JSON field names, CLI operations and bot observation contracts
are unchanged; omitted settings intentionally change gameplay. Explicit settings
continue to override the selected baseline, including `prey.count: 0` and
`kiosk.enabled: false`.

Recorded replay retains explicit `dynamicRadius: false` and supplied
`radiusUpdates`, along with its other replay-specific overrides. Existing
recordings, fixtures, performance result JSON and historical reports are not
rewritten. The former `step-default` case retains its name and its compact
sandbox workload; its name is historical, rather than a promise to follow future
gameplay defaults.

Battle Dome is outside this change. No experimental refill implementation is
promoted, and the reported prey visual-pickup mismatch remains unresolved.
