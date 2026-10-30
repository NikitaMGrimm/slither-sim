# Contributing

Keep each PR focused on one problem. Discuss larger features or interface changes
with the maintainer before starting; small fixes can go straight to a PR.
Use one task, branch and worktree; preserve others' work.
Start with the [API guides](README.md#choose-an-interface) and
[rules and limits](docs/server-rules.md), or use the [documentation map](docs/README.md)
for research or maintenance tasks.

## Find the change

Paths are repository-relative. Read the guide, trace the source owner and callers,
then run the matching test.

| Task | Guide and source / tests | Focused check |
| --- | --- | --- |
| Mechanics, motion, food, prey | [Rules, limits and next steps](docs/server-rules.md#packaged-regression-evidence); `src/simulator.rs`, `src/physics.rs`, `src/prey.rs`; matching `tests/*.rs` and `tests/fixtures/` | `cargo test --locked --test movement_turns` (choose the affected test) |
| Custom Battledome rules and presets | [Battledome](docs/configuration.md#battledome); `src/simulator.rs`, `src/spawn.rs`; `tests/battledome.rs` | `cargo test --locked --test battledome` |
| Bot JSON API and lifecycle | [Client API](docs/client-api.md), [direct API](docs/bot-api.md); `src/main.rs`, `src/client/mod.rs`, `src/client/received.rs`; `tests/client_contract.rs`, `tests/client_runner.rs`, `tests/transport.rs` | `cargo test --locked --test client_contract` |
| In-process C ABI | [C ABI](docs/c-api.md); `ffi/src/lib.rs`, `ffi/include/slither_sim.h`, `examples/ffi_bot.py`; `ffi/tests/abi.rs` | `cargo test --locked -p slither-sim-ffi` |
| Python package | [Python package](docs/python.md); `python/src/lib.rs`, `python/slither_sim.pyi`, `python/examples/`; `python/tests/test_session.py` | `uv run --locked --only-group tools python -m maturin build --locked -m python/Cargo.toml`, then `python python/tests/run.py target/wheels` |
| Wire decoder and local host | [Server guide](docs/raw-server.md); `src/client/protocol.rs`, `src/client/server.rs`; `tests/client_protocol.rs`, `tests/client_server.rs` | `cargo test --locked --test client_protocol` |
| Capture, audit and recorded replay | [Research workflow](docs/research-workflow.md); `tools/fidelity/evidence.py`, `tools/fidelity/prepare-fidelity-replay.py` and adjacent tests; `examples/fidelity_replay.rs` | Matching Python/Node test; `cargo test --locked --example fidelity_replay` for Rust replay |
| Core library / WASM consumer | [Configuration](docs/configuration.md), `examples/basic.rs`; `src/lib.rs`, `src/observation/mod.rs`; `tests/core_consumer.rs` | `cargo test --locked --no-default-features --test core_consumer`; explicit `wasm` group for target changes |
| Simulator or client performance | [Performance](docs/performance.md), [workloads](docs/performance-workloads.md); `examples/perf_baseline.rs`, `tools/perf_compare.py`, `tools/perf_build.py`; `tools/test_perf_*.py` | `cargo test --locked --example perf_baseline`, or the matching Python test; measure when claiming a speed change |
| Documentation | Relevant guide and its linked source/examples | `python -B -m unittest discover -s tools -p test_documentation.py`; no research dependencies |

## Setup and checks

Run focused tests first, then the relevant group from the repository root.
Rust needs Rust 1.99+ and platform build tools; the check runner uses
standard-library Python 3.11+. Documentation-only changes need accuracy/link
review and the documentation test above, without a Rust build.

| Group | Command | Coverage / prerequisites |
| --- | --- | --- |
| Rust | `python tools/check.py rust` | `lint` (formatting; Clippy on default, core-only and single-feature builds) then `test` (default/core-only/bot-only/networking/C ABI tests, doctests and a Python crate build); no uv or Node |
| Tools | `uv run --locked --only-group tools python tools/check.py tools` | Python/Node tests, local bot/C ABI smoke tests and Python package tests; Python 3.12, uv, Node 22+ (24 recommended) and Rust |
| Package | `python tools/check.py package` | Local crate verification; no registry publication |
| WASM | `python tools/check.py wasm` | Core-only and bot-only target checks; install `wasm32-unknown-unknown` first |

Before the tools group, run `uv sync --locked --only-group tools`. `all` runs
`rust`, `tools` and `package`; WASM is explicit. Append `--list` to inspect commands
without running them. Commands report their time; CI adds a job-summary timing
table. Tests use doubles or loopback, never outside game servers.
PR CI runs the same runner automatically: `lint`,
WASM and package once on Linux, `test` on Linux, Windows, macOS and Rust 1.99,
and `tools` on Linux, Windows and macOS. The weekly
[extended checks](https://github.com/NikitaMGrimm/slither-sim/blob/main/.github/workflows/extended.yml) add release-profile tests,
Linux ARM tests and a RustSec audit; they block nothing and can be dispatched.

The tools group's fidelity suite includes transport measurement regressions
(`tools/fidelity/test_transport_parity.py`). Run them alone with
`uv run --locked --only-group tools python -B -m unittest discover -s tools/fidelity -p test_transport_parity.py`.
The [local transport check](docs/research-workflow.md#transport-parity) is an
explicit measurement, separate from unit tests. Record current-host gate failures
as baselines; do not loosen official tolerances.

## Evidence proportional to the change

- Explain the problem and resulting behavior with a reproducer, relevant tests
  and, when useful, a short before/after example such as a request and response.
- **Documentation:** review accuracy and links against current sources.
- **Tool/API fixes:** minimal reproducer and regression using existing evidence
  or clearly labelled synthetic inputs; no fresh capture required.
- **Behavior-preserving optimizations:** demonstrate output parity on relevant
  cases; provide matched before/after measurements only when claiming performance.
  A [comparison](docs/performance.md#compare-a-change) on the catalog rows that
  exercise the changed code is enough, with its numbers in the PR. If no row
  does, add one to the [workload catalog](docs/performance-workloads.md) instead
  of a one-off harness. Reserve a dated research study for capacity and
  live-server claims.
- **Mechanics corrections:** cite supporting evidence and add a focused regression
  fixture. Collect more only if existing evidence cannot support the correction.
- **New empirical or fitted claims:** follow the [research workflow](docs/research-workflow.md)
  with the complete relevant study, failed/inconclusive attempts, selection rules,
  uncertainty and applicable held-out validation. Explain inapplicable methods;
  do not add ceremonial requirements.

The [evidence comparison workflow](docs/research-workflow.md#compare-contributed-evidence)
screens contributed bundles for declared reporting differences; its statistical
report supports review without approving physics changes or replacing fixtures.

Keep bounded, selected [regression fixtures](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/fixtures/README.md#selecting-and-updating-cases)
in Git; keep full recordings, credentials and generated outputs outside it.
Use Conventional Commits and LF; avoid unrelated formatting. Maintainers review
ownership, callers, contracts and evidence independently of contributor tests;
passing tests alone is not acceptance. Collection or publication needs separate
authorization.

Preserve third-party license notices. For external data, note its source and any
sharing restrictions in `REPORT.md`.

A maintainer reviews the current revision and evidence before merging. Required
CI must pass; new commits need relevant checks and renewed review. PRs may merge
behind main: the client and simulator change mostly separate files, main reruns
every check after each merge, and releases wait for main's run.
Auto-merge can then finish an approved PR. Independent agent review is useful
evidence, but agents cannot approve their GitHub account's own PR. The owner-authorized
[release automation](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/releasing.md) can merge compatible patch release PRs
containing only version/changelog metadata after required CI. Other release
bumps require maintainer review and merge; publication then runs automatically.

For geometry or motion changes, optional before/after plots, diagrams or clips
can help. Attach or link PR-only media; do not commit it.

## Rationale and evidence

Keep non-obvious reasons, units, invariants and model limits beside the owning
code. Link formulas/defaults to their declarations instead of repeating them in
an agent guide. Brief comments should label rules as measured,
protocol/client-derived or provisional and link canonical notes/regressions when useful.

Keep current instructions in `docs/`. Put dated studies and selected results in
`research/<topic>/YYYY-MM-DD-<question>/REPORT.md`; follow the
[research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md) for placement and access labels.
Evidence notes/fixtures own methods, source/archive hashes, selected windows,
failed cases and uncertainty. Tests consume selected fixtures; synthetic cases
establish their stated contract. Change a rule's owner, linked evidence and
focused regression together. Preserve historical plans and contradictory results;
do not regenerate expectations to agree with new code.

## AI assistance

Review and verify AI work. End the PR description with a short footer naming
actual models, harnesses and roles, including material delegated work.
Use display names; report review and checks accurately. Example:

> [!NOTE]
> 🤖 **Agent assistance: GPT 6.1 Sol via Codex** — implementation; supporting work (verification).

Group credits by harness in one closing note. Neither transcripts nor a reviewer
quota are required.

## Interfaces and packaging

Interfaces are experimental. Incompatible bot changes require a new
[contract identifier](docs/client-api.md#interface-versions), updated examples
and the JSON contract fixture. Keep units, field meanings and lifecycle behavior
consistent; readers ignore additional response fields. Physics changes can alter
trajectories without changing the API.

Tick behavior changes require a new checkpoint `rulesRevision`, even when JSON
and ABI shapes are unchanged. Exact restore accepts only the current rules
revision; retain explicit rejection tests for older revisions rather than
silently continuing their state under new rules. Change the checkpoint format
only when its serialized shape changes.

The root `Cargo.toml`'s `[workspace.package]` owns the simulator version, inherited
by the C ABI crate and Python package; `pyproject.toml` owns the tools version.
Within `0.x`, incompatible Rust API changes bump the minor version. Group
unreleased changes in the changelog; no permanent freeze or multi-version support
is promised.

Write one user-facing changelog entry per change to what ships: runtime
behavior, APIs, packaged tools, examples or guides. Link its PR when useful. A
study that changes none of these, including a rejected candidate, gets only its
[research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md)
row: the changelog ships in the package, so any entry there prompts a patch
release. Avoid repeating generated commit titles in a second prose list.

The `package` check builds the local crate. It ships the library, binary,
user guides and examples, the release guide and the fixtures its unit tests read;
integration tests, other fixtures, evidence tools, schemas and other maintainer
files require the Git checkout.
[Historical provenance](docs/provenance.md) records imported components and adaptations;
routine edits do not refresh import hashes. Per-study source pins remain required.
Do not publish releases, change repository visibility,
collaborators or licensing without the owner's authorization.
