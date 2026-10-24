# Documentation

Start with the [quick start](../README.md#quick-start) to run a local bot.
For a code change, start with [contributing](../CONTRIBUTING.md): setup, source
owners, focused tests and the evidence each kind of change needs.

## Use the simulator

| Task | Guide |
| --- | --- |
| Write a bot that runs on simulation or WebSocket backends | [Portable client API](client-api.md) |
| Control all slots and worlds directly, or embed the Rust core | [`serve` controller and Rust library](bot-api.md) |
| Run `serve` sessions in-process from Python | [Python package](python.md) |
| Run `serve` sessions in-process from C or another language | [C ABI](c-api.md) |
| Host a shared world or connect the stock browser | [WebSocket server](raw-server.md) |
| Choose world, timing and network settings | [Configuration](configuration.md) |
| Understand implemented rules and approximations | [Mechanics and delivery limits](server-rules.md) |

## Contribute and investigate

| Task | Guide |
| --- | --- |
| Find source owners and run checks | [Contributor task map](../CONTRIBUTING.md#find-the-change), [setup and checks](../CONTRIBUTING.md#setup-and-checks) |
| Compare simulator or client performance | [Performance](performance.md) |
| Add a workload or use a specialized harness | [Performance workloads](performance-workloads.md) |
| Collect, audit, replay or compare evidence | [Evidence workflow](research-workflow.md) |
| Read the evidence file contract | [Bundle format](evidence-format.md) |
| Add a regression fixture | [Fixture guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/fixtures/README.md) |
| Read dated results and counterexamples | [Research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md) |

Maintainer references:
[PR comparisons](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md),
[releasing](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/releasing.md),
[provenance](provenance.md) and [changelog](../CHANGELOG.md).

Update the guide that owns the behavior. Put distinct dated studies under
`research/<topic>/YYYY-MM-DD-<question>/REPORT.md`; a routine fix needs no new
report. Keep selected regressions in `tests/fixtures/` and full recordings,
binaries and generated outputs outside Git. Current guides ship in the Rust
crate; use repository URLs for research, tools, integration tests and maintainer
files excluded from that package.
