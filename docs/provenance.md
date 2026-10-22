# Provenance

Initial imported components come from `NikitaMGrimm/slither-arena-rs`, commit
`aab1149460c0f5606fbdc7d73e295ba79d094789` (2026-10-16).
SlitherSim builds and tests independently of that repository.

| Manifest | Imported components |
| --- | --- |
| [extraction.json](extraction.json) | Simulator mechanics, tests, fixtures and lockfile. |
| [toolkit-import.json](toolkit-import.json) | 17 capture, decoder, audit and replay files and their tests. |
| [client-import.json](client-import.json) | Receipt tracker, minimap decoder, latency model and selected tests. |

Each entry records its original and packaged SHA-256, source path and adaptations
through the historical snapshot `f4d9f8b994c8714429ebe4cb1618209a34d9c45b`.
These import records are retained as attribution and extraction history. Ordinary
contributions do not refresh them to match evolving source files; Git records
those changes. Evidence bundles separately preserve the exact software used for
each study. Mechanics and stepping order were retained at extraction; adaptations
include crate naming, removal of unused helpers and standalone interfaces.
The Rust protocol encoder/decoder, network runtime, WebSocket host, JSON
controllers, study schemas and evidence packager were added here.

The source-linked food presentation layer and anonymous `client-presentation.json`
fixture were subsequently adapted from the training repository's October 3 work;
pooled ambient weights are pinned at `a0587bc87fb010d29e7dfdde00e5ea83587078e2`.
See [presentation](presentation.md) for retained observations and model limits.

Generic body-clearance and respawn helpers were subsequently adapted from
`src/simulator.rs` at `c2fb7285febb4273034e30699dc880715a334423`. Selected
observations and refill-rate controls expose existing core paths with public
validation. Their adaptations are recorded without replacing the initial pins.

## Audit the historical import

From a Git checkout containing the snapshot's history (a source ZIP or shallow
checkout may not contain it):

```sh
uv run --locked --only-group tools python tools/verify_extraction.py
```

This reads the snapshot's committed manifests and files, never the current
working files. It is an optional history audit, not a normal contribution check.
Use `--revision COMMIT` only to inspect another historical packaged snapshot;
changing that argument does not validate a new scientific claim.

If you have the original repository, also verify its pinned Git bytes:

```sh
uv run --locked --only-group tools python tools/verify_extraction.py --source-repo /path/to/slither-arena-rs
```

## Protocol references

Packet layouts were checked against retained captures and the stock client parser
`game1107249518.js`, SHA-256
`5ad8aa8f06d1e71bad2945001bab077f8d4d279c093a046d19b6f26ac6b62095`.
Challenge tests retain 11 archived exchanges; leaderboard and replay fixtures
identify their exact source records. Synthetic cases are labelled separately.
Official game assets are not distributed.

These references establish packet layouts and observed behavior, not closed-server
algorithms. Local ranking, record, session and team support is described in the
[server guide](raw-server.md); mechanics limits are in [server rules](server-rules.md).

Initial implementation assistance: GPT 6.1 Sol via Codex and Claude Opus 5.5 via
the Claude Agent harness. Source evidence and verification are recorded separately.
