# Contributing

Keep each PR focused on one problem. For larger features or interface changes,
discuss the scope with the maintainer before starting; small fixes can go straight
to a PR. Use one task, branch and worktree; preserve other contributors' work.
Start with the [API guides](README.md#choose-an-interface) and
[rules and limits](docs/server-rules.md), or use the [documentation map](docs/README.md)
to find a research or maintenance task.

## Find the change

Paths below are relative to the repository root. Read the relevant guide, trace
the source owner and its callers, then start with the matching test.

| Task | Guide and source / tests | Focused check |
| --- | --- | --- |
| Mechanics, motion, food, prey | [Rules, limits and next steps](docs/server-rules.md#packaged-regression-evidence); `src/simulator.rs`, `src/physics.rs`, `src/prey.rs`; matching `tests/*.rs` and `tests/fixtures/` | `cargo test --locked --test movement_turns` (choose the affected test) |
| Bot JSON API and lifecycle | [Client API](docs/client-api.md), [direct API](docs/bot-api.md); `src/main.rs`, `src/client/mod.rs`, `src/client/received.rs`; `tests/client_contract.rs`, `tests/client_runner.rs`, `tests/transport.rs` | `cargo test --locked --test client_contract` |
| In-process C ABI | [C ABI](docs/c-api.md); `ffi/src/lib.rs`, `ffi/include/slither_sim.h`, `examples/ffi_bot.py`; `ffi/tests/abi.rs` | `cargo test --locked -p slither-sim-ffi` |
| Python package | [Python package](docs/python.md); `python/src/lib.rs`, `python/slither_sim.pyi`, `python/examples/`; `python/tests/test_session.py` | `uv run --locked --only-group tools python -m maturin build --locked -m python/Cargo.toml`, then `python python/tests/run.py target/wheels` |
| Wire decoder and local host | [Server guide](docs/raw-server.md); `src/client/protocol.rs`, `src/client/server.rs`; `tests/client_protocol.rs`, `tests/client_server.rs` | `cargo test --locked --test client_protocol` |
| Capture, audit and recorded replay | [Research workflow](docs/research-workflow.md); `tools/fidelity/evidence.py`, `tools/fidelity/prepare-fidelity-replay.py` and adjacent tests; `examples/fidelity_replay.rs` | Matching Python/Node test; `cargo test --locked --example fidelity_replay` for Rust replay |
| Core library / WASM consumer | [Configuration](docs/configuration.md), `examples/basic.rs`; `src/lib.rs`, `src/observation/mod.rs`; `tests/core_consumer.rs` | `cargo test --locked --no-default-features --test core_consumer`; explicit `wasm` group for target changes |
| Simulator or client performance | [Performance](docs/performance.md), [workloads](docs/performance-workloads.md); `examples/perf_baseline.rs`, `tools/perf_compare.py`, `tools/perf_build.py`; `tools/test_perf_*.py` | `uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/NAME` (output parity, then paired timings) |
| Documentation | Relevant guide and its linked source/examples | `python -B -m unittest discover -s tools -p test_documentation.py`; no research dependencies |

## Evidence proportional to the change

- Explain the problem and resulting behavior. Include a reproducer and relevant
  tests; add a short before/after example when useful, such as a request and response.
- **Documentation:** review accuracy and links against current sources.
- **Tool/API fixes:** minimal reproducer and regression using existing evidence
  or clearly labelled synthetic inputs. Fresh capture is not required.
- **Behavior-preserving optimizations:** demonstrate output parity on relevant
  cases; provide matched before/after measurements only when claiming performance.
- **Mechanics corrections:** cite the existing evidence that justifies the rule
  and add a focused regression fixture. Collect more only if that evidence cannot
  support the correction.
- **New empirical or fitted claims:** follow the [research workflow](docs/research-workflow.md)
  with the complete relevant study, failed/inconclusive attempts, selection rules,
  uncertainty and held-out validation where applicable. State why a method does
  not apply rather than adding a ceremonial requirement.

The [evidence comparison workflow](docs/research-workflow.md#compare-contributed-evidence)
screens contributed bundles for declared reporting differences. Its statistical
report supports review; it neither approves physics changes nor replaces fixtures.

Keep bounded, selected [regression fixtures](docs/research-workflow.md#fixture-and-protocol-evolution)
in Git and full recordings, credentials and generated outputs outside it. Use
Conventional Commits and LF; avoid unrelated formatting. Run the
[relevant offline checks](docs/research-workflow.md#offline-checks); PR CI runs
automatically. Maintainer review checks ownership, callers, contracts and evidence
independently of contributor-written tests. Passing those tests alone is not
acceptance. Tests stay offline; collection or publication needs separate authorization.

Preserve third-party license notices. For external data, note its source and any
sharing restrictions in `REPORT.md`.

A maintainer reviews the current PR revision and its evidence before merging.
Required CI must pass with the latest main; new commits need the relevant checks
and review again. Auto-merge can finish an approved PR once these requirements
pass. An independent agent review is useful evidence, but agents using the same
GitHub account cannot approve that account's own PR. The owner-authorized
[release automation](docs/releasing.md) can merge compatible patch release PRs
containing only version/changelog metadata after required CI. Other release
bumps require maintainer review and merge; publication then runs automatically.

For geometry or motion changes, a before/after plot, diagram or clip can help.
Visualizations are optional; attach or link PR-only media rather than committing it.

## Rationale and evidence

Put non-obvious reasons, units, invariants and model limitations beside the owning
code. Keep formulas/defaults in their implementation; link to their declarations
instead of maintaining another explanation in a separate agent guide. A short
comment should identify whether a rule is measured, protocol/client-derived or
provisional, and point to its canonical note and regression when useful.

Keep current instructions in `docs/`. Put dated studies and selected results in
`research/<topic>/YYYY-MM-DD-<question>/REPORT.md`; follow the
[research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md) for placement and access labels.
Keep experimental methods, source/archive hashes, selected windows, failed cases
and uncertainty in the evidence note/fixture, with full recordings outside Git.
Tests consume selected fixtures; synthetic cases establish their stated contract.
When changing a rule, update its owner, linked evidence and focused regression
together. Preserve historical plans and contradictory results rather than
regenerating expectations to agree with the new code.

## AI assistance

If you use AI, review its work and verify the result. End the PR description
with a short footer naming actual models, harnesses and roles, including material
delegated work. Use display names and report review and checks accurately. Example:

> [!NOTE]
> 🤖 **Agent assistance: GPT 6.1 Sol via Codex** — implementation; supporting work (verification).

Group multiple credits by harness in the same closing note. Transcripts and a
fixed number of reviewers are not required.

## Interfaces and packaging

Interfaces are experimental. Incompatible bot changes need a new
[contract identifier](docs/client-api.md#interface-versions), updated examples
and the JSON contract fixture. Keep units, field meanings and lifecycle behavior
consistent; readers ignore additional response fields. Physics changes can alter
trajectories without changing the API.

The root `Cargo.toml`'s `[workspace.package]` owns the simulator version, which
the C ABI crate and the Python package inherit; `pyproject.toml` owns the tools version.
Within `0.x`, incompatible Rust API changes bump the minor version. Keep unreleased
changes together in the changelog; no permanent freeze or multi-version support
is promised.

The `package` check group verifies a local crate containing Rust tests and fixtures.
Evidence tools and schemas require the Git checkout.
[Historical provenance](docs/provenance.md) records imported components and adaptations;
routine edits do not refresh import hashes. Per-study source pins remain required.
Do not publish releases, change repository visibility,
collaborators or licensing without the owner's authorization.
