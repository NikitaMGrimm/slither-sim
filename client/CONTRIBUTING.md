# Contributing

Use Rust 1.99.0, Node.js 24 and wasm-bindgen CLI 0.2.129. See the
[README](README.md) for setup and run commands, [architecture](docs/architecture.md)
for ownership, and [testing](docs/verification.md) for checks.

Keep changes focused, preserve unrelated work, use LF and Conventional Commits,
and add focused regression coverage for changed behavior. Avoid compatibility
layers for obsolete alpha APIs. Keep game policy in Rust and platform glue thin.

Commit original code and labelled synthetic fixtures only. The private local
reference JS is a specification to port idiomatically. Never commit official
code, artwork, credentials or captures; see [source and license](docs/provenance.md).

The root [CONTRIBUTING.md](../CONTRIBUTING.md) rules for commits, PRs and AI
disclosure apply here too. Use the client scope, such as `feat(client):`.

Enable hooks with `git config core.hooksPath client/tools/hooks`. Stage and inspect your
diff, run `npm run check`, and describe results and unrun checks in the PR.
Tests and CI must not contact game servers or download official source. Use the
root slither-sim binary for manual local play.
