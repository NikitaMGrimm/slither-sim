# Evidence workflow

Start with the [evidence tier](../CONTRIBUTING.md#evidence-proportional-to-the-change)
for your change and [development setup/checks](../CONTRIBUTING.md#setup-and-checks).

- [Try the offline demonstration](#try-the-complete-offline-workflow).
- [Plan a measurement](#plan-a-real-measurement), including [controlled routes](#controlled-snake-and-food-routes).
- [Audit, analyze and replay](#inspect-analyze-and-reproduce).
- [Package and submit](#package-and-submit) or [compare contributed bundles](#compare-contributed-evidence).
- [Add or update a fixture](#add-or-update-a-fixture).

Evidence tools require Python 3.12, uv, Rust 1.99+ and Node.js 22+
(24 recommended). Rust-only and documentation work can skip this setup:

```sh
uv sync --locked --only-group tools
```

Run from the checkout with a fresh output directory per study; never overwrite
plans, captures or bundles.

## Try the complete offline workflow

```sh
uv run --locked --only-group tools python tools/fidelity/worked_study.py --output outputs/evidence-demo
uv run --locked --only-group tools python tools/fidelity/evidence.py audit outputs/evidence-demo
uv run --locked --only-group tools python tools/fidelity/evidence.py pack outputs/evidence-demo --output outputs/evidence-demo.tar.gz
uv run --locked --only-group tools python tools/fidelity/evidence.py verify outputs/evidence-demo.tar.gz
```

The first command builds the simulator/replay example, generates and audits six
seconds of steering traffic, then replays a three-second window, without network
or browser access. Inspect `REPORT.md`, `analysis/results.json`, `analysis/replay/`
and `fixtures/`.

Data is **synthetic, local-simulator**: causal heading reconstruction is compared
with a known-state control and timing/heading perturbations. Corruption and
truncation have separate tests; initialization errors are retained. Passing checks
establish pipeline agreement, not server mechanics or playing strength.
Repeated runs reuse data identities; thresholds were chosen during development,
not blinded validation.

## Plan a real measurement

Collection requires an explicit protocol-19 WebSocket IPv4 address/port and
`/slither` path; tools do not discover servers. Recordings contain raw received
packets and sent controls, without server-only state.

Copy [the movement plan](https://github.com/NikitaMGrimm/slither-sim/blob/main/evidence/templates/movement-plan.json). Specify:

- Study ID, question, competing explanations and method.
- Sampling unit, attempt budget, exclusions and held-out validation.
- Operator-supplied protocol-19 IP/port and actual `dataOrigin`: `local-simulator`
  for the template's loopback `127.0.0.1:8766` at `/slither`, or `community-server`
  for another compatible server. Set HTTP(S) `capture.origin` only if required
  by the endpoint.

Freeze confirmatory predictions and selection rules before collection; label
exploratory results. A plan hash cannot prove when or why it was written.
Fit and validate on separate data.

Initialize and inspect without connecting:

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py init --plan path/to/your-plan.json --output outputs/your-study
uv run --locked --only-group tools python tools/fidelity/collect.py outputs/your-study --dry-run
```

To collect on that server:

```sh
uv run --locked --only-group tools python tools/fidelity/collect.py outputs/your-study
```

Each invocation durably reserves one attempt, without automatic retries or
replacements. Limits are 1–3 clients and 3,600 seconds per attempt, with
scenario-specific client counts validated; the template uses one client for
60 seconds. The wrapper passes the endpoint/Origin, logs stdout/stderr in `logs/`,
and stops the child after the duration plus 30 seconds for draining. It records
exit status in `attempts.jsonl` before auditing; audit exceptions cannot erase
the outcome or release the reservation.

Failures, cancellation, timeout and incomplete gzip files remain recorded; exit
zero alone is not successful acquisition. Other advertised protocols are
quarantined with raw bytes retained and the observed version in
`setupSignatures[].protocolVersion`. Unsupported login challenges fail without
executing JavaScript and require a reviewed parser update.

For paired-prey acquisition, add these fields to `capture` alongside the server
and optional Origin; use the same `init`, `collect`, `audit` and `pack` commands:

```json
{"scenario": "paired-prey", "bots": 2, "seconds": 480,
 "mode": "crossing", "direction": "diagonal-positive"}
```

Paired plans require exactly two clients, 1–3,600 seconds and explicit `mode`
`overlap` or `crossing`. Fixed `direction` is optional for crossing, invalid for
overlap. Audits check both members, source hashes and mode/direction against the
frozen plan. Capture completion certifies neither valid crossings nor
statistical coverage.

Managed plans require explicit duration; its ceiling is an operational budget,
not a server constant.

Low-level paired-death and feeding-shape collectors need their own matching
coordinator and attempt accounting.

### Controlled snake and food routes

The managed workflow also accepts these acquisition scenarios:

#### Direct-snake routes

```json
{"scenario": "paired-snake", "bots": 2, "seconds": 480,
 "mode": "crossing", "direction": "x"}
```

The direct-snake route references A's own received pose while B moves between
fixed near/far waypoints. It continues through peer disappearance from an
opponent cache, an observable outcome. Plans require exactly two clients, explicit
crossing direction and 1–3,600 seconds. Normalized directions give Euclidean
800/2,300-unit rail offsets, including diagonals. A nominally orbits an 80-unit
circle; B holds each achieved endpoint for two seconds. These settings and safety
margins are acquisition controls, not recovered server visibility rules.

Only B records canonical `paired-snake-endpoint` witnesses; completed legs exclude
initial approach. Shared trials bind identities, center and axis;
`paired-snake-trial-end` records abandonment or capture-end censoring before reset.
Logs retain avoidance interruptions, actual anchors, cache membership, intended
and sent headings. Counters and intended waypoints alone certify neither
uninterrupted traversal nor fresh causal queries.
Endpoint recognition and new headings require both own anchors at most 150 ms
old. Staleness pauses control and counts as an interruption, retaining the trial's
center, targets and hold deadline. Both anchors must refresh after pause entry
before resuming. Ages over 500 ms or no reacquisition within 1,000 ms abandon the
trial. Pausing suppresses new headings and boost; the last delivered heading
continues physically. Holds and leg deadlines keep running; interrupted legs
cannot establish uninterrupted round trips. These engineering bounds are separate
from frozen analysis freshness screens and opponent-cache absence.

#### Food lanes

```json
{"scenario": "food-lanes", "bots": 1, "seconds": 480}
```

The food route uses one client for 1–480 seconds, without paired mode/direction.
At one interior site it schedules eight reciprocal passes in A/B/B/A lane blocks,
75 units apart within one 300-unit sector row. The first four slots are
preassigned calibration, the last four holdout; approach/transfers are excluded.
Failure retains missing slots and stops without reconnecting or replacing the
site. `food-lane-phase-end` records achieved endpoints and interruptions;
`food-lane-route-end` retains the terminal outcome. Initial screening requires
received opponent body polylines, including extrapolated tips, more than 800 units
from both lanes, west transfer and approach. This conservative acquisition margin
cannot guarantee future or unreceived safety; the danger veto remains active.

Both scenarios use the standard recorder, source/plan pins, attempt ledger and
audit/package commands. An aborted route can produce an intact recording. Before
scaling a study, inspect route outcomes and packet-derived denominators separately
from integrity. Freeze fit/validation splits, freshness screens and required
coverage in the analysis plan.
Food announcements and explicit clears describe delivery episodes; unannounced
empty sectors never establish a known subscription or global food absence.

#### Paired-prey routes

For two-client visibility crossings, paired-prey supports fixed
`x`, `y`, `diagonal-positive` and `diagonal-negative` directions:

```sh
node tools/fidelity/slither-paired-prey.mjs --out outputs/paired-diagonal --server 127.0.0.1:8766 --seconds 480 --mode crossing --direction diagonal-positive --plan path/to/paired-plan.json
```

Offsets are max-axis distances: near 650 (about 919 units diagonally), far 1,850,
reference 450. After recording an endpoint, B holds that relative waypoint for
two seconds before traversing again. `crossingHeldOffset` and `crossingHoldUntil`
describe the hold; `intendedOffset` records the commanded offset even when
`crossingLeg` names the upcoming leg. These are acquisition controls, not measured
server ranges or timers.

Logs retain projected along-route/cross-track distances, visibility, freshness,
interruptions and completed legs; intended routes or phase names cannot prove
achieved crossings. Before connecting, freeze candidate geometry/retention,
calibration/held-out legs, attempt budget and required coverage. Preserve failed
joins and empty cohorts; neither validates a new visibility rule.

Abandoning an active crossing trial records `paired-prey-trial-end` in both
archives before clearing state, even on the reacquisition update or when a client
cannot run movement. `endedAt` is coordinator monotonic update time; enclosing
`t` is recorder time. The event retains old pair/target/trial identities, leg,
hold and endpoint counts. `reason` names the collector branch: `pair_unavailable`
does not distinguish death from disconnection; `primary_visibility_lost` means
the primary cache is absent or too old. Neither proves a server subscription
change. Match recipients by trial and `endedAt`: one shared control event, not
independent observations. Trials active at capture end are right-censored.

Endpoint advancement requires the reference near its assigned offset, both snake
anchors at most 150 ms old and the reference prey report at most 350 ms old:
collector/study quality gates, not server constants. `endpointVisits` includes
initial approach; `completedLegs` counts subsequent legs between achieved
endpoints. B records each exact start/end witness once in `crossingEndpoint`.
Analysis must still check continuous geometry, avoidance-free travel, clock
alignment, peer continuity and held-out coverage.

Paired collector tests rehearse complete near/far round trips with actual
quantized, rate-limited sent commands. Point robots use explicit synthetic
speed/turn settings, exact fresh anchors and no obstacles; stationary/moving
targets cover both signs of each route. Tests protect control wiring without
depending on measured physics defaults or claiming live acquisition success.

#### Paired collision trials

`slither-paired-collision.mjs` coordinates two identities through a compact
tandem lateral-offset ladder and head-on crossings with reversed roles/join
order. It records both own streams, actual sent bytes, staging failures,
qualification samples and post-event deaths in the ordinary archive format.
`paired-collision` capture plans require exactly two bots and at most 600 seconds;
no paired `mode` or `direction` fields apply. Outside endpoints require an
explicit official-server plan and `--allow-outside yes`; the default is loopback.

The [unexecuted pilot and commands](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-collision-pilot/README.md)
freeze population selection, trial qualification, uncertainty and stop rules.
The offline preparer binds retained selection metadata without probing any
server. `collect.py` rejects pilot two after failed integrity, zero qualified
ladder/head-on trials or excessive third-party contamination in pilot one.
Only the coordinator owns reconnection, waiting for both close handshakes before
the next ordered joins. Run no other game collectors during acquisition.

For a complete **localhost-only** engineering check, build the binary and run:

```sh
node --test tools/fidelity/slither-paired-collision.test.mjs
node --test tools/fidelity/slither-paired-collision.local.test.mjs
cargo test --locked --example collision_curve
cargo run --locked --example collision_curve
```

The local regression uses a production host with two socket players and no
background bots or teleports. The separate synthetic curve uses the public
Simulator API. These verify current simulator outcomes, not online laws or
live-server feasibility. Raw local test archives remain outside Git, including
failures; the test prints their location. Use `CARGO_BUILD_JOBS=2` and
`RAYON_NUM_THREADS=2` on a shared machine.

### Capture records

Raw frames retain direction, bytes and monotonic send/receipt times. Metadata
pins plan/collector sources, wall-clock origin, connection/life boundaries, ping
sends/replies/RTT and censored requests, including post-death observation. Server
elapsed headers supply a separate received clock. Audits derive RTT/arrival-gap
summaries from these frames without a second timing log. Neither clock identifies
one-way latency or hidden processing ticks. Use the shared audit/decoder: JSONL
metadata records are objects, frames are three-element arrays.

## Inspect, analyze and reproduce

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py audit outputs/your-study
```

Audits check plan/attempt accounting, file membership, sources, receipts, gzip
completion, duplicates and batch consistency. Missing attempts are incomplete;
corrupt, interrupted or mismatched captures are quarantined. Check owner/process
and ledger before removing a leftover `.study.lock`.

Analyze packet families, byte counts and cadence from an audited complete capture:

```sh
uv run --locked --only-group tools python tools/fidelity/packet_census.py outputs/your-study/raw/ATTEMPT/bot0.jsonl.gz --output outputs/your-study/analysis/packets.json
```

Create `analysis/` first. The census rejects existing output, incomplete captures
and non-19 setups. It reports archive/tool hashes, separate server/receipt gaps,
active/post-death counts and prey updates per known-prey second, distinguishing
entry, removal, reconnect and censored lifetimes. It neither repeats audit
integrity/RTT analysis nor infers world populations from recipient counts.

### Transport parity

Measure complete elapsed-header protocol-19 captures on receipt and server clocks:

```sh
uv run --locked --only-group tools python tools/fidelity/transport_parity.py measure CAPTURE.jsonl.gz --output outputs/transport.json
uv run --locked --only-group tools python tools/fidelity/transport_parity.py compare tests/fixtures/transport-official-near-server.json outputs/transport.json
uv run --locked --only-group tools python tools/fidelity/local_capture.py --config arena.json --seconds 60 --out outputs/local-transport
```

Set `"delivery": "official"` under `simulation` in that host config; the default
responsive pacing is not official behavior.

`measure` accepts multiple captures, records input/decoder/census/tool SHA-256s
and reports per-capture/setup-epoch and pooled statistics. `compare` prints four
initial gates; exit 1 means failure (including insufficient samples), 2 invalid
input. Optional `--tolerances FILE` supplies JSON with explanatory `comment`
fields. Limits use `max + allowance * max(spread, floor)`, requiring 100 samples
per gated statistic. The common allowance is the smallest of {0.5, 1, 2} passing
all six leave-one-capture-out comparisons: 1. Only integer-header frame-gap p50
uses a 1 ms spread floor; receipt-derived metrics have none. The
[reference selection and limitations](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-server-timing-references/REPORT.md)
pin all six completed #111 batches **unfiltered**, using the same measurement as
`measure`/`local_capture` candidates. Masked development and held-out summaries
remain separate server characterization, with whole-window masking and the
first attempt's held-out exceedance retained. Community recordings are excluded;
raw inputs require local access. `local_capture.py` defaults to this near-server reference. Its
`--reference tests/fixtures/transport-official-reference.json` option retains the
older home-route/collector envelope with its original labels and selection.

Frame cadence and the header-delta histogram expose batching. Anchor correction
is `v_local * gap - 42` world units: local speed is nine anchor spacings divided
by their duration, centered on the measured interval (four neighboring intervals
on either side), separately on each clock. Windows with over 15% reported speed
variation are skipped. The stock client resets `chl` on each anchor and smooths
`dchl` over 53 frames; larger absolute corrections indicate more interpolation
reset work. The report includes RMS and absolute p50/p95/p99, also for snakes
with at least 100 body anchors. It does not simulate the rendered correction.
Own/other per-snake rotation/speed-report gaps use server time and separate
appearance epochs. Packet-family counts/rates reuse the census. Excess delivery
delay is receipt minus cumulative server time, minus a centered rolling minimum
over +/-600 frames, separately after each setup and excluding its frame.

Samples overlap and correlate; receipt timing includes network jitter and
collector stalls. Reports retain `collector-timing` metadata and final counters
(missing older metadata stays unknown), without summing repeated cumulative
counters. No clock here identifies absolute one-way latency or guarantees
stock-client playing equivalence. Unfiltered captures (including the retained
home reference) can have collector stalls that widen their envelope. Reference
and candidate use the same +/-600-frame delay gauge, not #111's slope-corrected
+/-10-second gauge. False-reject control is only the LOO check over six captures;
batch drift was observed, with no population error-rate guarantee or independent
held-out validation. The original masked development envelope and its 1.096 ms
held-out delay p90 exceeding 1.090 ms remain documented as the first attempt.

`local_capture.py` starts `cargo run --release --locked -j 2 -- host --config FILE`,
runs the headless collector, stops both process trees, then measures/compares.
A new output directory retains capture path, frozen config, Git head, logs and
comparison table; the Cargo target is isolated by default. Config must bind a
numeric IPv4 loopback address: other addresses fail before startup, and the host
receives the validated bind override. It never discovers or contacts outside game
servers. Choose representative local world/load in `arena.json`; a small world
cannot establish full-load parity.

Before export, audit the complete study; inspect attempt ledger, source pins and
quarantine status. Retain the audit and selected window identity with analysis.
The standalone exporter reads the supplied capture without auditing the study or
freezing sources; packaging creates the snapshot. Never replace collection-time
source pins to pass a later audit.

Export a preselected complete window:

```sh
uv run --locked --only-group tools python tools/fidelity/prepare-fidelity-replay.py outputs/your-study/raw/ATTEMPT/bot0.jsonl.gz --start 2 --duration 3 --name movement --output outputs/your-study/fixtures/replay.json
cargo run --locked --example fidelity_replay -- outputs/your-study/fixtures/replay.json free
```

The exporter maps sends/receipts onto a server timeline; exact processing ticks
remain unobserved. `free` replay does not overwrite future poses; `motion_only`
is a separately labelled mass-control sensitivity. Record commands, parameters,
tool versions, initialization, missing exposure and uncertainty. Do not tune
window, clock offset or heading estimator to favorable results.

Concurrent clients share a batch; frames, lives, reconnects, filenames and new
UUIDs do not automatically create independent trials. Justify replication across
sessions/times/servers; preserve every planned stratum, failure and inconclusive
result. Distinguish server generation from broadcast filtering, and received
from reconstructed state.

For same-process clients, verify shared batch, `timeOrigin` and collector source
before comparing caches on common monotonic receipt time. At T, consume only
each stream's prefix received by T, preserving packet order and separate field
ages. Never subtract independent connection elapsed clocks or refresh earlier
poses with later packets. Local cache absence needs a positively retained peer
reference and cannot establish world absence. Later continuity checks select
samples retrospectively; keep them separate from causal reconstruction. Shared
receipt time establishes neither simultaneous server geometry nor one-way latency.
The
[exploratory receipt-time recheck](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-visibility-recheck/REPORT.md#exploratory-common-receipt-time-replay)
records a bounded example and its missing validation coverage.

## Package and submit

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py pack outputs/your-study --output outputs/your-study.tar.gz
uv run --locked --only-group tools python tools/fidelity/evidence.py verify outputs/your-study.tar.gz
```

Packaging snapshots sources/data, inventories hashes, recomputes the audit and
verifies before exclusively writing the bundle. Verification never executes
bundled code. See [bundle format](evidence-format.md).

| Exit | Meaning |
| --- | --- |
| 0 | Artifact checks passed. |
| 2 | Incomplete or quarantined evidence, retained for review. |
| 1 | Structural failure: paths, schema, hashes or I/O. |

Scientific status stays `unreviewed`. Submit bundle URL/SHA-256, report,
reproduction commands and a distilled regression fixture, following
[contribution requirements](../CONTRIBUTING.md#evidence-proportional-to-the-change)
for recording storage and external-data restrictions. Review must trace claims
to exact windows, alternatives and held-out checks; byte integrity cannot
establish a scientific conclusion.

## Compare contributed evidence

Copy [comparison.json](https://github.com/NikitaMGrimm/slither-sim/blob/main/evidence/templates/comparison.json).
Replace dummy digests/rationale and name every planned baseline/candidate bundle,
each containing exactly one complete acquisition batch in the audited bundle
format. [Replication limits](#inspect-analyze-and-reproduce) apply. The template
has one placeholder per cohort; testing needs six eligible batches per cohort.
Effect/exposure thresholds are illustrative choices, not measured server tolerances.

Freeze metrics, effect thresholds, sampling/size/boost strata and exclusions
before confirmatory acquisition; retain the declaration with the study.
Retrospective comparisons are exploratory. Digests prove neither plan timing
nor batch independence/exchangeability.

Place local bundle paths relative to the submission file, then run offline:

```sh
uv run --locked --only-group tools python tools/fidelity/compare_evidence.py path/to/comparison.json --output outputs/comparison
```

The tool verifies every bundle, recomputes audits/packet summaries with this
checkout's tools and records tool hashes, without executing bundled scripts or
analysis. Duplicate archives, payloads or capture/batch IDs, overlapping
same-server acquisitions, or changed collector/setup/scenario/origin/framing make
cohorts incomparable. It also compares recorded size/control options, excluding
endpoint/nickname. Match other conditions in the declared design; metadata cannot
remove hidden population, behavior or network confounding.

Available scalar summaries are combined `M`/`V` server-gap medians, leaderboard
server-gap medians, known-prey update server-gap medians, updates per known-prey
second, and mean measured ping RTT. Eligible epoch summaries are averaged per
client, then clients within a batch; batches receive equal weight. `minSamples`
gates each epoch's gap count, known prey update count, or each client's RTT count.
Missing client/batch exposure makes that metric inconclusive, not zero. Each
eligible metric needs `minUnits` batches in both cohorts.

Two-sided label-permutation tests compare the batch scalar means, enumerating
all labelings when affordable or using fixed-seed random permutations with the
conservative `(extreme+1)/(draws+1)` estimate. Holm adjustment includes every
declared metric, assigning untestable metrics p=1. A signal needs both adjusted
p at or below `alpha` and the declared minimum absolute mean shift. These choices
follow standard [permutation-test conventions](https://docs.scipy.org/doc/scipy/reference/generated/scipy.stats.permutation_test.html);
they do not establish exchangeability or sensitivity to every distribution change.

`report.json` retains every bundle identity, declaration, batch value, missing
exposure, effect and p-value; `REPORT.md` summarizes results:
`drift_detected`, `no_drift_detected`, `inconclusive` or `incomparable`, always
scientifically unreviewed. No detected drift does not establish equivalence;
reporting/acquisition changes can cause drift. Screening cannot prove new physics
or automatically replace baselines, fixtures or rules.
Unsupported protocols require a separately reviewed decoder update.

For CI, publish submission JSON and pinned bundles on public GitHub release or
attachment hosts (raw GitHub JSON also works for submissions). Replace local
`source` paths with immutable HTTPS asset URLs and compute each SHA-256.
Submit those identities through the [evidence issue](https://github.com/NikitaMGrimm/slither-sim/blob/main/.github/ISSUE_TEMPLATE/evidence.yml).
A maintainer runs **Evidence comparison** from the default branch with the
submission URL and digest. The manual workflow uses read-only permissions,
trusted checkout code, bounded HTTPS downloads with checked redirects, and no
bundled code execution. It uploads the JSON/Markdown report for review; a drift
or inconclusive result is an artifact, not an automatic scientific pass/fail.
Ordinary PR CI tests this pipeline offline without downloading contributed data.

## Add or update a fixture

Use the [fixture guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/fixtures/README.md) for selection, independent
expectations, protocol labels and replay-export provenance; apply the
[evidence tier](../CONTRIBUTING.md#evidence-proportional-to-the-change) for your change.
