"""Close a complete per-row checkpoint, including qualification and RTT sources."""
from pathlib import Path
import hashlib
import json
import tarfile
import urllib.request


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checkpoint(root, directory, api, endpoint):
    meta = api("snapshot", label=directory.name)
    assert meta["complete"] and meta["allMembersVerified"]
    path = directory / "server.tar.gz"
    urllib.request.urlretrieve(endpoint + "/" + meta["file"], path)
    assert path.stat().st_size == meta["bytes"] and sha(path) == meta["sha256"]
    (directory / "server-retrieval.json").write_text(json.dumps(meta) + "\n")
    for source, target in [("clients.jsonl", "clients.jsonl"), ("icmp.log", "icmp.log")]:
        path = root / source
        data = path.read_bytes() if path.exists() else b""
        prefix = data[:data.rfind(b"\n") + 1]
        # Every row needs RTT samples; an empty ICMP log must not qualify.
        assert prefix or source != "icmp.log", "ICMP log is missing or has no complete line"
        (directory / target).write_bytes(prefix)
    for name in ["pins.json", "build-ready.json"]:
        (directory / name).write_bytes((root / name).read_bytes())
    if (root / "control-manifest.json").exists():
        for name in ["control-manifest.json", "run.py", "checkpoint.py", "server_agent.py"]:
            (directory / name).write_bytes((root / name).read_bytes())
    generated = {"row-manifest.json", "row.tar.gz", "row.tar.gz.tmp", "row-ready.json", "row-ready.tmp"}
    files = [path for path in directory.iterdir() if path.is_file() and path.name not in generated]
    manifest = {path.name: sha(path) for path in files}
    manifest_path = directory / "row-manifest.json"
    manifest_path.write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
    temporary = directory / "row.tar.gz.tmp"
    with tarfile.open(temporary, "w:gz", compresslevel=1) as archive:
        for path in files + [manifest_path]:
            archive.add(path, arcname=path.name)
    with tarfile.open(temporary) as archive:
        assert len(archive.getmembers()) == len(manifest) + 1
        for name, digest in manifest.items():
            with archive.extractfile(name) as stream:
                assert hashlib.file_digest(stream, "sha256").hexdigest() == digest, name
    output = directory / "row.tar.gz"
    temporary.replace(output)
    ready = {"complete": True, "allMembersVerified": True, "requiredSourcesVerified": True,
             "archiveBytes": output.stat().st_size, "archiveSha256": sha(output),
             "manifestSha256": sha(manifest_path)}
    path = directory / "row-ready.tmp"
    path.write_text(json.dumps(ready, indent=2) + "\n")
    path.replace(directory / "row-ready.json")
