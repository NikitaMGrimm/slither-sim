"""Regression checks for failure diagnostics and temporary child cleanup."""

from pathlib import Path
import io
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import perf_baseline


class ProcessFailures(unittest.TestCase):
    def test_host_diagnostics_keep_bytes_read_with_announcement(self):
        witness = b"Listening on ws://127.0.0.1:1/slither\nfatal witness\n"
        with subprocess.Popen([sys.executable, "-c", f"import os; os.write(2, {witness!r})"],
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as process:
            process.wait(timeout=10)
            reader = perf_baseline.Lines(process.stderr)
            announcement = reader.read(time.perf_counter() + 10)
            self.assertEqual(reader.pending, b"fatal witness\n")
            output = io.BytesIO()
            output.write(announcement + b"\n")
            reader.drain(output)
            self.assertEqual(output.getvalue(), witness)

    def test_early_exit_retains_stderr(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "fails.py"
            script.write_text("import sys\nprint('failure witness', file=sys.stderr)\nsys.exit(3)\n")
            errors = root / "stderr.txt"
            with self.assertRaises(EOFError):
                perf_baseline.native(sys.executable, str(script), 1, errors)
            self.assertIn("failure witness", errors.read_text())

    def test_timeout_terminates_and_waits_for_child(self):
        processes = []
        real_popen = subprocess.Popen

        def spawn(*args, **kwargs):
            process = real_popen(*args, **kwargs)
            processes.append(process)
            return process

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "waits.py"
            script.write_text("import time\ntime.sleep(60)\n")
            with patch.object(perf_baseline.subprocess, "Popen", spawn), patch.object(
                    perf_baseline.Lines, "read", side_effect=TimeoutError("deadline witness")):
                with self.assertRaises(TimeoutError):
                    perf_baseline.native(sys.executable, str(script), 1, root / "stderr.txt")
            self.assertEqual(len(processes), 1)
            self.assertIsNotNone(processes[0].poll())


if __name__ == "__main__":
    unittest.main()
