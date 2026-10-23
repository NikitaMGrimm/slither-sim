"""Regression checks for failure diagnostics and temporary child cleanup."""

from pathlib import Path
import gzip
import hashlib
import io
import json
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

import perf_baseline
import perf_compare


class ProcessFailures(unittest.TestCase):
    def test_native_and_trace_close_handles_after_stop_or_pipe_failure(self):
        class CloseFailure(io.BytesIO):
            def close(self):
                if not self.closed:
                    super().close()
                    raise OSError("cleanup witness")

        for entry in ("native", "trace"):
            for failure in ("stop", "pipe"):
                with self.subTest(entry=entry, failure=failure), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    stdout = CloseFailure() if failure == "pipe" else io.BytesIO()
                    child = Mock(stdin=None, stdout=stdout, stderr=None)
                    opened_errors = []

                    def spawn(*args, **kwargs):
                        opened_errors.append(kwargs["stderr"])
                        return child

                    try:
                        with patch.object(perf_baseline.subprocess, "Popen", side_effect=spawn), patch.object(
                                perf_baseline.Lines, "read", side_effect=TimeoutError("read witness")), patch.object(
                                perf_baseline, "stop", side_effect=OSError("cleanup witness") if failure == "stop" else None):
                            with self.assertRaisesRegex(OSError, "cleanup witness"):
                                if entry == "native":
                                    perf_baseline.native(Path(sys.executable), "observe", 1, root / "stderr.txt")
                                else:
                                    perf_baseline.capture_trace(Path(sys.executable), "observe", 1, root)
                        self.assertEqual(len(opened_errors), 1)
                        self.assertEqual([stdout.closed, opened_errors[0].closed], [True, True])
                    finally:
                        stdout.close()
                        for errors in opened_errors:
                            errors.close()

    def test_native_and_trace_close_stderr_after_spawn_failure(self):
        for entry in ("native", "trace"):
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                opened_errors = []

                def spawn(*args, **kwargs):
                    opened_errors.append(kwargs["stderr"])
                    raise OSError("spawn witness")

                try:
                    with patch.object(perf_baseline.subprocess, "Popen", side_effect=spawn):
                        with self.assertRaisesRegex(OSError, "spawn witness"):
                            if entry == "native":
                                perf_baseline.native(Path(sys.executable), "observe", 1, root / "stderr.txt")
                            else:
                                perf_baseline.capture_trace(Path(sys.executable), "observe", 1, root)
                    self.assertEqual(len(opened_errors), 1)
                    self.assertTrue(opened_errors[0].closed)
                finally:
                    for errors in opened_errors:
                        errors.close()

    def test_host_cleanup_survives_client_stop_or_pipe_failure(self):
        for failure in ("stop", "pipe"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                host = Mock(stdin=None, stdout=None, stderr=io.BytesIO(b"fatal witness\n"))
                client = Mock(stdin=Mock(), stdout=Mock(), stderr=None)
                host.poll.return_value = 0
                if failure == "pipe":
                    client.stdin.close.side_effect = OSError("client cleanup witness")

                def stop(process):
                    if failure == "stop" and process is client:
                        raise OSError("client cleanup witness")

                with patch.object(perf_baseline.subprocess, "Popen", side_effect=[host, client]), patch.object(
                        perf_baseline.Lines, "read", side_effect=[b"Listening on ws://127.0.0.1:1/slither",
                                                                 TimeoutError("request witness")]), patch.object(
                        perf_baseline, "stop", side_effect=stop) as stopped:
                    with self.assertRaisesRegex(OSError, "client cleanup witness"):
                        perf_baseline.e2e(Path(sys.executable), 1, True, Path(temporary))
                self.assertEqual(stopped.call_args_list[-1].args, (host,))
                client.stdout.close.assert_called_once()
                self.assertTrue(host.stderr.closed)
                self.assertIn("fatal witness", (Path(temporary) / "host-stderr.txt").read_text())

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

    def test_trace_archive_preserves_child_bytes_and_retains_failures(self):
        # Synthetic child output, not simulator traces: spans several pipe reads and has UTF-8.
        emitted = b"".join(json.dumps({"tick": i, "state": "x" * (i % 97), "note": "\u00e9"},
                                      ensure_ascii=False).encode() + b"\n" for i in range(3000))
        line_end = emitted.index(b"\n") + 1
        children = {"valid": ("data", 0), "failed": (f"data[:{line_end}]", 3),
                    "incomplete": (f"data[:{line_end + 5}]", 0)}
        for name, (written, code) in children.items():
            with self.subTest(child=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "trace.bin").write_bytes(emitted)
                script = root / f"{name}.py"
                script.write_text("from pathlib import Path\nimport sys\n"
                                  "data = (Path(__file__).parent / 'trace.bin').read_bytes()\n"
                                  "sys.stderr.write('failure witness')\n"
                                  f"sys.stdout.buffer.write({written})\nsys.exit({code})\n")
                if name != "valid":
                    with self.assertRaisesRegex(RuntimeError, "invalid/incomplete"):
                        perf_baseline.capture_trace(sys.executable, str(script), 1, root)
                    self.assertEqual(Path(f"{script}-trace-stderr.txt").read_text(), "failure witness")
                    continue
                record = perf_baseline.capture_trace(sys.executable, str(script), 1, root)
                archive = Path(f"{script}.jsonl.gz").read_bytes()
                self.assertEqual(gzip.decompress(archive), emitted)
                self.assertEqual(archive[4:8], bytes(4))
                self.assertEqual(record, {"case": str(script), "iterations": 1, "lines": 3000,
                                          "uncompressed_bytes": len(emitted),
                                          "sha256": hashlib.sha256(emitted).hexdigest()})

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
