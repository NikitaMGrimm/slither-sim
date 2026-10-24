"""The A/A calibration pilot's schedule, records and per-job estimates, with scripted children."""

import contextlib
import io
import itertools
import json
import math
from pathlib import Path
import re
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

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

    def run_pilot(self, names, seconds, deadline=1e9):
        """seconds(case, m, pair_index, block) -> one block's wall time."""
        workloads = catalog([row(name, iterations=100) for name in names]).select(suite="ci")
        output = self.directory / f"out-{len(self.calls)}"
        output.mkdir()

        def native(executable, case, iterations, stderr_path, profile=False, *, deadline=None, blocks=None):
            index = stderr_path.name.split("-")[-3]
            index = None if index == "warmup" else int(index)
            self.calls.append((Path(executable).name, case, blocks, index))
            self.now += 1
            if self.now > deadline:
                self.now = deadline
                raise TimeoutError("performance run deadline exceeded during native child")
            timings = [{"wallSeconds": seconds(case, blocks, index, k), "operations": iterations}
                       for k in range(blocks)]
            return {"process_wall_seconds": 0.5 * blocks + 0.25, "cpu_seconds": None,
                    "workload": {"kind": "report", "case": case, "iterations": iterations, "mode": "timing",
                                 "timing": None, "blocks": timings}}

        with patch.object(perf_calibration, "native", side_effect=native), \
                patch.object(perf_calibration, "time", SimpleNamespace(perf_counter=lambda: self.now)), \
                contextlib.redirect_stdout(io.StringIO()):
            data = perf_calibration.calibrate(self.directory / "perf_baseline", workloads, output, deadline)
        return data, output

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
