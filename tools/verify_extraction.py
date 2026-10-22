"""Audit the historical import snapshot, optionally against its original source."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


IMPORT_SNAPSHOT = "f4d9f8b994c8714429ebe4cb1618209a34d9c45b"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def git_bytes(root, revision, path):
    return subprocess.run(
        ["git", "-C", str(root), "show", f"{revision}:{path}"],
        check=True, stdout=subprocess.PIPE,
    ).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", type=Path)
    parser.add_argument("--revision", default=IMPORT_SNAPSHOT,
                        help="historical packaged snapshot to audit; requires its Git history")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    failures = []
    count = 0
    manifests = ("extraction.json", "toolkit-import.json", "client-import.json")
    for name in manifests:
        manifest = json.loads(git_bytes(root, args.revision, f"docs/{name}"))
        for entry in manifest["files"]:
            relative = Path(entry["path"])
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError(f"unsafe manifest path: {relative}")
            packaged = git_bytes(root, args.revision, relative.as_posix())
            if digest(packaged) != entry["extractedSha256"]:
                failures.append(f"extracted file differs: {relative}")
            if args.source_repo:
                source_relative = Path(entry.get("sourcePath", entry["path"]))
                if source_relative.is_absolute() or ".." in source_relative.parts:
                    raise ValueError(f"unsafe source manifest path: {source_relative}")
                original = git_bytes(args.source_repo, manifest["sourceCommit"], source_relative.as_posix())
                if digest(original) != entry["sourceSha256"]:
                    failures.append(f"source file differs: {relative}")
            count += 1
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Verified {count} historical imports at {args.revision}; current working files are not checked")


if __name__ == "__main__":
    main()
