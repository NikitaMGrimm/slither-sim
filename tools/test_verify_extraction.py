"""Historical import audits stay valid when the current implementation evolves."""

from contextlib import redirect_stdout
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import verify_extraction


@unittest.skipUnless(shutil.which("git"), "Git is needed for historical source audits")
class HistoricalImportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "--quiet")
        (self.root / "docs").mkdir()
        for name in ("extraction", "toolkit-import", "client-import"):
            relative = f"{name}.txt"
            content = name.encode()
            (self.root / relative).write_bytes(content)
            manifest = {"sourceCommit": "unused", "files": [{
                "path": relative, "sourceSha256": hashlib.sha256(content).hexdigest(),
                "extractedSha256": hashlib.sha256(content).hexdigest(),
            }]}
            (self.root / "docs" / f"{name}.json").write_text(json.dumps(manifest), encoding="utf-8")
        self.commit()

    def git(self, *arguments):
        return subprocess.run(["git", "-C", str(self.root), *arguments],
                              check=True, capture_output=True).stdout

    def commit(self):
        self.git("add", ".")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                 "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null",
                 "commit", "--quiet", "-m", "test snapshot")

    def run_audit(self):
        with patch.object(verify_extraction, "__file__", str(self.root / "tools/verify_extraction.py")):
            with patch.object(sys, "argv", ["verify_extraction.py", "--revision", "HEAD"]):
                with redirect_stdout(io.StringIO()) as output:
                    verify_extraction.main()
        return output.getvalue()

    def test_uncommitted_source_and_manifest_edits_do_not_relabel_history(self):
        (self.root / "extraction.txt").write_text("new implementation", encoding="utf-8")
        (self.root / "docs/extraction.json").write_text("{}", encoding="utf-8")
        self.assertIn("Verified 3 historical imports", self.run_audit())

    def test_historical_hash_mismatch_is_rejected(self):
        path = self.root / "docs/extraction.json"
        manifest = json.loads(path.read_text(encoding="utf-8"))
        manifest["files"][0]["extractedSha256"] = "0" * 64
        path.write_text(json.dumps(manifest), encoding="utf-8")
        self.commit()
        with self.assertRaisesRegex(SystemExit, "extracted file differs"):
            self.run_audit()


if __name__ == "__main__":
    unittest.main()
