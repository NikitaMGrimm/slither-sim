"""Regression coverage for unavailable data, paired interpretation and report text."""

import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import pr_report


def metric(value, samples=6, unit="rad/s"):
    return {"schema": 1, "metrics": [{"id": "turn", "label": "Turn error", "value": value,
                                        "unit": unit, "samples": samples}]}


class ReportTests(unittest.TestCase):
    def test_missing_data_and_changed_denominators_are_not_improvements(self):
        self.assertEqual(pr_report.metric_rows(metric(2), None)[0]["reading"], "Unavailable")
        self.assertEqual(pr_report.metric_rows(metric(2), metric(1, samples=5))[0]["reading"], "Unavailable")
        self.assertEqual(pr_report.metric_rows(metric(2), metric(1, unit="different"))[0]["reading"], "Unavailable")
        self.assertEqual(pr_report.metric_rows(metric(0), metric(1))[0]["reading"], "Higher error")
        self.assertEqual(pr_report.metric_rows(metric(2), metric(1))[0]["reading"], "Lower error")

    def test_nonfinite_negative_and_duplicate_metrics_are_rejected(self):
        for value in (float("nan"), float("inf"), -1):
            with self.assertRaises(ValueError):
                pr_report.metric_rows(metric(value), metric(1))
        duplicate = metric(1)
        duplicate["metrics"] *= 2
        with self.assertRaises(ValueError):
            pr_report.metric_rows(duplicate, metric(1))

    def test_cargo_targets_disambiguate_names_and_retain_failures(self):
        log = """     Running tests/movement_turns.rs (target/debug/deps/movement_turns-abc.exe)
test same_name ... FAILED
test ignored_case ... ignored, documented reason
error: test failed, to rerun pass `--test movement_turns`
     Running tests\\physics.rs (target/debug/deps/physics-def.exe)
test same_name ... ok
"""
        self.assertEqual(pr_report.parse_tests(log), {
            "movement_turns::same_name": "FAILED", "movement_turns::ignored_case": "ignored", "physics::same_name": "ok"})
        self.assertEqual(pr_report.parse_tests("error: could not compile"), {})
        self.assertEqual(pr_report.transition(None, "ok"), "unavailable")
        self.assertEqual(pr_report.transition("FAILED", "ok"), "fixed")
        self.assertEqual(pr_report.transition("ok", "FAILED"), "regressed")
        self.assertEqual(pr_report.transition("ignored", "ok"), "changed")

    def performance(self, directory, ratios, cpu=True):
        pairs, rows = [], []
        for repetition, ratio in enumerate(ratios):
            pairs.append({"cpu_ratio_candidate_over_baseline": ratio if cpu else None,
                          "wall_ratio_candidate_over_baseline": ratio})
            for label, value in (("baseline", 1), ("candidate", ratio)):
                rows.append({"case": "observe", "label": label, "repetition": repetition,
                             "cpu_seconds": value if cpu else None,
                             "workload": {"timing": {"wallSeconds": value}}})
        (directory / "summary.json").write_text(json.dumps({"observe": {"pairs": pairs, "iterations": 1000}}))
        (directory / "results.jsonl").write_text("\n".join(json.dumps(row) for row in rows))
        return pr_report.performance_rows(directory)[0]

    def test_mixed_pair_directions_and_small_shifts_stay_inconclusive(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            self.assertEqual(self.performance(directory, [.8, .8, 1.1])["reading"], "Inconclusive")
            self.assertEqual(self.performance(directory, [.98, .99, .99])["reading"], "Inconclusive")
            self.assertEqual(self.performance(directory, [.8, .85, .9])["reading"], "Faster")
            self.assertEqual(self.performance(directory, [1.1, 1.2, 1.3])["reading"], "Slower")
            self.assertEqual(self.performance(directory, [.8, .85, .9], cpu=False)["metric"], "operation wall")

    def test_incomplete_render_does_not_claim_success_and_escapes_table_cells(self):
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40, "run_url": "https://example.invalid/run",
                  "platform": "Test platform", "errors": ["release build unavailable"],
                  "fidelity": pr_report.metric_rows(metric(1), None), "removed_tests": ["physics::removed"]}
        text = pr_report.render(report)
        self.assertIn("| Unavailable |", text)
        self.assertIn("| 1.000000 | — | Unavailable | 6 |", text)
        self.assertIn("0 fixed; 0 regressed; 0 unavailable", text)
        self.assertIn("removal is not counted as a fix", text)
        self.assertIn("**Incomplete:** release build unavailable", text)
        self.assertTrue(text.startswith("> [!NOTE]\n> 🤖 **GPT 6.1 Sol writing on behalf of NikitaMGrimm**"))
        self.assertEqual(pr_report.cell("a|b\nc`d"), "a\\|b c'd")

    def test_publisher_identity_and_staleness_regressions(self):
        subprocess.run(["node", "--test", str(Path(__file__).with_name("pr_report_comment.test.cjs"))], check=True)


if __name__ == "__main__":
    unittest.main()
