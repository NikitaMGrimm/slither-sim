"""Shared-deadline regressions; mocked children/clocks, no benchmark execution."""

import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

import perf_baseline
import perf_compare
from test_perf_baseline import catalog


class SharedDeadline(unittest.TestCase):
    def test_native_exit_wait_timeout_is_a_builtin_timeout_with_diagnostics(self):
        child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
        child.poll.return_value = 0
        child.wait.side_effect = subprocess.TimeoutExpired("fixture", .5)
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_baseline.subprocess, "Popen", return_value=child), patch.object(
                perf_baseline.Lines, "read", return_value=b"{}"), patch.object(
                perf_baseline.time, "perf_counter", return_value=10):
            errors = Path(temporary) / "stderr.txt"
            with self.assertRaises(TimeoutError) as caught:
                perf_baseline.native(Path(sys.executable), "observe", 1, errors, deadline=10.5)
            self.assertIsInstance(caught.exception.__cause__, subprocess.TimeoutExpired)
            self.assertEqual(errors.with_suffix(".stdout.bin").read_bytes(), b"{}\n")
            failure = json.loads(errors.with_suffix(".failure.json").read_text())
            self.assertEqual(failure["type"], "TimeoutError")
            self.assertIn("exit", failure["error"])
        self.assertTrue(child.stdout.closed)

    def test_native_failures_preserve_exact_stdout_bytes(self):
        for name, payload, ending, exception in (
                ("partial", b'{"x":\xff', "import time; time.sleep(60)", TimeoutError),
                ("exit-timeout", b'{}\r\ntrailing\xff', "import time; time.sleep(60)", TimeoutError),
                ("exit-code", b'{}\r\ntrailing\xff', "sys.exit(3)", RuntimeError),
                ("malformed", b'not JSON\xff\r\ntrailing\xfe', "", ValueError)):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                errors = Path(temporary) / "stderr.txt"
                script = Path(temporary) / "fixture.py"
                script.write_text(f"import sys\nsys.stdout.buffer.write({payload!r})\n"
                                  f"sys.stdout.buffer.flush()\n{ending}\n", encoding="utf-8")
                executable = sys._base_executable  # Avoid the Windows venv launcher child.
                budget = 1 if exception is TimeoutError else 30
                with self.assertRaises(exception):
                    perf_baseline.native(executable, str(script), 1, errors,
                                         deadline=perf_baseline.time.perf_counter() + budget)
                stdout = errors.with_suffix(".stdout.bin")
                self.assertEqual(stdout.read_bytes(), payload)
                failure = json.loads(errors.with_suffix(".failure.json").read_text())
                self.assertEqual(failure["stdout"], str(stdout))
                self.assertEqual(failure["stderr"], str(errors))
                self.assertEqual(failure["command"], [executable, str(script), "1"])
                if name in ("exit-code", "malformed"):
                    self.assertEqual(failure["exit_code"], 3 if name == "exit-code" else 0)
                else:
                    self.assertIsInstance(failure["exit_code"], int)

    def test_metadata_uses_remaining_budget_and_never_spawns_after_expiry(self):
        with patch.object(perf_baseline.time, "perf_counter", return_value=10), patch.object(
                perf_baseline.subprocess, "check_output", return_value="fixture\n") as command:
            self.assertEqual(perf_baseline.command_output("git", deadline=10.5), "fixture")
            self.assertEqual(command.call_args.kwargs["timeout"], .5)
            perf_baseline.command_output("git", deadline=100)
            self.assertEqual(command.call_args.kwargs["timeout"], perf_baseline.COMMAND_TIMEOUT_SECONDS)
            command.reset_mock()
            with self.assertRaises(TimeoutError):
                perf_baseline.command_output("git", deadline=9)
            command.assert_not_called()

    def test_expired_native_budget_creates_no_child_or_error_file(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_baseline.time, "perf_counter", return_value=10), patch.object(
                perf_baseline.subprocess, "Popen") as spawn:
            errors = Path(temporary) / "stderr.txt"
            with self.assertRaises(TimeoutError):
                perf_baseline.native(Path(sys.executable), "observe", 1, errors, deadline=9)
            spawn.assert_not_called()
            self.assertFalse(errors.exists())

    def test_native_read_and_wait_share_the_shortened_deadline(self):
        for deadline, expected in [(None, 130), (10.5, 10.5), (200, 130)]:
            child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
            child.poll.return_value = 0
            clocks = [10, 10.1, 10.3] if deadline is None else [10, 10.1, 10.2, 10.3]
            with self.subTest(deadline=deadline), tempfile.TemporaryDirectory() as temporary, patch.object(
                    perf_baseline.subprocess, "Popen", return_value=child) as spawn, patch.object(
                    perf_baseline.Lines, "read", return_value=b"{}") as read, patch.object(
                    perf_baseline.time, "perf_counter", side_effect=clocks), patch.object(
                    perf_baseline, "process_metrics", return_value={}):
                row = perf_baseline.native(Path(sys.executable), "observe", 1,
                                           Path(temporary) / "stderr.txt", deadline=deadline)
                read.assert_called_once_with(expected)
                self.assertAlmostEqual(child.wait.call_args.kwargs["timeout"], expected - 10.1)
                self.assertAlmostEqual(row["process_wall_seconds"], .3)
                self.assertTrue(spawn.call_args.kwargs["stderr"].closed)
            self.assertTrue(child.stdout.closed)

    def test_native_timeout_kills_waits_and_closes_actual_handles(self):
        child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None)
        child.poll.return_value = None
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_baseline.subprocess, "Popen", return_value=child) as spawn, patch.object(
                perf_baseline.Lines, "read", side_effect=TimeoutError("deadline witness")), patch.object(
                perf_baseline.time, "perf_counter", return_value=10):
            with self.assertRaisesRegex(TimeoutError, "deadline witness"):
                perf_baseline.native(Path(sys.executable), "observe", 1,
                                     Path(temporary) / "stderr.txt", deadline=10.5)
            child.kill.assert_called_once()
            child.wait.assert_called_once_with(timeout=10)
            self.assertTrue(spawn.call_args.kwargs["stderr"].closed)
        self.assertTrue(child.stdout.closed)

    def test_default_native_wait_keeps_legacy_grace_near_expiry(self):
        child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
        child.poll.return_value = 0
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_baseline.subprocess, "Popen", return_value=child), patch.object(
                perf_baseline.Lines, "read", return_value=b"{}") as read, patch.object(
                perf_baseline.time, "perf_counter", side_effect=[10, 129.95, 130.04]), patch.object(
                perf_baseline, "process_metrics", return_value={}):
            perf_baseline.native(Path(sys.executable), "observe", 1, Path(temporary) / "stderr.txt")
            read.assert_called_once_with(130)
            child.wait.assert_called_once_with(timeout=.1)
        self.assertTrue(child.stdout.closed)

    def test_native_rejects_a_report_finishing_after_the_deadline(self):
        child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
        child.poll.return_value = 0
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_baseline.subprocess, "Popen", return_value=child) as spawn, patch.object(
                perf_baseline.Lines, "read", return_value=b"{}"), patch.object(
                perf_baseline.time, "perf_counter", side_effect=[10, 10.1, 10.7]), patch.object(
                perf_baseline, "process_metrics") as metrics:
            with self.assertRaises(TimeoutError):
                perf_baseline.native(Path(sys.executable), "observe", 1,
                                     Path(temporary) / "stderr.txt", deadline=10.5)
            metrics.assert_not_called()
            self.assertTrue(spawn.call_args.kwargs["stderr"].closed)
        self.assertTrue(child.stdout.closed)

    def test_compare_keeps_partial_rows_and_never_starts_the_next_child(self):
        now = [10.0]

        def result(*args, **kwargs):
            self.assertEqual(kwargs["deadline"], 11)
            now[0] = 12
            return {"case": "observe", "cpu_seconds": 1,
                    "workload": {"timing": {"wallSeconds": 1}}}

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "fixture.exe"
            binary.write_bytes(b"not an executable")
            output = root / "attempt"
            argv = ["perf_compare", "--baseline", str(binary), "--candidate", str(binary),
                    "--output", str(output), "--cases", "observe", "--repeats", "1",
                    "--max-wall-seconds", "1"]
            with patch.object(sys, "argv", argv), patch.object(
                    perf_compare, "load_catalog", return_value=catalog()) as load, patch.object(
                    perf_compare, "command_output", return_value="fixture") as metadata, patch.object(
                    perf_compare, "native", side_effect=result) as native, patch.object(
                    perf_compare.time, "perf_counter", side_effect=lambda: now[0]), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(TimeoutError):
                    perf_compare.main()
            native.assert_called_once()
            self.assertTrue(all(call.kwargs == {"deadline": 11}
                                for call in metadata.call_args_list + load.call_args_list))
            self.assertEqual(len((output / "results.jsonl").read_text().splitlines()), 1)
            self.assertFalse((output / "summary.json").exists())
            failure = json.loads((output / "failure.json").read_text())
            self.assertEqual(failure["type"], "TimeoutError")
            self.assertEqual(failure["max_wall_seconds"], 1)
            self.assertEqual(failure["elapsed_wall_seconds"], 2)

    def test_compare_rejects_nonpositive_or_nonfinite_budgets(self):
        for value in ("0", "-1", "nan", "inf"):
            with self.subTest(value=value), patch.object(sys, "argv", [
                    "perf_compare", "--baseline", sys.executable, "--candidate", sys.executable,
                    "--output", "unused", "--max-wall-seconds", value]), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    perf_compare.main()
                self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
