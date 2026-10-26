# Clean server timing references, 2026-10-26

The default transport comparison now uses completed near-server data from
[#111](https://github.com/NikitaMGrimm/slither-sim/pull/111), rather than the
home-route/collector envelope. No new collection, simulator cadence change,
latency fit or runtime default change is part of this work. Raw archives remain
local-only; the new transport/cadence summaries contain no gameplay bytes.
The separate scheduler fixture contains four bounded logical-packet witnesses.

## Audit

Audited current `src`, `tests`, `tools/fidelity`, configuration/workflow guides,
fixture inventory and historical fidelity records. “Home” below means the earlier
retained home-era collection; receiver/collector differences are not identified
as physical server delays. Mechanics fixtures can contain timestamps without
being delivery calibrations. Historical studies keep their original corpora,
results, failures and labels; updating a current reference does not rewrite them.

| Item / consumer | Source | Server or route intent | Action |
| --- | --- | --- | --- |
| `transport-official-reference.json`; original transport-parity study | 53 admitted of 58 home headless archives, complete manifest/hashes in fixture | Former default server delivery envelope includes route/collector effects | Retain byte-for-byte as explicit home-route case; never relabel |
| `local_capture.py --reference`; research-workflow compare example | Previously the home transport fixture | Server transport parity | Default/example now `transport-official-near-server.json`; explicit home override remains |
| `transport_parity.py compare`; `transport_tolerances.json` four gates | Per-capture quantiles from caller-selected reference | Server delivery when using new default; route case when home supplied | Preserve metric formulas, minimum 100 samples and zero spread allowance; rederive clean development envelope below |
| `transport_parity.py measure`; synthetic `test_transport_parity.py` | Caller-provided raw data; synthetic tests | Measurement contract, no built-in official corpus | Add optional whole-interval selection for offline builder; ordinary measurements unchanged; test mask/baseline/window boundaries and default override |
| `client-reporting-cadence.json`; fixture-version test | Home `official-r1/bot0.jsonl.gz`, SHA-256 `dd65ff79bf5cb258cf0d72c35af22d07e9c6ff06068b6679e1ec011bafcb8a60`, first setup epoch | Server-header prey/leaderboard timing, not receipt-tail fit | Retain original labels/counts; stop using it as current simulator cadence reference |
| `unchanged_prey_positions_still_correct_client_prediction`; new nominal HUD test | New all-six-batch clean server-header summary | Nominal server release model | Use `client-reporting-cadence-near-server.json`; compare prey period to median within one simulation tick, and HUD periods to medians; do not adopt observed extrema as universal gates |
| `death-jitter-fresh.json`; `tests/death_food.rs` | Home `fidelity-next-movement-20260927/bot0.jsonl.gz`, SHA-256 `cbb4dac061d7a555905bdaeb36895c2119834eb3c52506fab7530827dc9b1738`; first eligible validation death, server anchor age 7 ms | Server spatial scatter/order; “jitter” is spatial, not transport | Retain unchanged; clarify beside test |
| `LatencyConfig::measured_route()`; fitted-shape test; connection/pit/reconfiguration tests using it | Home network-path study; fitted training/held-out sessions | Route/collector example and delay-mechanism tests | Retain numbers; label home route in API and test name; no server-profile substitution |
| `LatencyConfig::default/fixed/one_way`; remaining latency, client simulation/network/server/received tests | Synthetic clocks, parameters and loopback peers | Configurable mechanism/lifecycle contracts | Retain; no captured delay thresholds or external servers |
| Motion six-tick and mass 15-tick gates; shared-phase/changed-value/order tests in client simulation | Home packet-scheduler study (91 archives, including older footer-less captures) | Provisional server release model | Retain rules and synthetic/model assertions; link clean counterexamples beside constants; new decoder tests accept them |
| Prey ordinary one-third-second / exact-11 210 ms guard; sharing/reentry/age tests | Home scheduler/delivery-profile studies | Provisional server release model | Retain; document 3044 ms clean gap and unidentified rearm rule beside code; decoder retains stale prey and accepts correction |
| HUD defaults 2.504/3.008 s, maintenance 0.504 s, food interest 2.6 s; shared-clock and post-death tests | Home scheduler, visibility and presentation studies | Server release/interest model | HUD medians rechecked on clean remote corpus (2504/3008 ms); keep existing values/model phase tests. This timing task does not refit geometry, visibility or food lifecycle |
| Per-tick input/pong handling; 7 ms physics default; six-tick decision default | Home input-reflection observation (13.2 vs 13.4 ms route RTT), client/physics constants and chosen local schedule | Server processing model, not a recovered absolute server wait | Retain; #111 has no controlled input intervention or identified replacement physics tick |
| 250 ms client ping interval, five-second ping range; network deadlines/read budgets | Stock-client protocol constant or local bounded API choices | Client/connection policy | Retain; not home-tail calibration |
| `src/client/server.rs` thread schedule; `host_load.rs` and raw-server guide | Home/local tail-jitter study, synthetic local host load and explicitly configured 40 ms RTT | Local host scheduling/performance, not official server envelope | Retain local measurement definitions and provenance; no captured reference default |
| `client-presentation.json` (`cadence_seconds`, post-death close receipts); presentation tests | Home presentation captures and stock-client constants | Server presentation plus historical collector-close observations | Retain; timing fields do not calibrate current transport gates; current HUD timing tests use new fixture |
| `client-food-interest.json`; simulation interest tests | Home visibility/food recipient captures | Server subscription/receipt order, geometric state | Retain original selected witnesses; no home delivery-tail threshold to replace |
| `client-hud-leaderboard.json`; `tests/client_hud.rs` | Home live protocol-19 HUD packets | Decoder values/order/receipt state | Retain exact byte regression, not a server timing envelope |
| `client-protocol-paired-coil.json`; `server-body-snapshots.json`; client protocol/physics tests | Home paired-coil and boost captures (individual hashes in fixtures) | Decoder/body reconstruction and ordered receipts | Retain historical replay clocks and bytes; no server delay claim |
| `movement-turn-runs.json`, `speed-ramp-validation.json`, `cruise-retention-pair.json`, `cruise-growth-retention-20260930.json`; movement/physics/retention tests | Earlier home official movement cohorts, pinned per fixture | Server motion on header clock, not network delivery | Retain fitted mechanics and held-out regressions; clean timing data is not replacement motion validation |
| `body-updates.json`, `body-updates-larger.json`, `food-growth.json`, `boost-circle-food.json`, `boundary-contact.json`, `arena-radius-updates.json`, `official-collision-cases.json` | Earlier home/browser/headless mechanics evidence, labels retained | Server geometry/food/collision reconstruction; some synthetic boundary expectations | Retain; timestamps do not set delivery gates |
| `death-food.json`, `death-food-prospective.json`, `prey-outward-motion.json`, `prey-nutrition.json`, `prey-motion-quantization.json`, `prey-flight-speed-fresh.json` | Earlier home official mechanics captures | Server geometry/physical motion/nutrition, not correction-delivery law | Retain evidence and exceptions; no transport-tail calibration |
| `client-contract-v2.json`, `serve-errors-v3.json`, `tools/fidelity/fixtures/protocol19-setup.json`; contract tests; inline challenge bytes | Synthetic contracts or retained protocol bytes | API/decoder support, not measured cadence | Retain literal historical protocol/contract tags |
| `packet_census.py`, capture-quality and replay/export tools; adjacent tests | Explicit caller inputs; synthetic regression streams | Descriptive recipient observables / integrity / decoding | No fixed official reference to repoint; retain both clocks and source identities |
| `compare_evidence.py`, `fetch_evidence.py`, `evidence/templates/comparison.json`; comparison tests | Explicit hash-pinned baseline/candidate bundles; synthetic examples | Caller-selected reporting drift/RTT, not a bundled official server population | No implicit home corpus. Keep effect sizes/statistical method; route RTT stays route RTT |
| Network-path, tail-jitter, scheduler, delivery-profiles, official-capture and transport-parity research scripts/results | Frozen home studies; #108 includes collector contamination | Historical evidence and model fit | Preserve; not current clean server gates |
| #111 `timing.py`, analyzers, plans, seals, selected results and tests | Remote: four interrupted original/first-continuation archives quarantined; six completed second-continuation batches | Server-clock exceptions and effective near-server delivery | Preserve frozen gauges/masks/gates and failures; derive only from six completed batches |
| New source/held-out consistency tests | #111 source pins and new fixtures, offline | Reference admission and split, not independent scientific validation | Check source identities, whole-window masks, removal/reappearance resets and retained held-out exceedance |

## Selection and reproducibility

Read #111's [report](../2026-10-26-vps-timing/REPORT.md),
[selected results](../2026-10-26-vps-timing/selected-results.json) and
[source pins](../2026-10-26-vps-timing/source-pins.json) first. Only
`secondContinuation` completed streams are admitted. [build.py](build.py) verifies
their compressed SHA-256s and capture-quality audits, plus the pinned development
and validation output hashes, before measuring. It reads existing local files
only. All six archives verified; no earlier interrupted stream is admitted.

The transport fixture keeps four development batches as its calibration
`captures`, two held-out batches separately, and all six unfiltered summaries.
For calibration and held-out summaries, admit #111's epochs with at least 1000
post-burn-in frames and 30 seconds; require a full one-second burn-in before
every selected interval. Use its frozen telemetry stall windows and recorded
coverage. A gap needs both original adjacent receipts and the whole interval
clean; never join across removed frames. Anchor correction needs all ten
neighboring anchors' receipt span clean, in addition to the original ≤15% speed
spread condition. Minima, long gaps and zero gaps are not trimmed by value.

Transport formulas remain nearest-rank quantiles, nine-anchor-spacing local
speed, and receipt-minus-server minus the centered ±600-post-setup-frame minimum.
Compute the delay gauge on the full epoch **before** selection. This is deliberately
the existing transport metric, **not** #111's slope-corrected ±10-second gauge;
their numerical p99s must not be interchanged. Clean frame rates use summed
admitted adjacent intervals; unfiltered summaries keep their original complete
setup-to-last-frame durations. Summary fixtures omit family rows and histograms.

Cadence uses all six batches, reset at setup/connection and prey removal or
reappearance. Count consecutive known-prey `j`, `l`, and combined `M`/`V` pairs,
only when the whole interval passes the mask. These descriptive server-header
statistics have no fitted claim or independent held-out accuracy assertion.
Clean prey gaps: **5000**, min/median/p95/max **210/338/340/3044 ms**;
leaderboard **883**, **2988/3008/3013/3028 ms**; minimap **1089**,
**2494/2504/2507/2536 ms**. The simulator's nominal 336 ms tick-quantized prey
interval is within one 7 ms tick of the observed median; HUD medians match its
existing 2504/3008 ms periods. These checks do not assert universal bounds.

Each new fixture pins literal protocol **19**, source hashes, builder/decoder
identity, selection and limits. The cadence/transport summaries have no client
frame contract; the scheduler fixture pins **slither-client-v2**. Old fixtures,
inventory labels and selected expectations remain intact. Hashes establish
byte identity, not authenticated server origin or scientific validity.

With access to the existing second-continuation raw copy identified by #111:

```sh
uv run --locked --only-group tools python -B research/fidelity/2026-10-26-server-timing-references/build.py --raw <existing-raw-copy> --out <new-output-directory>
```

The output directory must not exist. The script uses standard-library Python,
one foreground process, and does not import the numerical fitting stack.
The first development invocation failed before writing output because it treated
the selected-results `eligibility` boolean as an object (`TypeError: 'bool' object
is not subscriptable`). Corrected the field access; two complete six-stream
derivations then succeeded. The retained preliminary derivation preceded the
final tool source pin; only the final derivation supplies committed fixtures.

## First attempt: masked development envelope

The method is unchanged: each upper limit is the **maximum eligible per-capture
quantile**, at least **100 samples** per metric, plus zero times its spread.
The corpus is now clean development data. No candidate simulator measurement
was used to tune it. These are exploratory envelopes, not confidence bounds.

| Metric | Old home upper limit | New clean development upper limit | Held-out low / second |
| --- | ---: | ---: | ---: |
| Absolute anchor receipt p50, world units | 0.941 | 0.859193 | 0.802715 / 0.834882 |
| Absolute anchor receipt p95, world units | 7.599 | 2.393579 | 2.303883 / 2.384703 |
| Server frame-gap p50, ms | 14 | 8 | 7 / 7 |
| Transport excess-delay p90, ms | 13.771 | 1.090 | 1.089 / 1.096 |

Rounded display values only; `compare` uses full fixture precision. Low held-out
passes all four gates. Second held-out exceeds the delay p90 envelope by
**0.006 ms**; the other three gates pass. Preserve that failure, without using
held-out data to expand the envelope or claiming independent validation passed.
Full unfiltered summaries preserve the effect of masked collector intervals.
No localhost acquisition was needed or run for this reference/data change.

## Correction: comparison rule fixed before evaluation

The first attempt above is retained, including its held-out failure. A default
comparison must apply the same measurement to reference and candidate: use all
six completed batches' **unfiltered** summaries, without the telemetry mask,
epoch admission or burn-in used for server characterization. Keep the masked
development and held-out summaries separately in the fixture.

Before evaluating replacement limits, fix this selection rule: use one common
`spreadAllowance`, the **smallest value in {0.5, 1, 2}** for which each of the six
official captures passes **all four gates** against the reference built from the
other five captures. Retain the existing minimum 100 samples per metric and
`max + allowance * (max - min)` formula, using full stored floating-point
precision. If none passes, report failure rather than adding another allowance.
The final default uses all six unfiltered captures with the chosen allowance.
No simulator candidate or new data participates in selection.

This envelope's false-reject rate is controlled only by that leave-one-capture-out
check over six captures. It supplies no population-level error-rate guarantee or
independent held-out validation; the same six captures select the allowance.
Batch drift was observed: the later batch-3 delay quantiles exceed batches 1–2,
and even their masked values exceeded the first attempt's development envelope.
The unfiltered comparison includes collector stalls on both sides.

## Scheduler witnesses and model decision

Selection is fixed by the existing held-out question: minimum clean motion gap,
minimum clean standalone mass gap, first clean motion-before-mass pair, maximum
clean repeat-prey gap. Ties choose first batch/frame. Each case retains archive
digest, connection, post-setup archive frame/packet indices, server and receipt
clock differences, setup, appearance and selected logical packets. The decoder
test explicitly reframes selected logical packets; omitted traffic is not
presented as a verbatim wire frame. Raw stream traversal checks that prey gaps
do not cross another same-prey correction or `y` removal/reappearance.

| Case | Held-out batch / connection / entity | Frame(s) | Server gap |
| --- | --- | --- | ---: |
| Motion minimum | low-3 / 2 / 401 | 14948→14952 | 24 ms |
| Standalone mass minimum | second-3 / 2 / 104 | 17932→17942 | 84 ms |
| Motion before mass, same frame | second-3 / 2 / 221 | 19360, packets 0→1 | 0 ms |
| Long repeat-prey correction | low-3 / 6 / 27656 | 57677→58052, 374 intervening frames | 3044 ms |

Rust tests assert decoded values and receipt-state updates for the short gaps,
same-frame ordering, and prey persistence/correction after long silence. These
are counterexamples to exact cadence/order bounds, not enough evidence to
identify a new hidden trigger/rearm law. Simulator release rules stay unchanged.

**No near-server latency profile is added.** #111's held-out clean excess
p50/p90/p99 is **0.636/1.096/2.322 ms** (low) and
**0.661/1.102/2.372 ms** (second), with >10 ms episode rates
**0.001965/0.001935 per second**. This strongly rejects treating the earlier home
pauses as a universal server process, but does not identify one-way propagation,
uplink distribution, direction coupling, or physical pause durations. No new
development fit and held-out model validation was performed; inventing a profile
from those few summaries would not meet that requirement. `measured_route()`
remains the labeled home-route/collector example and zero latency remains the
runtime default.

## Verification

Focused tests cover mask/window selection, default/home override, original
transport regressions, fixture labels, the three new Rust decoder tests (four
witnesses), nominal prey/HUD periods and unchanged death-food regressions.
The complete Rust group passed (format, four Clippy configurations, default/core
tests and doctests, C ABI tests, Python crate check and basic example). The tools
group passed: **197** general Python tests, **41 + 185** fidelity Python tests,
**253** Node tests, local transport/bot/C ABI smoke checks, wheel build and **28**
wheel tests (**one skipped**). All **7** documentation checks passed. The first
documentation run identified a missing research-index link; added it and reran
successfully. No required check remains unrun.
Local checks run with at most four logical CPUs, `CARGO_BUILD_JOBS=2`, Rust test
threads/Rayon limited to two; explicit Cargo commands use `-j 2`. No external
game server, new capture or long-running background worker is used.

Limits: two endpoints and one near-server collector setup, correlated samples,
selected witnesses and retrospective reference selection; no population-wide
clock law, renderer equivalence, one-way latency or new fitted model is claimed.
