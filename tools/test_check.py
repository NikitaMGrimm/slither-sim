"""Check selection and failure handling without invoking compilers or child suites."""

from contextlib import redirect_stdout
import fnmatch
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import check


class CheckRunnerTests(unittest.TestCase):
    def setUp(self):
        # Keep fake failures out of the real job summary when CI runs these tests.
        environment = patch.dict(os.environ)
        environment.start()
        self.addCleanup(environment.stop)
        os.environ.pop("GITHUB_STEP_SUMMARY", None)

    def test_all_is_host_only_and_explicit_groups_are_deduplicated(self):
        self.assertEqual(check.select_groups(["tools", "all", "wasm", "tools"]),
                         ["tools", "rust", "package", "wasm"])
        self.assertNotIn("wasm", check.select_groups(["all"]))

    def test_list_never_starts_children(self):
        with patch.object(check.subprocess, "run") as run, redirect_stdout(io.StringIO()) as output:
            self.assertEqual(check.main(["rust", "--list"]), 0)
        run.assert_not_called()
        self.assertIn("--no-default-features", output.getvalue())

    def test_default_lists_every_host_group_without_optional_wasm(self):
        with patch.object(check.subprocess, "run") as run, redirect_stdout(io.StringIO()) as output:
            self.assertEqual(check.main(["--list"]), 0)
        run.assert_not_called()
        for group in check.HOST_GROUPS:
            self.assertIn(f"[{group}]", output.getvalue())
        self.assertNotIn("[wasm]", output.getvalue())

    def test_failure_stops_before_later_commands(self):
        with patch.object(check.subprocess, "run", return_value=subprocess.CompletedProcess([], 7)) as run:
            with redirect_stdout(io.StringIO()):
                self.assertEqual(check.main(["rust", "tools"]), 7)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.kwargs["cwd"], check.ROOT)

    def test_rust_is_lint_then_test(self):
        self.assertEqual(check.commands("rust"), check.commands("lint") + check.commands("test"))
        lint = check.commands("lint")
        for features in ([], ["--features", "networking"], ["--features", "parallel"]):
            self.assertIn(check.clippy("--no-default-features", *features), lint)
        # Feature checks build the root package alone, never with the whole workspace.
        self.assertTrue(all("--workspace" not in command for command in lint if "--no-default-features" in command))

    def test_job_summary_records_each_command_until_the_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / "summary.md"
            results = [subprocess.CompletedProcess([], 0), subprocess.CompletedProcess([], 3)]
            with patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}), \
                    patch.object(check.subprocess, "run", side_effect=results), \
                    redirect_stdout(io.StringIO()) as output:
                self.assertEqual(check.main(["lint"]), 3)
            rows = summary.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(rows), 2 + 2 + 1)
        self.assertRegex(rows[2], r"^\| lint \| `cargo fmt --all --check` \| \d+\.\d \| ok \|$")
        self.assertTrue(rows[3].endswith("| exit 3 |"))
        self.assertIn("[lint] ", output.getvalue())

    def test_no_summary_outside_ci_or_when_listing(self):
        with tempfile.TemporaryDirectory() as directory:
            summary = Path(directory) / "summary.md"
            with patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary)}), \
                    redirect_stdout(io.StringIO()):
                self.assertEqual(check.main(["rust", "--list"]), 0)
            self.assertFalse(summary.exists())

    def test_steps_keep_every_command_in_order(self):
        for group in check.GROUPS:
            self.assertEqual([command for step in check.steps(group) for command in step], check.commands(group))
        concurrent = [step for step in check.steps("tools") if len(step) > 1]
        self.assertEqual([[command[3] if command[0] != "node" else "node" for command in step] for step in concurrent],
                         [["discover", "discover", "discover", "node"]])

    def test_fidelity_halves_partition_the_suite(self):
        names = sorted(path.name for path in (check.ROOT / "tools/fidelity").glob("test_*.py"))
        halves = [sorted(name for name in names if fnmatch.fnmatch(name, pattern)) for pattern in check.FIDELITY_HALVES]
        self.assertTrue(all(halves))
        self.assertEqual(sorted(halves[0] + halves[1]), names)
        self.assertFalse(set(halves[0]) & set(halves[1]))

    def test_concurrent_output_is_replayed_in_command_order(self):
        slow = [sys.executable, "-c", "import time; time.sleep(0.3); print('first')"]
        fast = [sys.executable, "-c", "import sys; print('second'); sys.exit(3)"]
        buffer = io.BytesIO()
        stdout = io.TextIOWrapper(buffer, encoding="utf-8")
        with redirect_stdout(stdout):
            results = check.run_concurrently([slow, fast], os.environ.copy())
        stdout.flush()
        self.assertEqual(buffer.getvalue().decode().split(), ["first", "second"])
        self.assertEqual([code for _, code in results], [0, 3])
        self.assertGreaterEqual(results[0][0], 0.3)

    def test_a_hung_concurrent_command_is_stopped_and_its_output_shown(self):
        hung = [sys.executable, "-c", "import time; print('started', flush=True); time.sleep(30)"]
        quick = [sys.executable, "-c", "print('done')"]
        buffer = io.BytesIO()
        stdout = io.TextIOWrapper(buffer, encoding="utf-8")
        with redirect_stdout(stdout), patch.object(check.sys, "stderr", io.StringIO()):
            results = check.run_concurrently([hung, quick], os.environ.copy(), timeout=1)
        stdout.flush()
        self.assertEqual(buffer.getvalue().decode().split(), ["started", "done"])
        self.assertEqual([code for _, code in results], [check.TIMED_OUT, 0])
        self.assertLess(results[0][0], 10)

    def test_a_failing_concurrent_command_stops_later_steps(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "later"
            ok = [sys.executable, "-c", "pass"]
            failing = [sys.executable, "-c", "import sys; sys.exit(5)"]
            later = [sys.executable, "-c", f"open({str(marker)!r}, 'w').close()"]
            with patch.object(check, "steps", return_value=[[ok, failing], [later]]), \
                    redirect_stdout(io.TextIOWrapper(io.BytesIO(), encoding="utf-8")):
                self.assertEqual(check.main(["tools"]), 5)
            self.assertFalse(marker.exists())

    def test_a_command_that_cannot_start_is_named_and_stops_its_step(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / "finished"
            slow = [sys.executable, "-c", f"import time; time.sleep(0.5); open({str(marker)!r}, 'w').close()"]
            missing = ["slither-sim-no-such-command"]
            errors = io.StringIO()
            with patch.object(check, "steps", return_value=[[slow, missing]]),                     redirect_stdout(io.TextIOWrapper(io.BytesIO(), encoding="utf-8")),                     patch.object(check.sys, "stderr", errors):
                self.assertEqual(check.main(["tools"]), 1)
            time.sleep(1)  # A surviving child would have written the marker by now.
            self.assertFalse(marker.exists())
        self.assertIn("Cannot run slither-sim-no-such-command", errors.getvalue())

    def test_tool_children_use_the_running_python_and_expanded_node_paths(self):
        commands = check.commands("tools")
        self.assertEqual(commands[0][:2], ["cargo", "build"])
        self.assertEqual(commands[1][0], check.sys.executable)
        node = next(command for command in commands if command[:2] == ["node", "--test"])
        self.assertGreater(len(node), 2)
        self.assertTrue(all("*" not in path for path in node))

    def test_smoke_uses_the_same_cargo_target_directory(self):
        with patch.dict(os.environ, {"CARGO_TARGET_DIR": "some target"}):
            commands = check.commands("tools")
        expected = check.ROOT / "some target" / "debug"
        for script, option in (("examples/bot.py", "--binary"), ("examples/ffi_bot.py", "--library")):
            command = next(command for command in commands if script in command)
            self.assertEqual(Path(command[command.index(option) + 1]).parent, expected)
        wheels = check.ROOT / "some target" / "wheels"
        build = next(command for command in commands if "maturin" in command)
        self.assertEqual(Path(build[build.index("--out") + 1]), wheels)
        test = next(command for command in commands if "python/tests/run.py" in command)
        self.assertEqual(Path(test[-1]), wheels)


if __name__ == "__main__":
    unittest.main()
