# Working in this repository

SlitherSim is a standalone simulator, bot interface and evidence toolkit.
Read README.md and the relevant API guide before changing behavior; consult
[server rules](docs/server-rules.md) for mechanics and
[research workflow](docs/research-workflow.md) before collecting evidence.

- Use the [task map and evidence tiers](CONTRIBUTING.md#find-the-change). Keep
  changes small and deterministic. Tool/API fixes need a minimal repro and
  regression, not fresh capture. Mechanics corrections need supporting evidence
  and a fixture; new fitted claims need appropriate held-out validation.
  Incompatible bot API changes need a new affected contract ID.
- Keep debug snapshots separate from bot observations. Preserve raw capture bytes,
  frozen plans and failed attempts. Label synthetic data and distinguish measured
  behavior from inferred rules. Hashes verify consistency, not scientific claims.
- Use Conventional Commits and LF; avoid unrelated formatting. Start with focused
  checks, then the relevant [check group](docs/research-workflow.md#offline-checks).
  Rust/docs work needs no research stack. Python tooling uses locked uv;
  simulator examples use only the standard library. PR CI runs automatically.
- Use one task, branch and worktree. Discuss large/API changes first; small fixes
  can go directly to a PR. Inspect owners and callers; tests written with the
  change do not replace independent source/evidence checks by maintainers.
- Keep selected fixtures in Git and full recordings outside it. Follow
  [fixture evolution](docs/research-workflow.md#fixture-and-protocol-evolution):
  pin protocol/contract labels, preserve valid regressions, and independently
  justify changed expectations. A newer default does not relabel older evidence.
- [Provenance](docs/provenance.md) is a historical import record, not a routine
  current-file hash gate. Preserve per-study source hashes and failed attempts.
- Keep a short PR footer naming actual AI models, harnesses and roles; no
  transcripts or mandatory reviewer count. See [contributing](CONTRIBUTING.md#ai-assistance).
- Offline tests must not contact outside game servers.
  Do not collect from outside servers, publish releases or change
  visibility, collaborators or licensing without the owner's authorization.
