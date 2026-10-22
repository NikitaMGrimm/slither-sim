# Provenance

Imported components come from `NikitaMGrimm/slither-arena-rs`, commit
`aab1149460c0f5606fbdc7d73e295ba79d094789` (2026-10-16).
SlitherSim builds and tests independently of that repository.

| Manifest | Imported components |
| --- | --- |
| [extraction.json](extraction.json) | Simulator mechanics, tests, fixtures and lockfile. |
| [toolkit-import.json](toolkit-import.json) | 17 capture, decoder, audit and replay files and their tests. |
| [client-import.json](client-import.json) | Receipt tracker, minimap decoder, latency model and selected tests. |

Each entry records its original and packaged SHA-256, source path and adaptations.
Intentional edits update packaged hashes and adaptation notes while retaining the
original source hashes. Mechanics and stepping order were retained; adaptations
include crate naming, removal of unused helpers and standalone interfaces.
The Rust protocol encoder/decoder, network runtime, WebSocket host, JSON
controllers, study schemas and evidence packager were added here.

## Verify imports

From a Git checkout:

```sh
uv run --locked --only-group tools python tools/verify_extraction.py
```

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
