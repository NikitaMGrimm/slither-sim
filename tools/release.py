"""Check release CI, a version tag and any previous upload of the same commit."""

import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import time
import tomllib
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = "https://crates.io/api/v1/crates"


def require_main_ci(commit, timeout=300, dispatch_ref=None):
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"], cwd=ROOT, check=True)
    deadline = time.monotonic() + timeout
    dispatched = False
    while True:
        run_id = subprocess.check_output([
            "gh", "run", "list", "--workflow", "checks.yml", "--event", "push", "--branch", "main",
            "--commit", commit, "--limit", "1", "--json", "databaseId", "--jq", ".[0].databaseId // empty",
        ], cwd=ROOT, text=True).strip()
        if not run_id and dispatch_ref:
            run_id = subprocess.check_output([
                "gh", "run", "list", "--workflow", "checks.yml", "--event", "workflow_dispatch",
                "--branch", dispatch_ref, "--commit", commit, "--limit", "1", "--json", "databaseId",
                "--jq", ".[0].databaseId // empty",
            ], cwd=ROOT, text=True).strip()
            if not run_id and not dispatched:
                subprocess.run(["gh", "workflow", "run", "checks.yml", "--ref", dispatch_ref],
                               cwd=ROOT, check=True)
                dispatched = True
        if run_id:
            subprocess.run(["gh", "run", "watch", run_id, "--exit-status", "--interval", "15"], cwd=ROOT, check=True)
            return
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f"No main CI run appeared for {commit} within {timeout} seconds")
        time.sleep(min(5, remaining))


def fetch(url):
    request = Request(url, headers={"User-Agent": "slither-sim-release (https://github.com/NikitaMGrimm/slither-sim)"})
    with urlopen(request, timeout=30) as response:
        return response.read()


def github(endpoint, *args):
    return json.loads(subprocess.check_output(["gh", "api", endpoint, *args], cwd=ROOT, text=True))


def github_pages(endpoint):
    pages = github(endpoint, "--paginate", "--slurp")
    return [item for page in pages for item in page]


def registry_versions(name):
    # This endpoint includes yanked versions, which also consumed a publication slot.
    return json.loads(fetch(f"{REGISTRY}/{name}"))["versions"]


def cooldown_ready(name, now=None):
    versions = registry_versions(name)
    releases = github_pages(f"repos/{os.environ['GH_REPO']}/releases?per_page=100")
    timestamps = [item["created_at"] for item in versions]
    timestamps.extend(item["published_at"] for item in releases if not item["draft"])
    if not timestamps:
        raise ValueError("Cannot establish the previous publication time")
    times = [datetime.fromisoformat(value.replace("Z", "+00:00")) for value in timestamps]
    if any(value.tzinfo is None for value in times):
        raise ValueError("Publication timestamps must include a timezone")
    next_release = max(times) + timedelta(hours=12)
    ready = (now or datetime.now(timezone.utc)) >= next_release
    if not ready:
        print(f"Next publication is allowed at {next_release.isoformat()}", file=sys.stderr)
    return ready


def package_at(commit):
    manifest = subprocess.check_output(["git", "show", f"{commit}:Cargo.toml"], cwd=ROOT, text=True)
    return tomllib.loads(manifest)["package"]


def published_commit(name, version):
    url = f"{REGISTRY}/{name}/{version}"
    try:
        metadata = json.loads(fetch(url))["version"]
    except HTTPError as error:
        if error.code == 404:
            return None
        raise
    data = fetch(f"{url}/download")
    if hashlib.sha256(data).hexdigest() != metadata["checksum"]:
        raise ValueError("Published archive checksum mismatch")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        with archive.extractfile(f"{name}-{version}/.cargo_vcs_info.json") as source:
            identity = json.load(source)["git"]
    if identity.get("dirty"):
        raise ValueError("Published archive came from a dirty checkout")
    return identity["sha1"]


def prepare(tag, package, commit):
    if tag != f"v{package['version']}":
        raise ValueError("Tag must be v followed by the exact Cargo.toml version")
    if package.get("publish") != ["crates-io"]:
        raise ValueError("Manifest must restrict publishing to crates-io")
    existing = published_commit(package["name"], package["version"])
    if existing is not None and existing != commit:
        raise ValueError("Version is already published from a different commit; bump the version")
    prerelease = "-" in package["version"].split("+", 1)[0]
    return {"version": package["version"], "already_published": str(existing is not None).lower(),
            "prerelease": str(prerelease).lower()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag", nargs="?")
    parser.add_argument("--commit", default="HEAD")
    parser.add_argument("--check-ci", action="store_true")
    parser.add_argument("--ci-ref")
    parser.add_argument("--check-cooldown", action="store_true")
    args = parser.parse_args()
    commit = subprocess.check_output(["git", "rev-parse", f"{args.commit}^{{commit}}"],
                                     cwd=ROOT, text=True).strip()
    if args.check_ci:
        require_main_ci(commit, dispatch_ref=args.ci_ref)
        return
    if not args.tag:
        parser.error("a version tag is required")
    package = package_at(commit)
    result = prepare(args.tag, package, commit)
    if args.check_cooldown:
        # Completing an existing upload is recovery of the same release, not a new upload.
        ready = result["already_published"] == "true" or cooldown_ready(package["name"])
        result["ready"] = str(ready).lower()
    for key, value in result.items():
        print(f"{key}={value}")


if __name__ == "__main__":
    main()
