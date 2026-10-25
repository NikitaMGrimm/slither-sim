"""Synthetic checkpoint immutability, upload handshake and untrusted archive checks."""

import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch
import zipfile

import pr_report
import pr_report_archive
import pr_report_progress
import perf_build


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
            (output / "done").unlink()
            control = pr_report_progress.control_directory(output)
            control.mkdir()
            (control / "quiesced").touch()  # Collector can fail before writing done.
            with patch.object(pr_report_progress.time, "sleep", side_effect=AssertionError("waited after collector exit")):
                self.assertFalse(pr_report_progress.wait(output, 3, time.monotonic() + 1))

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

    def test_upload_steps_require_always_cleanup_and_positive_quiescence(self):
        workflow = (pr_report.ROOT / ".github/workflows/pr-report.yml").read_text()
        cleanup = workflow.index("name: Stop and reap collector before uploads")
        terminal = workflow.index("name: Upload comment data")
        full = workflow.index("name: Upload results")
        self.assertLess(cleanup, terminal)
        self.assertLess(terminal, full)
        self.assertIn("if: always()", workflow[cleanup:terminal])
        self.assertIn("if: success() && steps.cleanup.outputs.safe_to_upload == 'true'", workflow[terminal:full])
        self.assertIn("if: always() && steps.cleanup.outputs.safe_to_upload == 'true'", workflow[full:])

    def test_strict_windows_job_query_termination_and_timeout_errors_fail_closed(self):
        job = object.__new__(perf_build.Job)
        job.handle = 123
        job.process_handles = Mock(return_value=[])
        job.accounting = lambda: SimpleNamespace(ActiveProcesses=1)
        job.ctypes = SimpleNamespace(byref=lambda x: x, sizeof=lambda x: 1,
                                    get_last_error=lambda: 5, WinError=lambda n: OSError(n, "synthetic Windows job API failure"))
        job.kernel = SimpleNamespace(QueryInformationJobObject=Mock(return_value=0),
                                     TerminateJobObject=Mock(return_value=1))
        with self.assertRaises(OSError):
            job.active(verify=True)
        job.kernel.TerminateJobObject.return_value = 0
        with self.assertRaises(OSError):
            job.terminate(verify=True)
        job.kernel.TerminateJobObject.return_value = 1
        job.kernel.QueryInformationJobObject.return_value = 1
        with patch.object(perf_build.time, "perf_counter", side_effect=[0, 1]), self.assertRaises(TimeoutError):
            job.terminate(wait_seconds=0.5, verify=True)
        job.process_handles.return_value = [456]
        job.kernel.WaitForSingleObject = Mock(return_value=258)
        job.kernel.CloseHandle = Mock()
        with self.assertRaises(TimeoutError):
            job.terminate(verify=True)
        job.kernel.CloseHandle.assert_called_once_with(456)
        job.kernel.WaitForSingleObject.return_value = 0xFFFFFFFF
        with self.assertRaises(OSError):
            job.terminate(verify=True)
        self.assertEqual(job.kernel.CloseHandle.call_count, 2)
        job.close = Mock()
        job.terminate = Mock(side_effect=OSError("strict termination failed"))
        tree = object.__new__(perf_build.ProcessTree)
        tree.job, tree.process = job, Mock()
        with self.assertRaises(OSError):
            tree.stop(verify=True)
        job.close.assert_called_once()
        tree.process.wait.assert_called_once_with(timeout=30)

    def test_cleanup_error_or_missing_supervisor_withholds_acknowledgment(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name) / "report"
            control = pr_report_progress.control_directory(output)
            control.mkdir()
            self.assertFalse(pr_report_progress.cleanup(output, timeout=0))
            tree = Mock()
            tree.process.poll.return_value = 0
            tree.stop.side_effect = OSError("synthetic quiescence query failed")
            with patch.object(pr_report_progress, "ProcessTree", return_value=tree):
                code = pr_report_progress.supervise(output, ["synthetic"], time.monotonic() + 1)
            self.assertEqual(code, 1)
            self.assertFalse((control / "quiesced").exists())
            self.assertTrue((control / "cleanup-failure.json").is_file())
            self.assertFalse(pr_report_progress.cleanup(output))


class Supervision(unittest.TestCase):
    def exercise(self, after_spawn, *, cancel_waiter=False):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name) / "report"
            output.mkdir()
            control = pr_report_progress.control_directory(output)
            control.mkdir()
            beat, ready = output / "heartbeat", output / "child-ready"
            grandchild = ("import pathlib,sys,time\n"
                          "beat,ready=map(pathlib.Path,sys.argv[1:])\n"
                          "ready.touch()\n"
                          "deadline=time.monotonic()+15\n"
                          "while time.monotonic()<deadline:\n"
                          "    with beat.open('a') as stream: stream.write('.')\n"
                          "    time.sleep(.02)\n")
            collector = ("import pathlib,subprocess,sys,time\n"
                         f"ready=pathlib.Path({str(ready)!r})\n"
                         f"subprocess.Popen([sys.executable,'-c',{grandchild!r},{str(beat)!r},str(ready)])\n"
                         "while not ready.exists(): time.sleep(.01)\n" + after_spawn)
            result = []
            thread = threading.Thread(target=lambda: result.append(pr_report_progress.supervise(
                output, [sys.executable, "-c", collector], time.monotonic() + 20)))
            thread.start()
            try:
                deadline = time.monotonic() + 10
                while not beat.exists() and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertTrue(beat.exists(), "synthetic descendant never started")
                if cancel_waiter:
                    environment = {**os.environ, "PR_REPORT_JOB_START": str(time.monotonic()),
                                   "GITHUB_OUTPUT": str(output / "wait-output")}
                    waiter = subprocess.Popen([sys.executable, str(Path(pr_report_progress.__file__)), "wait",
                                               "--output", str(output), "--sequence", "2"], env=environment,
                                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                    try:
                        time.sleep(.1)
                        self.assertIsNone(waiter.poll())
                        waiter.terminate()  # Current runner step dies; supervisor survives.
                        waiter.wait(timeout=5)
                    finally:
                        if waiter.poll() is None:
                            waiter.kill()
                            waiter.wait(timeout=5)
                else:
                    thread.join(timeout=10)
                self.assertTrue(pr_report_progress.cleanup(output, timeout=10))
                thread.join(timeout=5)
                self.assertFalse(thread.is_alive())
                self.assertEqual(result, [0])
                record = json.loads((control / "result.json").read_text())
                size = beat.stat().st_size
                time.sleep(.2)
                self.assertEqual(beat.stat().st_size, size, "descendant writes continued after upload acknowledgment")
                log = output.with_suffix(".collector.log")
                renamed = log.with_suffix(".closed")
                log.rename(renamed)  # Windows refuses this while the output handle is open.
                if record["stopped"] or record["returncode"]:
                    (output / "report.json").write_text(json.dumps({"snapshot": {"state": "complete"}}))
                    completed = subprocess.run([sys.executable, str(Path(pr_report_progress.__file__)), "finish",
                                                "--output", str(output)], capture_output=True, text=True,
                                               env={**os.environ, "PR_REPORT_JOB_START": str(time.monotonic())}, timeout=5)
                    self.assertNotEqual(completed.returncode, 0)
                    self.assertIn("Collector failed or was stopped", completed.stderr)
                self.assertTrue(pr_report_progress.cleanup(output, timeout=1), "cleanup must be idempotent")
                return record
            finally:
                pr_report_progress.cleanup(output, timeout=5)
                thread.join(timeout=20)

    def test_cancelled_waiter_cleanup_reaps_collector_and_active_descendant(self):
        record = self.exercise("time.sleep(15)\n", cancel_waiter=True)
        self.assertTrue(record["stopped"])

    def test_normal_collector_exit_still_reaps_lingering_descendant(self):
        record = self.exercise("sys.exit(0)\n")
        self.assertEqual(record, {"returncode": 0, "stopped": False})

    def test_clean_cleanup_does_not_promote_failed_collector_to_success(self):
        record = self.exercise("sys.exit(7)\n")
        self.assertEqual(record, {"returncode": 7, "stopped": False})

    @unittest.skipUnless(os.name == "nt", "production Windows kill-on-close ownership")
    def test_killed_supervisor_leaves_no_upload_acknowledgment(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name) / "report"
            output.mkdir()
            control = pr_report_progress.control_directory(output)
            control.mkdir()
            beat = output / "heartbeat"
            writer = (f"import time\nfrom pathlib import Path\np=Path({str(beat)!r})\n"
                      "while True:\n    with p.open('a') as stream: stream.write('.')\n    time.sleep(.02)\n")
            script = ("import sys,time\n"
                      f"if sys.stdin.buffer.read(1) != {perf_build.RELEASE!r}: sys.exit(1)\n"
                      f"from pathlib import Path\nsys.path.insert(0,{str(Path(pr_report_progress.__file__).parent)!r})\n"
                      "from pr_report_progress import supervise\n"
                      f"sys.exit(supervise(Path({str(output)!r}),[{sys.executable!r},'-c',{writer!r}],float('inf')))\n")
            # Kill the actual supervisor, not a venv redirector. A fixture-owned job
            # contains failure paths, but must not stop members before the assertions.
            job = perf_build.Job()
            members = []
            supervisor = None
            try:
                supervisor = subprocess.Popen([sys._base_executable, "-c", script], stdin=subprocess.PIPE,
                                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                              creationflags=subprocess.CREATE_NO_WINDOW)
                job.assign(supervisor)
                supervisor.stdin.write(perf_build.RELEASE)
                supervisor.stdin.close()
                # Startup is separate from the killed-supervisor/cleanup checks.
                deadline = time.monotonic() + 30
                while (not beat.exists() or not beat.stat().st_size) and time.monotonic() < deadline:
                    time.sleep(.01)
                self.assertTrue(beat.exists() and beat.stat().st_size, "synthetic writer never started")
                members = job.process_handles()
                supervisor.kill()
                supervisor.wait(timeout=30)
                deadline = time.monotonic() + 30
                for handle in members:
                    status = job.kernel.WaitForSingleObject(handle, max(0, int((deadline - time.monotonic()) * 1000)))
                    self.assertEqual(status, 0, "job member survived the killed supervisor")
                size = beat.stat().st_size
                time.sleep(.2)
                self.assertEqual(beat.stat().st_size, size)
                log = output.with_suffix(".collector.log")
                log.rename(log.with_suffix(".closed"))
                self.assertFalse((control / "quiesced").exists())
                self.assertFalse(pr_report_progress.cleanup(output, timeout=.05))
            finally:
                try:
                    if supervisor is not None and supervisor.poll() is None:
                        supervisor.kill()
                    job.terminate(verify=True)
                finally:
                    job.close()
                    for handle in members:
                        job.kernel.CloseHandle(handle)
                    if supervisor is not None:
                        supervisor.stdin.close()
                        supervisor.wait(timeout=30)


if __name__ == "__main__":
    unittest.main()
