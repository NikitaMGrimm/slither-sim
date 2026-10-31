# Releasing

Contributors record user-facing changes in the changelog and follow
[versioning guidance](../../CONTRIBUTING.md#interfaces-and-packaging).

`release-pr.yml` uses release-plz to maintain one rolling PR, updating
`Cargo.toml`, `Cargo.lock` and the changelog after main merges. The version
heading goes above the curated Unreleased entries, without generated commit
titles. Comparing packaged
files with crates.io excludes tools-only releases; packaged docs, tests and
examples can trigger one. The root `Cargo.toml`'s `[workspace.package]` version is inherited
by the unpublished C ABI crate and Python package, including lockfile entries.
The Python tools version is independent.

The [release performance comment](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md#rolling-release-evidence)
consolidates PR evidence. Explicit previous-release versus pinned-candidate
measurements use the [on-demand workflow](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/release-performance.md).
Neither changes release preparation or publication authorization.

## Shared protocol bootstrap (owner-gated)

`crates/slither-protocol` has independent SemVer (initially 0.1.0), an MIT
license file and publishable metadata. A0 adds the real reader without a root
runtime dependency. The agreed order is A0, then B (client path dependency),
then owner-approved protocol publication, then A (simulator dependency).
Neither this plan nor A0 enables publication. `release-plz.toml` explicitly
disables protocol release preparation until the following owner-gated TODOs
are complete; the existing publisher still selects only slither-sim.

- TODO (owner): check crates.io name availability/ownership and register the
  protocol's first publication and Trusted Publisher identity. Do not assume
  the simulator's registration authorizes a new package or bootstrap a token.
- TODO (owner-approved release work): register independent protocol version and
  changelog handling in release-plz; breaking changes before 1.0 bump its minor
  version. Preserve `[workspace.package]` for simulator/ABI/Python and the rolling
  simulator release PR's version intent.
- TODO: make release tooling package-aware before enabling protocol release
  preparation/publication. Verify and publish the protocol archive **before**
  any slither-sim version that needs it; wait for the registry version to be
  visible before root packaging/publication. Root dependencies then specify
  both `path` and a compatible registry `version`, optional under `networking`.
  Preserve exact-commit CI, extracted/normalized archive identity, immutable
  root tags, retries/idempotence and cooldown/authorization checks for each
  selected package. Adding a workspace member is not a publisher implementation.
- TODO before A: exercise Rust 1.99 multi-package verification with the actual
  dependency edge (`cargo +1.99.0 package --locked -p slither-protocol -p
  slither-sim --registry crates-io`). A0's package group selects both independent
  archives and verifies them; it cannot prove the later unpublished dependency
  bootstrap. Do not bypass archive verification with `--no-verify` or a patch.
- TODO before A delivery: verify extracted protocol/root archives and normalized
  manifests, root CLI/library, C ABI and Python maturin sdist/wheel source
  closure. A working workspace is not proof that published consumers resolve.

The client stays unpublished and B needs no crates.io release. First
publication, trusted-publisher setup and any change to the release workflow
require separate owner authorization. No protocol release workflow is enabled
by A0; ordinary simulator releases keep their existing guards.

## Manual releases

To publish, manually merge the rolling `chore(release)` PR after required checks
pass. Its exact version-bump commit publishes without the automatic cooldown,
for patch, minor or major bumps. CI, package verification, immutable tags and
published-source checks still apply.

Manual merge starts the next automatic merge's 12-hour wait: 08:10 UTC permits
20:10 UTC even if upload finishes later. The hourly job acts on eligibility;
GitHub can delay it. No empty release is made.

This exception requires a user-merged, same-repository release-bot PR changing
only manifest version, its lockfile entries and changelog. Code/bot merges, tags
and dispatch inputs do not qualify. Publication rechecks provenance; retries
retain original merge authorization and source, never a later main revision.

## Automatic patches

Owner-authorized `release.yml` checks hourly at minute 23 UTC (subject to GitHub
delays). It merges compatible patch release PRs only when:

- The version advances by one patch, such as `0.2.0` to `0.2.1`, and release-plz
  reports a successful Rust API compatibility check.
- The branch includes current main, required checks pass and GitHub permits the
  protected merge. Only the manifest's version, its lockfile entries and changelog
  may differ. The merge is pinned to the inspected PR head.
- At least 12 hours have elapsed since the latest crates.io upload or published
  GitHub Release, including prereleases and yanked versions. For a verified
  manual release, its merge time replaces that version's upload/publication time
  as the timer's start. A different, newer publication still resets the wait.

Automatic/tag-triggered publication is serialized. The publisher verifies CI on
the exact main commit, dry-runs packaging and rechecks cooldown or manual-merge
authorization before crates.io Trusted Publishing, without a saved token.
Scheduling promises no release frequency and unchanged packages produce none.

Bot pushes do not start ordinary push workflows with `GITHUB_TOKEN`. Bot PRs
start approval-required PR checks. Preparation approves only this repository's
release-bot run after verifying the branch contains current main plus release
metadata. This starts the real protected PR checks; a separate dispatched
branch run does not satisfy that gate. After the protected merge, the publisher
tags the selected commit and explicitly
dispatches checks on that tag if no main push run exists. Later main commits
cannot change the selected release's source. Branch protection remains enabled.

## Python package

After GitHub Release creation, `release.yml` builds the released commit through
`python-wheels.yml`: same-version `slither-sim` stable-ABI wheels for Linux
x86-64/ARM64 (manylinux2014), macOS arm64/x86-64 and Windows x86-64, tested on
Python 3.11, plus a build-tested source distribution. The `pypi` job uses PyPI
Trusted Publishing. PRs changing `python/` build the same distributions.

Python upload failure leaves the crate release complete and later versions
unblocked. Retry by dispatching Release with `vVERSION`; existing PyPI files
are skipped. Source fixes need a new version.

## Release binaries

`release-binaries.yml` attaches the CLI and `slither-sim-ffi` shared library for
Linux x86-64/ARM64, macOS arm64/x86-64 and Windows x86-64. Archives
(`slither-sim-TARGET.tar.gz`, Windows `.zip`) include the C header, LICENSE and
README. Linux's manylinux2014 builds require glibc 2.17+ and reject newer symbols
in either binary. Windows links the C runtime statically. Each runner tests its
extracted archive with `serve` reset/step and `examples/ffi_bot.py`. Workflow PRs
build the same archives. Builds are unsigned and unnotarized.

Among binary jobs, only `release-assets` has write access; it requires all five builds. It
deletes existing `SHA256SUMS`, replaces archives with `gh release upload --clobber`,
then uploads new checksums. Per-file replacement can leave partial/mixed archives
after interruption, but no stale `SHA256SUMS`.

Asset failure leaves crate completion and later versions unblocked. Dispatch
Release with `vVERSION` to rebuild all archives from the tag and replace their
checksums. This also retries Python, skipping existing PyPI files; neither
upload's failure blocks the other.

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

Actions settings must permit PR creation. Workflows request their own write
permissions; default token permissions can stay read-only. No PAT is needed.

The crate's [trusted publisher](https://crates.io/crates/slither-sim/settings/new-trusted-publisher)
must allow owner `NikitaMGrimm`, repository `slither-sim`, workflow `release.yml`,
with no environment.

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

After a successful upload but failed GitHub Release creation, reconciliation
repairs that version before advancing. It verifies published checksum,
clean-checkout flag and original commit, without reuploading or waiting for a
new slot. Tag/identity mismatches, registry/network failures or failed CI stop
publication. Retry transient CI failures; source fixes need a new version.

Run [Release PR](https://github.com/NikitaMGrimm/slither-sim/actions/workflows/release-pr.yml)
on main to refresh preparation. It pauses while a merged version is unpublished,
then refreshes after publication to include intervening merges in the next PR.
