# Evidence bundle v1

A bundle is a gzip-compressed tar of regular files ([schema](https://github.com/NikitaMGrimm/slither-sim/blob/main/schemas/evidence-bundle-v1.schema.json)).
Raw captures retain schema-1 JSONL/gzip bytes.

```text
manifest.json
plan.json
plan.sha256
attempts.jsonl
REPORT.md
raw/*.jsonl.gz
analysis/*
fixtures/*
logs/*
software/*
```

| File | Contract |
| --- | --- |
| `plan.json` | [Study plan](https://github.com/NikitaMGrimm/slither-sim/blob/main/schemas/study-plan-v1.schema.json), copied byte-for-byte by `init` and identified by SHA-256. Later edits/reformatting are rejected. Unknown fields require a schema revision. |
| `attempts.jsonl` | [Ledger](https://github.com/NikitaMGrimm/slither-sim/blob/main/schemas/attempt-event-v1.schema.json): one durable `started` reservation and one `finished` result per attempt, with status, exit code and raw members. Missing finishes remain incomplete. Timestamps include UTC offsets; commands are provenance strings, never verifier input to execute. |
| `manifest.json` | Study ID, plan digest, data origin, audit and every other file's exact size/SHA-256. `scientificStatus` is always `unreviewed`. |
| `REPORT.md` | Findings, reproduction and limitations; external-data sources and sharing restrictions. |
| `software/` | Required source snapshot: core, Cargo inputs, scripts, schemas, replay examples, Python lockfile and license. `software/LICENSE` covers bundled project code. |

Allowed origins are `official-server`, `community-server` and `local-simulator`.
Label synthetic data `local-simulator` in plan and capture header; origin labels
do not authenticate servers.

## Source pins

Collector digests must match the bundled source snapshot. Synthetic studies also
record `captureProvenance.sources.files`, mapping relative paths to SHA-256.
`combinedSha256` hashes the map as compact UTF-8 JSON: sorted keys, comma/colon
separators, no trailing newline. Missing, malformed or mismatched pins quarantine
captures even with a matching archive inventory.

## Verification and bounds

The packager snapshots data, audits and inventories that snapshot, rejects
symlinks/case collisions, verifies the result and writes exclusively. The verifier
stages regular files temporarily, checks exact membership and hashes, then
recomputes the audit; it never executes bundled scripts or extracts links.

Version 1 requires the original 33-file `V1_SOURCE_FLOOR` in `evidence.py` and
checks all declared source pins. Later checkout additions do not change
historical bundle requirements.

Paths must be portable across Windows/Linux: no traversal, absolute/drive paths,
backslashes, device names, trailing dots, case aliases or file/directory collisions.

| Limit | Maximum |
| --- | --- |
| Files | 10,000 |
| Single file | 512 MiB |
| Total file bytes | 2 GiB |
| JSON/ledger metadata | 4 MiB |
| Expanded text per raw gzip capture | 256 MiB |

Exceeded bounds fail validation without truncation. Retained corrupt/interrupted
raw files leave audits quarantined/incomplete with CLI exit 2.

Hashes detect byte changes, not honest acquisition, independent trials, server
versions or scientific validity. Sampling, timing/visibility limits, alternative
explanations and held-out validation still need review.
