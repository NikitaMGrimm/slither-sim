# Documentation

Start with the [quick start](../README.md#quick-start) to run a local bot.
For a code change, use the [contributor task map](../CONTRIBUTING.md#find-the-change)
to find its source owner and focused check.

## Use and embed

| Goal | Guide |
| --- | --- |
| Write a bot using received client state | [Portable bot API](client-api.md) |
| Control a world directly or embed the Rust core | [Geometric controller and library](bot-api.md) |
| Connect the stock browser or host WebSocket clients | [Server guide](raw-server.md) |
| Choose world, latency and network settings | [Configuration reference](configuration.md) |
| Understand food colors and renderer differences | [Client presentation](presentation.md) |

## Investigate and contribute

| Goal | Guide |
| --- | --- |
| Find supported mechanics and useful next contributions | [Rules and evidence limits](server-rules.md) |
| Check visibility, spawning, minimap and ecology assumptions | [Host fidelity](host-fidelity.md) |
| Plan, collect, audit or replay evidence | [Evidence workflow](research-workflow.md) |
| Read or produce a portable evidence bundle | [Bundle format](evidence-format.md) |
| Measure simulator and client costs | [Performance](performance.md), with observation, retained-client and deadline guides linked there |
| Understand automatic PR measurements | [PR comparisons](pr-reports.md) |

## Maintain and trace history

| Goal | Guide or record |
| --- | --- |
| Run current checks | [Offline check groups](research-workflow.md#offline-checks) |
| Understand version selection and publication | [Releasing](releasing.md) |
| Trace imported components and reference sources | [Provenance](provenance.md) |
| Inspect the initial compatibility checks | [October 22 verification record](verification.md) |
| Find dated studies, including failed and inconclusive attempts | [Research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md) |
| See release changes | [Changelog](../CHANGELOG.md) |

## Where changes belong

Update an existing guide when its commands or implemented behavior change. Keep
each explanation with its owner and link to it from related guides. Add a guide
when a distinct task needs its own instructions; link it here. This index groups
guides by task without changing established URLs.

Put distinct dated studies under `research/<topic>/YYYY-MM-DD-<question>/REPORT.md`
and link them from the research index. Keep attempts and amendments to one study
together, preserving frozen plans, counterexamples and source identities. A
routine documentation or code fix usually needs no new study. Full recordings,
binaries, generated outputs and changing handoffs stay outside Git; the
[research placement guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md#adding-or-updating-material)
covers access labels and selected result data.

Current guides ship in the Rust crate; research records do not. Use repository
URLs when linking from packaged guides to research. Old archive-entry pages and
anchors remain available for existing links.
