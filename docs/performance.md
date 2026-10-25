# Performance checks

These offline synthetic workloads measure simulator/client costs. They do not
measure official servers or bot playing strength. For a behavior-preserving
optimization, check output parity before interpreting timings.

Automatic PR comparisons use a separate [covered sequential protocol](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/performance-statistics-v2.md),
with early resolution, a finite 30-minute job cap and at most three interim
comment updates. The commands below remain fixed-repeat descriptive comparisons.

The separate [on-demand release comparison](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/release-performance.md)
measures a pinned published release against an explicit candidate commit.

## Compare a change

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

This builds the base ref and current working tree with the same harness, checks
full-state traces, records supported phase profiles and runs adjacent alternating
timing pairs. Use a new output directory outside the checkout or under ignored
`outputs/`. Finish other builds/tests first and arrange a quiet measurement window;
record interruptions and background activity.

| Option | Purpose |
| --- | --- |
| `--base REF` | Named runtime baseline; the candidate is the current working tree. |
| `--suite ci` / `--suite full` | Choose the catalog suite; `ci` is the default. |
| `--cases NAME ...` | Select explicit workloads; unknown names fail before measurement. |
| `--repeats N` | Measured pairs after one untallied warmup; default 5. |
| `--no-profiles` | Skip separate instrumented runs. |
| `--max-wall-seconds N` | Bound measurement time after builds; see below. |

The candidate snapshot includes tracked edits/deletions and untracked non-ignored
files. A temporary Git index leaves your index, HEAD, stash and files untouched.
The candidate's measurement examples, selected tests and fixtures are overlaid on
the detached baseline so both runtimes use the same harness; it must compile
against the base API.

Each side has a separate persistent target under `target/perf-suite/` (or
`CARGO_TARGET_DIR`). A lock refuses concurrent use; check its owner before removing
a stale lock. Binaries are freshly built, copied to the output directory and bound
to source/tree, harness, toolchain and binary hashes. Hashes are checked around
timing. Temporary worktrees are removed after the run.

## Read the results

| Exit | Meaning |
| --- | --- |
| 0 | Complete, with equal traces. |
| 3 | Complete, with changed traces or workload definitions; timings are labeled behavior-changed. |
| 1 | Incomplete; inspect `failure.json`, logs and retained partial outputs. |

`summary.md` and `summary.json` contain paired ratios; `comparison.json` retains
the plan, parity results and build identities. Both catalogs must select the same
names, iterations and trace windows. Incomplete attempts cannot support a speed
claim; changed behavior may be appropriate for a mechanics correction but is not
a behavior-preserving optimization.

Operation wall time excludes setup, trace output and report serialization.
Process CPU/wall includes them; peak working set measures resident pages, not
allocation or total system memory. Process CPU and peak metrics are Windows-only
and explicitly unavailable elsewhere. Short CPU samples can be quantized to
15.625 ms. Instrumented phase profiles are separate runs with clock/counter
overhead; do not interpret them as unprofiled speedups.

Inspect living populations, admitted food and operation counts before comparing
throughput. Compare identical workloads, seeds, sample sizes, machine, toolchain,
build flags and harness bytes. These fixtures are not a production distribution;
small or mixed timing shifts remain inconclusive.

Trace mode is untimed and retains canonical full simulator state, serialized
RNG/motion state, events and relevant observations/client frames/wire bytes.
Compare complete decompressed bytes and require a terminal scenario record.
Real-time host checks use deterministic wire replay and host contract tests,
without asserting equal wall-clock receipts.

## Workload selection

```sh
cargo run --release --locked --example perf_baseline -- --list
```

The catalog owns names, suites, sample sizes, resolved definitions and profile
support. The [workload guide](performance-workloads.md) explains scenario
lifecycles, validity checks, adding cases and the specialized observation/retained
client harnesses. [PR comparisons](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md) use the same catalog.

## Observation batches

`-bot` rows time a step followed by `observe_all_into`; with `--profile`, the
report's `observeSeconds` holds the observation interval alone. Observation
changes have been measured on `official-128-bot` and `dome-proxy-96-bot`, sequentially
and with `--parallel`, and on the frozen `observe-*` fixtures. Filtering food by
each slot's vision dominates batch cost. A batch builds a food grid only with
at least 8 observers, at least 32,768 observers × pellets and a vision radius
from 0 to 2^40; smaller batches scan the food list directly. That grid is rebuilt
for each such batch, with finer cells only when enough observers repay them. It
is a small share of a sequential batch, but it runs before the parallel split. The [observation food index study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md)
has the phase breakdown and explains why the grid is not reused across steps.

## Controller transport

The workloads above time only the Rust side. `tools/perf_transport.py` times what a
[`serve` controller](bot-api.md) experiences, including reading and parsing each
JSON reply. It generates one deterministic request script per scenario (seed 7,
six decision ticks, headings that never read observations, so every mode simulates
the same worlds). The scenario table at the top of the script defines `small`,
`official-128`, `dense-64`, `heavy-food` and the 16-world `worlds-16`. Each
scenario runs in three modes:

- `server` pipes the whole script into `serve` and drains stdout through a pipe.
  It times from the end of the reset reply to EOF: simulation, serialization and
  pipe writes, with no client in the loop.
- `python` and `node` are closed-loop reference controllers using only `json` or
  `JSON.parse` (`tools/perf_transport_client.py` and `.mjs`). Per step they time
  **wait** (write the request, block for the reply line: server plus pipe),
  **parse** and **decide**, which folds every living slot's nearest visible food
  into a checksum. Decisions are not sent.

```sh
cargo build --release --locked --bin slither-sim
uv run --locked --only-group tools python -B tools/perf_transport.py --output outputs/transport/run
```

It needs Node.js 22+ on `PATH`, starts only the given executable and contacts no
network service. The table reports mean reply size and median ms/step with the
min–max over `--repeats` (default 3); the mode order rotates between repeats.
`--output` names a new directory for a private binary copy, the request scripts,
`report.json` with every raw sample and `summary.md`. `--scenarios`, `--modes`
and `--steps` bound a run; `--threads N` passes `--threads N` to every `serve`.
The run fails if any mode or repeat receives different reply bytes or the
clients' checksums differ. The reset reply, process startup and request encoding
(scripts are pre-encoded) are excluded from per-step times. Protocol-specific
code is limited to the request encoders and each client's `decode`/`decide`
functions.

The [transport v2 study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-transport-v2/REPORT.md)
holds the `slither-sim-json-v1` baseline, the v2 results and the thread comparison.

## Comparison deadlines

`--max-wall-seconds` gives metadata commands and measurement children one shared
monotonic deadline; builds happen before it starts. Leave margin for startup,
file operations and cleanup. The option cannot extend the existing 30-second
metadata or 120-second native-child limits. It is not an OS CPU/memory cap.

Expiry prevents the next child or interrupts the current one. Cleanup kills the
owned child, waits up to ten seconds and closes its pipes/error file. Flushed rows
and diagnostics remain, and `failure.json` records the budget and elapsed time.
Failed native attempts retain exact stdout bytes in a `.stdout.bin` file and
failure metadata in `.failure.json`, beside that attempt's stderr file (replacing
its `.txt` suffix). These diagnostics never count as completed measurements.
An incomplete run has no comparison summary. Rehash retained binaries before a
separate attempt; use a fresh output directory.

## Compare copied executables

For specialized harnesses or an existing copied-binary study, build each source
snapshot with the checked-in lockfile and release profile, using separate targets.
For the baseline harness and CLI:

```sh
cargo build --release --locked --bin slither-sim --example perf_baseline -j 2
```

Copy each executable out of its mutable Cargo target before timing. Record the
build command/log, immutable source identity (including edits), toolchain and
binary SHA-256 at build time. A binary hash or cached build alone does not prove
which sources produced it. The copied-binary runners do not enforce that check;
reject uncertain inputs and rebuild before parity checks and timing.

```sh
python -B tools/perf_compare.py --baseline PATH/TO/BASELINE --candidate PATH/TO/CANDIDATE --output outputs/perf-comparison
```

This mode defaults to the catalog's `ci` suite and accepts `--suite`/`--cases`;
it does not trace. Both catalogs must agree. The separate `tools/perf_baseline.py`
runner also accepts `--harness` and `--cli` for end-to-end reports. Its
`e2e-client` includes Python/JSON/process overhead; `e2e-host` additionally runs a
temporary loopback host and is cadence-limited. The host is stopped on success or
error. Hosted receipts have no deterministic response hash.

The runners use standard-library Python. Existing locked environments can be
reused with `uv run --project PATH --locked --only-group tools --no-sync --offline
python -B ...`. Preserve raw results before cleaning ignored outputs. Dated
measurements and decisions live in the
[research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md).
