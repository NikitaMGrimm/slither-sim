<!-- Read CONTRIBUTING.md first. Keep this PR focused on one problem;
discuss larger features or interface changes in an issue before starting. -->

## Change

<!-- Describe the problem and resulting behavior. Link a related issue if useful.
Include a short before/after example when it helps: input/output, packet bytes,
or error text. -->

## Validation

<!-- List checks actually run and their results. Name unrun checks and material limits.
Use CONTRIBUTING.md's evidence tier:
- Docs: accuracy and local links.
- Tool/API fix: minimal repro and regression; existing or labelled synthetic inputs.
- Behavior-preserving optimization: output parity; measurements for performance claims.
- Mechanics correction: supporting evidence and focused fixture.
- New empirical/fitted claim: complete relevant study, failed attempts, selection,
  uncertainty and held-out validation where applicable; see docs/research-workflow.md.

Keep selected fixtures in Git and full recordings outside it. Add a case per scenario,
retain valid regressions and justify expectation corrections independently of the code
under test. Pin protocol/contract labels; distinguish recorded packets from derived
expectations, and observed from synthetic inputs.
Optional geometry/motion media belongs in PR attachments or links. -->

- [ ] Relevant checks pass; unrun checks are identified.
- [ ] Relevant evidence tier and fixture/version requirements are met (or inapplicable).
- [ ] Contribution terms in CONTRIBUTING.md are met; recordings stay outside Git.

<!-- If AI helped, end with the disclosure described in CONTRIBUTING.md#ai-assistance.
Name actual models, harnesses and roles, including material delegated work. Example:

> [!NOTE]
> 🤖 **Agent assistance: GPT 6.1 Sol via Codex** — implementation; supporting work (verification).

Group multiple credits by harness in the same closing note; no transcript or reviewer quota.
Omit the note if no AI was used. -->
