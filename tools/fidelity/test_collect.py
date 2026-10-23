import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

import collect
import evidence


class CollectTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        source = evidence.ROOT / "evidence/templates/movement-plan.json"
        self.study = evidence.create_study(source, self.directory / "study")

    @patch("collect.shutil.which", return_value="node")
    def test_dry_run_does_not_create_an_attempt_or_a_process(self, _which):
        with patch.object(sys, "argv", ["collect.py", str(self.study), "--dry-run"]), patch("collect.subprocess.Popen") as process:
            with contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(collect.main(), 0)
        process.assert_not_called()
        reply = json.loads(output.getvalue())
        self.assertTrue(reply["dryRun"])
        self.assertIn("127.0.0.1:8766", reply["command"])
        self.assertEqual((self.study / "attempts.jsonl").read_bytes(), b"")
        self.assertEqual(list((self.study / "raw").iterdir()), [])

    @patch("collect.shutil.which", return_value="node")
    def test_origin_is_explicit_and_forwarded_from_the_frozen_plan(self, _which):
        self.assertNotIn("--origin", collect.capture_command(self.study, self.directory / "unused"))
        plan = evidence.read_plan(self.study)
        plan["capture"]["origin"] = "http://localhost:8080"
        source = self.directory / "origin-plan.json"
        source.write_text(json.dumps(plan), encoding="utf-8")
        study = evidence.create_study(source, self.directory / "with-origin")
        command = collect.capture_command(study, self.directory / "unused")
        self.assertEqual(command[-2:], ["--origin", "http://localhost:8080"])

    @patch("collect.shutil.which", return_value="node")
    def test_paired_routes_use_the_paired_driver_and_declared_controls(self, _which):
        for mode, direction in (("overlap", None), ("crossing", None),
                                ("crossing", "x"), ("crossing", "y"),
                                ("crossing", "diagonal-positive"), ("crossing", "diagonal-negative")):
            with self.subTest(mode=mode, direction=direction):
                plan = evidence.read_plan(self.study)
                plan["capture"].update(scenario="paired-prey", mode=mode, bots=2, seconds=480)
                if direction:
                    plan["capture"]["direction"] = direction
                source = self.directory / "pair-plan.json"
                source.write_text(json.dumps(plan), encoding="utf-8")
                study = evidence.create_study(source, self.directory / f"pair-{mode}-{direction}")
                command = collect.capture_command(study, self.directory / "unused")
                self.assertEqual(Path(command[1]).name, "slither-paired-prey.mjs")
                self.assertEqual(command[command.index("--mode") + 1], mode)
                self.assertNotIn("--bots", command)
                self.assertNotIn("--scenario", command)
                self.assertEqual("--direction" in command, direction is not None)
                if direction:
                    self.assertEqual(command[command.index("--direction") + 1], direction)

    @patch("collect.shutil.which", return_value="node")
    def test_paired_snake_dispatches_the_explicit_direction_without_headless_flags(self, _which):
        for direction in ("x", "y", "diagonal-positive", "diagonal-negative"):
            with self.subTest(direction=direction):
                plan = evidence.read_plan(self.study)
                plan["capture"].update(scenario="paired-snake", bots=2, seconds=1800,
                                       mode="crossing", direction=direction)
                source = self.directory / "snake-plan.json"
                source.write_text(json.dumps(plan), encoding="utf-8")
                study = evidence.create_study(source, self.directory / f"snake-{direction}")
                command = collect.capture_command(study, self.directory / "unused")
                self.assertEqual(Path(command[1]).name, "slither-paired-snake.mjs")
                self.assertEqual(command[command.index("--seconds") + 1], "1800")
                self.assertEqual(command[command.index("--direction") + 1], direction)
                self.assertEqual(command[command.index("--mode") + 1], "crossing")
                self.assertNotIn("--bots", command)
                self.assertNotIn("--scenario", command)

    @patch("collect.shutil.which", return_value="node")
    def test_food_lanes_dispatches_the_fixed_single_client_driver(self, _which):
        plan = evidence.read_plan(self.study)
        plan["capture"].update(scenario="food-lanes", bots=1, seconds=480,
                               origin="http://localhost:8080")
        source = self.directory / "food-lanes-plan.json"
        source.write_text(json.dumps(plan), encoding="utf-8")
        study = evidence.create_study(source, self.directory / "food-lanes")
        command = collect.capture_command(study, self.directory / "unused")
        self.assertEqual(Path(command[1]).name, "slither-food-lanes.mjs")
        self.assertEqual(command[command.index("--seconds") + 1], "480")
        self.assertEqual(command[command.index("--origin") + 1], "http://localhost:8080")
        self.assertNotIn("--bots", command)
        self.assertNotIn("--scenario", command)
        self.assertNotIn("--mode", command)
        self.assertEqual((study / "attempts.jsonl").read_bytes(), b"")

    @patch("collect.shutil.which", return_value="node")
    def test_audit_failure_keeps_finished_exit_code_and_consumes_attempt(self, _which):
        process = Mock()
        process.wait.return_value = process.returncode = 0
        process.poll.return_value = 0
        with patch("collect.subprocess.Popen", return_value=process), \
                patch("collect.evidence.audit_study", side_effect=ValueError("synthetic audit failure")):
            with self.assertRaisesRegex(ValueError, "synthetic audit failure"):
                collect.collect(self.study)
        starts, ends = evidence.read_attempts(self.study)
        self.assertEqual(len(starts), 1)
        self.assertEqual(len(ends), 1)
        self.assertEqual(next(iter(ends.values()))["exitCode"], 0)
        with patch("collect.subprocess.Popen") as launch:
            with self.assertRaisesRegex(ValueError, "no automatic replacements"):
                collect.collect(self.study)
        launch.assert_not_called()

    @patch("collect.shutil.which", return_value="node")
    def test_start_failure_is_recorded_and_never_replaced(self, _which):
        with patch("collect.subprocess.Popen", side_effect=OSError("start failed")):
            reply = collect.collect(self.study)
        self.assertEqual(reply["status"], "failed")
        self.assertEqual(reply["audit"]["status"], "incomplete")
        starts, ends = evidence.read_attempts(self.study)
        self.assertEqual(len(starts), 1)
        self.assertEqual(len(ends), 1)
        self.assertIn("start failed", next(iter(ends.values()))["error"])
        with self.assertRaisesRegex(ValueError, "no automatic replacements"):
            collect.collect(self.study)

    @patch("collect.shutil.which", return_value="node")
    def test_timeout_stops_only_the_created_process_and_keeps_attempt(self, _which):
        process = Mock()
        process.wait.side_effect = [subprocess.TimeoutExpired("node", 90), -15]
        process.poll.return_value = None
        process.returncode = -15
        with patch("collect.subprocess.Popen", return_value=process):
            reply = collect.collect(self.study)
        process.terminate.assert_called_once()
        self.assertEqual(reply["status"], "failed")
        self.assertEqual(reply["exitCode"], -15)
        self.assertEqual(len(evidence.read_attempts(self.study)[0]), 1)

    @patch("collect.shutil.which", return_value="node")
    def test_interruption_is_not_relabelled_completion(self, _which):
        process = Mock()
        process.wait.side_effect = [KeyboardInterrupt, -15]
        process.poll.return_value = None
        process.returncode = -15
        with patch("collect.subprocess.Popen", return_value=process):
            reply = collect.collect(self.study)
        self.assertEqual(reply["status"], "interrupted")
        self.assertEqual(reply["audit"]["status"], "incomplete")

    def test_modified_plan_is_rejected_before_starting(self):
        with (self.study / "plan.json").open("a") as stream:
            stream.write("\n")
        with patch("collect.subprocess.Popen") as process:
            with self.assertRaisesRegex(ValueError, "frozen plan"):
                collect.collect(self.study)
        process.assert_not_called()
        self.assertEqual((self.study / "attempts.jsonl").read_bytes(), b"")


if __name__ == "__main__":
    unittest.main()
