"""Offline source-binding regression; no recordings or performance reruns."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import unittest

FOLDER = Path(__file__).resolve().parent
REPO = FOLDER.parents[2]


def normalized(text):
    text = text.split("\n#[cfg(test)]\nmod ", 1)[0]
    text = text.split("\nthread_local! { static TRANSACTION_OBSERVER", 1)[0]
    return ("\n".join(line for line in text.splitlines()
                       if not re.search(r"(?:slither_client|crate)::state::transaction_phase\(", line))).strip() + "\n"


class SourceIdentity(unittest.TestCase):
    def test_reviewed_blobs_and_historical_normalized_identities(self):
        data = json.loads((FOLDER / "identities.json").read_text(encoding="utf-8"))
        for arm in data["arms"].values():
            for path, expected in arm["reviewed_source_sha256"].items():
                blob = subprocess.check_output(["git", "show", f'{arm["reviewed_commit"]}:{path}'], cwd=REPO)
                self.assertEqual(hashlib.sha256(blob).hexdigest(), expected)
                self.assertEqual(hashlib.sha256(normalized(blob.decode()).encode()).hexdigest(),
                                 arm["runtime_source_sha256"][path])

    def test_source_delta_identity(self):
        data = json.loads((FOLDER / "identities.json").read_text(encoding="utf-8"))
        self.assertEqual(hashlib.sha256((FOLDER / "candidate-measured-to-reviewed.patch").read_bytes()).hexdigest(),
                         data["source_identity_notes"]["candidate_delta_sha256"])


if __name__ == "__main__":
    unittest.main()
