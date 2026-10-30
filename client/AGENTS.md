# Coding rules

These rules add to the root AGENTS.md for files under client/ and win where they
differ, except for the root commit, PR, AI-disclosure and offline-test rules.
Run the commands below from client/.

- Keep protocol, game state, presentation and input/session/HUD policy in the
  shared Rust core. Keep platform adapters thin. Authoritative simulation belongs
  in the root slither-sim crate.
- Preserve the Canvas client as the visual reference while developing the shared
  wgpu client. Read README.md and docs/architecture.md before changing behavior.
- Never put official code, assets, credentials or raw captures in Git or public
  reports. The private local reference JS is the behavioral specification: port
  idiomatically rather than copying it. Label authored synthetic fixtures.
- Use Rust 1.99.0, Node 24, wasm-bindgen CLI 0.2.129, LF and Conventional Commits
  with the client scope, such as feat(client): or fix(client):.
  Generated dist/, target/ and external comparison inputs stay untracked.
- Make the smallest complete change. Preserve unrelated work. Avoid compatibility
  shims for obsolete alpha APIs, speculative frameworks and duplicated policy.
- Start with focused tests; run npm run check before delivery. See
  docs/verification.md. Report failed or unrun checks accurately.
- Stage the intended changes and run node tools/check-repository.mjs before
  committing. Use --pre-push with Git's push input for outgoing commits, or
  --history for a full audit. Do not set core.hooksPath in a shared multi-worktree
  checkout; tools/hooks remains available for single-clone opt-in.
- Tests and CI use authored synthetic data and never connect to game servers or
  fetch the official client. Manual play uses a separately running local simulator.
- Keep PRs reviewable and follow the root PR template and AI-assistance footer.
  PRs are squash-merged: rebase instead of merging main into a branch. Client
  changes stay out of the root CHANGELOG.md, which ships with the simulator crate.
  Review consequential behavior, protocol or security changes independently when
  possible.
