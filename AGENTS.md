# Working in this repository

SlitherSim is a standalone simulator, bot interface and evidence toolkit.
Before changing behavior, read README.md and the relevant API guide; use
[server rules](docs/server-rules.md) for mechanics and
[research workflow](docs/research-workflow.md) before collecting evidence.

`client/` holds slither-client, the Rust rewrite of the protocol-19 browser
client. For files under `client/`, `client/AGENTS.md` adds client rules and wins
where they differ, except that the commit, PR, AI-disclosure and offline-test
rules below apply everywhere.

- Use the [task map and evidence tiers](CONTRIBUTING.md#find-the-change).
  Keep changes small and deterministic. Tool/API fixes need a minimal repro and
  regression, not fresh capture. Mechanics corrections need supporting evidence
  and a fixture; new fitted claims need appropriate held-out validation.
  Incompatible bot API changes need a new affected contract ID.
- Keep debug snapshots separate from bot observations. Preserve raw capture bytes,
  frozen plans and failed attempts. Label synthetic data and distinguish measured
  behavior from inferred rules. Hashes verify consistency, not scientific claims.
- Use Conventional Commits and LF; avoid unrelated formatting. Start with focused
  checks, then the relevant [check group](CONTRIBUTING.md#setup-and-checks).
  Rust/docs need no research stack. Python tooling uses locked uv;
  simulator examples use only the standard library. PR CI runs automatically.
- Use one task, branch and worktree. Discuss large/API changes first; small fixes
  can go straight to a PR. Inspect owners and callers; contributor tests do not
  replace independent maintainer source/evidence checks.
- Keep current guides in `docs/` and dated studies with selected results under
  `research/`; follow the [placement guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md).
  Preserve historical evidence paths/hashes. Packaged guides must use repository
  URLs for research excluded from the crate.
- Keep selected fixtures in Git and full recordings outside it. Follow
  [fixture evolution](tests/fixtures/README.md#selecting-and-updating-cases):
  pin protocol/contract labels, preserve valid regressions and independently
  justify changed expectations. New defaults do not relabel old evidence.
- [Provenance](docs/provenance.md) is a historical import record, not a routine
  current-file hash gate. Preserve per-study source hashes and failed attempts.
- Name actual AI models, harnesses and roles in a short PR footer; no transcripts
  or reviewer quota. See [contributing](CONTRIBUTING.md#ai-assistance).
- Offline tests must not contact outside game servers.
  Do not collect from outside servers, publish releases or change
  visibility, collaborators or licensing without the owner's authorization.
- Keep each rule's rationale beside its code, linking the canonical evidence
  note/fixture. Distinguish measured behavior, protocol/client constants and models;
  preserve counterexamples. See [evidence placement](CONTRIBUTING.md#rationale-and-evidence).
