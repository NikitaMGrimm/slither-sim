"""Synthetic checkpoint immutability, upload handshake and untrusted archive checks."""

import json
import os
from pathlib import Path
import stat
import tempfile
import time
import unittest
from unittest.mock import patch
import zipfile

import pr_report
import pr_report_archive
import pr_report_progress


class Progress(unittest.TestCase):
    def test_snapshots_are_immutable_bounded_and_plan_bound(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name)
            writer = pr_report.Snapshots(output)
            report = {"base": "a" * 40, "merge": "b" * 40, "head": "c" * 40,
                      "run_url": "https://example.invalid", "platform": "synthetic",
                      "performance_plan_sha256": "d" * 64}
            with patch.dict(os.environ, {"PR_REPORT_CHECKPOINT_UPLOAD": "0"}):
                for _ in range(6):
                    writer.checkpoint(report, time.perf_counter() + 1)
            self.assertEqual(writer.sequence, 3)
            report["performance_plan_sha256"] = "e" * 64
            for index in range(1, 4):
                directory = output / "progress" / str(index)
                saved = json.loads((directory / "report.json").read_text())
                self.assertEqual(saved["performance_plan_sha256"], "d" * 64)
                self.assertEqual(saved["snapshot"]["sequence"], index)
                self.assertTrue((directory / "ready").exists())
                with self.assertRaises(FileExistsError):
                    pr_report.save_report(directory, report)

    def test_handshake_resumes_only_preceding_uploaded_snapshot(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name)
            for index in (1, 2):
                directory = output / "progress" / str(index)
                directory.mkdir(parents=True)
                (directory / "ready").touch()
            self.assertTrue(pr_report_progress.wait(output, 1, time.monotonic() + 1))
            self.assertFalse((output / "progress/1/resume").exists())
            self.assertTrue(pr_report_progress.wait(output, 2, time.monotonic() + 1))
            self.assertTrue((output / "progress/1/resume").exists())
            self.assertFalse((output / "progress/2/resume").exists())
            (output / "done").touch()
            self.assertFalse(pr_report_progress.wait(output, 3, time.monotonic() + 1))
            self.assertTrue((output / "progress/2/resume").exists())

    def test_archive_rejects_paths_duplicates_symlinks_and_oversize(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            for index, extra in enumerate(("../report.json", "report.json", "oversize", "symlink", None)):
                archive = root / f"{index}.zip"
                with zipfile.ZipFile(archive, "w") as source:
                    source.writestr("report.md", "report")
                    if extra == "symlink":
                        info = zipfile.ZipInfo("report.json")
                        info.external_attr = (stat.S_IFLNK | 0o777) << 16
                        source.writestr(info, "target")
                    else:
                        source.writestr("report.json", "x" * 1_000_001 if extra == "oversize" else "{}")
                    if extra and extra not in ("oversize", "symlink"):
                        source.writestr(extra, "{}")
                output = root / f"out-{index}"
                if extra:
                    with self.assertRaises(ValueError):
                        pr_report_archive.unpack(archive, output)
                    self.assertFalse(output.exists())
                else:
                    pr_report_archive.unpack(archive, output)
                    self.assertEqual((output / "report.json").read_text(), "{}")


if __name__ == "__main__":
    unittest.main()
