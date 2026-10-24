# Evidence workflow

Choose the [evidence tier](../CONTRIBUTING.md#evidence-proportional-to-the-change)
before starting. Existing evidence can support mechanics corrections; routine
tool/API fixes need a reproducer and regression, not a new study. New empirical
or fitted claims need the complete relevant study and its limitations.

Go directly to the task you need:

- [Try the offline demonstration](#try-the-complete-offline-workflow).
- [Plan a measurement](#plan-a-real-measurement), including [controlled routes](#controlled-snake-and-food-routes).
- [Audit, analyze and replay](#inspect-analyze-and-reproduce).
- [Package and submit](#package-and-submit) or [compare contributed bundles](#compare-contributed-evidence).
- [Evolve a fixture or protocol](#fixture-and-protocol-evolution).
- [Run offline checks](#offline-checks); documentation and Rust-only work can start there.

For the evidence workflow below, use Python 3.12, uv, Rust 1.86+ and Node.js 22+
(24 recommended). Rust-only and documentation work can skip this setup:

```sh
uv sync --locked --only-group tools
```

Run commands from the checkout. Use a fresh output directory per study; existing
plans, captures and bundles are never overwritten.

## Try the complete offline workflow

```sh
uv run --locked --only-group tools python tools/fidelity/worked_study.py --output outputs/evidence-demo
uv run --locked --only-group tools python tools/fidelity/evidence.py audit outputs/evidence-demo
uv run --locked --only-group tools python tools/fidelity/evidence.py pack outputs/evidence-demo --output outputs/evidence-demo.tar.gz
uv run --locked --only-group tools python tools/fidelity/evidence.py verify outputs/evidence-demo.tar.gz
```

The first command builds the simulator/replay example, generates six seconds of
steering traffic, audits it and replays a three-second window. It uses no network
or browser. Inspect `REPORT.md`, `analysis/results.json`, `analysis/replay/` and
`fixtures/`.

All data is labelled **synthetic, local-simulator**. The study compares causal
heading reconstruction with a known-state control and timing/heading perturbations;
corruption and truncation have separate tests. Initialization errors are retained.
Passing checks establish pipeline agreement, not server mechanics or playing
strength. Repeated runs reuse data identities, and thresholds were chosen during
development, not blinded validation.

## Plan a real measurement

Collection tools target protocol-19 WebSocket servers at an explicit IPv4
address/port and `/slither` path. They do not discover servers. The template
targets the local host at `127.0.0.1:8766`; use `dataOrigin: local-simulator` for
that source, or `community-server` for another compatible server. Set an HTTP(S)
`capture.origin` only when required by the endpoint. Recordings contain raw
received packets and sent controls; they do not include server-only state.

Copy [the movement plan](https://github.com/NikitaMGrimm/slither-sim/blob/main/evidence/templates/movement-plan.json). Specify:

- Study ID, question, competing explanations and method.
- Sampling unit, attempt budget, exclusions and held-out validation.
- Operator-supplied protocol-19 IP/port, optional HTTP(S) `capture.origin` and
  actual `dataOrigin`. The template uses loopback `127.0.0.1:8766` and `/slither`.

For confirmatory work, freeze predictions and selection rules before collection.
Keep exploratory results labelled exploratory; a plan hash cannot prove when or
why a plan was written. Fit and validate on separate data.

Initialize and inspect the command without connecting:

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py init --plan path/to/your-plan.json --output outputs/your-study
uv run --locked --only-group tools python tools/fidelity/collect.py outputs/your-study --dry-run
```

To collect on that server:

```sh
uv run --locked --only-group tools python tools/fidelity/collect.py outputs/your-study
```

Each invocation durably reserves one attempt; there are no automatic retries or
replacement captures. Limits are 1–3 clients and 3,600 seconds per attempt; the
template uses one client for 60 seconds. Scenario-specific client counts are
validated. The wrapper passes the endpoint/Origin, saves stdout/stderr in `logs/`,
and stops the child after the duration plus 30 seconds for draining. It saves
the process exit status in `attempts.jsonl` before auditing the recordings; an
audit exception does not erase that outcome or make the attempt available again.

Failures, cancellation and timeout remain in the ledger, including incomplete gzip
files. Exit zero alone is not successful acquisition. Other advertised protocols
are quarantined with their raw bytes retained; `setupSignatures[].protocolVersion`
records the observed version. Unsupported login challenges fail without executing
JavaScript and require a reviewed parser update.

For paired-prey acquisition, set these fields in the plan's `capture` object
alongside its server and optional Origin, then use the same `init`, `collect`,
`audit` and `pack` commands:

```json
{"scenario": "paired-prey", "bots": 2, "seconds": 480,
 "mode": "crossing", "direction": "diagonal-positive"}
```

Paired plans require exactly two clients, 1–3,600 seconds and an explicit `mode`
of `overlap` or `crossing`. A fixed `direction` is optional for crossing and
invalid for overlap. Audits compare the recorded mode/direction against the frozen
plan as well as checking both members and source hashes. A completed capture
does not certify a valid crossing or statistical coverage.

The duration ceiling is an operational capture budget, not a server constant.
The paired CLI and `recordPair` still default to 480 seconds when duration is
omitted; managed plans require an explicit duration. Historical 480-second
studies retain their declared duration and scientific/control settings.

Low-level paired-death and feeding-shape collectors still need their own matching
coordinator and attempt accounting.

### Controlled snake and food routes

The managed workflow also accepts these separate acquisition scenarios:

```json
{"scenario": "paired-snake", "bots": 2, "seconds": 480,
 "mode": "crossing", "direction": "x"}
```

The direct-snake route uses A's own received pose as a reference while B moves
between fixed near/far waypoints. It keeps running when the peer disappears from
an opponent cache; that disappearance is an observable outcome. Plans require
exactly two clients, an explicit crossing direction and 1–3,600 seconds.
Directions are normalized: the 800/2,300-unit rail offsets are Euclidean,
including diagonals. A nominally orbits an 80-unit reference circle; B holds each
achieved endpoint for two seconds. These settings and safety margins are
acquisition controls, not recovered server visibility rules.

Only B records canonical `paired-snake-endpoint` witnesses. Initial approach is
excluded from completed legs. A shared trial binds identities, center and axis;
`paired-snake-trial-end` records abandonment or capture-end censoring before
reset. Avoidance interruptions, actual anchors, cache membership, intended and
sent headings remain available for analysis. Counters and intended waypoints
alone do not certify an uninterrupted traversal or fresh causal query.
Both own anchors must be at most 150 ms old for endpoint recognition and new
heading commands. A brief stale interval pauses control while retaining the
trial's center, targets and hold deadline; it counts as an interruption. Both
anchors must refresh after pause entry before control resumes. Ages over 500 ms
or failure to reacquire within 1,000 ms abandon the trial. Pausing suppresses new
headings and boost; the last delivered heading continues physically. Holds and
leg deadlines keep running, and interrupted legs cannot establish an
uninterrupted round trip. These are engineering bounds, separate from the frozen
analysis freshness screens and opponent-cache absence.

```json
{"scenario": "food-lanes", "bots": 1, "seconds": 480}
```

The food route uses one client for 1–480 seconds, with no paired mode/direction.
It selects one interior site, then schedules eight reciprocal passes in A/B/B/A
lane blocks, 75 units apart within one 300-unit sector row. The first four slots
are preassigned calibration and the last four holdout; approach and transfers
are excluded. Route failure retains missing slots and stops without reconnecting
or selecting a replacement site. `food-lane-phase-end` records achieved endpoints
and interruptions; `food-lane-route-end` preserves the terminal outcome.
Initial site screening requires received opponent body polylines, including
their extrapolated tips, to clear both lanes, the west transfer and the approach
by more than 800 units. This conservative acquisition margin cannot guarantee
future or unreceived safety; the route's danger veto remains active.

Both scenarios use the standard recorder, source/plan pins, managed attempt
ledger and audit/package commands. The driver can complete an intact recording
of an aborted route. Inspect the route outcome and packet-derived denominators
separately from integrity status before scaling a study. Fit/validation splits,
freshness screens and required coverage belong in the frozen analysis plan.
Food announcements and explicit clears describe delivery episodes; unannounced
empty sectors never establish a known subscription or global food absence.

For two-client visibility crossings, the paired-prey collector supports fixed
`x`, `y`, `diagonal-positive` and `diagonal-negative` directions:

```sh
node tools/fidelity/slither-paired-prey.mjs --out outputs/paired-diagonal --server 127.0.0.1:8766 --seconds 480 --mode crossing --direction diagonal-positive --plan path/to/paired-plan.json
```

Offsets are max-axis distances. The current near offset is 650 (about 919 units
on a diagonal), the far offset is 1,850, and the reference offset is 450. After
recording an endpoint, B keeps that relative waypoint for two seconds before the
next traversal. `crossingHeldOffset` and `crossingHoldUntil` describe the hold;
`intendedOffset` records the commanded offset even when `crossingLeg` already
names the upcoming leg. These are acquisition controls, not measured server
ranges or timers. Older source-pinned runs retain their 800-unit near waypoint
and immediate reversal.

The logs retain projected along-route/cross-track distances, visibility, freshness,
interruptions and completed legs. An intended route or phase name is not proof of
an achieved crossing. Freeze candidate geometry/retention, calibration/held-out
legs, attempt budget and required coverage before connecting. Preserve failed
joins and empty cohorts; they cannot validate a new visibility rule.

When an active crossing trial is abandoned, both archives retain a
`paired-prey-trial-end` event before clearing its state, including on the same
update as reacquisition or when a client cannot run movement. `endedAt` is the
coordinator's monotonic update time; the enclosing `t` is the recorder time.
The event preserves the old pair/target/trial identities, leg, hold and endpoint
counts. Its `reason` describes the collector branch: `pair_unavailable` does not
distinguish death from disconnection, and `primary_visibility_lost` means the
primary cache is absent or too old. Neither proves a server subscription change.
Match the event across recipients by trial and `endedAt`; it is one shared
control event, not two independent observations. A trial still active at capture
end is right-censored. Older source-pinned archives lack these events; do not
infer their reset reasons from a later phase name.

Crossing endpoint advancement requires the reference near its assigned offset,
both snake anchors at most 150 ms old and the reference prey report at most
350 ms old. These are collector/study quality gates, not server constants.
`endpointVisits` includes the initial approach; `completedLegs` counts subsequent
legs between achieved endpoints. Observer B records each exact start/end witness
once in `crossingEndpoint`. Earlier collector versions counted endpoint arrivals
as completed legs; their source-pinned recordings retain that historical meaning.
These endpoint checks do not establish continuous geometry, avoidance-free travel,
clock alignment, peer continuity or held-out coverage; analysis must still check them.

The paired collector tests also rehearse complete near/far round trips using the
actual quantized, rate-limited sent commands. Their point robots have explicit
synthetic speed/turn settings, exact fresh anchors and no obstacles; stationary
and moving targets cover both signs of each route. This protects control wiring
without tying it to measured physics defaults or claiming live acquisition success.

Raw frames retain direction, bytes and monotonic send/receipt timestamps. Metadata
pins the plan/collector sources, wall-clock origin, connection/life boundaries,
ping sends/replies/RTT and censored requests, including post-death observation.
Server elapsed headers give a separate received clock. The audit derives RTT and
arrival-gap summaries from those same frames; no second timing log is needed.
These clocks cannot identify one-way latency or hidden processing ticks.
JSONL metadata records are objects; frame records are three-element arrays.
Use the shared audit/decoder rather than treating every record as an object.

## Inspect, analyze and reproduce

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py audit outputs/your-study
```

Audits check plan/attempt accounting, file membership, sources, receipts, gzip
completeness, duplicates and batch consistency. Missing attempts are incomplete;
corrupt, interrupted or mismatched captures are quarantined. Before removing a
leftover `.study.lock`, check its owner/process and ledger.

For packet families, byte counts and cadence, analyze an audited complete capture:

```sh
uv run --locked --only-group tools python tools/fidelity/packet_census.py outputs/your-study/raw/ATTEMPT/bot0.jsonl.gz --output outputs/your-study/analysis/packets.json
```

Create `analysis/` first. The census refuses existing output, incomplete captures
and non-19 setups. It includes archive/tool hashes, separate server/receipt gaps,
active/post-death counts and prey reporting rates per known-prey second. Entry,
removal, reconnect and censored lifetimes remain distinct. It does not repeat the
audit's integrity/RTT analysis or infer world populations from recipient counts.

Before export, audit the complete study and inspect its attempt ledger, source
pins and quarantine status. Preserve that audit result and the selected window
identity with the analysis. The standalone exporter reads the supplied capture;
it does not perform a study audit or freeze a source snapshot. Packaging below
creates the source snapshot. Do not replace collection-time source pins to make
a later audit pass.

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

Concurrent clients share a batch. Frames, lives, reconnects, filenames and new
UUIDs are not automatically independent trials. Justify replication across
sessions/times/servers and preserve every planned stratum, failed attempt and
inconclusive result. Distinguish server generation from broadcast filtering and
received state from reconstructed state.

For clients recorded in the same process, verify shared batch, `timeOrigin` and
collector source before comparing their caches on the common monotonic receipt
clock. At time T, consume only each stream's packet prefix received by T, keeping
packet order and separate field ages. Never subtract independent connection
elapsed clocks or let later packets refresh an earlier pose. Local cache absence
is meaningful only against a positively retained peer reference; it is not world
absence. Any later continuity checks select the sample retrospectively and must
stay separate from causal state reconstruction. Shared receipt time does not
establish simultaneous server geometry or one-way latency. The
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

Scientific status stays `unreviewed`. Submit the bundle URL/SHA-256, report,
reproduction commands and a distilled regression fixture. Keep full recordings
outside Git. For external data, note its source and sharing restrictions in `REPORT.md`.
Review must trace claims to exact windows, alternatives and held-out checks;
byte integrity alone cannot establish a scientific conclusion.

## Compare contributed evidence

Copy [comparison.json](https://github.com/NikitaMGrimm/slither-sim/blob/main/evidence/templates/comparison.json). Replace its dummy
digests and acquisition rationale, and name every planned baseline/candidate
bundle. Each bundle must contain exactly one complete acquisition batch under
the existing audited bundle format. Concurrent bots, reconnects and packets do
not add independent samples. The template lists only one placeholder per cohort;
it needs six eligible batches per cohort before testing. Its effect/exposure
thresholds are illustrative choices, not measured server tolerances.

Freeze metrics, effect thresholds, sampling/size/boost strata and exclusions
before confirmatory acquisition; preserve the declaration with that study.
Retrospective comparisons remain exploratory. A digest does not prove when a
plan was written or that acquisition batches are independent/exchangeable.

Place local bundle paths relative to the submission file, then run offline:

```sh
uv run --locked --only-group tools python tools/fidelity/compare_evidence.py path/to/comparison.json --output outputs/comparison
```

The tool verifies every bundle, recomputes capture audits and packet summaries
with this checkout's tools, and records their hashes. Bundled scripts/analysis
are never executed. Duplicate archives, payloads or capture/batch IDs, overlapping
same-server acquisitions, and changed collector/setup/scenario/origin/framing
make cohorts incomparable. Recorded size/control options are compared too
(endpoint and nickname excluded). Match other conditions in the declared design;
metadata cannot remove hidden population, behavior or network confounding.

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
exposure, effect and p-value; `REPORT.md` summarizes the outcome. Results are
`drift_detected`, `no_drift_detected`, `inconclusive` or `incomparable`, always
scientifically unreviewed. No detected drift is not equivalence; drift can come
from reporting or acquisition changes. This is a screening tool, not proof of
new physics, and does not automatically replace a baseline, fixture or rule.
Unsupported protocols require a separately reviewed decoder update.

For CI, publish a submission JSON and its pinned bundles on public GitHub release
or attachment hosts; a raw GitHub JSON URL also works for the submission. Replace
local `source` paths with immutable HTTPS asset URLs and compute each SHA-256.
Submit those identities through the [evidence issue](https://github.com/NikitaMGrimm/slither-sim/blob/main/.github/ISSUE_TEMPLATE/evidence.yml).
A maintainer runs **Evidence comparison** from the default branch with the
submission URL and digest. The manual workflow uses read-only permissions,
trusted checkout code, bounded HTTPS downloads with checked redirects, and no
bundled code execution. It uploads the JSON/Markdown report for review; a drift
or inconclusive result is an artifact, not an automatic scientific pass/fail.
Ordinary PR CI tests this pipeline offline without downloading contributed data.

## Fixture and protocol evolution

Keep bounded selected cases in `tests/fixtures/`; keep full recordings and study
bundles outside Git. Add a case for each new scenario and preserve valid old
regressions. Avoid wholesale fixture reformatting. Record the source study/window
and distinguish raw recorded packets from decoder- or model-derived expectations;
label observed and synthetic inputs explicitly. Synthetic cases test a specified
contract, not an empirical server claim.

Pin the wire protocol, API contract and fixture schema where relevant for each
fixture/test, using the fixture version inventory documented in
[the fixture guide](../tests/fixtures/README.md). Current wire support is protocol 19;
a future default may change. Older evidence keeps its original labels. A default
change needs explicit new cases and compatibility/rejection expectations, not a
bulk relabelling of recordings. Retain old decoder/rejection coverage according
to the versions actually supported; retained historical fixtures do not promise
runtime legacy support or establish support for an unverified newer version.

Expected results must have an independent justification: recorded bytes, a
specified contract, a checked calculation or supporting evidence. Regenerating
expectations from the changed implementation cannot by itself approve a regression.
If an expectation was wrong, document its independent correction and preserve the
original evidence. Deprecating a case requires a reason tied to the supported
contract or invalid evidence, with a replacement or retained rejection test where
applicable. A passing decoder cross-check is not independent proof of its model.

Keep a measured calibration gate tied to its recorded corpus. Test configurable
mechanisms with deliberately synthetic parameters, so a new supported default
does not require changing many unrelated lifecycle tests. Protocol bytes,
rejection cases, invariants and frozen-corpus selection checks stay explicit.
New conflicting observations need a new corpus and explained regime/supersession;
replacing old data merely to make the current model pass hides a regression.

For a new regression, retain a small selected raw packet/window with the archive
SHA-256, record indices, wire version, decoder identity and selection rule. Derive
expected fields with the existing decoder or replay exporter, label that derivation,
then have the focused test consume the fixture. Keep the full audited study outside
Git; the [worked study](https://github.com/NikitaMGrimm/slither-sim/blob/main/tools/fidelity/worked_study.py) demonstrates this path.

The standalone replay exporter hashes and decodes one temporary archive snapshot.
Its schema-1 `provenance` retains the archive, selected window/connection, wire
protocol and decoder/exporter source digests. These additional metadata fields
leave replay values unchanged; older fixtures keep their original provenance.
The exporter checks gzip completion and window coverage, but does not perform
the full capture/study audit. Its output labels that limit explicitly; audit the
study separately before treating the derivative as evidence.

## Offline checks

Run focused tests for the changed owner first (see the
[task map](../CONTRIBUTING.md#find-the-change)), then the relevant group from the
repository root. Documentation-only changes need accuracy/link review, not builds
or the research stack. Rust 1.86+ and platform build tools suffice for `rust` and
`package`; the runner itself uses standard-library Python (3.11+).

| Group | Command | Coverage / prerequisites |
| --- | --- | --- |
| Rust | `python tools/check.py rust` | Cargo-only fmt, default, core-only and C ABI tests, workspace Clippy and doctests; no uv or Node |
| Tools | `uv run --locked --only-group tools python tools/check.py tools` | Root tools/fidelity Python, Node and local bot and C ABI smoke tests; locked uv, Node 22+ and Rust |
| Package | `python tools/check.py package` | Local `cargo package` verification; no registry publication |
| WASM | `python tools/check.py wasm` | Explicit target check; install `wasm32-unknown-unknown` first |

For tools, prepare the environment with `uv sync --locked --only-group tools`.
`all` runs the host groups (`rust`, `tools`, `package`); WASM is explicit and needs
its installed target. Append `--list` to a group command to print commands without
executing them. Direct focused Cargo commands are also valid, for example:

```sh
cargo test --locked --test client_protocol
cargo test --locked --no-default-features --test core_consumer
```

Tests use doubles or loopback, never outside game servers. uv supplies Python for
Node audit subprocesses; worked-study tests build this checkout's Rust binaries.
PR CI automatically invokes the check runner. Report checks actually run and
material gaps; a test pass does not establish scientific validity. Import
attribution remains in [historical provenance](provenance.md); routine edits do
not refresh import manifests. Per-study source identity checks still apply.
