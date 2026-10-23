# Evidence workflow

Choose the [evidence tier](../CONTRIBUTING.md#evidence-proportional-to-the-change)
before starting. Existing evidence can support mechanics corrections; routine
tool/API fixes need a reproducer and regression, not a new study. New empirical
or fitted claims need the complete relevant study and its limitations.

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

Copy [the movement plan](../evidence/templates/movement-plan.json). Specify:

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
and stops the child after the duration plus 30 seconds for draining.

Failures, cancellation and timeout remain in the ledger, including incomplete gzip
files. Exit zero alone is not successful acquisition. Other advertised protocols
are quarantined with their raw bytes retained; `setupSignatures[].protocolVersion`
records the observed version. Unsupported login challenges fail without executing
JavaScript and require a reviewed parser update.

Low-level paired death/prey and feeding-shape collectors need their own matching
coordinator and attempt accounting; the managed wrapper does not cover those modes.

For two-client visibility crossings, the paired-prey collector supports fixed
`x`, `y`, `diagonal-positive` and `diagonal-negative` directions:

```sh
node tools/fidelity/slither-paired-prey.mjs --out outputs/paired-diagonal --server 127.0.0.1:8766 --seconds 480 --mode crossing --direction diagonal-positive --plan path/to/paired-plan.json
```

Offsets are max-axis distances; diagonal `(800,800)` is about 1,131 units away.
The logs retain projected along-route/cross-track distances, visibility, freshness,
interruptions and completed legs. An intended route or phase name is not proof of
an achieved crossing. Freeze candidate geometry/retention, calibration/held-out
legs, attempt budget and required coverage before connecting. Preserve failed
joins and empty cohorts; they cannot validate a new visibility rule.

Raw frames retain direction, bytes and monotonic send/receipt timestamps. Metadata
pins the plan/collector sources, wall-clock origin, connection/life boundaries,
ping sends/replies/RTT and censored requests, including post-death observation.
Server elapsed headers give a separate received clock. The audit derives RTT and
arrival-gap summaries from those same frames; no second timing log is needed.
These clocks cannot identify one-way latency or hidden processing ticks.

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

Copy [comparison.json](../evidence/templates/comparison.json). Replace its dummy
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
Submit those identities through the [evidence issue](../.github/ISSUE_TEMPLATE/evidence.yml).
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
Git; the [worked study](../tools/fidelity/worked_study.py) demonstrates this path.

## Offline checks

Run focused tests for the changed owner first (see the
[task map](../CONTRIBUTING.md#find-the-change)), then the relevant group from the
repository root. Documentation-only changes need accuracy/link review, not builds
or the research stack. Rust 1.86+ and platform build tools suffice for `rust` and
`package`; the runner itself uses standard-library Python (3.11+).

| Group | Command | Coverage / prerequisites |
| --- | --- | --- |
| Rust | `python tools/check.py rust` | Cargo-only fmt, default and core-only tests, Clippy and doctests; no uv or Node |
| Tools | `uv run --locked --only-group tools python tools/check.py tools` | Root tools/fidelity Python, Node and local bot smoke tests; locked uv, Node 22+ and Rust |
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
