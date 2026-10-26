# Performance checks

Offline synthetic workloads measure simulator/client costs, not official servers
or bot playing strength. Check output parity before claiming a behavior-preserving
speedup.

Automatic PR comparisons use a [covered sequential protocol](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/performance-statistics-v2.md)
with early resolution, a 30-minute job cap and at most three interim comment updates.
The commands below use fixed-repeat descriptive comparisons.

The separate [on-demand release comparison](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/release-performance.md)
measures a pinned published release against an explicit candidate commit.

## Compare a change

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

Builds the base ref and working tree with one harness, checks full-state traces,
records supported phase profiles and times adjacent alternating pairs. Use a new
directory outside the checkout or under ignored `outputs/`. Finish builds/tests
first, arrange a quiet window and record interruptions/background activity.

| Option | Purpose |
| --- | --- |
| `--base REF` | Named runtime baseline; the candidate is the current working tree. |
| `--suite ci` / `--suite full` | Choose the catalog suite; `ci` is the default. |
| `--cases NAME ...` | Select explicit workloads; unknown names fail before measurement. |
| `--repeats N` | Measured pairs after one untallied warmup; default 5. |
| `--no-profiles` | Skip separate instrumented runs. |
| `--max-wall-seconds N` | Bound measurement time after builds; see below. |

The snapshot includes tracked edits/deletions and untracked non-ignored files.
A temporary index preserves your index, HEAD, stash and files. The candidate's
measurement examples, selected tests and fixtures overlay the detached baseline;
this shared harness must compile against the base API.

Separate persistent targets live under `target/perf-suite/` (or
`CARGO_TARGET_DIR`). Locks refuse concurrent use; verify ownership before removing
stale locks. Freshly built binaries are copied to output, bound to source/tree,
harness, toolchain and binary hashes, and rehashed around timing. Temporary
worktrees are removed afterward.

## Read the results

| Exit | Meaning |
| --- | --- |
| 0 | Complete, with equal traces. |
| 3 | Complete, with changed traces or workload definitions; timings are labeled behavior-changed. |
| 1 | Incomplete; inspect `failure.json`, logs and retained partial outputs. |

`summary.md`/`summary.json` hold paired ratios; `comparison.json` holds the plan,
parity and build identities. Catalogs must select identical names, iterations
and trace windows. `--base` overlays the candidate harness on both runtimes,
so added and retired workload names use that shared catalog. Copied executables
with one-sided names fail before timing with a plan-mismatch diagnostic; select
common names explicitly to compare them. Incomplete attempts support no speed claim. Changed behavior
may suit a mechanics correction, but is not a behavior-preserving optimization.

Operation wall time excludes setup, trace output and report serialization;
process CPU/wall includes them. Peak working set measures resident pages, not
allocation or total system memory. CPU/peak metrics are Windows-only, explicitly
unavailable elsewhere; short CPU samples can be quantized to 15.625 ms.
Separate instrumented profiles have clock/counter overhead and cannot establish
unprofiled speedups.

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

The [workload guide](performance-workloads.md#steady-state-lifecycle) defines
`-bot` profiles and `observeSeconds`; its [observation fixtures](performance-workloads.md#observation-workloads)
isolate materialization. The [food index study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md)
measures those fixtures and `official-128-bot`/`dome-proxy-96-bot`, sequentially
and with `--parallel`. Food visibility filtering dominates batch cost.

A batch builds a food grid with at least 8 observers, at least 32,768 observers ×
pellets and vision from 0 to 2^40; smaller batches scan directly. It is rebuilt
per batch, refining cells only when enough observers repay them. Construction is a small
share of sequential cost but precedes the parallel split; the study explains
why it is not reused across steps.

## Controller transport

`tools/perf_transport.py` measures [`serve` controller](bot-api.md) costs, including
reply reading/parsing, beyond the Rust-only workloads above. Its scenario table
defines `small`, `official-128`, `dense-64`, `heavy-food` and 16-world `worlds-16`.
Deterministic scripts use seed 7, six decision ticks and headings independent of
observations, so all three modes simulate identical worlds:

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

Requires Node.js 22+ on `PATH`; starts only the given executable, with no network
service. Reports mean reply size and median ms/step with min–max over `--repeats`
(default 3), rotating mode order between repeats. `--output` creates a directory
with a private binary copy, scripts, raw `report.json` and `summary.md`.
`--scenarios`, `--modes` and `--steps` bound runs; `--threads N` reaches every
`serve`. Different reply bytes across modes/repeats or client checksums fail the
run. Per-step times exclude reset, startup and request encoding (scripts are pre-encoded).
Protocol-specific code stays in request encoders and client `decode`/`decide`.

The [transport v2 study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-transport-v2/REPORT.md)
holds the `slither-sim-json-v1` baseline, the v2 results and the thread comparison.

## Comparison deadlines

`--max-wall-seconds` shares one monotonic deadline across metadata and measurement
children, after builds. Leave startup, file-operation and cleanup margin. It
cannot extend the 30-second metadata or 120-second native-child limits and is
not an OS CPU/memory cap.

Expiry blocks the next child or interrupts the current one. Cleanup kills it,
waits up to ten seconds and closes pipes/error files. Flushed rows remain;
`failure.json` records budget and elapsed time. Failed native attempts retain
exact stdout in `.stdout.bin` and metadata in `.failure.json`, replacing the
stderr file's `.txt` suffix. Diagnostics are not completed measurements;
incomplete runs have no summary. Rehash retained binaries and use fresh output
for another attempt.

## Compare copied executables

For specialized harnesses or copied-binary studies, build each snapshot with the
checked-in lockfile, release profile and separate targets. For the baseline/CLI:

```sh
cargo build --release --locked --bin slither-sim --example perf_baseline -j 2
```

Copy executables out of mutable Cargo targets into separate owned output storage
before timing. Record build command/log, immutable source identity (including edits), toolchain and binary
SHA-256 at build time. Hashes/caches alone do not prove source identity, and these
runners do not enforce it. Rebuild uncertain inputs before parity and timing.

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
