# Wall sliding: compatible projection, unresolved exact rule

The calibrated projection reproduces retained shallow rim contacts descriptively:
median conditional endpoint error is **1.026, 1.283 and 0.974 world units** across
the three recordings, versus **24.570, 24.924 and 27.302** for unconstrained motion.
This supports a sliding constraint. It does **not** identify the exact contact
lead, projection algorithm, damping or lethal cutoff.

Only **3 of 778** eligible intervals satisfy the frozen primary radius/freshness
gates, one per source. No alternative meets the predeclared rejection rule.
The local-server full-selector dry run produced **264 broad intervals and zero
primary intervals**. The power gate also fails under the declared uncertainty.
**No mechanics change and no official capture are proposed.** No outside game
server, discovery service, browser or remote machine was contacted in this study.

## Evidence, provenance and access

The simulator base was `d66ebc08ecc557187748280e2b8745aaabd92f2c` in a new branch
and worktree, separate from #239. The [plan](PLAN.md) was committed at
`404dca85fffa5839154b0e325b3eb851ca06b19f`, with two wording clarifications ending
at `372f7171`, all before new scoring or raw-position inspection. The final
retained-data implementation was committed at `400d6d4f`; source hashes and full
commit identities are in [selected-results.json](selected-results.json).

The canonical source is the authored historical Arena
`docs/research/boundary-evidence.md` at
`c2fb7285febb4273034e30699dc880715a334423`, with
`tools/fidelity/analyze-boundary.py` and `analyze-headless-boundary.py` at that pin.
The rule and fixture were already present in Arena's initial import
`895e6f6b5b8c4d54aefbcfae4a16c8e6b17a18ac`; this is the surviving implementation
provenance, not a claim that this import was the original experiment. The later
note records calibration and its failures. Those historical sources and full
recordings require the retained local archive. They are not redistributed here.

| Quantity | Evidence class and limit |
| --- | --- |
| Received setup/z radius | Protocol target, u24 world units; physical comparison input. It is not the eased drawn border or a proven timestamp of the server's continuous radius. |
| `16 * scale` contact lead | Fitted model. The 593 selected browser-fixture anchors have pooled radial residual median 0.697 for 16, versus 0.724 for 15. This does not identify 16 uniquely. |
| `sin(.7)` lethal outward cutoff | Approximate calibration from requested rim phases and terminal observations. No final collision pose is transmitted; the browser fixture has no lethal crossing. |
| `scale=clamp(1+(n-2)/106,1,6)` | Client-derived size convention used by the wall model. Stable-count intervals isolate it from growth/boost-loss phase changes; this is not a new server-size fit. |
| 42-unit anchors, u16 direction; 1/18 speed byte, .032 speed conversion | Protocol/client geometry and units. Angle and speed fields can be inherited rather than freshly present. |
| 7 ms integration | Existing simulator loop model; 3.5/14 ms sensitivity is reported. Neither integration choice identifies collision/send phase. |
| Projection, unchanged heading and immediate center-outside kill | Current calibrated algorithm, not recovered official server source. |

The MIT Rust client reference is pinned read-only at
[`8d0772c015d6b9b73b80253db8c7349aa600b201`](https://github.com/NikitaMGrimm/slither-client/tree/8d0772c015d6b9b73b80253db8c7349aa600b201).
Its `state.rs:774` replaces the received radius target; presentation easing and
`presentation/arena_edge.rs` use a separate displayed radius. Its anchor decoding
retains explicit absolute coordinates or advances the last anchor by the encoded
direction and announced spacing. It does not provide an authoritative wall clamp.
The Python decoder supplies these received coordinates; no predicted head,
rendered position or `Snake.head()` estimate enters selection. The client clone
remained clean; the owner's checkout was not used or changed. No client dependency
was added to the simulator workspace.

### Retained corpus

The frozen inventory is the prior radius study's **91-entry source manifest**.
All 91 compressed identities were located, hash-verified and fully parsed. Thirty-
three public inventory names are privacy aliases; their original files were
resolved by the same SHA-256 within the declared archive root. The preliminary
pass's missing-name failures remain external. No new source was added after scores.

**53** files contain completed initialized protocol-19 captures. The other **38**
have no setup and no capture-end marker; they stay excluded, including failed
acquisitions. They are not missing recordings and are not counted as negative rim
exposure. Only three admitted sources contain own anchors within 250 units of the
physical target radius. The other 50 admitted sources contribute zero near-rim
own-anchor exposure, not additional validation of sliding.

| Recording (`bot0.jsonl.gz`) | SHA-256 | Near-rim anchors | Eligible / primary |
| --- | --- | ---: | ---: |
| rim-angle-discovery-20260927 | `77cce972026da4cab77c5868c391fc046a1bb455ddf4e1ea86d6f875248e663f` | 1,618 | 479 / 1 |
| rim-angle-validation-20260927 | `61938fd7217126c22f95eff9e63ad49667759eab1a917756fe53a05bc04cef7f` | 859 | 160 / 1 |
| rim-boost-threshold-20260927 | `493fc1eb7eece8906e8c40a8355a0f79d302590f25b5c5273fa168e5cc55c481` | 561 | 139 / 1 |

The 593-row `tests/fixtures/boundary-contact.json` comprises three earlier browser
groups (82/352/159). Its recorded raw hashes survive, but those original browser
captures were unavailable in the named retained recording roots. Their raw bytes
were not reverified. Fixed-row lead-16 medians reproduce the note: 0.6441, 0.7417
and 0.6525 units. The fixture remains unchanged and is development evidence only.

Development is the browser fixture plus rim discovery. Other directories were
held out from new fitting, with the two known historical validation sources
reported separately. Their published outcomes were already visible: this is a
retrospective source split, not fresh blinded confirmation. No other directory
provides rim exposure. The selected intervals cover **nine lives**; packets and
adjacent intervals within a life are correlated.

## Frozen comparison and results

[PLAN.md](PLAN.md) defines all formulas and exclusions. P16 runs the **actual
Rust `boundary::constrain`** through the standalone research harness. P15 changes
only the contact lead to 15; P0 projects the head center; H kills at center-radius
contact without sliding; T16 projects outward velocity before contact correction;
D90 additionally damps tangential contact displacement by 10%. Heading is never
rotated by contact. There is no fitted delay, speed, heading, lead or initial pose.

Each conditional replay starts at one received anchor and ends at its consecutive
received anchor. It follows received heading/speed/count/radius changes in packet
order on the accumulated server-header clock. This is not a full-input replay or
a measurement of untransmitted heads between anchors. Broad eligibility requires
50–700 ms, center gaps -5..40, outward angles .35..0.68 radians, stable count and
no known other body anchor within 200 units. Empty visibility is not proof of no
hidden opponent. The primary stratum also requires unchanged received radius and
heading age at both endpoints no greater than 150 ms; the <=42 ms sensitivity
has **zero** retained intervals.

Median alive endpoint error, world units:

| Source | P16 current | P15 | P0 head projection | T16 velocity projection | D90 damping | Free diagnostic |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Discovery, 479 intervals | 1.026 | 1.120 | 8.692 | 1.048 | 4.383 | 24.570 |
| Validation, 160 | 1.283 | 1.638 | 10.018 | 1.196 | 3.813 | 24.924 |
| Boost threshold, 139 | 0.974 | 1.058 | 8.997 | 0.985 | 3.947 | 27.302 |

P16's p95 errors are **2.208 / 3.108 / 2.178**. All five projection candidates
survive all selected intervals. H predicts death in **778/778**; its post-death
endpoint is not a valid survival prediction, so it is excluded from alive-error
ranking. The unconstrained path is retained explicitly as a diagnostic. Those
conditional deaths support the qualitative sliding interpretation, not a robust
estimate of death timing under unknown headings and radius phase.

Actual speed/direction exposure is 490/229 cruise intervals in positive/negative
tangent directions, 33/0 boosted intervals, and 24/2 intermediate-speed intervals.
The boost-negative cell is absent. Labels describe received speed, not requested
boost intent. Count ranges and per-life/source denominators, signed radial and
tangential errors, model kills and empty strata remain in the selected JSON.

### Separation and uncertainty

Full coordinate/angle cells are used, including accumulated directed-anchor and
body-origin uncertainty. The budget adds heading/speed quantization, header
rounding, endpoint timing sensitivity j=0/7/14/42 ms, and received-radius variation
around the interval. A separate allowance covers possible turning during the age
of the last explicit heading. These are conditional engineering envelopes, not
confidence intervals or proven server event/send/clock bounds. Correlated origin
errors are conservatively retained; repeated packets do not average them away.

At j=42, the three primary interval budgets are **26.54, 23.71 and 47.01 units**.
In the broad validation cohort the held-report budget median is 30.15; including
heading staleness makes it 128.15. The 15/16 and finite-step projection differences
sit well below those envelopes. Smaller retrospective error does not select a
law. No alternative supplies five qualifying contradictions in each of two source
directories; only one primary interval exists in each directory to begin with.

Changing integration from 7 ms to 3.5 or 14 ms changes P16 endpoint-error magnitude
by at most **0.0023 / 0.0040 units**. Shifting received events together by +/-42 ms
changes it by up to **5.067 units**. The latter is only a phase sensitivity, not
an exhaustive hidden-event envelope. Integration resolution is not the main
identification limit.

The lethal cutoff remains unvalidated. The canonical note retains broader replay
premature deaths, radius-change confounding and terminal predictions that flip
when the held heading moves by one 8-bit quantum. Our surviving .35..68-angle
cohort does not test that .7 boundary. It cannot erase those counterexamples or
validate the simulator's complete death law.

## Power gate and full-selector local dry run

The authored grid covers seven counts, four outward angles, three radii, cruise/
boost and seven durations: **1,176 designs**. Sizes and sustained boost in this
optimistic calculation are not claims of practical attainability. Bounds use
the declared full-cell quantization and j=42 timing, with source/repetition
uncertainty preserved rather than divided by sqrt(sample count).

For stable contact, lead-15 separation is bounded by
`scale + 2*V*scale*T/(R-16*scale)`; the finite-step velocity/position projection
ceiling used is `2*V*V*.007*T/(R-16*scale)`. These are continuous-contact
perturbation bounds under the stated geometry, not bounds through arbitrary
radius discontinuities or lethal threshold crossings. Their maximum ratios to
twice the held-report budget are **0.1062** and **0.01813**. Longer holds cannot
overcome the systematic heading term for these close alternatives.

Ideal damping and free-motion signals pass some held-heading grid calculations,
but **none** passes with the declared 150 ms heading-staleness allowance. Most
long-duration grid rows would also require a new unreset arc estimator: the frozen
selector scores consecutive anchors, not a 1,200-second uninterrupted endpoint.
Among the **168** grid rows inside its 50–700 ms duration limit, **zero designs**
pass the full j=42 comparison for any candidate, even using zero nominal heading
age before the j uncertainty. This is a negative result for the frozen design,
not a universal impossibility proof for different instrumentation or estimators.

The **240-second local-server dry run** used a one-CPU release server, static
1,500-unit radius, official delivery profile, no food/prey/background bots and the
existing unmodified one-client `boundaryprobe`. Its archive is explicitly labelled
`local-simulator`; no official binding was supplied. The full frozen selector was
then run, including real packet decoding and both-endpoint freshness checks.
It produced **264 eligible broad intervals, zero primary intervals**, despite all
264 having unchanged received radius. **All 264 fail the 150 ms endpoint-age gate**;
the median maximum endpoint heading age is **255.5 ms**. The archive's final exact-
prefix integrity and unchanged-collector-source receipt verify. The collector
completed, and both task processes were stopped.

This dry run demonstrates a functioning acquisition path but **fails to yield
the cells required by this analysis**. Its five lives and failed eligibility stay
retained. The selector, delivery profile and collector were not retuned or rerun
to obtain a favorable result. Boost is not established by a nominal even-life
label; the local minimum-size actor did not provide the missing boosted cells.

**No capture plan is proposed.** More intervals from this selector do not fix its
freshness failure, and replication does not remove the close-model systematic
uncertainty. Any future design would first need an explicit received-heading/
anchor-time treatment and a passing full-selector local dry run before seeking
approval. This report authorizes no collection.

## Implementation checks, retained failures and reproduction

- Six authored Python checks cover interior motion, contact/damping distinction,
  packet-order/timing changes, count transitions, empty exposure and systematic
  power. Two authored native interval contracts check split-event movement and
  lethal-state retention. The harness calls the current library; no simulator
  behavior is changed.
- The 2,592-point native/Python boundary check found **68** branch disagreements,
  all at center-radius differences within 1.82e-12. A last-bit distance difference
  crosses the discontinuous center-outside kill gate, so downstream poses can
  differ substantially. These witnesses are retained, not hidden by a tolerance.
  This invalidates using the Python mirror as an exact P16 oracle. Final P16
  scores and sensitivities therefore execute Rust directly. It is a research-tool
  correction, not evidence for a simulator mechanics change.
- The preliminary Python pass, initial failed strict parity check, alias lookup
  correction and all derived rows/logs remain outside Git. The selected JSON
  identifies the unchanged final base-analysis hash and bounded assembly step.
  Candidate formulas, eligibility, split, uncertainty and decision never changed.

From the repository root, set shell affinity to one available logical CPU and
Cargo/Rayon/test thread counts to one. Supply external paths; no example below
discovers or contacts an outside server:

```powershell
$study = 'research/fidelity/2026-10-28-wall-sliding'
uv run --offline --no-project python -B -m unittest discover -s $study -p test_analysis.py
cargo build --offline --locked --release -j 1 --manifest-path "$study/harness/Cargo.toml" --target-dir "$external/target"
uv run --offline --no-project python -B "$study/verify.py" --native "$external/target/release/wall-study-probe.exe" --output "$external/native-check.json"
uv run --offline --no-project python -B "$study/analyze.py" --recordings "$retainedRoot" --source-map "$privateHashMap" --native "$external/target/release/wall-study-probe.exe" --output "$external/analysis"
# Build the local host separately, using -j 1; run no build during collection.
pwsh -NoProfile -File "$study/dry-run.ps1" -OutputRoot "$external/local" -ServerBinary "$external/target/release/slither-sim.exe"
uv run --offline --no-project python -B "$study/verify.py" --local "$external/local" --runner "$external/target/release/wall-study-probe.exe"
uv run --offline --no-project python -B "$study/select_results.py" --analysis "$external/analysis" --local "$external/local" --parity "$external/native-check.json" --output "$external/selected-results.json"
```

Full recordings, private path resolution, raw local traffic and detailed rows
remain outside Git. No official client JS/assets, credentials or private endpoint
information are included. The report supports keeping the existing calibrated
projection as a compatible model, with its identification limits intact.
