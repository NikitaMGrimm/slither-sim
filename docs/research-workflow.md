# Evidence workflow

Mechanics proposals need measurements and a regression fixture. Tool fixes need
a reproducer and tests; documentation changes do not require new captures.

Use Python 3.12, uv, Rust 1.86+ and Node.js 22+ (24 recommended):

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
Passing checks establish pipeline agreement, not official mechanics or playing
strength. Repeated runs reuse data identities, and thresholds were chosen during
development, not blinded validation.

## Plan a real measurement

Copy [the movement plan](../evidence/templates/movement-plan.json). Specify:

- Study ID, question, competing explanations and method.
- Sampling unit, attempt budget, exclusions and held-out validation.
- Operator-supplied protocol-19 IP/port, optional HTTP(S) `capture.origin` and
  actual `dataOrigin`. The template uses loopback `127.0.0.1:444` and `/slither`.

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

## Inspect, analyze and reproduce

```sh
uv run --locked --only-group tools python tools/fidelity/evidence.py audit outputs/your-study
```

Audits check plan/attempt accounting, file membership, sources, receipts, gzip
completeness, duplicates and batch consistency. Missing attempts are incomplete;
corrupt, interrupted or mismatched captures are quarantined. Before removing a
leftover `.study.lock`, check its owner/process and ledger.

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
outside Git. `REPORT.md` identifies rights holders, covered paths and reuse terms;
repository code/fixtures use MIT and bundled software retains its license.
Review must trace claims to exact windows, alternatives and held-out checks;
byte integrity alone cannot establish a scientific conclusion.

## Offline checks

```sh
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
uv run --locked --only-group tools python -m unittest discover -s tools/fidelity -p 'test_*.py'
uv run --locked --only-group tools node --test tools/fidelity/*.test.mjs
uv run --locked --only-group tools python tools/verify_extraction.py
```

Tests use doubles or loopback, never outside game servers. uv supplies Python for
Node audit subprocesses; worked-study tests build this checkout's Rust binaries.
CI is manual-only.
