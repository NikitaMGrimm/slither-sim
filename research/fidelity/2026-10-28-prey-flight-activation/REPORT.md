# Prey flight speed and activation: retained evidence and power

**Keep the current mechanics. Propose no capture.** Five retained official
recordings reproduce the original flight and activation evidence, but do not
separate the two close affine flight laws or square half-widths 315 and 320 under
the frozen uncertainty budgets. The negative capture result concerns this
observation model; it is not a proof that every future instrument must fail.

The main results are:

- The original **81-report flight count and published scores reproduce; all 13
  packaged fixture cases match exactly**. Endpoint interpolation has **.042018 wire-speed MAE**, versus
  .382864 for the old two-speed rule. Its exact constants remain unidentified.
- Across the five sources there are **553 flight rows**; the stricter
  visible-isolation/fresh-anchor/square-membership cohort has **123**. The two
  affine predictions have **zero disjoint rows** even at quantizer-only bounds.
- There are **663 activation rows and 21 width disagreements**. Eighteen follow
  an observed full-report mode change, one does not, and two lack a preceding
  ordinary full report. **Zero** disagreements robustly separate the widths,
  even before allowing a previous decision cycle.
- The affine laws differ by at most **.00877 wire units** on [5.79,14], versus
  **.06758** combined quantizer-only envelopes. The width difference is **5 world
  units**, versus **12** combined coordinate-only envelopes.

No game-server connection, simulator execution, Rust build or client-port use
was needed. Runtime, fixtures, contracts and the owner's checkouts are unchanged.

## Scope, origin and freeze

[PLAN.md](PLAN.md) was committed before new scores or raw inspection at
**`f228e6c632290b53e85fd1383ebad66a16767f6e`**. The prior exposure listed there
includes published historical results and existing fixtures. All five recordings
are **development** for this retrospective analysis. Their original prospective
or validation roles are historical; no new held-out set or independent trial is
created by reanalysis or a within-recording split.

The method follows [#236's frozen plan](../2026-10-28-turn-count-phase/PLAN.md)
and #238's plan/report inspected at `f2b4d884835c31fba2dc2941e2291741c8fc8ecf`.
At inspection GitHub reported #238 **open**, rather than merged. Its branch was
read without incorporating its changes. Simulator base is
`f9154e92c76aaf904e1031ef44196ec52bb783d2`.

[Provenance](../../../docs/provenance.md) identifies the Arena import at
`aab1149460c0f5606fbdc7d73e295ba79d094789`.
[Server rules](../../../docs/server-rules.md#movement-and-geometry) identify the
retained research snapshot `c2fb7285febb4273034e30699dc880715a334423`.
Read-only Git inspection of that repository supplies the following lineage.
The historical papers/tools are local-only source artifacts, not public download
links. Their SHA-256 identities are retained in [selected-results.json](selected-results.json).

| Sub-model | Canonical introduction and study | Fixture / current owner |
| --- | --- | --- |
| Provisional roaming/flee steering | Arena `895e6f6b5b8c4d54aefbcfae4a16c8e6b17a18ac` initializes the existing policy; later `a87ecb2d218efc06f3400fcb99f5c99c4687ef78`, `docs/prey-flee-jitter-audit.md`, tests its residual support/distribution without adopting a new law. | No measured fixture identifies the random policy. `src/simulator.rs::decide_prey` retains flee noise uniform ±.3, roaming retarget probability .3 and offset ±1.2; these are provisional, not fitted official constants. |
| Per-prey held decisions | Arena `d7af9f8f1f58ba7ec52cec4e8c2123f1fb42c9c4`, `docs/prey-decision-cadence.md`, `tools/prey-decision-cadence.py`. | [prey_decisions.rs](../../../tests/prey_decisions.rs) is synthetic schedule/lifecycle regression, not a measured timer fixture. Production is `src/simulator.rs`; the named file is a test, not a production module. |
| Flight speed | Arena `c5515c5c48891d9cb3f744b30886b5a47f39f4ae`, `docs/prey-flight-speed-audit.md`, `tools/analyze-prey-flight-speed.py`; earlier `333fc16` removed unsupported transition lag. | [prey-flight-speed-fresh.json](../../../tests/fixtures/prey-flight-speed-fresh.json), [prey.rs](../../../tests/prey.rs); implementation [src/prey.rs](../../../src/prey.rs), selected threat in `decide_prey`. |
| Activation square | Arena `d7af9f8f1f58ba7ec52cec4e8c2123f1fb42c9c4`, `docs/prey-activation-shape.md`, `tools/analyze-prey-activation-shape.py`; `cdbfae560732b06c7c843a0fd26ee4cf9555a942` records the prospective boundary check. `docs/research/prey-width-refinement.md` preserves the failed 315 promotion. | Historical `tests/fixtures/prey-activation-shape.json` contains three measured category witnesses. It was **not imported** into SlitherSim. Current [prey_activation.rs](../../../tests/prey_activation.rs) has authored geometric cases; production checks are in `decide_prey`. |
| Sustained outward replay | Arena `d6ae672c537c31e1d07f1a0f84f7e4f9dbdb099c`, `docs/prey-radial-mode-audit.md`, `tools/analyze-prey-outward-motion.py`; `358eed9ddecaee7a01ae566814e98ed49164875e`, `docs/prey-pulse-onset-audit.md`, retains the onset counterexample. | [prey-outward-motion.json](../../../tests/fixtures/prey-outward-motion.json), [prey_outward.rs](../../../tests/prey_outward.rs); production explicit `PreyMode::Outward` / `set_prey_mode` and `decide_prey`. No identified automatic trigger. |
| Coordinate flooring, common to motion comparisons | Arena `581d4a9ea45c624ba9d46dac6c95d01ed649889e`, `docs/prey-state-audit.md`, `tools/analyze-prey-motion.py`. | [prey-motion-quantization.json](../../../tests/fixtures/prey-motion-quantization.json), `Prey::advance_server_tick`. |

The flight, outward and quantization fixtures and decision/activation/outward
tests entered SlitherSim at `ee259414fa8c62388ed43e28c638a0c9ee851cbe`.
[extraction.json](../../../docs/extraction.json) records their import/adaptations.
At the retained Arena pin, studies live under `docs/research/` and analyzers
under `tools/fidelity/`. No private raw recording, historical source file,
community-server material or official JS/assets is copied into this PR.

## Original conventions and remaining identifiability

Prey full `j` is **16 bytes including type**, called a 15-byte update in the
older notes. Direction is ASCII 0/1/2; current and wanted angles are full u24
fields scaled by `2*pi/16777215`, speed is u16/1000. Coordinate corrections use
`1+3*u16`, whereas appearances use u24/5. World velocity is wire speed/.032;
turn rate is .029/.008 rad/s. Seven-ms flooring is a separate motion operation.
These scales are not interchangeable clock or quantization conventions.

The offline decoder accumulates incoming elapsed-ms headers per connection,
with setup at zero, as did the original flight analyzer. Packet order is kept;
snake heads are last anchor positions advanced through **reported** angle/speed
history. This estimates a head, not hidden server geometry. Anchor age and speed
field age are distinct. Body contraction is disabled only in this analysis:
it changes neither stored head base nor angle/speed history; an authored
regression checks that invariance. No stock client is cloned or compiled.

Ordinary decision evidence historically favored holding source speed/target
until near the next report: 96 of 135 fitted transitions in the first cohort
put the switch in the last 0–28 ms. A nominal one-third-second per-prey timer
checked on 7-ms simulation ticks yields 336 ms. Observed ordinary reports
near 337–338 ms cannot identify a literal timer versus a tick counter, initial
phase, or event-to-send lag. Synthetic lifecycle tests cannot settle those.

The earlier fitted speed line is `F(x)=2.74227+.59025*x`. The production
approximation is endpoint interpolation
`A(x)=6.153+clamp((x-5.79)/8.21,0,1)*4.844`. The superseded control B uses
6.153 below x=13.5, otherwise 10.997. Historically these candidates were fixed
before the fresh archive's speed relationship was inspected, although that
archive had already been used for displacement. No coefficient is refitted here.

Square shape was fixed before the controlled recording; 315 was a later frozen
boundary alternative. Activation uses strict per-axis limits, **any** known
eligible head, then nearest eligible threat for flight speed. Filtering only the
nearest head first loses the diagonal second-head witness. Three historical
activation fixture cases reproduce exactly here, including all retained head
distances, coordinates, ages and speeds. The historical 315 promotion required
strictly fewer errors and sufficient isolated rest coverage; it failed with tied
mistakes and only four isolated resting packets. This failure is retained.

| Priority here | Remaining residual / observation | Identifiability and decision |
| --- | --- | --- |
| 1. Flight relation | Original .042018 MAE; other selected sources .02063–.10980; large conditional outliers remain. | Explicit prey speed is strongest observable. Study A/F and causal-history sensitivity; do not identify the selected snake from nearest-visible status alone. |
| 2. Activation boundary | Square320 full-source balanced accuracy 87.5–96.1%; five-unit boundary question remains. | Controlled rest/flight crossings support a shape comparison. Study 315/320 with transition and uncertainty denominators. |
| 3. Steering distribution / decision clock | Prior jitter supports roughly ±.26 on several cohorts, but first-per-episode and reserved samples weaken the arcsine preference; ordinary report spacing is not a hidden decision timestamp. | Defer distribution and timer replacement. Unknown head, head age and decision age confound small differences. No fitted residual justifies replacing the provisional random law. |
| 4. Outward onset / population / removal | Historical 159/160 explicit 6.060 targets are within 1° of radial outward; selected sustained-motion fixture errors stay within nine units. Trigger is unresolved, rather than a bounded fitted residual. | Defer trigger inference. Exact11 announcements can follow physical motion; `y` entries/removals are visibility events, and absent snakes/deaths cannot be treated as world absence. |

The task's approximately .27 flight residual is background also repeated by
[#233](../2026-10-27-client-view/REPORT.md#prey-and-the-quantized-motion-counterexample),
whose report expressly did not rerun that original estimate. It is not a
specified universal MAE or acceptance threshold. This study reports its own
explicitly selected errors, retaining 34 rows above .27, without equating those
metrics or claiming that residual was eliminated.

## Frozen retained-data scores

All five sources match their frozen hashes, finish gzip decoding and contain
capture-end. Full source digests are in the plan and selected JSON. In total:
120,055 incoming frames; 5,303 prey `j`; 1,075 with present speed, all of these
full-layout reports; 5,303 with present heading/target. Thus inherited speed
must not be counted as 5,303 fresh observations. Selection counts below remain
source-specific. Appearance episodes and packets are correlated.

Flight selection is the historical full-layout 6.14<=speed<11, nearest known
head <450 and wanted-away error <.5. The strict sensitivity additionally requires
anchor age <=150 ms, no second known head within 700 and the selected head in
square320. This is **visible isolation**, not proof of an isolated physical actor.

| Source (all `bot0`) | Flight rows / episodes | A MAE | F MAE | B MAE | Strict rows / A MAE |
| --- | ---: | ---: | ---: | ---: | ---: |
| fidelity-prey-transitions-20260927 | 81 / 26 | .042018 | .046051 | .382864 | 13 / .000000 |
| prey-controlled-20260927-first | 128 / 22 | .055211 | .057538 | .638266 | 34 / .066773 |
| prey-boundary-validation-20260927 | 178 / 37 | .061780 | .063999 | .567860 | 53 / .094958 |
| prey-width-confirmation-20260927 | 114 / 23 | .109797 | .110574 | .696140 | 17 / .095717 |
| prey-steering-validation-20260927 | 52 / 11 | .020630 | .025125 | .197308 | 6 / .000050 |

The original source's ten same-prey/same-snake changed-speed pairs reproduce
the median change ratio **.573498**, and A's delta MAE **.213381**. Other sources
have 25/45/17/2 pairs, with median ratios .591626/.585200/.585871/.538187.
These support a continuous relationship over B, not precise affine constants.
F has slightly better RMSE than A in four sources despite worse MAE in all
five; selecting by point-score preference alone is unjustified. Both fit within
the same frozen uncertainty for all 123 strict rows at j=42 ms. All 13 strict
original-source rows are endpoint observations, so their zero A error supplies
no fresh intermediate-speed identification.

Every selected row and outlier remains external. The bounded JSON keeps each
source's maximum witness (connection/appearance/ID/server-time only). A's maxima
are .92104/.94336/1.99227/4.75288/.33592 wire units. For example, width-confirmation
connection1, prey20867, appearance1 at S=48446 reports prey10.997 against the
nearest known snake's 5.94444, last speed report 149 ms old. It fails the strict
cohort. This is a conditional association error, not evidence of wrong prey
velocity: another threat, stale state or changed threat identity remains possible.

The causal-history sensitivity carries the last reported snake speed at the
left edge of the preceding 336 ms and includes subsequent causal reports of
that same incarnation. It covers **541/553** rows, with 12 lacking left-edge
history; lifecycle/connection/setup boundaries reset it. Distance to A's history
envelope has source means .03008/.04684/.05205/.06726/.01732, compared with the
current-speed MAEs above. **101** eligible rows remain outside its quantizer-only
budget. This is not a recovered delay or exhaustive continuous speed envelope.
It neither explains all residuals nor identifies when/which snake the prey used.

Activation selection keeps ordinary full rest/flight reports, nearest known head
<=650 and anchor age <=150 ms. Both square candidates receive exactly the same
rows. The original published controlled, boundary, width and steering counts
and balanced accuracies reproduce.

| Source | Rows / episodes | Circle300 BA | Circle415 BA | Square315 BA | Square320 BA | 315 / 320 total errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| transitions | 113 / 30 | 69.64% | 66.58% | 95.30% | 95.36% | 6 / 4 |
| controlled | 164 / 22 | 69.91% | 71.40% | 96.83% | 95.54% | 5 / 5 |
| boundary validation | 220 / 41 | 75.34% | 72.64% | 96.96% | 95.89% | 9 / 9 |
| width confirmation | 101 / 23 | 72.09% | 78.84% | 98.84% | 96.09% | 2 / 2 |
| steering validation | 65 / 13 | 68.75% | 66.25% | 93.50% | 87.50% | 4 / 7 |

The isolated boundary subset instead favors320: **98.18% versus97.27%**, 89 rows
across21 episodes. The isolated width-confirmation subset has19 flight/4 rest
rows, preserving the old coverage failure. Steering-validation's65 rows have
seven square320 mistakes, rather than being hidden in a favorable pool.
Episode-balanced and first-per-episode scores and every confusion count are in
selected JSON; none supplies independent replication or promotes315.

## Uncertainty, conditional separation and power gate

The plan uses full quantizer cells because encoder rounding policy is not proved.
For speed, `E=.001+L*(1/18+16*(.001+2*j/1000))`, with L=.59025 and j=0/7/14/42
ms. Sixteen wire units/s is an assumed ordinary snake ramp ceiling, not a fitted
prey rule or measured service-age bound. Quantizer-only E is .033792; combined
2E is .067583, growing to .08647/.35090/.61534/1.67306 with timing. These are
conditional deterministic budgets, not confidence intervals. Unknown selected
threat and decision age are not bounded by packet timestamps.

Activation uses per-axis coordinate slack6, age-dependent snake heading/speed
extrapolation slack and relative-motion timing slack. A separate sensitivity
adds336 ms of decision age and48 per-axis flooring units. This tests labels over
coordinate boxes for **known heads**; it never converts a missing threat into
known physical absence. Even the j=0 snapshot has **zero robustly disjoint**
315/320 predictions among21 disagreements. At j=42 plus decision age, square320
has112/162/218/99/65 ambiguous rows in the five source cohorts, respectively.
The few robust labels are not a new exact activation law.

The capture gate asks what a **two-connection** actor/observer experiment would
need, before requesting any collection:

| Question | Required observation improvement | Frozen gate outcome |
| --- | --- | --- |
| A versus F | Maximum separation .00877; snake-speed full-cell uncertainty must be **below .005735**, after prey .001 uncertainty, with no remaining age/selection error. | Existing snake motion-speed cell is .055556, about9.7 times too wide. More reports do not average away that systematic bound. No passing protocol with the frozen reported-speed observation model. |
| 315 versus320 | Per-axis geometric half-width **below2.5**, including coordinates, extrapolation, flooring and decision age. | Coordinate allowance alone is6. With maximum relative wire speed25, timing alone would need to be below **3.2 ms** before paying any coordinate cost; a seven-ms decision phase already exceeds that. No passing boundary-crossing protocol with the frozen model. |

A longer plateau can reduce random errors and permit an independent snake-speed
trajectory estimator, but no such estimator with the required bound is validated
here. It would also need uninterrupted threat identity and prey flight exposure;
two observed connections do not control the prey or exclude unseen snakes.
Appearance speed's .001 quantum is finer, but an entry timestamp does not provide
a fresh measured speed at each later prey decision. Assuming a precise plateau
from repeated identical bytes would discard the uncertainty being tested.

A slower crossing or repeated crossings cannot by itself reduce the coordinate
cells below2.5, expose hidden decisions, or bound unobserved threat changes.
Smooth prey trajectory fitting is particularly unsuitable: each floored seven-ms
step can contribute a one-unit axis difference, and those differences need not
average to zero. [#233's witness](../2026-10-27-client-view/REPORT.md#prey-and-the-quantized-motion-counterexample)
shows37.371 units of smooth-versus-floored difference in335 ms under the existing
model. Total correction distance is not the flight/activation score.

**No capture is needed to decide the present comparison, and none is proposed.**
A two-connection capture does not pass the frozen gate without first establishing
a better independent speed/geometry/timing estimator. Feasibility beyond this
measurement model is unestablished; no universal impossibility is claimed.
Consequently there is no collection protocol to approve and no collection
authorization is implied. No newly supported mechanics correction was found.

## Reproduction, checks and evidence limits

[analyze.py](analyze.py) and [test_analysis.py](test_analysis.py) use standard-library
Python with the repository's locked uv environment. Both executable entry points
pin themselves to one available logical CPU; processing is serial and streaming.
The run used Python3.12.9. No Cargo target directory was created, so none required
removal. Full recordings and all four analysis attempts/derived rows remain
outside Git; only the latest bounded aggregate JSON is published. The first three
attempts were successful development runs before completing source/fixture audit
metadata and rejection checks; their scores were unchanged. No thresholds or
candidates were retuned.

```powershell
$study = 'research/fidelity/2026-10-28-prey-flight-activation'
# $archiveRoot is the read-only Arena Git repository with recordings/headless/.
# $external is a new directory outside tracked trees; existing output is refused.
uv run --locked --only-group tools python -B "$study/test_analysis.py"
uv run --locked --only-group tools python -B "$study/analyze.py" --archive-root "$archiveRoot" --output "$external"
```

The entry point verifies the committed plan's exact bytes, all five archive
hashes before decoding, terminal completeness, source artifact hashes and fixture
reproduction. It writes full derived rows only to the external directory.
Selected JSON contains code/test/decoder/fixture hashes, historical artifact
hashes and bounded per-source aggregates; it contains no endpoint, nickname,
machine/account detail or raw packet bytes.

Completed checks: ten authored regressions covering speed/control units,
strict regions/any-head eligibility, interval boxes, missing classes and episode
weighting, causal history, body-contraction head invariance, present-field
selection, missing/mismatched archive rejection, terminal receipt rejection,
fixture mismatches and power arithmetic; complete five-source replay; exact
13-case public flight fixture and three-case historical activation fixture
reproduction; documentation navigation, LF/diff and publication privacy checks.
No simulator runtime test group, client/browser/renderer acceptance or live-server
validation was run. No independent child review was used for this sub-agent task.

The selected source hashes are integrity pins, not evidence of independent
trials, causal isolation, current server-version stability or statistical power.
The public aggregate is reproducible only with access to the original retained
recordings; fixture/synthetic checks alone do not reproduce the complete corpus.
