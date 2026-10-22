"""Check release CI, a version tag and any previous upload of the same commit."""

import hashlib
import io
import json
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


def require_main_ci(commit, timeout=300):
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"], cwd=ROOT, check=True)
    deadline = time.monotonic() + timeout
    while True:
        run_id = subprocess.check_output([
            "gh", "run", "list", "--workflow", "checks.yml", "--event", "push", "--branch", "main",
            "--commit", commit, "--limit", "1", "--json", "databaseId", "--jq", ".[0].databaseId // empty",
        ], cwd=ROOT, text=True).strip()
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
    if len(sys.argv) != 2:
        raise SystemExit("Usage: python tools/release.py vVERSION | --check-ci")
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if sys.argv[1] == "--check-ci":
        require_main_ci(commit)
        return
    with (ROOT / "Cargo.toml").open("rb") as stream:
        package = tomllib.load(stream)["package"]
    for key, value in prepare(sys.argv[1], package, commit).items():
        print(f"{key}={value}")


if __name__ == "__main__":
    main()
