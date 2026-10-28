# Frozen offline prey flight and activation investigation

Commit this plan before any new candidate scores or raw-recording inspection.
Prior exposure includes #233's prey/flooring witness, #236's plan/report, #238's
plan/report at `f2b4d884835c31fba2dc2941e2291741c8fc8ecf` (still open when
inspected), the existing flight/outward/quantization fixtures, and the historical
flight, activation, cadence, jitter, width and radial study summaries/analyzers.
Published historical outcomes are known; this is retrospective development.

## Pins, selection and split

- Simulator base: `f9154e92c76aaf904e1031ef44196ec52bb783d2`.
- Historical Arena source pin: `c2fb7285febb4273034e30699dc880715a334423`.
- Flight introduction: `c5515c5c48891d9cb3f744b30886b5a47f39f4ae`;
  activation/decision clock: `d7af9f8f1f58ba7ec52cec4e8c2123f1fb42c9c4`;
  outward replay: `d6ae672c537c31e1d07f1a0f84f7e4f9dbdb099c`.
- Use only these complete official recordings, each `bot0.jsonl.gz` under the
  historical `recordings/headless/` root, verifying SHA-256 before decoding:

| Source | SHA-256 |
| --- | --- |
| fidelity-prey-transitions-20260927 | 282834351bce3a5e9c5bdf342b179a6ea1634eacca450b98c7924a52c7cf669a |
| prey-controlled-20260927-first | c65f2d0b248f851743963b067dc53767028eb63d2709c72f35766b4d3c45ba7e |
| prey-boundary-validation-20260927 | 264b132056f5b4527691955b08770e24daf36a86bb651860236a3ecf0d9d799f |
| prey-width-confirmation-20260927 | ff0aebb9b7b66f9e9773416e14a9328d7edbd7af7dfc785f890bd5dbe51cdcb9 |
| prey-steering-validation-20260927 | b7630b155d374914bb7e9497f5af602184e08339f1334c2e755f801970bf4248 |

All five are development here. Preserve their original prospective/validation
roles when describing history; no fresh held-out data is designated. Do not
search additional archives for favorable rows. Missing or mismatched archives
remain explicit failures. No community/private recording or full derived row is
published. Read-only historical Git inspection is permitted; no historical
checkout modification or execution of its collectors.

Use the current offline Python protocol decoder, hash it, and compare the
historical decoder/source conventions. No stock-client clone or Rust build is
needed. Complete gzip and capture-end are required. Reset state on connection,
setup and lifecycle boundaries. Preserve raw packet order and per-connection
accumulated server elapsed-ms time. Reproduce the historical 81 flight rows and
compact flight fixture before interpreting new scores; report discrepancies.

## Ranked scope and frozen candidates

1. Flight speed is directly present to .001 wire units in full prey `j` packets;
   the causal snake is inferred. Study the residual and the two near-identical
   historical affine laws, with the superseded two-speed law as a control.
2. Activation has explicit rest/flight labels and prior controlled crossings.
   Study the unresolved 315/320 boundary with old circles as shape controls.
3. Defer steering jitter/distribution and the exact hidden timer: heading is
   present, but the true threatening head/decision instant is unobserved; prior
   episode-level results weaken packet-wise distribution preferences.
4. Defer outward onset, population and rim deletion: radial target/sustained
   motion are supported, but appearances/removals do not identify births/deaths,
   the exact-11 announcement is not a physical onset timestamp, and missing
   snakes prevent causal attribution. Replaying known outward clips cannot
   identify their trigger.

Let x be snake wire speed and y prey wire speed (world speed = wire/.032).
Freeze A(x)=6.153+clamp((x-5.79)/8.21,0,1)*4.844;
F(x)=2.74227+.59025*x; B(x)=6.153 for x<13.5, otherwise 10.997.
No refitting, new width, or optimized lag. Score A/F/B on all historical eligible
flight rows: full 16-byte `j`, explicit 6.14<=y<11, nearest visible head distance
<450 and wanted-heading away error <.5 rad. Preserve this historical filter even
when its nearest head is outside the square. Separately report stricter rows:
anchor age <=150 ms, no second known head within 700, chosen head inside square320.
The last condition is a sensitivity cohort, not a new authoritative label.

Primary scores: MAE, RMSE, signed mean, p90/max absolute error, by source and
all/strict cohort; track/appearance counts; same-snake speed-change pairs
(adjacent selected rows, gap in (0,2000) ms, |snake delta|>=1). Report denominators
and every outlier. A causal-history sensitivity compares A/F at the current
reported speed with the range of that same snake's reported speeds in the prior
336 ms (including the carried initial value); exclude history gaps/life changes.
This envelope is not a fitted delay or proof of the authoritative speed history.

Activation: full `j`, y=3.8 or 6.14<=y<11, nearest visible head <=650 and last
anchor age <=150 ms. Predict flight if ANY known head is strictly inside circle
300, circle415, square315 or square320. Select eligible heads before choosing
nearest. Exclude exact11, 6.060, appearance/compact inherited-speed labels.
Report confusion counts, packet balanced accuracy, episode-balanced accuracy,
first-per-episode and isolated subset (second known head >700). Preserve absence
of either class as null, not zero. For every width disagreement, report whether
the previous explicit ordinary full report in the same episode changed mode;
do not select only settled or correctly classified rows.

## Uncertainty and separation

No packet, episode or concurrent observer is an independent trial by default.
No statistical confidence/power percentage is justified. These are conditional
worst-case sensitivity budgets, not measured server tolerances or timing bounds.

- Prey coordinate updates lie on a 3-unit grid. Motion floors each coordinate
  every 7 ms; smooth displacement is not the reference. A previous-state
  reconstruction must allow one unit per axis per modeled tick for flooring.
- Prey angle quantum is 2*pi/16777215; ordinary snake heading quantum is
  2*pi/256. Preserve present fields rather than treating inherited state as fresh.
- Full prey speed quantum is .001; snake motion-speed quantum is 1/18,
  appearance speed quantum .001. Use full cells, not an unproven rounding policy.
- For speed scoring use E=.001+L*(1/18+16*(.001+2*j/1000)) with
  L=max(4.844/8.21,.59025), j=0,7,14,42 ms per endpoint. Sixteen wire units/s
  is an explicitly assumed conservative ordinary snake ramp bound, not a new
  empirical prey law. Also show quantizer-only E=.001+L/18. Require prediction
  distance >2E for disjoint envelopes. Report both incompatibility and missing
  causal history; unseen threats and unknown decision age remain unbounded.
- For activation snapshot coordinates allow per-axis U=6 plus extrapolation
  error `age_s*(snake_sp/.032*(2*pi/256)+(1/18)/.032)` and relative motion
  `(snake_sp+prey_sp)/.032*(.001+2*j/1000)`. Use j=0,7,14,42. Separately
  add one 336-ms decision-age sensitivity, relative motion and ceil(.336/.007)
  flooring units per axis. For each model report robust correct/wrong/ambiguous
  labels over these boxes; robustness concerns KNOWN heads only. Do not classify
  an absent known threat as evidence of world absence. Circle bounds use the
  minimum/maximum Euclidean distance over the same coordinate boxes.

## Capture power gate

Compute maximum A/F separation over x in [5.79,14] (affine endpoints suffice),
compare with 2E, and compute the snake-speed precision needed after prey .001
quantization. For width315/320, compare five units with 2U, first with only
coordinate cells and then timing/flooring; repeat count cannot erase systematic
bounds. Test whether a long two-connection controlled plateau or crossing can
actually improve the governing bounds: one actor and one observer, no direct
control of prey or other snakes. A longer smooth trajectory cannot by itself
remove unknown prey decisions, threats or per-tick flooring. Distinguish failure
of this observation model from a theorem that every possible design fails.

If a feasible protocol passes these gates, freeze its duration, exposure gates,
directions/speeds, attempt cap and separate development/held-out allocation in
the report, marked **not approved**. Otherwise explicitly propose no capture.
Never collect during this task. A newly supported mechanics correction is a
stop-and-report result; do not change the simulator.

## Delivery and resources

One logical CPU, serial analysis, modest RAM; no sockets or simulator runs.
No runtime, fixture, contract, workspace dependency, owner-client or Arena edits.
Keep full input/derived rows external. Publish only bounded sanitized aggregates,
hashes and source pins. Research-only PR with this committed-first plan, report,
scripts/focused tests, selected JSON, one research index row and one Added line.
Record the plan commit and source/tool hashes. Conventional Commits and LF.
