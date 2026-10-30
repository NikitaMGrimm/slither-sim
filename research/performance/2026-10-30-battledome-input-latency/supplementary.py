"""Retain incomplete first-attempt probes explicitly as unqualified evidence.

Remote use: python3 supplementary.py CHECKPOINT_DIRECTORY OUTPUT_JSON.
These checkpoints lack generator-wide qualification and ICMP logs.
"""
from pathlib import Path
import collections
import hashlib
import json
import sys
import tarfile

from reduce import stats


def main():
    root, output = Path(sys.argv[1]), Path(sys.argv[2])
    result = []
    for label in ["01-solo", "02-80", "03-160"]:
        path = root / f"checkpoint-{label}.tar.gz"
        ready = json.loads((root / f"checkpoint-{label}-ready.json").read_text())
        assert ready["complete"] and ready["allMembersVerified"]
        assert path.stat().st_size == ready["bytes"]
        with path.open("rb") as stream:
            assert hashlib.file_digest(stream, "sha256").hexdigest() == ready["sha256"]
        with tarfile.open(path) as archive:
            with archive.extractfile("checkpoint-manifest.json") as stream:
                manifest = json.load(stream)
            for name, expected in manifest.items():
                with archive.extractfile(name) as stream:
                    assert hashlib.file_digest(stream, "sha256").hexdigest() == expected
            with archive.extractfile("window.json") as stream:
                window = json.load(stream)
            with archive.extractfile("probe.jsonl") as stream:
                records = [json.loads(line) for line in stream]
        start, end = window["startUnix"], window["endUnix"]
        sent = {value["id"]: value for value in records if value["type"] == "send" and start <= value["unix"] < end}
        echoes = [value for value in records if value["type"] == "echo" and value["id"] in sent and start <= value["unix"] < end]
        censors = [value for value in records if value["type"] == "censored" and value["id"] in sent]
        result.append({"label": label, "seats": window["seats"], "evidenceStatus": "unverified",
                       "reason": "First generator reclaimed; complete probe/capture checkpoint survives but generator qualification and ICMP sources do not",
                       "sourceArchiveSha256": ready["sha256"], "attempts": len(sent), "confirmed": len(echoes),
                       "inputToEffectMs": stats([value["milliseconds"] for value in echoes]),
                       "censored": dict(collections.Counter(value["reason"] for value in censors))})
    output.write_text(json.dumps({"rows": result, "firstAttempt320": "unavailable; no retained raw artifact"}, indent=2) + "\n")


if __name__ == "__main__":
    main()
