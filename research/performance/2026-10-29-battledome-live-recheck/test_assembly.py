"""Small synthetic controls for row provenance, receipt gates and topology."""
from contextlib import redirect_stdout
import copy
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

from assemble_results import generator_segment, row_archive, sha
from select_results import bind_archive
import tables


class ArchiveControls(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.folder = Path(self.temporary.name)
        self.raw = self.folder / "raw"
        self.raw.mkdir()
        (self.raw / "input.json").write_text('{"tick": 1}\n')
        archive = self.folder / "results.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            stream.add(self.raw / "input.json", arcname="input.json")
        self.receipt = {"label": "01-base-160", "arm": "base", "target": 160,
                        "archiveSha256": sha(archive), "archiveBytes": archive.stat().st_size,
                        "requiredSourcesVerified": True, "status": "complete"}
        self.write_receipt()
        self.row = {"label": "01-base-160", "arm": "base", "primary": {"bots": 160},
                    "sourceArchiveSha256": self.receipt["archiveSha256"]}

    def write_receipt(self):
        (self.folder / "retrieval-verification.json").write_text(json.dumps(self.receipt))

    def test_selection_binds_exact_raw_files_to_archive(self):
        self.assertEqual(bind_archive(self.raw), self.receipt)
        (self.raw / "input.json").write_text('{"tick": 2}\n')
        with self.assertRaises(AssertionError):
            bind_archive(self.raw)

    def test_matching_archive_does_not_accept_another_selections_digest(self):
        row_archive(self.row, self.folder, primary=True)
        self.row["sourceArchiveSha256"] = "0" * 64
        with self.assertRaisesRegex(AssertionError, "different archive"):
            row_archive(self.row, self.folder, primary=True)

    def test_both_receipt_gates_reject_primary_but_preserve_supplement(self):
        for field, value in [("requiredSourcesVerified", False), ("status", "incomplete")]:
            with self.subTest(field=field):
                before = self.receipt[field]
                self.receipt[field] = value
                self.write_receipt()
                with self.assertRaisesRegex(AssertionError, "incomplete or source-unverified"):
                    row_archive(copy.deepcopy(self.row), self.folder, primary=True)
                row = copy.deepcopy(self.row)
                row_archive(row, self.folder)
                self.assertEqual(row["evidenceStatus"], "unverified")
                data = {"rows": [], "interruptedAttempt": {"rows": [row]}}
                path = self.folder / "selected.json"
                path.write_text(json.dumps(data))
                import sys
                from unittest.mock import patch
                with patch.object(sys, "argv", ["tables.py", str(path)]), redirect_stdout(io.StringIO()) as output:
                    tables.main()
                self.assertIn("unverified archive; excluded", output.getvalue())
                self.receipt[field] = before
                self.write_receipt()

    def test_each_generator_segment_has_its_own_topology(self):
        types = [{"InstanceType": name, "VCpuInfo": {"DefaultVCpus": n, "DefaultCores": n, "DefaultThreadsPerCore": 1}}
                 for name, n in [("c8a.4xlarge", 16), ("c8a.2xlarge", 8)]]
        segments = [generator_segment({"type": "c8a.4xlarge"}, types, [1, 2, 3]),
                    generator_segment({"type": "c8a.2xlarge"}, types, [4])]
        self.assertEqual([v["vcpus"] for v in segments], [16, 8])
        self.assertEqual([v["physicalCores"] for v in segments], [16, 8])


if __name__ == "__main__":
    unittest.main()
