"""The A/A calibration pilot's schedule, records and per-job estimates, with scripted children."""

import contextlib
import io
import itertools
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import perf_baseline
import perf_calibration
import pr_report
from test_perf_baseline import catalog, row


class Calibration(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.now = 0.0
        self.calls = []

    def run_pilot(self, names, seconds, deadline=1e9, failure=None, via_main=False):
        """seconds(case, m, pair_index, block) -> one block's wall time."""
        workloads = catalog([row(name, iterations=100) for name in names]).select(suite="ci")
        output = self.directory / f"out-{len(list(self.directory.iterdir()))}"
        if not via_main:
            output.mkdir()

        def native(executable, case, iterations, stderr_path, profile=False, *, deadline=None, blocks=None):
            index = stderr_path.name.split("-")[-3]
            index = None if index == "warmup" else int(index)
            self.calls.append((Path(executable).name, case, blocks, index))
            self.now += 1
            if failure:
                error = failure(case, blocks, index, stderr_path.name.split("-")[-2])
                if error:
                    raise error
            if self.now > deadline:
                self.now = deadline
                raise TimeoutError("performance run deadline exceeded during native child")
            timings = [{"wallSeconds": seconds(case, blocks, index, k), "operations": iterations}
                       for k in range(blocks)]
            return {"process_wall_seconds": 0.5 * blocks + 0.25, "cpu_seconds": None,
                    "workload": {"kind": "report", "case": case, "iterations": iterations, "mode": "timing",
                                 "timing": None, "blocks": timings}}

        with patch.object(perf_calibration, "native", side_effect=native), \
                patch.object(perf_calibration, "time", SimpleNamespace(perf_counter=lambda: self.now,
                                                                     monotonic=lambda: self.now)), \
                contextlib.redirect_stdout(io.StringIO()):
            if via_main:
                binary = self.directory / "perf_baseline"
                binary.write_bytes(b"synthetic executable")
                with patch.object(sys, "argv", ["perf_calibration", "--binary", str(binary), "--group", "A",
                                                "--output", str(output)]), \
                        patch.dict(os.environ, {"PR_REPORT_JOB_START": "0"}), \
                        patch.object(perf_calibration, "load_catalog", return_value=catalog(
                            [row(name, iterations=100) for name in names])), \
                        patch.object(perf_calibration, "GROUPS", {"A": names}), \
                        patch.object(perf_calibration, "STAGE_CAP_SECONDS", deadline), \
                        patch.object(perf_calibration.subprocess, "check_output", return_value="fixture rustc"):
                    self.assertEqual(perf_calibration.main(), 0)
                data = json.loads((output / "summary.json").read_text())
            else:
                data, _ = perf_calibration.calibrate(self.directory / "perf_baseline", workloads, output, deadline)
        return data, output

    def test_failed_warmup_invalidates_only_its_cell_and_is_not_retried(self):
        for label in ("baseline", "candidate"):
            with self.subTest(label=label):
                self.calls.clear()
                self.now = 0

                def failure(case, m, index, side):
                    if (case, m, index, side) == ("first", 2, None, label):
                        return RuntimeError("warmup failure witness")

                summary, output = self.run_pilot(["first", "healthy"], lambda *args: 1.0,
                                                 failure=failure, via_main=True)
                cell = summary["workloads"]["first"]["cells"][1]
                self.assertEqual((cell["status"], cell["pairs"]), ("failed", 0))
                self.assertIn("warmup", cell["failure"])
                self.assertTrue(all(key not in cell for key in ("paired_residual_variance", "v_w", "v_b", "Q")))
                self.assertEqual(summary["workloads"]["first"]["pairs"]["2"], [])
                attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
                affected = [a for a in attempts if a["case"] == "first" and a["blocks"] == 2]
                self.assertEqual(len(affected), 1 if label == "baseline" else 2)
                self.assertTrue(all(a["warmup"] for a in affected))
                self.assertEqual((affected[-1]["label"], affected[-1]["status"]), (label, "failed"))
                self.assertIn("warmup failure witness", affected[-1]["error"])
                for name, workload in summary["workloads"].items():
                    for other in workload["cells"]:
                        if (name, other["blocks"]) != ("first", 2):
                            self.assertEqual((other["status"], other["pairs"]), ("complete", 6))
                            self.assertIn("Q", other)
                self.assertAlmostEqual(summary["workloads"]["first"]["cost_model"]["h"], .5)
                self.assertAlmostEqual(summary["workloads"]["first"]["cost_model"]["c"], 1)

    def test_invalid_warmup_record_does_not_admit_counted_pairs(self):
        summary, output = self.run_pilot(["first", "healthy"],
                                         lambda case, m, index, k: float("nan")
                                         if (case, m, index) == ("first", 2, None) else 1.0, via_main=True)
        cell = summary["workloads"]["first"]["cells"][1]
        self.assertEqual((cell["status"], cell["pairs"]), ("failed", 0))
        self.assertNotIn("Q", cell)
        self.assertEqual([c["pairs"] for c in summary["workloads"]["healthy"]["cells"]], [6, 6, 6])
        attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
        affected = [a for a in attempts if a["case"] == "first" and a["blocks"] == 2]
        self.assertEqual(len(affected), 1)
        self.assertEqual(affected[0]["status"], "failed")

    def test_exit_wait_timeout_writes_summary_and_retains_completed_pairs(self):
        for capped in (False, True):
            with self.subTest(capped=capped):
                self.calls.clear()
                self.now = 0

                def failure(case, m, index, label):
                    if (case, m, index, label) != ("first", 1, 4, "baseline"):
                        return None
                    if capped:
                        self.now = 500
                    child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
                    child.poll.return_value = 0
                    child.wait.side_effect = subprocess.TimeoutExpired("fixture", 120)
                    with patch.object(perf_baseline.subprocess, "Popen", return_value=child), \
                            patch.object(perf_baseline.Lines, "read", return_value=b"{}"), \
                            patch.object(perf_baseline, "time", SimpleNamespace(perf_counter=lambda: 0)):
                        try:
                            perf_baseline.native("fixture", case, 100, self.directory / "stderr.txt", deadline=120)
                        except TimeoutError as error:
                            return error

                summary, output = self.run_pilot(["first", "healthy"], lambda *args: 1.0,
                                                 deadline=500, failure=failure, via_main=True)
                cell = summary["workloads"]["first"]["cells"][0]
                self.assertEqual(cell["pairs"], 3)
                self.assertEqual(cell["status"], "incomplete" if capped else "failed")
                self.assertNotIn("Q", cell)
                self.assertEqual(len(summary["workloads"]["first"]["pairs"]["1"]), 3)
                self.assertEqual([c["pairs"] for c in summary["workloads"]["healthy"]["cells"]],
                                 [3, 3, 3] if capped else [6, 6, 6])
                attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
                last = next(a for a in attempts if a["case"] == "first" and a["blocks"] == 1
                            and a["pair_index"] == 4)
                self.assertEqual(last["status"], "incomplete_due_to_cap" if capped else "failed")

    def test_schedule_is_same_binary_warmed_and_balanced(self):
        data, output = self.run_pilot(["first", "second"], lambda case, m, index, k: 1.0)
        self.assertEqual({call[0] for call in self.calls}, {"perf_baseline"})
        warmups = [(case, m) for _, case, m, index in self.calls[::2] if index is None]
        self.assertEqual(warmups, [(case, m) for case in ("first", "second") for m in perf_calibration.BLOCKS])
        counted = [(case, m, index) for _, case, m, index in self.calls[::2] if index is not None]
        self.assertEqual(counted, [(case, m, index) for index, ms in enumerate(perf_calibration.BLOCK_ORDER, 1)
                                   for case in ("first", "second") for m in ms])
        self.assertEqual(sorted(perf_calibration.BLOCK_ORDER), sorted(itertools.permutations(perf_calibration.BLOCKS)))
        for position in range(3):
            self.assertEqual(sorted(order[position] for order in perf_calibration.BLOCK_ORDER), [1, 1, 2, 2, 4, 4])
        self.assertEqual(perf_calibration.ORDER, (1, -1, -1, 1, 1, -1))
        self.assertEqual([len(data["first"][m]) for m in perf_calibration.BLOCKS], [6, 6, 6])
        attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
        self.assertEqual(len(attempts), len(self.calls))
        self.assertTrue(all(a["status"] == "ok" for a in attempts))

    def test_deadline_leaves_incomplete_cells(self):
        data, output = self.run_pilot(["first"], lambda case, m, index, k: 1.0, deadline=6 + 2 * 4 + 0.5)
        self.assertEqual(sum(len(pairs) for pairs in data["first"].values()), 4)
        last = json.loads((output / "results.jsonl").read_text().splitlines()[-1])
        self.assertEqual(last["status"], "incomplete_due_to_cap")
        cells = [perf_calibration.estimate(data["first"][m], m) for m in perf_calibration.BLOCKS]
        self.assertTrue(all("paired_residual_variance" not in cell for cell in cells))

    def test_variance_components_match_a_hand_computation(self):
        # Pair effects b_r and block effects e_rk on the candidate's log time.
        b = [.02, -.01, .03, 0, -.02, .01]
        e = [[.01, -.01], [0, .02], [-.02, 0], [.01, .01], [-.01, .03], [.02, -.02]]

        pairs = [{"pair_index": r + 1, "order": perf_calibration.ORDER[r],
                  "baseline_blocks": [1.0, 1.0], "candidate_blocks": [math.exp(b[r] + e[r][k]) for k in range(2)],
                  "elapsed_seconds": 2.0} for r in range(6)]
        result = perf_calibration.estimate(pairs, 2)
        y = [b[r] + sum(e[r]) / 2 for r in range(6)]
        order = perf_calibration.ORDER
        mu, gamma = sum(y) / 6, sum(o * v for o, v in zip(order, y)) / 6
        s2 = sum((v - mu - gamma * o) ** 2 for v, o in zip(y, order)) / 4
        wms = sum((b[r] + e[r][k] - y[r]) ** 2 for r in range(6) for k in range(2)) / 6
        for key, value in (("mu_hat", mu), ("gamma_hat", gamma), ("paired_residual_variance", s2),
                           ("v_w", wms), ("v_b_untruncated", s2 - wms / 2), ("Q", s2 * 2.0)):
            self.assertAlmostEqual(result[key], value, places=15, msg=key)
        self.assertEqual(result["v_b"], max(0.0, s2 - wms / 2))
        model = perf_calibration.cost_model([{"blocks": m, "pairs": 6, "seconds_per_pair": 1 + 2 * m} for m in (1, 2, 4)])
        self.assertAlmostEqual(model["h"], 1)
        self.assertAlmostEqual(model["c"], 2)

    def test_a_block_count_mismatch_is_a_failed_attempt(self):
        workload = catalog([row("first", iterations=100)]).select(suite="ci")[0]
        report = {"kind": "report", "case": "first", "iterations": 100,
                  "blocks": [{"wallSeconds": 1.0, "operations": 100}]}
        self.assertIsNone(perf_calibration.child_error({"workload": report}, workload, 1))
        self.assertIsNotNone(perf_calibration.child_error({"workload": report}, workload, 2))
        report["blocks"][0]["operations"] = 99
        self.assertIsNotNone(perf_calibration.child_error({"workload": report}, workload, 1))

    def test_workflows_share_the_comparison_runner_setting_and_budget(self):
        root = pr_report.ROOT / ".github/workflows"
        comparison = (root / "pr-report.yml").read_text(encoding="utf-8")
        self.assertEqual(len(re.findall(r"runs-on: &runner \S+", comparison)), 1)
        self.assertEqual(comparison.count("runs-on: *runner"), 1)
        self.assertEqual(comparison.count("runs-on:"), 2)
        calibration = (root / "pr-report-calibration.yml").read_text(encoding="utf-8")
        self.assertIn("options: [windows-latest, ubuntu-latest]", calibration)
        self.assertIn("runs-on: ${{ inputs.os }}", calibration)
        self.assertIn(f"timeout-minutes: {pr_report.JOB_TIMEOUT_SECONDS // 60}\n", calibration)
        self.assertIn("workflow_dispatch:", calibration)
        self.assertNotIn("pull_request", calibration)


if __name__ == "__main__":
    unittest.main()
