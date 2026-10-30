"""Select only from complete, digest-bound row checkpoints; keep failed gates.

Remote use: python3 assemble.py ROOT MASTER_PINS OUTPUT.
ROOT/rows/LABEL contains row.tar.gz and row-ready.json from checkpoint.py.
"""
from pathlib import Path
import hashlib
import json
import sys
import tarfile
import tempfile

from reduce import host_intervals, icmp_records, load_records, summarize_row


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_members(archive_path, manifest_name):
    with tarfile.open(archive_path) as archive:
        with archive.extractfile(manifest_name) as stream:
            manifest = json.load(stream)
    seen = set()
    # Read compressed members in physical order; repeated random seeks through
    # a gzip stream can otherwise re-inflate the whole recording per member.
    with tarfile.open(archive_path, "r|gz") as archive:
        for member in archive:
            assert member.name not in seen
            seen.add(member.name)
            if member.name == manifest_name:
                continue
            assert member.name in manifest
            with archive.extractfile(member) as stream:
                assert hashlib.file_digest(stream, "sha256").hexdigest() == manifest[member.name], member.name
    assert seen == set(manifest) | {manifest_name}


def main():
    root, master_path, output = map(Path, sys.argv[1:])
    master = json.loads(master_path.read_text())
    assert master["runtime"] == "9cd6ba6922c0a63b525ba88b69f86c8558142596"
    assert master["runtimeSrcUnchanged"] and master["portable"]
    build = json.loads((root / "build-ready.json").read_text())
    assert build["complete"] and build["allMembersVerified"] and build["requiredSourcesVerified"]
    build_path = root / "build-sources.tar.gz"
    assert build_path.stat().st_size == build["archiveBytes"]
    assert sha(build_path) == build["archiveSha256"]
    verify_members(build_path, "build-manifest.json")
    with tarfile.open(build_path) as archive:
        with archive.extractfile("pins.json") as stream:
            assert json.load(stream) == master
        with archive.extractfile("source-manifest.json") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == master["sourceManifestSha256"]
        with archive.extractfile("main.tar") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == master["runtimeArchiveSha256"]
        for name, expected in master["binaries"].items():
            with archive.extractfile("bin/" + name) as stream:
                assert hashlib.file_digest(stream, "sha256").hexdigest() == expected
    selected = []
    (root / "analysis").mkdir(exist_ok=True)
    for label in ["01-solo", "02-80", "03-160", "04-320"]:
        directory = root / "rows" / label
        receipt = json.loads((directory / "row-ready.json").read_text())
        assert receipt["complete"] and receipt["allMembersVerified"] and receipt["requiredSourcesVerified"]
        archive = directory / "row.tar.gz"
        assert archive.stat().st_size == receipt["archiveBytes"]
        assert sha(archive) == receipt["archiveSha256"]
        verify_members(archive, "row-manifest.json")
        with tempfile.TemporaryDirectory(dir=root / "analysis") as temporary:
            work = Path(temporary)
            with tarfile.open(archive) as stream:
                stream.extractall(work, filter="data")
            assert sha(work / "row-manifest.json") == receipt["manifestSha256"]
            # Bind the selected row to the exact build it was checked against.
            assert json.loads((work / "pins.json").read_text()) == master
            assert json.loads((work / "build-ready.json").read_text()) == build
            window = json.loads((work / "window.json").read_text())
            assert window["label"] == label
            assert window["seats"] == {"01-solo": 1, "02-80": 80, "03-160": 160, "04-320": 320}[label]
            assert window.get("selectionRole", "primary") == "primary"
            server = json.loads((work / "server-retrieval.json").read_text())
            assert server["complete"] and server["allMembersVerified"]
            assert (work / "server.tar.gz").stat().st_size == server["bytes"]
            assert sha(work / "server.tar.gz") == server["sha256"]
            verify_members(work / "server.tar.gz", "manifest.json")
            host = work / "host"
            host.mkdir()
            with tarfile.open(work / "server.tar.gz") as stream:
                stream.extractall(host, filter="data")
            intervals = host_intervals(host / ("solo.jsonl" if label == "01-solo" else "loaded.jsonl"))
            clients = [json.loads(line[7:]) for line in (work / "clients.jsonl").read_text().splitlines()
                       if line.startswith("CLIENT ")]
            result = summarize_row(work, intervals, icmp_records(work / "icmp.log"), clients)
            result["sourceArchiveSha256"] = receipt["archiveSha256"]
            if (work / "control-manifest.json").exists():
                controls = json.loads((work / "control-manifest.json").read_text())
                assert all(sha(work / name) == digest for name, digest in controls.items())
                result["controllerManifestSha256"] = sha(work / "control-manifest.json")
            result["archive"] = {"bytes": receipt["archiveBytes"],
                                 "manifestSha256": receipt["manifestSha256"],
                                 "completeReceiptAndSourcesVerified": True}
            result["evidenceStatus"] = ("qualified" if result["generatorQualified"]
                                        and result["minimumSampleQuotaPass"] else "unverified")
            selected.append(result)
    pins = dict(master, buildArchive=build)
    pins["analysisSha256"] = {name: sha(Path(__file__).with_name(name))
                              for name in ["assemble.py", "reduce.py", "historical.py", "tables.py", "test_reduce.py"]}
    output.write_text(json.dumps({"pins": pins, "rows": selected}, indent=2) + "\n")
    print(json.dumps({"rows": len(selected), "qualified": sum(row["evidenceStatus"] == "qualified" for row in selected)}))


if __name__ == "__main__":
    main()
