"""Reconcile the rolling release PR and select an immutable publication commit."""

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib

import release


def git(*args):
    return subprocess.check_output(["git", *args], cwd=release.ROOT, text=True).strip()


def repo_path(suffix):
    return f"repos/{os.environ['GH_REPO']}/{suffix}"


def refresh_main():
    subprocess.run(["git", "fetch", "origin", "main", "--no-tags"], cwd=release.ROOT, check=True)


def is_patch(previous, proposed):
    if not re.fullmatch(r"\d+\.\d+\.\d+", previous) or not re.fullmatch(r"\d+\.\d+\.\d+", proposed):
        return False
    old = tuple(map(int, previous.split(".")))
    new = tuple(map(int, proposed.split(".")))
    return new[:2] == old[:2] and new[2] == old[2] + 1


def bot_pr(pr):
    return (pr["user"]["login"] == "github-actions[bot]"
            and pr["head"]["repo"]["full_name"] == os.environ["GH_REPO"]
            and pr["head"]["ref"].startswith("release-plz-")
            and pr["base"]["ref"] == "main" and not pr["draft"])


def eligible_patch(pr, package, files, main):
    marker = re.findall(r"<!-- release-plz (\S+) (\S+) (\S+) (\S+) -->", pr["body"] or "")
    if len(marker) != 1 or not bot_pr(pr) or pr["base"]["sha"] != main:
        return False
    name, previous, proposed, semver = marker[0]
    return (name == package["name"] and previous == package["version"]
            and is_patch(previous, proposed) and semver == "compatible"
            and pr["mergeable_state"] == "clean"
            and {item["filename"] for item in files} == {"Cargo.toml", "Cargo.lock", "CHANGELOG.md"})


def metadata_only(main, head, package):
    subprocess.run(["git", "fetch", "origin", head], cwd=release.ROOT, check=True)
    if git("merge-base", main, head) != main:
        return False
    before = tomllib.loads(git("show", f"{main}:Cargo.toml"))
    after = tomllib.loads(git("show", f"{head}:Cargo.toml"))
    if not is_patch(package["version"], after["package"]["version"]):
        return False
    after["package"]["version"] = before["package"]["version"]
    if before != after:
        return False
    before_lock = tomllib.loads(git("show", f"{main}:Cargo.lock"))
    after_lock = tomllib.loads(git("show", f"{head}:Cargo.lock"))
    for item in after_lock["package"]:
        if item["name"] == package["name"] and "source" not in item:
            if not is_patch(package["version"], item["version"]):
                return False
            item["version"] = package["version"]
    return before_lock == after_lock


def merge_patch(package):
    prs = release.github_pages(repo_path("pulls?state=open&base=main&per_page=100"))
    candidates = [pr for pr in prs if bot_pr(pr)]
    if len(candidates) != 1:
        return
    pr = release.github(repo_path(f"pulls/{candidates[0]['number']}"))
    if not bot_pr(pr):
        return
    ensure_pr_checks(pr)
    files = release.github_pages(repo_path(f"pulls/{pr['number']}/files?per_page=100"))
    main = git("rev-parse", "origin/main")
    if not eligible_patch(pr, package, files, main) or not metadata_only(main, pr["head"]["sha"], package):
        print("Release PR awaits compatible patch metadata, current main and successful checks.", file=sys.stderr)
        return
    # The endpoint enforces branch protection; the SHA prevents merging an uninspected update.
    result = release.github(repo_path(f"pulls/{pr['number']}/merge"), "--method", "PUT",
                            "-f", f"sha={pr['head']['sha']}", "-f", "merge_method=squash")
    if not result["merged"]:
        raise ValueError("GitHub did not merge the release PR")
    return result["sha"]


def version_commit(version):
    # Later main commits may retain this version. Publish its original version-bump commit.
    commits = git("log", "--first-parent", "--format=%H", "origin/main", "--", "Cargo.toml").splitlines()
    candidate = None
    for commit in commits:
        if release.package_at(commit)["version"] != version:
            break
        candidate = commit
    if candidate is None:
        raise ValueError("Cannot find the main commit introducing this version")
    return candidate


def ensure_tag(tag, commit):
    existing = git("tag", "--list", tag)
    if existing:
        if git("rev-parse", f"refs/tags/{tag}^{{commit}}") != commit:
            raise ValueError("Existing version tag points to a different commit")
    else:
        release.github(repo_path("git/refs"), "--method", "POST", "-f", f"ref=refs/tags/{tag}",
                       "-f", f"sha={commit}")


def select_automatic():
    refresh_main()
    package = release.package_at("origin/main")
    version = package["version"]
    versions = release.registry_versions(package["name"])
    releases = release.github_pages(repo_path("releases?per_page=100"))
    completed = {item["tag_name"] for item in releases if not item["draft"]}
    unfinished = [item for item in versions if f"v{item['num']}" not in completed]
    if unfinished:
        previous = max(unfinished, key=lambda item: item["created_at"])
        commit = release.published_commit(package["name"], previous["num"])
        package = release.package_at(commit)
    elif any(item["num"] == version for item in versions):
        if not release.cooldown_ready(package["name"]):
            return
        merged = merge_patch(package)
        if not merged:
            return
        refresh_main()
        package = release.package_at(merged)
        commit = merged
    else:
        if not release.cooldown_ready(package["name"]):
            return
        commit = version_commit(version)
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"],
                   cwd=release.ROOT, check=True)
    tag = f"v{package['version']}"
    # Identity and tag checks also apply to same-version recovery.
    release.prepare(tag, release.package_at(commit), commit)
    ensure_tag(tag, commit)
    return {"tag": tag, "commit": commit}


def dispatch_pr_checks():
    output = json.loads(os.environ["RELEASE_PR"])
    if not output.get("number"):
        return
    pr = release.github(repo_path(f"pulls/{output['number']}"))
    if not bot_pr(pr):
        raise ValueError("CI dispatch requires a same-repository release bot PR")
    ensure_pr_checks(pr)


def ensure_pr_checks(pr):
    runs = subprocess.check_output([
        "gh", "run", "list", "--workflow", "checks.yml", "--event", "workflow_dispatch",
        "--branch", pr["head"]["ref"], "--commit", pr["head"]["sha"],
        "--limit", "1", "--json", "databaseId", "--jq", ".[0].databaseId // empty",
    ], cwd=release.ROOT, text=True).strip()
    if not runs:
        subprocess.run(["gh", "workflow", "run", "checks.yml", "--ref", pr["head"]["ref"]],
                       cwd=release.ROOT, check=True, stdout=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("select", "prepare", "dispatch-pr-checks"))
    args = parser.parse_args()
    if args.command == "dispatch-pr-checks":
        dispatch_pr_checks()
        return
    if args.command == "prepare":
        package = release.package_at("HEAD")
        # Do not propose another version while a merged release is still unpublished.
        ready = release.published_commit(package["name"], package["version"]) is not None
        print(f"ready={str(ready).lower()}")
        return
    event = os.environ["GITHUB_EVENT_NAME"]
    tag = os.environ.get("RELEASE_TAG", "")
    if event == "schedule" or (event == "workflow_dispatch" and tag == "auto"):
        result = select_automatic()
    elif event == "push" or tag:
        if not tag:
            tag = os.environ["GITHUB_REF_NAME"]
        if not re.fullmatch(r"v[0-9][0-9A-Za-z.+-]*", tag):
            raise ValueError("Expected a version tag")
        commit = git("rev-parse", f"refs/tags/{tag}^{{commit}}")
        result = {"tag": tag, "commit": commit}
    else:
        result = None  # Empty dispatch retains the authentication-only connection test.
    for key, value in (result or {}).items():
        print(f"{key}={value}")


if __name__ == "__main__":
    main()
