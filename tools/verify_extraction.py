"""Verify documented import hashes, optionally against the original Git commit."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    failures = []
    count = 0
    manifests = ("extraction.json", "toolkit-import.json", "client-import.json")
    for name in manifests:
        manifest = json.loads((root / "docs" / name).read_text(encoding="utf-8"))
        for entry in manifest["files"]:
            relative = Path(entry["path"])
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError(f"unsafe manifest path: {relative}")
            path = root / relative
            if not path.is_file() or digest(path.read_bytes()) != entry["extractedSha256"]:
                failures.append(f"extracted file differs: {relative}")
            if args.source_repo:
                source_relative = Path(entry.get("sourcePath", entry["path"]))
                if source_relative.is_absolute() or ".." in source_relative.parts:
                    raise ValueError(f"unsafe source manifest path: {source_relative}")
                original = subprocess.run(
                    ["git", "-C", str(args.source_repo), "show", f"{manifest['sourceCommit']}:{source_relative.as_posix()}"],
                    check=True,
                    stdout=subprocess.PIPE,
                ).stdout
                if digest(original) != entry["sourceSha256"]:
                    failures.append(f"source file differs: {relative}")
            count += 1
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"Verified {count} imported files across {len(manifests)} extraction manifests")


if __name__ == "__main__":
    main()
