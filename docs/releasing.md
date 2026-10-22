# Releasing

`release.yml` uses crates.io Trusted Publishing; no saved API token is needed.
The repository owner manages releases. Contributors submit version changes by PR.

## One-time connection

Open [the crate's trusted publisher form](https://crates.io/crates/slither-sim/settings/new-trusted-publisher).
Select GitHub, owner `NikitaMGrimm`, repository `slither-sim`, workflow filename
`release.yml`. Leave the optional environment empty and click **Add**.

In [Actions → Release](https://github.com/NikitaMGrimm/slither-sim/actions/workflows/release.yml),
choose **Run workflow**, branch `main`. This verifies authentication without
publishing. After switching to trusted publishing, revoke the old token in
[API Tokens](https://crates.io/settings/tokens) and run `cargo logout --registry crates-io`
in each local environment where it was saved. Logout alone does not revoke a token.

## Publish a version

Update `Cargo.toml`, `Cargo.lock` and the changelog in a PR. Merge it, then tag
that release commit (replace the example version):

```sh
git switch main
git pull --ff-only
git tag v0.1.1
git push origin v0.1.1
```

The workflow requires the tag to match the manifest and the commit to belong to
main. It allows five minutes for that commit's main CI run to appear, waits for
it to pass, verifies the package, publishes to
crates.io and creates a GitHub Release with generated notes. Prerelease tags get
prerelease releases. Do not move published tags or reuse published versions.

If publication succeeded but release creation failed, rerun the failed workflow.
It skips an existing crate only when its archive identifies the same clean commit.
Network errors or a different published commit fail the run.
