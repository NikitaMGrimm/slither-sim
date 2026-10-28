# Boost speed ramp: retained evidence and capture power

The original **1,567 constant-size ramps reproduce exactly** from their three
retained recordings. The current law's slope RMSE is **0.10137 wire-speed/s**,
versus 0.14529 for its former empirical approximation. The .305 law is favored
over the tested alternatives; this does not identify its coefficient or the
4/3 ratio as literal closed-server constants.

There are **387 additional size-crossing intervals**, containing 691 count
events. Normalized progress and direct speed differ by at most **0.00930 wire
units**, below one **0.05556** speed byte even before timing uncertainty.
None of these intervals separates their predictions under the frozen budgets.
Old/new-size sampling is unresolved too.

**No mechanics correction is established. No capture is proposed.** Monotone
ramps are underpowered for latent-state identification at the retained wire
precision. Longer partial cycles fail the frozen input-timing power gate even
under unrealistically favorable continuous feeding. This is a negative power
result for the stated observation/control model, not proof that every possible
future instrument or experiment must fail. No game connection was opened;
simulator, fixtures, contracts and the owner's client checkout are unchanged.

## Canonical origin and freeze

[Provenance](../../../docs/provenance.md) records the Arena import at
`aab1149460c0f5606fbdc7d73e295ba79d094789`;
[server rules](../../../docs/server-rules.md#movement-and-geometry) point to the
retained research snapshot `c2fb7285febb4273034e30699dc880715a334423` and explicitly
limit its 1,567 ramps to fixed-size slopes. Following that history identifies:

| Artifact | Canonical introduction |
| --- | --- |
| .305, .015/.02 and nominal clock law | Arena `91c52fa5396c5a0afba632e60ca8bb8c9a84aa28`, September 28, 2026, `fix: derive boost acceleration from validated progress rule` |
| Study and analyzer at that commit | `docs/speed-equations.md`; `tools/analyze-speed-equations.py` |
| Original selected regression | `tests/fixtures/speed-ramp-validation.json`, added in the same commit |
| Latent-state implementation, separately | Arena `b166d5d3f0a328e3c62f5bd23cc125dcf39f7d82`, `feat: retain cruise speed through idle growth` |
| Later retained study/analyzer | `docs/research/speed-equations.md`; `tools/fidelity/analyze-speed-equations.py` at `c2fb7285febb4273034e30699dc880715a334423` |
| SlitherSim fixture import | `ee259414fa8c62388ed43e28c638a0c9ee851cbe`; current [fixture](../../../tests/fixtures/speed-ramp-validation.json), consumed by `independently_recorded_speed_ramps` in [physics.rs](../../../tests/physics.rs) |

This was **adoption of an independently sourced fixed candidate**, rather than
a new fit of .305 to these recordings. The earlier empirical approximation was
`15.45/(1+.39*(scale-1))` rising, multiplied by -4/3 falling. The Android 3.06
offline mechanism supplied the replacement. Its historical APK SHA-256 is
`b581a1bfb9fc1951d5b03cf4d2cd844f6e559012595b9e803554deb7625e7917`.
The historical audit checked source and bytecode; this study inspected retained
Git notes/analyzers, and did not download, execute or freshly decompile that APK.
Its offline endpoint undershoot was never established online.

[PLAN.md](PLAN.md) was committed **before new scoring/raw inspection** at
**`3b4266150c7c4ab26ffc9583c957a391abb9820b`**. Prior exposure to the historical
summary and some fixture samples is declared there. Its candidates, cohort,
selection and budgets remain unchanged. All sources are **development** here:
historical separate-source validation remains historical, and no post hoc split
of already published recordings is called held-out.

Simulator base is `ffa6e032` (full identity in selected JSON). The stock-client
comparison [#233 formula map](../2026-10-27-client-view/FORMULA-MAP.md) establishes
that reported speed drives client travel; .1/.3 visual easing is presentation.
The [#236 plan](../2026-10-28-turn-count-phase/PLAN.md) supplied the study template.
This analysis needs no Rust client build, clone or workspace dependency.

## Original units, clock, count and selection

Let `v` be wire speed: motion packets report byte/18, while appearance speed uses
u16/1000 and is excluded as an interior ramp sample. World velocity is `v/.032`.
For decoded live-anchor count n, with fam separate:

```text
s = clamp(1 + (n-2)/106, 1, 6)
b = 5.39 + .4*s
dq/dt = +(.015/.008)/(1+.305*(s-1))  rising
dq/dt = -(.02/.008)/(1+.305*(s-1))   falling
v = b + (14-b)*q
```

The 8 ms reference is distinct from the approximately 7 ms server loop and
the .032 velocity conversion. The Android offline clock multiplies by 1.3;
the adopted online candidate removes that factor. Original slope scoring uses
the connection's accumulated incoming elapsed-ms headers, with setup at zero.
Unlike the older turn study's introducing reader, the speed study's reader
already reset setup's initial epoch. No fitted timestamp offset was necessary.

The historical analyzer feeds a packet before reading `snake.sct`, the length of
its live decoded anchor list. Appearance/removal/reconnect and **every count
change** split a series. Changed speed reports form maximal same-sign runs,
with positive adjacent gaps <=70 ms; first and last reports are trimmed,
then >=4 points are required. Matching setup speeds are 5.39/.4/14. Completed
archives require capture-end. Count is constant inside all selected controls,
so old/new-size phase is unobservable there. All 1,567 are opponent ramps:
their held boost inputs are unknown. A monotone run is an observed label.

The historical discovery source `run1-boost` had 902 ramps and an incomplete
archive admitted by an old override. It is excluded here. The three completed
checks below were preselected originally; all their intervals are retained here.
The prior RMSE near 0.1 is a **slope error in wire-speed/s**, not a speed error
of 0.1 wire units. The 2,469 historical discovery+check ramps must not be
confused with the 1,567 packaged validation ramps.

## Reproduction and fixed-size controls

The complete archives contain 67,339 incoming frames and 20,768 changed speed
reports. The current offline decoder reconstructs **exact per-interval metadata,
timestamps and speed samples** for every fixture interval; comparison sorts by
connection/life/start because flush order is not interval identity.

| Source | Ramps | Tracks | Current RMSE | Former empirical RMSE |
| --- | ---: | ---: | ---: | ---: |
| run3-large-radius-20260927 | 343 | 40 | .09953 | .17056 |
| run3-single-20260927 | 203 | 35 | .10759 | .12598 |
| visibility-covariates-20260928 | 1,021 | 170 | .10071 | .13953 |
| Combined, 2..343 segments | 1,567 | 245 | .10137 | .14529 |

Selected duration totals **525.644 s**; there are **838 rises and 729 falls**.
Tracks are received incarnations, not independent players; ramps within a track
and recordings are correlated. Duration-weighted current RMSE is .08205.

For OLS weights w, the original half-cell slope bound is `sum(abs(w))/36`.
It excludes 21 current-law ramps versus 148 empirical-law ramps. Since the actual
encoder rounding policy is not established, the frozen conservative bound uses
`sum(abs(w))/18`, plus `abs(predicted_rate)*sum(abs(w))*(.0005+j/1000)` for
per-report jitter j. These sensitivity bounds are not confidence intervals or
established event-to-send age limits. They assume a coherent accumulated clock;
independent header drift/truncation is not measured here.

| Candidate | Slope RMSE | Outside conservative bound, j=0 / 7 / 14 / 42 ms |
| --- | ---: | --- |
| Current .305, ratio 4/3; P305 and V305 | .10137 | 4 / 0 / 0 / 0 |
| No size denominator, P0 | 3.53177 | 1,004 / 761 / 601 / 221 |
| .39 in progress denominator, P390 | .59851 | 611 / 204 / 90 / 0 |
| Equal rise/fall rates, ratio 1 | 2.93335 | 732 / 729 / 656 / 144 |
| Offline 1.3 clock | 4.53436 | 1,567 / 1,558 / 1,213 / 113 |
| Former empirical law | .14529 | 20 / 1 / 0 / 0 |

At j=7 ms, every fall disagrees with the equal-rate candidate while every current
prediction is admitted. At j=42 ms, .305 and .39 both remain admitted throughout;
the gross no-size, equal-rate and offline-clock alternatives still fail some
controls. These are **conditional** distinctions. They do not fit an exact beta
or exact ratio, test all nearby laws, or establish a hard timing bound.
P305 and V305 are algebraically identical at constant size. Their identical
control scores carry no information about latent progress retention.

## Growth/loss coverage and unresolved state

The second selector uses the same sign, gap, endpoint and minimum-point rules,
but preserves count changes inside a lifetime. It requires a count event
strictly inside the retained report times and retains all intervening events
in packet order, including events sharing an endpoint timestamp. Fam-only
updates do not count. Appearance/removal/setup/reconnect still split history.

| Source | Intervals | Growth / loss / mixed | Count events | Duration s |
| --- | ---: | --- | ---: | ---: |
| run3-large-radius | 43 | 26 / 11 / 6 | 60 | 18.074 |
| run3-single | 50 | 40 / 8 / 2 | 82 | 20.134 |
| visibility-covariates | 294 | 230 / 36 / 28 | 549 | 117.679 |
| Combined | 387 | 296 / 55 / 36 | 691 | 155.887 |

There are **223 rising and 164 falling** intervals, 142 received tracks, sizes
2..273, at most five count events per interval, and maximum duration .807 s.
They overlap the same recordings as the controls and are not an independent test.

Each prediction initializes from the first retained speed/count once, then
integrates the frozen law through every count event. P305 retains q and refreshes
speed from size while q moves; V305 integrates speed directly. There is no
per-event reinitialization, parameter fit or selected favorable residual.

For same-timestamp packets, prediction compares `(server_ms, packet_ordinal)`:
a count after a speed report cannot affect that report, while a count before it
is already effective. [PLAN.md, candidates 1 and 7](PLAN.md#exact-competing-predictions)
state `v=b(n)+g(n)*q` and size effective "immediately at its received event";
candidates 3..5 inherit the progress rule. We therefore recompose speed on a
count change at interior `0<q<1`, without advancing q even if elapsed time is
zero. V305 retains direct speed, idle q=0 retains stored speed, and full q=1
remains 14. Ties introduced by shifted events also keep original ordinals.
The plan does not explicitly spell out zero-elapsed packet ties; this is the
immediate-event interpretation used here, not an amendment to its literal
wording or a newly established authoritative server phase.

| Model | Mean interval speed RMSE | Maximum speed error |
| --- | ---: | ---: |
| P305, normalized progress | .02834 | .83717 |
| V305, direct speed | .02842 | .83679 |
| P0 | .47216 | 2.98567 |
| P390 | .12381 | .67398 |
| Equal rates | .40228 | 1.96470 |
| Offline 1.3 | .87010 | 2.37520 |

The **maximum P305/V305 prediction separation is .0092992**, only **0.1674
speed bytes**. Zero intervals separate them at any frozen sensitivity, including
j=0. The small RMSE difference is not evidence for retaining q.

The cross-size budget is `2/18 + max_abs_rate*(.001+2*j/1000)` for a speed
difference from the initial report. Count events shift jointly earlier/later by
j+7 ms; scored points within j+8 ms of an event are excluded. Nominal/early/late
predictions define a sensitivity envelope, not exhaustive independent hidden
event-phase uncertainty. There is no claim that arbitrary input reversals,
report age or a literal authoritative tick are observed.

Both P305 and V305 have **9 / 2 / 1 / 0** incompatible intervals for
j=0/7/14/42 ms; exposure is 387/387/387/**376** intervals. Eleven intervals
lose all scored points at 42 ms and remain uninformative. The two j=7 failures
are retained in selected JSON: a 7..8-segment rise, maximum error .41024, and a
76..77-segment mixed fall, maximum error .83717. Both candidates miss the same
cases, so these residuals do not identify a correction to the growth rule.

For old/new-size sensitivity, shifting every count's effect by one 7-ms loop
changes P305 predictions by at most **.0039347** wire units; the direct-speed
analogue and both delayed-model scores are retained in JSON. This comparison
cannot identify which size the authoritative server samples. The current
simulator samples speed before boost loss and anchor growth; that source-order
fact from #233 remains a model boundary, not a newly measured server phase.

## Negative capture-power result

These are authored calculations, not captured data. Legal growth adds at most
one segment per 42-unit anchor. Even unlimited pending nutrition cannot make a
large instantaneous count jump. Travel is at most 437.5 world units/s.
The largest normalized full-rise duration over scales 1..6 is **1.34667 s**;
full falls take 3/4 as long. Allowing the size to grow during the rise still gives
at most **15** anchor crossings, including favorable initial anchor phase.
At most `.4/106` cruise change per segment gives a universal latent-state
speed-effect ceiling of **.05660** over one monotone ramp. That is below the
**.11111** single-prediction difference budget, and below the **.22222** combined
two-candidate budget, even at ideal timing. Size saturation only reduces power.

| Initial count | Constant-size rise / fall s | Growth-count ceiling allowing ramp lengthening | Refresh-effect ceiling |
| --- | --- | ---: | ---: |
| 2 | .53333 / .40000 | 6 | .02264 |
| 9 | .54408 / .40806 | 6 | .02264 |
| 37 | .58704 / .44028 | 7 | .02642 |
| 106 | .69293 / .51970 | 8 | .03019 |
| 214 | .85867 / .64400 | 10 | .03774 |
| 343 | 1.05663 / .79247 | 12 | .04528 |
| 430 | 1.19014 / .89260 | 13 | .04906 |
| 500 | 1.29756 / .97317 | 14 | .05283 |

Counts above 343 are extrapolated power calculations, not validated captures.
Growth ceilings use an analytic upper bound on ramp lengthening, not the
initial-count duration alone. Normal loss is much slower: the current mass law
charges at most .0812 times size decay and <1.08 jitter per .155-s drop. Even
the deliberately excessive allowance of **one segment lost per drop** gives
only 4..9 losses over these nominal full ramps. A single old/new-size event has
speed-effect ceiling **.004252**, including one tick's slope change. Summing a
generous 15-event monotone phase bound gives **.010945**, also below one byte.

No finite count of monotone intervals removes this bounded uncertainty without
an independently justified quantizer/noise/timing model. This does not assume
that deterministic speed bytes become IID noise through repetition.

Could partial boost cycles accumulate enough growth while avoiding q=0/1?
The frozen calculation tests **72 schedules**: eight initial sizes, q bands
.2..4/.4..6/.6..8, growth, normal loss, and generous one-loss-per-drop variants,
each lasting 30 s. They use 1-ms authored integration, anchor-limited growth and
unlimited feeding as a favorable ceiling. This does not establish that enough
nutrition could actually be acquired. Normal loss uses the existing mass law
with maximum jitter and initial spent-fam zero; it is an illustrative schedule,
while the generous loss family provides the upper sensitivity.

At q>=.2, overcoming just the combined .22222 byte budget requires at least
**74 growth events**, or at least **7.104 s** even at unattainably favorable
full-boost travel throughout. Monotone ramp duration cannot supply that count.
Cycles can: their largest ideal growth effect is .53194 over 202 events.
But every binary boost transition has uncertain effective timing. The frozen
bound accumulates `(rise_rate+fall_rate)*j` per transition, alongside observation
timing. It is conservative and can become wider than the speed range; such a
gate is uninformative, not evidence of a measured giant error.

**Zero of 72 schedules passes at j>=7 ms.** Twenty-seven pass only at ideal
j=0, including nine deliberately excessive loss schedules. The most timing-
tolerant growth case starts at 343, reaches scale saturation at 532, has
189 growth events and 141 input transitions in 30 s. Its ideal separation is
.49806; its maximum admissible per-transition timing uncertainty is only
**.14095 ms**, even granting unlimited nutrition. Neither the retained data nor
the proposed acquisition controls establish that precision; one ordinary loop
is approximately 7 ms. Normal-loss schedules never pass even at j=0.

The power gate therefore supplies **no qualified capture protocol**. Collection
is unnecessary to repeat already informative coarse slope controls, and the
latent-state/phase question has no feasible protocol under this frozen bounded
timing model. No capture is authorized or requested. This conclusion does not
rule out future geometry-based instruments or a separately demonstrated input
timing model; those would need their own power justification before collection.

## Artifacts, failures and reproduction

Full recordings, derived rows and every intermediate run remain **local-only**,
outside Git in the task's `boost-ramp-20261028-*` evidence directories. These
historical game observations are not redistributed as raw frames; only bounded
aggregates, hashes and sanitized packet-index witnesses are committed. Hashes
identify the retained material without granting download access or proving
scientific claims. [selected-results.json](selected-results.json) contains all
three archive hashes, simulator/plan/source pins, analyzer/test/decoder hashes,
aggregate scores, failed-window summaries and external artifact digests.

Retained authoring failures/corrections:

- The first unit-test attempt compared two falls after both had saturated.
  The regression now compares an interior point; the final suite passes.
- Run 1 stopped on exact list-order comparison in `run3-single`: 203 intervals
  matched exactly after identity sorting. No timestamps or samples changed.
- Run 2 initially omitted count events sharing an endpoint timestamp. Review
  found and corrected that predictor state loss before acceptance; a synthetic
  lifecycle/endpoint regression was added. The final result has 691 events,
  rather than the intermediate 678; interval selection remains 387. Both
  intermediate results remain outside Git.
- The power calculation was tightened to allow growth to lengthen a rise;
  using only its initial-size duration would not be a valid growth ceiling.
- A subsequent review found that prediction discarded retained packet ordinals
  and deferred normalized-progress recomposition to positive elapsed time.
  Both defects are corrected as described above. Four new trajectory regression
  methods cover count packets before/after same-timestamp reports, both ramp
  directions, growth/loss, all scored candidates, shifted ties and endpoints.
  The previous predictor fails the new ordering regressions; the corrected
  predictor passes. The complete three-archive analysis was regenerated in a
  fresh external output directory; prior outputs remain retained.

The packet-order correction changes the following point scores, with **no
conclusion flip**:

| Quantity | Before correction | After correction |
| --- | ---: | ---: |
| Size-crossing intervals / count events | 387 / 691 | 387 / 691 |
| Maximum P305/V305 separation, wire units | .009299231468 | .009299231468 |
| Same separation / one speed byte | .167386166431 | .167386166431 |
| P305 mean interval RMSE | .028332247548 | .028336590280 |
| P305 immediate/delayed maximum separation | .004799031127 | .003934720572 |
| V305 mean interval RMSE | .028422602970 | .028422602970 |
| Capture schedules / passing at j>=7 ms | 72 / 0 | 72 / 0 |

All fixed-size scores, extracted input-row hashes, compatibility gates and the full
power result remain unchanged. The speed-byte comparison is still under a
fifth of a byte; no capture is proposed and no mechanics correction is supported.
The owner's changelog and research-index wording from `f2b4d884` is preserved:
none of the numbers or comparisons in those entries changed.

Reproduce with access to the three pinned archives, using a fresh external
output directory and their historical relative source names:

```powershell
uv run --locked --only-group tools python -B research/fidelity/2026-10-28-boost-ramp/analyze.py --recordings <external-headless-directory> --output <fresh-external-output>
uv run --locked --only-group tools python -B -m unittest discover -s research/fidelity/2026-10-28-boost-ramp -p test_analysis.py
uv run --locked --only-group tools python -B -m unittest discover -s tools -p test_documentation.py
```

The analyzer verifies archive hashes before decoding, requires capture-end and
matching setup, refuses output overwrite, and fails before scoring an unreproduced
fixture cohort. It performs no network calls. It runs serially with one-CPU
affinity, reusing #236's hash-pinned affinity helper. No Rust/Cargo build or
external target directory was needed. No SSH, AWS, browser or game runtime ran.
Tests use authored arithmetic/selection/lifecycle cases and archived inputs;
they do not establish authoritative server internals. No fresh held-out,
prospective, phone or browser validation is claimed.

Local verification: **20 analysis tests and seven documentation tests pass**;
the complete three-archive extraction reproduces all fixture intervals and the
selected JSON's source/tool hashes and totals verify. `git diff --check` passes.
Rust suites were not run because no Rust/runtime/fixture/contract file changed.
