# Contributing

Start with the [API guides](README.md#choose-an-interface) and
[rules and limits](docs/server-rules.md).

## Changes

- Keep fixes focused and include a reproducer and relevant tests.
- Mechanics changes need an evidence note and a regression fixture. Separate
  observations from inferred rules and validate fits on separate data.
- Measurements and data-only contributions follow the
  [research workflow](docs/research-workflow.md). Submit a bundle URL, SHA-256,
  report, analysis commands and any distilled fixture; retain failed attempts.
- Use Conventional Commits and LF. Keep recordings, credentials and generated
  outputs outside Git. Run the [offline checks](docs/research-workflow.md#offline-checks).
  CI is manual-only and runs when the owner requests it.

## Interfaces and packaging

Interfaces are experimental. Incompatible bot changes need a new
[contract identifier](docs/client-api.md#interface-versions), updated examples
and the JSON contract fixture. Keep units, field meanings and lifecycle behavior
consistent; readers ignore additional response fields. Physics changes can alter
trajectories without changing the API.

`Cargo.toml` owns the simulator version; `pyproject.toml` owns the tools version.
Within `0.x`, incompatible Rust API changes bump the minor version. Keep unreleased
changes together in the changelog; no permanent freeze or multi-version support
is promised.

`cargo package --locked` verifies a local crate containing Rust tests and fixtures.
Evidence tools and schemas require the Git checkout. Registry publishing is disabled.
[Provenance](docs/provenance.md) records imported components and adaptations.

## Contribution terms

You retain copyright. Submitting code, documentation or fixtures offers them
under [MIT](LICENSE) and confirms your right to contribute them. Preserve
third-party attribution and licenses. Evidence archives specify their rights
holder, covered paths and reuse terms in `REPORT.md`.
