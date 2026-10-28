# Frozen offline turn/count-phase protocol

Frozen before new fixture/run scoring or raw-recording inspection in this study.
Earlier knowledge: #233's source map and presentation aggregates; the original
111 study's published summary, the nine-run fixture's schema/method/source names,
and existing tests. The historical summary already reports model comparisons;
therefore this is retrospective development analysis, not blinded confirmation.

## Pins and evidence split

- Simulator base: `4dd66ab00ab8b0acb2b9ea1432a43f9fd31a8196`.
- Client: `8d0772c015d6b9b73b80253db8c7349aa600b201`, clean external clone / git dependency.
- Canonical introduction: Arena commit `4252ca440850470e9c7a5b8f9b2d0162d36e078d`,
  `docs/movement-fidelity.md`, `tools/analyze-movement-fidelity.py`, `src/physics.rs`.
  Later retained version: `c2fb7285febb4273034e30699dc880715a334423`,
  `docs/research/movement-fidelity.md` and `tools/fidelity/analyze-movement-fidelity.py`.
- Development: all nine existing `tests/fixtures/movement-turn-runs.json` runs,
  plus their two complete retained raw sources:
  `run3-small-circle-20260927/bot0.jsonl.gz`, SHA-256
  `86143ad92a958552fa3865b623c55f533528fc5a8d759007c55d5662ad88666e`, and
  `run3-large-radius-20260927/bot0.jsonl.gz`, SHA-256
  `0317f6bbfb7075e019295727e41071e816799d4adbb97d4f6a06ce51748c2a94`.
  Use exactly the fixture's connection, snake incarnation and anchor-index bounds.
- No newly designated held-out set. These sources were historical validation
  of the original rule; their published outcomes and selected fixtures are already
  visible. New subdivisions within these same recordings do not create independence.
- No other retained source will be searched for a favorable interval. Missing
  exposure/fields remain missing. Full source recordings and all derived rows stay
  external; publish bounded aggregates, hashes and witness indices only.

## Candidates and predictions

Let `Q_D(x)=.033/.008 * (.13+.87*((7-clamp(1+(x-2)/D,1,6))/6)^2)` rad/s.
Predictions use announced coefficient .033; any different setup is reported and
excluded from this coefficient-specific comparison, not silently normalized.

1. D106: `Q_106(n)`; D111: `Q_111(n)`.
2. Fixed offset: `Q_106(max(2,n-k))`, individually report k=1 and k=5;
   additionally select one global integer k in 0..20 by minimum duration-weighted
   squared angular-rate error across the nine development runs, ties to smaller k.
   This is a descriptive development fit, never per-size tuning or validation.
3. Event phase: for D=106 and D=111 separately, compare count before an anchor,
   immediately after it, one anchor earlier and one anchor later. Stable counts
   make these candidates identical within each denominator. Do not infer a hidden
   7-ms tick phase from an anchor-lag sensitivity.
4. Count substitutions, separately labelled diagnostics: client's logical count,
   retained physical anchor count, dying count, physical minus dying, logical+fam,
   and displayed length. Dying count/fam are not standalone segment counts; report
   their values and test only the explicitly named composed substitutions.
   Record before/after packets and count ranges, never conflate retained points
   with authoritative physical membership.

Primary score: signed fixture angle / fixture midpoint-time duration and model
angle error over exactly that duration. Preserve every run, size/mode/direction,
maximum and duration-weighted RMSE. Independently replay raw traffic through the
pinned client's decoder/presentation and reconstruct the historical logical count
convention. If original fixture summaries cannot be reproduced, report the exact
discrepancy before using a new estimator. No simulator behavior change.

## Uncertainty and separation

Triplet/chord gates retain the original method: 39..45-unit chords, positive
adjacent server gaps <=700 ms, wanted margin >=.55, >=5 consecutive triplets.
The nine fixture runs are fixed selections; do not reselect them with new gates.
Adjacent triplets overlap; a sustained run/track is the unit, not each packet.

Use a conservative conditional angular budget
`A=2*asin(sqrt(2)/39)+2*pi/65536` for the two endpoint chord directions: one-unit
coordinate-cell uncertainty and a full directed-anchor angle quantum. Interior
chord errors telescope for consecutive signed changes. Report these quantizer
assumptions; they do not prove hidden official coordinates or encode policy.
Use timing half-width `B=.001+2*jitter_ms/1000`, with per-endpoint jitter
sensitivities 0, 7, 14 and 42 ms. Compare observed angle to each predicted
`Q*T` with `A+Q*B`. Also report the minimum timing jitter needed to admit a
candidate. Packet timestamps do not bound unknown event-to-send lag; no
unconditional timing exclusion or new physics truth follows from these bounds.

## Authored geometry and lifecycle probes

Use the pinned client on deterministic synthetic curved bodies with 8 and 37
logical anchors. Exercise mass-only, growth anchor, non-growth anchor, tail loss
and subsequent animation. Separately record logical count, physical count,
dying count, fam, displayed length and scang after each action. Compare
insert-then-contract (client) against contract-then-insert (simulator's existing
source order), using .43 contraction, 42 spacing and exact sequential neighbors.
Check point identities before comparing positions. This is authored geometry,
not online authoritative-body evidence and not a mechanics correction request.

## Capture power and proposed protocol (not approved)

For sizes 9,37,100,174,244,293, calculate pairwise constant-rate separation.
For two candidates require `abs(Qa-Qb)*T > 2*A+(Qa+Qb)*B` before claiming
disjoint conditional uncertainty intervals; round required durations up to five
seconds, capped at 120 s. Stable event-phase candidates have infinite duration
requirements because their predictions coincide. Assess growth/tail phase using
one-segment rate differences times 7 ms and contrast with quantization/time bounds.

Proposed acquisition remains two connections maximum, actor plus nearby observer,
both directions and boost/cruise plateaus, stable counts for >=1 s before/after
each scored arc. Aim for three separated arcs per size/direction/mode cell,
distributed across two development and two later held-out invocations; four
invocations of at most 20 minutes, no replacement of failed attempts. Freeze
final sizes/durations from the above separation calculation before acquisition.
No capture is authorized by this document or the current task.

## Resource and completion rules

One logical CPU, Cargo jobs/Rayon/test threads 1, serial build/harness work,
modest memory and external disposable targets removed after checks. Offline only;
no game sockets, official JS/assets, owner-client modifications, runtime changes,
or workspace dependency. A supported new mechanics correction is reported to the
owner instead of implemented. Publish one research PR with a plain changelog bullet.
