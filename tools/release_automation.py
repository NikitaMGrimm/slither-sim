"""Reconcile the rolling release PR and select an immutable publication commit."""

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import PurePosixPath
import re
import subprocess
import sys
import time
import tomllib

import release


@contextmanager
def selected_package(name):
    previous = os.environ.get("RELEASE_PACKAGE")
    os.environ["RELEASE_PACKAGE"] = name
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop("RELEASE_PACKAGE", None)
        else:
            os.environ["RELEASE_PACKAGE"] = previous


def protocol_dependency(commit):
    manifest = tomllib.loads(git("show", f"{commit}:Cargo.toml"))
    return release.PROTOCOL in manifest.get("dependencies", {})


def protocol_prerequisite(commit):
    """Each new package upload has its own immutable-source and release gate."""
    if release.selected_name() == release.PROTOCOL or not protocol_dependency(commit):
        return {"ready": "true", "protocol_needed": "false"}
    with selected_package(release.PROTOCOL):
        package = release.package_at(commit)
        result = publication_ready(release.version_tag(package), commit)
    return {"ready": result["ready"], "protocol_needed": str(result["already_published"] != "true").lower()}


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


def release_markers(pr):
    return re.findall(r"<!-- release-plz (\S+) (\S+) (\S+) (\S+) -->", pr["body"] or "")


def metadata_files(names):
    files = {"Cargo.lock"}
    for name in names:
        path = release.MANIFESTS[name]
        files.update((path, str(PurePosixPath(path).with_name("CHANGELOG.md"))))
    return files


def eligible_patch(pr, package, files, main):
    markers = release_markers(pr)
    chosen = [m for m in markers if m[0] == package["name"]]
    if (len(chosen) != 1 or len({m[0] for m in markers}) != len(markers)
            or not bot_pr(pr) or pr["base"]["sha"] != main
            or any(m[0] not in release.MANIFESTS for m in markers)):
        return False
    return (chosen[0][1] == package["version"]
            and all(is_patch(previous, proposed) and semver == "compatible"
                    for _, previous, proposed, semver in markers)
            and pr["mergeable_state"] == "clean"
            and {item["filename"] for item in files} == metadata_files(m[0] for m in markers))


def metadata_only(main, head, package, patch_only=True, markers=None):
    subprocess.run(["git", "fetch", "origin", head], cwd=release.ROOT, check=True)
    if git("merge-base", main, head) != main:
        return False
    before = tomllib.loads(git("show", f"{main}:Cargo.toml"))
    after = tomllib.loads(git("show", f"{head}:Cargo.toml"))
    old_root, new_root = release.version_table(before)["version"], release.version_table(after)["version"]
    versions = {}
    changed = set()
    if old_root != new_root:
        versions[before["package"]["name"]] = (old_root, new_root)
        changed.add("slither-sim")
    release.version_table(after)["version"] = old_root
    if before != after:
        return False
    # Independently versioned protocol metadata; all other workspace manifests
    # stay immutable. Inherited root/ABI/Python lock entries move together.
    for member in after.get("workspace", {}).get("members", []):
        path = f"{member}/Cargo.toml"
        old = tomllib.loads(git("show", f"{main}:{path}"))
        new = tomllib.loads(git("show", f"{head}:{path}"))
        name = old["package"]["name"]
        if name == release.PROTOCOL:
            previous, proposed = old["package"]["version"], new["package"]["version"]
            if previous != proposed:
                versions[name] = (previous, proposed)
                changed.add(name)
            new["package"]["version"] = previous
        elif release.inherits_version(old) and old_root != new_root:
            versions[name] = (old_root, new_root)
        if old != new:
            return False
    if package["name"] not in changed or versions[package["name"]][0] != package["version"]:
        return False
    if markers is not None and ({m[0]: m[1:3] for m in markers} != {name: versions[name] for name in changed}
                                or len(markers) != len(changed)):
        return False
    if patch_only and any(not is_patch(*pair) for pair in versions.values()):
        return False
    actual = set(git("diff", "--name-only", main, head).splitlines())
    if actual != metadata_files(changed):
        return False
    before_lock = tomllib.loads(git("show", f"{main}:Cargo.lock"))
    after_lock = tomllib.loads(git("show", f"{head}:Cargo.lock"))
    for item in after_lock["package"]:
        if item["name"] in versions and "source" not in item:
            previous, proposed = versions[item["name"]]
            if item["version"] != proposed:
                return False
            item["version"] = previous
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
    if not eligible_patch(pr, package, files, main) or not metadata_only(
            main, pr["head"]["sha"], package, markers=release_markers(pr)):
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
    commits = git("log", "--first-parent", "--format=%H", "origin/main", "--", release.MANIFESTS[release.selected_name()]).splitlines()
    candidate = None
    for commit in commits:
        if release.package_at(commit)["version"] == version:
            candidate = commit
        elif candidate is not None:
            break
    if candidate is None:
        raise ValueError("Cannot find the main commit introducing this version")
    return candidate


def manual_release_merge(commit):
    """A maintainer's merge of the bot's metadata-only PR authorizes publication."""
    parents = git("rev-list", "--parents", "-n", "1", commit).split()
    if len(parents) < 2:
        return None
    parent = parents[1]
    before = release.package_at(parent)
    package = release.package_at(commit)
    if package["version"] == before["version"]:
        return None
    associated = release.github_pages(repo_path(f"commits/{commit}/pulls?per_page=100"))
    candidates = [pr for pr in associated if pr.get("merge_commit_sha") == commit]
    if len(candidates) != 1:
        return None
    pr = release.github(repo_path(f"pulls/{candidates[0]['number']}"))
    if (not bot_pr(pr) or not pr.get("merged") or pr.get("merge_commit_sha") != commit
            or not pr.get("merged_at") or (pr.get("merged_by") or {}).get("type") != "User"):
        return None
    markers = release_markers(pr)
    marker = [m for m in markers if m[0] == package["name"]]
    if (len(marker) != 1 or len({m[0] for m in markers}) != len(markers)
            or any(m[0] not in release.MANIFESTS for m in markers)
            or marker[0][:3] != (package["name"], before["version"], package["version"])):
        return None
    files = set(git("diff", "--name-only", parent, commit).splitlines())
    if (files == metadata_files(m[0] for m in markers)
            and metadata_only(parent, commit, before, patch_only=False, markers=markers)):
        return pr


def automatic_ready(package):
    pr = manual_release_merge(version_commit(package["version"]))
    manual = {"version": package["version"], "merged_at": pr["merged_at"]} if pr else None
    return release.cooldown_ready(package["name"], manual_release=manual)


def ensure_tag(tag, commit):
    existing = git("tag", "--list", tag)
    if existing:
        if git("rev-parse", f"refs/tags/{tag}^{{commit}}") != commit:
            raise ValueError("Existing version tag points to a different commit")
    else:
        release.github(repo_path("git/refs"), "--method", "POST", "-f", f"ref=refs/tags/{tag}",
                       "-f", f"sha={commit}")


def publication_target(commit):
    subprocess.run(["git", "merge-base", "--is-ancestor", commit, "origin/main"],
                   cwd=release.ROOT, check=True)
    package = release.package_at(commit)
    tag = release.version_tag(package)
    release.prepare(tag, package, commit)
    ensure_tag(tag, commit)
    return {"tag": tag, "commit": commit}


def select_merged():
    refresh_main()
    commit = git("rev-parse", f"{os.environ['GITHUB_SHA']}^{{commit}}")
    if manual_release_merge(commit):
        return publication_target(commit)


def publication_ready(tag, commit):
    package = release.package_at(commit)
    result = release.prepare(tag, package, commit)
    # Revalidate the merge at upload time; a dispatch input or tag alone grants no exception.
    ready = (result["already_published"] == "true" or manual_release_merge(commit) is not None
             or automatic_ready(release.package_at(f"{commit}^1")))
    result["ready"] = str(ready).lower()
    return result


def select_automatic():
    refresh_main()
    package = release.package_at("origin/main")
    version = package["version"]
    versions = release.registry_versions(package["name"])
    releases = release.github_pages(repo_path("releases?per_page=100"))
    completed = {item["tag_name"] for item in releases if not item["draft"]}
    unfinished = [item for item in versions if release.version_tag({**package, "version": item["num"]}) not in completed]
    if unfinished:
        previous = max(unfinished, key=lambda item: item["created_at"])
        commit = release.published_commit(package["name"], previous["num"])
        package = release.package_at(commit)
    elif any(item["num"] == version for item in versions):
        if not automatic_ready(package):
            return
        merged = merge_patch(package)
        if not merged:
            return
        refresh_main()
        package = release.package_at(merged)
        commit = merged
    else:
        commit = version_commit(version)
        if not manual_release_merge(commit) and not automatic_ready(release.package_at(f"{commit}^1")):
            return
    return publication_target(commit)


def start_pr_checks():
    output = json.loads(os.environ["RELEASE_PR"])
    if not output.get("number"):
        return
    pr = release.github(repo_path(f"pulls/{output['number']}"))
    if not bot_pr(pr):
        raise ValueError("CI approval requires a same-repository release bot PR")
    ensure_pr_checks(pr)


def ensure_pr_checks(pr):
    if not bot_pr(pr):
        raise ValueError("CI approval requires a same-repository release bot PR")
    refresh_main()
    main = git("rev-parse", "origin/main")
    files = release.github_pages(repo_path(f"pulls/{pr['number']}/files?per_page=100"))
    markers = release_markers(pr)
    if not markers or any(m[0] not in release.MANIFESTS for m in markers):
        raise ValueError("Release PR must identify only supported packages")
    if (pr["base"]["sha"] != main
            or {item["filename"] for item in files} != metadata_files(m[0] for m in release_markers(pr))
            or not metadata_only(main, pr["head"]["sha"], release.package_at(main, markers[0][0]),
                                 patch_only=False, markers=markers)):
        print("Release PR must contain only current release metadata before CI approval.", file=sys.stderr)
        return
    for attempt in range(7):
        runs = json.loads(subprocess.check_output([
            "gh", "run", "list", "--workflow", "checks.yml", "--event", "pull_request",
            "--branch", pr["head"]["ref"], "--commit", pr["head"]["sha"],
            "--limit", "1", "--json", "databaseId,conclusion",
        ], cwd=release.ROOT, text=True))
        if runs:
            break
        if attempt == 6:
            raise TimeoutError("The release PR's Checks run has not appeared; retry preparation")
        time.sleep(5)
    if runs[0]["conclusion"] == "action_required":
        run_id = runs[0]["databaseId"]
        run = release.github(repo_path(f"actions/runs/{run_id}"))
        if (run["event"] != "pull_request" or run["head_sha"] != pr["head"]["sha"]
                or not any(item["number"] == pr["number"] for item in run["pull_requests"])):
            raise ValueError("Pending CI run does not match the inspected release PR")
        # GITHUB_TOKEN-created PR runs require approval. Dispatch runs do not satisfy
        # this repository's strict protected-PR gate; start its real merge CI instead.
        subprocess.run(["gh", "api", "--method", "POST", repo_path(f"actions/runs/{run_id}/approve")],
                       cwd=release.ROOT, check=True, stdout=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("select", "prepare", "start-pr-checks", "publication-ready", "wait-protocol"))
    args = parser.parse_args()
    if args.command == "start-pr-checks":
        start_pr_checks()
        return
    if args.command == "wait-protocol":
        release.wait_protocol(os.environ["RELEASE_COMMIT"])
        return
    if args.command == "publication-ready":
        commit = git("rev-parse", f"{os.environ['RELEASE_COMMIT']}^{{commit}}")
        result = publication_ready(os.environ["RELEASE_TAG"], commit)
        if result["ready"] == "true":
            result.update(protocol_prerequisite(commit))
        for key, value in result.items():
            print(f"{key}={value}")
        return
    if args.command == "prepare":
        # Do not propose another version while a merged release is still unpublished.
        ready = all(release.published_commit(name, release.package_at("HEAD", name)["version"]) is not None
                    for name in release.MANIFESTS)
        print(f"ready={str(ready).lower()}")
        return
    event = os.environ["GITHUB_EVENT_NAME"]
    tag = os.environ.get("RELEASE_TAG", "")
    result = None
    if event == "schedule" or (event == "workflow_dispatch" and tag == "auto"):
        for name in release.MANIFESTS:
            with selected_package(name):
                result = select_automatic()
            if result:
                result["package"] = name
                break
    elif event == "push" and os.environ.get("GITHUB_REF") == "refs/heads/main":
        for name in release.MANIFESTS:
            with selected_package(name):
                result = select_merged()
            if result:
                result["package"] = name
                break
    elif event == "push" or tag:
        if not tag:
            tag = os.environ["GITHUB_REF_NAME"]
        if not re.fullmatch(r"(?:slither-protocol-)?v[0-9][0-9A-Za-z.+-]*", tag):
            raise ValueError("Expected a version tag")
        commit = git("rev-parse", f"refs/tags/{tag}^{{commit}}")
        result = {"tag": tag, "commit": commit,
                  "package": release.PROTOCOL if tag.startswith("slither-protocol-") else "slither-sim"}
    for key, value in (result or {}).items():
        print(f"{key}={value}")


if __name__ == "__main__":
    main()
