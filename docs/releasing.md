# Releasing

`release-pr.yml` maintains one rolling release PR with release-plz, updating
`Cargo.toml`, `Cargo.lock` and the changelog after merges to main. It compares
packaged files with crates.io: tools-only changes do not force a crate release,
while packaged docs, tests and examples can. The Python tools version remains
independent.

## Automatic patches

The owner authorizes automatic compatible patch releases. `release.yml` checks
hourly, at minute 23 UTC (GitHub can delay scheduled runs). It merges the bot's
release PR only when all of these hold:

- The version advances by one patch, such as `0.2.0` to `0.2.1`, and release-plz
  reports a successful Rust API compatibility check.
- The branch includes current main, required checks pass and GitHub permits the
  protected merge. Only the manifest's version, its lockfile entry and changelog
  may differ. The merge is pinned to the inspected PR head.
- At least 12 hours have elapsed since the latest crates.io upload or published
  GitHub Release, including prereleases and yanked crate versions.

The publisher serializes automatic and tag-triggered runs, verifies CI on the
exact main commit being released, dry-runs the package and checks the cooldown
again before uploading. It uses crates.io Trusted Publishing, without a saved
API token. A scheduled check is an opportunity to release, not a promise of two
releases per day. There are no empty releases when the package is unchanged.

Bot pushes do not start ordinary push workflows with `GITHUB_TOKEN`. Bot PRs
start approval-required PR checks. Preparation approves only this repository's
release-bot run after verifying the branch contains current main plus release
metadata. This starts the real protected PR checks; a separate dispatched
branch run does not satisfy that gate. After the protected merge, the publisher
tags the selected commit and explicitly
dispatches checks on that tag if no main push run exists. Later main commits
cannot change the selected release's source. Branch protection remains enabled.

## Incompatible changes

Within `0.x`, incompatible Rust changes require a minor bump, such as `0.3.0`.
These PRs require maintainer review and merge; automation never merges them.
Compatible additions use patch bumps while pre-1.0. Use Conventional Commits
with `!` or `BREAKING CHANGE:` for incompatible JSON/CLI or behavioral contracts
that Rust API checks cannot detect, and update affected contract identifiers.
Physics changes still need their evidence and meaningful changelog notes in the
original PR. Automated version selection does not establish scientific fidelity.

After a maintainer merges a version bump, the next eligible scheduled run
publishes its original version-bump commit, even if main advances afterward.
Do not move version tags or reuse published versions.

## Connection and recovery

The repository Actions settings must allow GitHub Actions to create pull
requests. Individual workflows request their own write permissions; the default
token permissions can remain read-only. No PAT is needed.

The crate's [trusted publisher](https://crates.io/crates/slither-sim/settings/new-trusted-publisher)
must allow GitHub owner `NikitaMGrimm`, repository `slither-sim`, workflow filename
`release.yml`, with no environment. This existing connection is unchanged.

If migrating from a saved crates.io token, revoke it in
[API Tokens](https://crates.io/settings/tokens) and run
`cargo logout --registry crates-io` wherever it was saved. Logout alone does not
revoke a token.

In [Actions → Release](https://github.com/NikitaMGrimm/slither-sim/actions/workflows/release.yml),
run the workflow on main with an empty `tag` to test authentication only. Use
`auto` to reconcile immediately, or `vVERSION` to retry a specific existing tag.
All new uploads retain the 12-hour guard, including manually pushed tags.
An ineligible run exits without publishing; a later scheduled run retries.

If upload succeeded but GitHub Release creation failed, reconciliation repairs
that same version before advancing. It verifies the published archive checksum,
clean-checkout flag and original commit, and does not upload again or wait for a
new release slot. A mismatched tag or published identity, registry/network
failure, or failed CI stops publication. Rerun failed CI after a transient error;
source corrections need a new version rather than moving an existing tag.

Run [Release PR](https://github.com/NikitaMGrimm/slither-sim/actions/workflows/release-pr.yml)
on main to refresh preparation manually. It pauses while a merged version is
still unpublished and is refreshed again after publication, so merges arriving
during a release are included in the next proposal.
