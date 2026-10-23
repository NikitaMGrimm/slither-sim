"""Regression checks for failure diagnostics and temporary child cleanup."""

from pathlib import Path
import io
import json
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import perf_baseline
import perf_compare


class ProcessFailures(unittest.TestCase):
    def test_manifest_command_timeout_is_enforced(self):
        with patch.object(perf_baseline, "COMMAND_TIMEOUT_SECONDS", .05):
            with self.assertRaises(subprocess.TimeoutExpired):
                perf_baseline.command_output(sys.executable, "-c", "import time; time.sleep(1)")

    def test_manifest_timeout_records_failure_in_both_runners(self):
        with tempfile.TemporaryDirectory() as temporary:
            for module, options in [
                    (perf_baseline, ["--harness", sys.executable, "--cli", sys.executable]),
                    (perf_compare, ["--baseline", sys.executable, "--candidate", sys.executable])]:
                with self.subTest(runner=module.__name__):
                    output = Path(temporary) / module.__name__
                    with patch.object(sys, "argv", [module.__name__, *options, "--output", str(output)]), patch.object(
                            module, "command_output", side_effect=subprocess.TimeoutExpired("git", 30)):
                        with self.assertRaises(subprocess.TimeoutExpired):
                            module.main()
                    self.assertEqual(json.loads((output / "failure.json").read_text())["type"], "TimeoutExpired")

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
