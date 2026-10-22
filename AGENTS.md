# Working in this repository

SlitherSim is a standalone simulator, bot interface and evidence toolkit.
Read README.md and the relevant API guide before changing behavior; consult
[server rules](docs/server-rules.md) for mechanics and
[research workflow](docs/research-workflow.md) before collecting evidence.

- Keep changes small and deterministic. Mechanics changes need evidence and a
  regression fixture; fitted rules need held-out validation. Incompatible bot API
  changes need a new protocol ID.
- Keep debug snapshots separate from bot observations. Preserve raw capture bytes,
  frozen plans and failed attempts. Label synthetic data and distinguish measured
  behavior from inferred rules. Hashes verify consistency, not scientific claims.
- Use Conventional Commits and LF. Run `cargo fmt --check`,
  `cargo test --all-targets --locked` and
  `cargo clippy --all-targets --locked -- -D warnings`.
- Run tool checks from the research workflow through the locked uv environment.
  New Python tooling uses uv; simulator examples use only the standard library.
- Work in this checkout. Preserve source hashes and record intentional import
  adaptations in the provenance manifests; imports are not an automatic sync.
- Offline tests must not contact outside game servers. CI is manual-only;
  do not dispatch it without an explicit request. Do not publish or change
  visibility, collaborators or licensing without the owner's authorization.
