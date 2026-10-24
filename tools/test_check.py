"""Check selection and failure handling without invoking compilers or child suites."""

from contextlib import redirect_stdout
import io
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import check


class CheckRunnerTests(unittest.TestCase):
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

    def test_tool_children_use_the_running_python_and_expanded_node_paths(self):
        commands = check.commands("tools")
        self.assertEqual(commands[0][0], check.sys.executable)
        node = commands[2]
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
