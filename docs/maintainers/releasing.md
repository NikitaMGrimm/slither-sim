# Releasing

This is the maintainer runbook. Contributors record user-facing changes in the
changelog and follow the [versioning guidance](../../CONTRIBUTING.md#interfaces-and-packaging).

`release-pr.yml` maintains one rolling release PR with release-plz, updating
`Cargo.toml`, `Cargo.lock` and the changelog after merges to main. It compares
packaged files with crates.io: tools-only changes do not force a crate release,
while packaged docs, tests and examples can. The version lives in the root
`Cargo.toml`'s `[workspace.package]`; the unpublished C ABI crate and the Python
package inherit it, so their lockfile entries move with the crate's. The Python
tools version remains independent.

## Manual releases

To publish now, manually merge the rolling `chore(release)` PR after its required
checks pass. This starts publication of that exact version-bump commit, without
waiting for the automatic cooldown. It works for patch, minor and major bumps.
CI, package verification, immutable tags and published-source checks still apply.

The manual merge starts a new 12-hour wait for the next automatic merge. For
example, merging at 08:10 UTC makes the next automatic merge eligible at 20:10
UTC, even if the upload completes a few minutes after the merge. The hourly job
acts on that eligibility; GitHub scheduling can delay it. No empty release is made.

The exception requires a same-repository release-bot PR merged by a user, with
only the manifest version, its lockfile entries and changelog changed. Ordinary
code merges, bot merges, tags and dispatch inputs do not grant it. Publication
rechecks that provenance before uploading. A retry retains the original merge
authorization and source commit rather than selecting a later main revision.

## Automatic patches

The owner authorizes automatic compatible patch releases. `release.yml` checks
hourly, at minute 23 UTC (GitHub can delay scheduled runs). It merges the bot's
release PR only when all of these hold:

- The version advances by one patch, such as `0.2.0` to `0.2.1`, and release-plz
  reports a successful Rust API compatibility check.
- The branch includes current main, required checks pass and GitHub permits the
  protected merge. Only the manifest's version, its lockfile entries and changelog
  may differ. The merge is pinned to the inspected PR head.
- At least 12 hours have elapsed since the latest crates.io upload or published
  GitHub Release, including prereleases and yanked versions. For a verified
  manual release, its merge time replaces that version's upload/publication time
  as the timer's start. A different, newer publication still resets the wait.

The publisher serializes automatic and tag-triggered runs, verifies CI on the
exact main commit being released, dry-runs the package and checks the cooldown
or manual-merge authorization again before uploading. It uses crates.io Trusted
Publishing, without a saved API token. A scheduled check is an opportunity to release, not a promise of two
releases per day. There are no empty releases when the package is unchanged.

Bot pushes do not start ordinary push workflows with `GITHUB_TOKEN`. Bot PRs
start approval-required PR checks. Preparation approves only this repository's
release-bot run after verifying the branch contains current main plus release
metadata. This starts the real protected PR checks; a separate dispatched
branch run does not satisfy that gate. After the protected merge, the publisher
tags the selected commit and explicitly
dispatches checks on that tag if no main push run exists. Later main commits
cannot change the selected release's source. Branch protection remains enabled.

## Python package

Each crate publication also publishes the `slither-sim` Python package at the
same version. After the GitHub Release, the same `release.yml` run builds the
released commit through `python-wheels.yml`: stable-ABI wheels for Linux x86-64
and ARM64 (manylinux2014), macOS arm64 and x86-64 and Windows x86-64, each
tested on Python 3.11, and a source distribution tested by building it. The
`pypi` job uploads them with PyPI Trusted Publishing. PRs that change `python/`
build the same distributions.

The Python upload is not part of release completion. If it fails, the crate
release stays complete and the next version proceeds normally. To repair it,
dispatch Release with `vVERSION`; files already on PyPI are skipped, so a partial
upload completes. A source fix needs a new version.

## Release binaries

The same run also attaches prebuilt archives to the GitHub Release.
`release-binaries.yml` builds the `slither-sim` CLI and the `slither-sim-ffi`
shared library for Linux x86-64 and ARM64, macOS arm64 and x86-64 and Windows
x86-64. Each archive (`slither-sim-TARGET.tar.gz`, or `.zip` on Windows) also
holds the C header, LICENSE and a short README. Linux builds run in the
manylinux2014 container, so they need glibc 2.17+; the build fails if either file
requires a newer symbol. Windows builds link the C runtime statically. Each runner
tests its extracted archive with a `serve` reset and step and with
`examples/ffi_bot.py`. PRs that change the workflow build the same archives. The
builds are not signed or notarized.

The `release-assets` job, the only one with write access, runs only when all
five builds succeed. It deletes any existing `SHA256SUMS`, replaces the archives
with `gh release upload --clobber` and uploads the new `SHA256SUMS` last.
Replacement is per file, so an interrupted upload can leave a partial or mixed
archive set without a `SHA256SUMS`, but never with a stale one. Like the Python
upload, it is not part of release completion: a failure leaves the crate release
complete, and the next version proceeds normally. To repair it, dispatch Release
with `vVERSION`. The rerun rebuilds every archive from the tagged commit and
replaces the uploaded set and its checksums. It also retries the Python upload,
which skips files already on PyPI; either upload can fail without stopping the
other.

## Incompatible changes

Within `0.x`, incompatible Rust changes require a minor bump, such as `0.3.0`.
These PRs require maintainer review and merge; automation never merges them.
Compatible additions use patch bumps while pre-1.0. Use Conventional Commits
with `!` or `BREAKING CHANGE:` for incompatible JSON/CLI or behavioral contracts
that Rust API checks cannot detect, and update affected contract identifiers.
Physics changes still need their evidence and meaningful changelog notes in the
original PR. Automated version selection does not establish scientific fidelity.

Do not move version tags or reuse published versions.

## Connection and recovery

The repository Actions settings must allow GitHub Actions to create pull
requests. Individual workflows request their own write permissions; the default
token permissions can remain read-only. No PAT is needed.

The crate's [trusted publisher](https://crates.io/crates/slither-sim/settings/new-trusted-publisher)
must allow GitHub owner `NikitaMGrimm`, repository `slither-sim`, workflow filename
`release.yml`, with no environment. This existing connection is unchanged.

PyPI's [trusted publisher](https://pypi.org/manage/project/slither-sim/settings/publishing/)
for project `slither-sim` must allow owner `NikitaMGrimm`, repository
`slither-sim`, workflow `release.yml` and environment `pypi`. Required reviewers
on the `pypi` GitHub environment would hold each upload for approval. Until the
first upload creates the project, this is a pending publisher on the account's
[Publishing page](https://pypi.org/manage/account/publishing/), registered on
October 24, 2026.

If migrating from a saved crates.io token, revoke it in
[API Tokens](https://crates.io/settings/tokens) and run
`cargo logout --registry crates-io` wherever it was saved. Logout alone does not
revoke a token.

In [Actions → Release](https://github.com/NikitaMGrimm/slither-sim/actions/workflows/release.yml),
run the workflow on main with an empty `tag` to test authentication only. Use
`auto` to reconcile immediately, or `vVERSION` to retry a specific existing tag.
Tags and dispatch inputs alone do not bypass the 12-hour guard; a verified
manual release merge does. An ineligible run exits without publishing; a later
scheduled run retries.

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
