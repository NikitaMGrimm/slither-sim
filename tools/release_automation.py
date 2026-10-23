"""Reconcile the rolling release PR and select an immutable publication commit."""

import argparse
import json
import os
import re
import subprocess
import sys
import time
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


def metadata_only(main, head, package, patch_only=True):
    subprocess.run(["git", "fetch", "origin", head], cwd=release.ROOT, check=True)
    if git("merge-base", main, head) != main:
        return False
    before = tomllib.loads(git("show", f"{main}:Cargo.toml"))
    after = tomllib.loads(git("show", f"{head}:Cargo.toml"))
    proposed = after["package"]["version"]
    if proposed == package["version"] or (patch_only and not is_patch(package["version"], proposed)):
        return False
    after["package"]["version"] = before["package"]["version"]
    if before != after:
        return False
    before_lock = tomllib.loads(git("show", f"{main}:Cargo.lock"))
    after_lock = tomllib.loads(git("show", f"{head}:Cargo.lock"))
    for item in after_lock["package"]:
        if item["name"] == package["name"] and "source" not in item:
            if item["version"] != proposed:
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
    marker = re.findall(r"<!-- release-plz (\S+) (\S+) (\S+) (\S+) -->", pr["body"] or "")
    if len(marker) != 1 or marker[0][:3] != (package["name"], before["version"], package["version"]):
        return None
    files = set(git("diff", "--name-only", parent, commit).splitlines())
    if (files == {"Cargo.toml", "Cargo.lock", "CHANGELOG.md"}
            and metadata_only(parent, commit, before, patch_only=False)):
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
    tag = f"v{package['version']}"
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
    unfinished = [item for item in versions if f"v{item['num']}" not in completed]
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
    if (pr["base"]["sha"] != main
            or {item["filename"] for item in files} != {"Cargo.toml", "Cargo.lock", "CHANGELOG.md"}
            or not metadata_only(main, pr["head"]["sha"], release.package_at(main), patch_only=False)):
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
    parser.add_argument("command", choices=("select", "prepare", "start-pr-checks", "publication-ready"))
    args = parser.parse_args()
    if args.command == "start-pr-checks":
        start_pr_checks()
        return
    if args.command == "publication-ready":
        commit = git("rev-parse", f"{os.environ['RELEASE_COMMIT']}^{{commit}}")
        result = publication_ready(os.environ["RELEASE_TAG"], commit)
        for key, value in result.items():
            print(f"{key}={value}")
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
    elif event == "push" and os.environ.get("GITHUB_REF") == "refs/heads/main":
        result = select_merged()
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
