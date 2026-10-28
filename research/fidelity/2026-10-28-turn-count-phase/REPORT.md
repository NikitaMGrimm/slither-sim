# Turn scale, count convention and event phase

## Decisions

The retained nine-run fixture reproduces exactly through the pinned Rust client's
anchor reconstruction: **134 overlapping triplets, 25.814 seconds, eight tracks**
from two recordings. Its original `segments` convention is the decoder's live
anchor-list length. It equals the Rust client's logical count and its retained
physical-point count **minus dying points** on every selected anchor.

Using that convention, duration-weighted angular-rate RMSE is **0.06759 rad/s for
106 versus 0.01199 for 111**. A development-fitted global integer offset of 9 gives
0.02915. These are retrospective point scores, not new held-out validation.

The long 174-segment arc separates 106 from the observed angle under every frozen
timing sensitivity through 42 ms per endpoint. That does not identify a literal
server denominator: no hard event-to-send timing bound has been established.
With a 7 ms endpoint sensitivity, **fixed offsets 7 and 8 remain compatible with
all nine runs** despite 111's better point score. The bounded integer-offset family
is not rejected under that uncertainty model.

**Zero selected anchors change logical count.** Before/after/adjacent-anchor phase
candidates coincide within each denominator in all nine intervals. More steady
circles cannot distinguish those phase candidates. Fam, dying retention and
displayed length do not supply the required large count deficit.

No new mechanics correction is established. Simulator behavior, the existing 111
rule, fixtures and the owner client remain unchanged. No game connection was opened.
The proposed capture protocol is **not approved**.

## Canonical provenance and the freeze

The introducing change is historical Arena commit
`4252ca440850470e9c7a5b8f9b2d0162d36e078d`, **“fix: calibrate size-dependent turning
with recorded replays”** (September 27, 2026). Its canonical study is
`docs/movement-fidelity.md`; analyzer `tools/analyze-movement-fidelity.py`; model
owner `src/physics.rs`. It reports a descriptive calibration optimum of 110.8,
adopts rounded 111, and evaluates the fixed candidate on historical `run3-*` data.
The later retained source pin is `c2fb7285febb4273034e30699dc880715a334423`, with
the study/analyzer moved under `docs/research/` and `tools/fidelity/`.

The public repository's canonical retained regression is
[movement-turn-runs.json](../../../tests/fixtures/movement-turn-runs.json), consumed
by [movement_turns.rs](../../../tests/movement_turns.rs). It entered this repository
in `ee259414fa8c62388ed43e28c638a0c9ee851cbe`. The source/import relationship is
documented by [provenance](../../../docs/provenance.md) and the historical source
pin in [server rules](../../../docs/server-rules.md#movement-and-geometry).
The old study and original reader were inspected as retained local Git objects,
not fetched from a game server or copied into this PR. Their full historical
source is local-only; these paths/hashes do not provide a public download.

Before reading raw traffic or computing new scores, [PLAN.md](PLAN.md) was committed
at **`b588af3c4d25286c226e4ed2f88bb4128ad32e03`**. Its formulas, fixed-offset family,
selection, uncertainty budgets and exclusion rules remain unchanged. All nine
fixture runs and their two source recordings are **development** in this study.
They retain their original historical validation role, but already published
outcomes and selected regressions are not a fresh held-out set. No new source or
favorable within-source interval was substituted.

| Pin / evidence | Identity |
| --- | --- |
| Simulator base, including fixtures | `4dd66ab00ab8b0acb2b9ea1432a43f9fd31a8196` |
| Clean external client / Git dependency | `8d0772c015d6b9b73b80253db8c7349aa600b201` |
| small-circle raw archive | `86143ad92a958552fa3865b623c55f533528fc5a8d759007c55d5662ad88666e` |
| large-radius raw archive | `0317f6bbfb7075e019295727e41071e816799d4adbb97d4f6a06ce51748c2a94` |

Full recordings and derived rows remain outside Git in task evidence
`turn-count-phase-20261028`, access **local-only**. The original recording-relative
paths and selected connection/incarnation/index bounds are already in the fixture.
[selected-results.json](selected-results.json) retains aggregate scores, raw-source
hashes, tool/binary/plan hashes, native audit counts and per-run count summaries.
No official JS, assets, credentials, addresses, nicknames or personal details are
included. The research crate has its own workspace and no simulator dependency;
the simulator workspace acquires no client dependency.

## Reconstructing the original count and clock

At the introducing pin, `tools/slither_protocol.py` defines `Snake.sct` as
`len(self.pts)`. Appearance creates one item per decoded body anchor; the head is
stored separately. Non-growth anchors remove a tail and append a front; growth
appends a front; tail packets remove one item. Fam is separate. The analyzer reads
`snake.sct` **after** feeding each anchor and takes a run's median count. There is
no extra head segment, displayed-length adjustment or fractional-mass contribution
to that count. All selected intervals have an exactly constant count, so median
versus time-weighted count aggregation makes no difference here.

The new [harness](harness/src/main.rs) feeds complete retained frames to the pinned
client decoder for validation, then processes their commands in original order
without inter-command animation. It records before/after logical count, physical
retained points, dying points, fam, displayed length and scale/turn factor, plus
received-state body length. Animation advances on a fixed 60 Hz receipt clock for
retention/length observations; turn angles and durations use server headers and
raw anchor positions. These are numeric state observations, not browser rendering.

The first strict timestamp check failed for a provenance reason. The original
reader zeroed the first incoming frame and then accumulated **every** elapsed
header, including setup's initial epoch. The current setup-relative timeline
starts at zero on setup. The selected connections therefore differ by constant
offsets **3639, 65535, 3938, 12552 or 18548 ms**. The harness now preserves both
clocks by executing the original header-accumulation rule, without fitting an
offset. Every original first/last timestamp and anchor index then matches.
Constant offsets cancel from all midpoint durations. All nine reconstructed
angles match the fixture exactly; duration discrepancies are at floating summation
precision. The failed first attempt is retained externally.

The fixed intervals and their neighboring triplets also pass the original gates
without reselection: minimum wanted margin .5843 rad, chords 42 units within
floating rounding, and adjacent server gaps 91..269 ms. The conservative frozen
39-unit chord budget is retained rather than tightened after seeing these values.

The two complete native replays cover **68,403 gameplay frames and 26,423 anchors**.
Only the nine frozen intervals are scored. The six larger fixture intervals cover
169, 174, 237, 241, 242 and 260 segments; smaller runs cover 5 and 9. There is no
37- or 100-segment selected interval, and this study does not extend the original
fit's claimed range through 293 or beyond.

## Frozen comparisons and conditional uncertainty

For count x and denominator D, the frozen rate is

`Q_D(x)=.033/.008 * (.13+.87*((7-clamp(1+(x-2)/D,1,6))/6)^2)` rad/s.

Compare D106, D111, fixed offsets `Q_106(max(2,n-k))` for k=1 and k=5, and one
global development fit over integer k=0..20, with duration-weighted squared error
and smaller-k tie breaking. Do not choose a different k per size. Event-phase
sensitivities use before/after/previous/next-anchor logical count under each D.
Count substitutions under D106 use retained physical count, physical-minus-dying,
logical+fam and displayed length. Fam/dying values are reported independently;
neither is mislabeled as a standalone segment count.

Each original interval sums consecutive signed chord-direction changes and their
midpoint-time spans. Interior direction errors telescope. The frozen conditional
angular budget is **A=0.072636 rad**, combining one-unit coordinate-cell uncertainty
on 39-unit minimum chords and a full u16 direction quantum. For per-endpoint jitter
j in 0,7,14,42 ms, the time half-width is `B=.001+2*j/1000` seconds. A candidate is
compatible when `abs(observed_angle-Q*T) <= A+Q*B`.

These are sensitivity assumptions, not empirical confidence intervals. Grid
resolution does not prove hidden authoritative quantization, and packet timestamps
do not bound event-to-send lag. The chord-midpoint construction approximates
continuous turning between anchor events; variable speed, service delay and
unknown inputs remain limits. We do not turn overlapping triplets into independent
samples or assign a probability of equivalence.
The timing budget also assumes a coherent accumulated server clock with endpoint
quantization; independent per-frame truncation or clock-rate error is not bounded
by this study.

| Segments / mode / direction | Duration s | Observed rad/s | Error 106 / 111 rad/s | 106 compatible at j=7 / 42 ms | Minimum j admitting 106, ms |
| --- | ---: | ---: | ---: | --- | ---: |
| 5 / cruise / right | 2.4350 | 4.08560 | .00562 / .00714 | yes / yes | 0 |
| 9 / boost / right | .6795 | 3.99961 | .04683 / .05035 | yes / yes | 0 |
| 9 / boost / left | .6790 | 4.07598 | .02954 / .02602 | yes / yes | 0 |
| 169 / boost / right | .7695 | 2.55975 | .07197 / .00887 | yes / yes | 0 |
| 174 / cruise / left | 14.6040 | 2.51142 | .06503 / .00071 | no / no | 178.77 |
| 237 / cruise / left | 2.0195 | 2.03716 | .07426 / .00206 | no / yes | 19.20 |
| 241 / cruise / right | 1.0140 | 2.04096 | .10638 / .02951 | no / yes | 8.61 |
| 242 / cruise / left | 1.8145 | 2.00619 | .07864 / .00164 | no / yes | 17.68 |
| 260 / cruise / right | 1.7990 | 1.89877 | .09483 / .01568 | no / yes | 26.65 |

D111 is compatible in all nine rows even at j=0. D106 is incompatible in five rows
at j=7, four at j=14, and only the long 174-segment row at j=42. Small/short arcs do
not separate the models under this budget. The 174-segment run contributes 56.6%
of total scored duration; this is not balanced replication across sizes or tracks.

| Point-score candidate | Duration-weighted RMSE, rad/s |
| --- | ---: |
| D106 | .06759 |
| D111 | .01199 |
| Fixed offset 1 | .06092 |
| Fixed offset 5 | .03862 |
| Development-fitted fixed offset 9 | .02915 |

Considering the entire frozen integer family rather than just its point optimum,
the offsets compatible with **all** runs are: none at j=0; **7,8** at j=7;
**7,8,9** at j=14; **6..10** at j=42. Thus a point-fit win is insufficient to
reject the family. No held-out inference is available for the fitted k=9.

## Count substitutions and event-phase coverage

Across all 134 selected centers, received body length equals logical count,
and physical-minus-dying equals logical count. Retained physical points can
substantially exceed it; they are not authoritative server body membership.
For the long 174-segment arc, medians are:

| Logical | Physical retained | Dying | Fam | Displayed length |
| ---: | ---: | ---: | ---: | ---: |
| 174 | 200 | 26 | .16079 | 173.56079 |

Using these separately time-weighted D106 substitutions gives rate errors
0.06503 (logical), 0.29117 (physical retained), 0.06503 (physical-minus-dying),
0.06673 (logical+fam), and 0.06177 (displayed length) rad/s on that arc.
Subtracting the actual dying count returns the original convention; counting
dying points generally worsens large-snake under-turning. Adding positive fam
does not supply a lower effective count. Displayed length differs by less than
one segment here, far short of the **7.748-segment** equivalent deficit at n=174.

Exact equality of the unsaturated formulas requires
`n_106=2+106*(n_111-2)/111`; the deficit `5*(n-2)/111` varies with size.
That algebra rules out an exact universal offset over all sizes. The finite,
uncertain retained intervals nevertheless allow some fixed offsets, as above.

There are no logical-count changes at the selected anchor events or among their
adjacent-anchor phase samples. All four phase predictions coincide for each D.
The data therefore cannot distinguish old/new count sampling during growth or
loss. This is absent transition exposure, not evidence that event phase is absent.

## Authored insertion/contraction and lifecycle probes

The pinned client was given original synthetic curved bodies of 8 and 37 points,
42-unit edges, .04-radian curvature increments, .43 contraction and explicit
growth/non-growth anchors. Each result exactly matches **insert then sequentially
contract**, including the proper point identities. A source-equivalent
**contract then insert** calculation, with physical tail removal on non-growth,
differs by **8.4972 world units maximum** among matched living points in each of
the four cases. This demonstrates the known source-order distinction; it does
not compare an online server body to a picture or establish a new mechanics defect.

Separate lifecycle probes establish:

- Fam-only replacement updates fam but changes neither logical count nor scang.
- Growth increments logical count immediately, recomputes scang and appends a
  physical point. Non-growth adds a retained point and marks a tail dying without
  changing logical count. Tail removal decrements logical count immediately.
- Displayed length has a separate correction clock. In these authored probes it
  remains unchanged on the first 8 ms update and begins changing on the 16 ms
  observation. An initial test incorrectly expected the first 8 ms step to change
  it; the test/probe was corrected and the failure retained. It settles to
  logical+fam-.6; none of this length state feeds scang.

These probes contain no official bytes/assets and change neither client source
nor simulator physics. They cannot recover the hidden server's count phase.

## Conditional capture power and frozen proposal

[CAPTURE-PROTOCOL.json](CAPTURE-PROTOCOL.json) is a **proposal, not approval**.
For constant-rate candidates a and b, disjoint uncertainty envelopes require

`abs(Qa-Qb)*T > 2*A + (Qa+Qb)*(.001+2*j/1000)`.

The table gives minimum seconds for D106 versus D111 and the fitted offset-9
candidate versus D111. It is a deterministic separation budget, not 80% or 95%
statistical power; unknown between-arc variance precludes such a claim here.

| Count | D106 vs D111, j=7 / 42 ms | Offset 9 vs D111, j=7 / 42 ms |
| --- | ---: | ---: |
| 9 | 75.77 / 236.77 | 3.57 / 11.20 |
| 37 | 15.31 / 46.49 | 3.25 / 9.90 |
| 100 | 5.65 / 15.99 | 5.42 / 15.43 |
| 174 | 3.41 / 8.81 | 21.03 / 54.59 |
| 244 | 2.64 / 6.18 | 15.03 / 35.42 |
| 293 | 2.38 / 5.17 | 7.52 / 16.48 |

The resulting proposed primary counts/durations are **174/60 s, 244/40 s,
293/20 s**. At j=42 ms, every integer fixed offset k=0..20 has disjoint prediction
envelopes from D111 at at least one of those three size/duration pairs, assuming
the actual observations follow one candidate within the stated budgets. The
protocol records that per-k coverage; it does not claim the future data will
follow D111 or that the timing assumption is already justified.

Request three separated intervals per size/direction/mode cell: **36 intervals**
over increasing/decreasing turns and cruise/boost, nominally 24 scored minutes.
Distribute one interval per cell over two development invocations and one per
cell in each of two later held-out invocations. Four invocations are bounded at
20 minutes each, with two connections maximum (actor and nearby observer).
Acquire exact target counts; score actual received count, not a rounded size bin.
Keep all failed acquisition and missing cells; no replacements or threshold changes.

**Feasibility and identification gates remain:** long stable boost plateaus may
be unavailable because boost spends mass. Qualify that coverage before requesting
a full acquisition; unfilled boost cells remain inconclusive, not silently replaced
by cruise. Count must be stable for at least one second before/after a scored arc,
wanted-heading margin and visibility continuity must hold, and both raw clocks,
outgoing controls, mass/tail/growth events and life boundaries must be retained.
Input/visibility uncertainty and other snakes are reasons to retain exclusions,
not infer unobserved world absence.

Stable-arc phase candidates are identical for **any duration**. For one count
increment, the frozen 7-ms phase sensitivity produces only **4.24e-5 to 7.46e-5 rad**
across planned sizes 9..293, below one u16 anchor-direction quantum
**9.59e-5 rad**, before service-time uncertainty. This is not proof that repeated
well-controlled transitions are statistically unidentifiable; it shows why a
single growth event and this stable-circle protocol cannot select a hidden tick
phase. A separate transition design and a justified event-to-send timing bound
would be needed before proposing that capture. No capture is performed here.

## Reproduction, checks and limits

Use standard-library Python and Rust 1.99+. Set shell affinity to one CPU and
`CARGO_BUILD_JOBS=1`, `RAYON_NUM_THREADS=1`, `RUST_TEST_THREADS=1`. The Python entry
point enforces one available CPU and serial native runs. The client Git revision
and dependencies are locked; this task built with Cargo `--offline`. Full external
recordings are required for native reconstruction; aggregate fixture arithmetic
and authored probes do not require game access.
The run used Rust 1.99.0 (`b940084d7`) and Python 3.13.13.

```powershell
$study = 'research/fidelity/2026-10-28-turn-count-phase'
# $external is a new output directory outside tracked trees.
# $archiveRoot contains the fixture's original recordings/headless/... paths.
cargo build --offline --locked --release -j 1 --manifest-path "$study/harness/Cargo.toml" --target-dir "$external/client-target"
python "$study/analyze.py" --archive-root "$archiveRoot" --binary "$external/client-target/release/turn-count-research.exe" --output "$external/analysis"
$env:TURN_COUNT_BINARY = "$external/client-target/release/turn-count-research.exe"
python -B -m unittest discover -s "$study" -p test_analysis.py
cargo fmt --check --manifest-path "$study/harness/Cargo.toml"
cargo clippy --offline --locked -j 1 --all-targets --manifest-path "$study/harness/Cargo.toml" --target-dir "$external/client-target" -- -D warnings
cargo clean --target-dir "$external/client-target"
```

Completed: locked offline release build; formatting and all-target Clippy with
warnings denied; eight Python model/geometry/clock regressions; both complete raw
replays and all nine fixture reproductions; frozen-plan/tool/source hash checks;
documentation navigation and privacy/diff checks. No simulator runtime test group,
browser/reference-JS rendering or live-server acceptance was run. No runtime source
changed. Disposable target and repeated exported input files are removed after
checks; raw inputs, failed-attempt notes and derived evidence remain external.
Independent read-only source/evidence review found no supported actionable defects;
that review ran no builds, harnesses, tests or network operations.
