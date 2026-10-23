"""Regression coverage for unavailable data, paired interpretation and report text."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import pr_report


def metric(value, samples=6, unit="rad/s"):
    return {"schema": 1, "metrics": [{"id": "turn", "label": "Turn error", "value": value,
                                        "unit": unit, "samples": samples}]}


class ReportTests(unittest.TestCase):
    def test_shared_build_outputs_are_copied_before_clean_and_failures_never_reuse_them(self):
        for failure in (None, "clean-release", "clean-dev", "candidate-build"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                checkout = root / "checkout"
                for directory in ("examples", "tests/fixtures"):
                    (checkout / directory).mkdir(parents=True, exist_ok=True)
                for name in ("perf_baseline", "fidelity_metrics"):
                    (checkout / f"examples/{name}.rs").write_text("// synthetic harness\n")
                for name in pr_report.TESTS:
                    (checkout / f"tests/{name}.rs").write_text("#[test]\nfn synthetic() {}\n")
                (checkout / "Cargo.lock").write_text("# synthetic lock\n")
                output = root / "report"
                output.mkdir()
                work = root / "report-work"
                suffix = ".exe" if os.name == "nt" else ""
                events, target_paths = [], set()

                def add_worktree(args, **kwargs):
                    shutil.copytree(checkout, work / "base")

                def command(args, cwd, log, environment=None, timeout=600):
                    label = "candidate" if cwd == checkout else "baseline"
                    if args[0] == "cargo":
                        target = Path(environment["CARGO_TARGET_DIR"])
                        target_paths.add(target)
                        if args[1] == "clean":
                            profile = args[args.index("--profile") + 1] if "--profile" in args else "dev"
                            events.append(("clean-" + profile, label))
                            self.assertEqual((work / f"bin-baseline/perf_baseline{suffix}").read_bytes(), b"baseline")
                            log.write_text("synthetic clean result\n")
                            return int(failure == "clean-" + profile)
                        events.append((args[1], label))
                        if args[1] == "build":
                            log.write_text("synthetic build result\n")
                            if label == "candidate" and failure == "candidate-build":
                                return 1  # Old target files deliberately remain present.
                            directory = target / "release/examples"
                            directory.mkdir(parents=True, exist_ok=True)
                            for name in ("perf_baseline", "fidelity_metrics"):
                                (directory / (name + suffix)).write_bytes(label.encode())
                        else:
                            log.write_text("Running tests/physics.rs\ntest synthetic ... ok\n")
                    elif Path(args[0]).name == "fidelity_metrics" + suffix:
                        log.write_text(json.dumps(metric(1)))
                    return 0

                environment = {"GITHUB_REPOSITORY": "owner/repo", "PR_NUMBER": "35",
                               "GITHUB_RUN_ID": "100", "GITHUB_RUN_ATTEMPT": "1",
                               "GITHUB_SERVER_URL": "https://github.com"}
                with patch.object(pr_report, "ROOT", checkout), patch.object(pr_report, "git", side_effect=["c" * 40, "b" * 40, "a" * 40]), \
                        patch.object(pr_report.subprocess, "run", side_effect=add_worktree), \
                        patch.object(pr_report.subprocess, "check_output", return_value="synthetic rustc"), \
                        patch.object(pr_report, "command", side_effect=command), \
                        patch.object(pr_report, "CASES", ()), patch.object(pr_report, "performance_rows", return_value=[]), \
                        patch.dict(os.environ, environment):
                    report = pr_report.collect(output, "a" * 40)
                self.assertEqual(target_paths, {work / "target"})
                self.assertEqual((work / f"bin-baseline/perf_baseline{suffix}").read_bytes(), b"baseline")
                self.assertEqual(events[:3], [("build", "baseline"), ("test", "baseline"), ("clean-release", "baseline")])
                if failure != "clean-release":
                    self.assertEqual(events[3], ("clean-dev", "baseline"))
                if failure:
                    self.assertTrue(report["errors"])
                    self.assertNotIn("candidate", report["binaries"])
                    self.assertFalse((work / "bin-candidate").exists())
                    if failure.startswith("clean-"):
                        self.assertNotIn(("build", "candidate"), events)
                else:
                    self.assertEqual((work / f"bin-candidate/perf_baseline{suffix}").read_bytes(), b"candidate")
                    self.assertFalse(report["errors"])

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
            pairs.append({"repetition": repetition, "cpu_ratio_candidate_over_baseline": ratio if cpu else None,
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
        self.assertIn("| ⚠️ Unavailable |", text)
        self.assertIn("| 1.000000 | — | ⚠️ Unavailable | 6 |", text)
        self.assertNotIn("🟢 Passed", text)
        self.assertIn("removal is not counted as a fix", text)
        self.assertIn("**Incomplete:** release build unavailable", text)
        self.assertTrue(text.startswith(pr_report.MARKER + "\n## Comparison\n"))
        self.assertNotIn("[!NOTE]", text)
        self.assertNotIn("on behalf of", text)
        self.assertNotIn("do not block merging", text)
        self.assertEqual(pr_report.cell("a|b\nc`d"), "a\\|b c'd")

    def test_displayed_costs_come_from_the_same_pair_as_the_paired_delta(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            self.performance(directory, [1.1, .9, 1.0])
            raw = [json.loads(line) for line in (directory / "results.jsonl").read_text().splitlines()]
            for row in raw:
                multiplier = [1, 2, 100][row["repetition"]]
                row["cpu_seconds"] *= multiplier
            (directory / "results.jsonl").write_text("\n".join(json.dumps(row) for row in raw))
            row = pr_report.performance_rows(directory)[0]
            self.assertEqual(row["baseline"], 100)
            self.assertEqual(row["candidate"], 100)
            self.assertEqual(row["ratio"], row["candidate"] / row["baseline"])

    def test_publisher_identity_and_staleness_regressions(self):
        subprocess.run(["node", "--test", str(Path(__file__).with_name("pr_report_comment.test.cjs"))], check=True)

    def test_aborted_test_binary_keeps_its_missing_cases_in_the_denominator(self):
        completed = {f"movement_turns::{name}": "ok" for name in ("a", "b", "c")}
        expected = list(completed) + [f"physics::case{index}" for index in range(19)]
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40, "run_url": "https://example.invalid/run",
                  "platform": "Test platform", "expected_tests": expected,
                  "tests": {"baseline": completed, "candidate": completed}}
        text = pr_report.render(report)
        self.assertIn("3/22; 19 unavailable", text)
        self.assertIn("⚠️ 19 unavailable", text)
        self.assertIn("**Incomplete report:**", text)
        self.assertNotIn("3/3 pass", text)

    def test_suite_inventory_includes_ignored_tests_and_excludes_helpers(self):
        source = '#[test]\nfn normal() {}\n#[test]\n#[ignore = "reason"]\nfn ignored() {}\nfn helper() {}'
        self.assertEqual(pr_report.test_names(source), {"normal", "ignored"})

    def test_color_cues_preserve_mixed_metrics_and_uncertainty(self):
        performance = [{"case": case, "metric": metric_name, "baseline": 1, "candidate": ratio,
                        "ratio": ratio, "ratio_min": ratio, "ratio_max": ratio, "reading": reading}
                       for case, metric_name, ratio, reading in (
                           ("observe", "process CPU", .8, "Faster"),
                           ("protocol", "operation wall", 1.2, "Slower"),
                           ("portable", "process CPU", 1.01, "Inconclusive"))]
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40,
                  "run_url": "https://example.invalid/run", "platform": "Test platform", "performance": performance}
        text = pr_report.render(report)
        self.assertIn("observe (process CPU)", text)
        self.assertIn("protocol (operation wall)", text)
        self.assertIn("🟢 Faster", text)
        self.assertIn("🔴 Slower", text)
        self.assertIn("🟡 Inconclusive", text)
        visible, details = text.split("<details>", 1)
        self.assertNotIn("display heuristic", visible)
        self.assertIn("display heuristic", details)

    def test_unchanged_failures_and_ignored_cases_are_not_green_passes(self):
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40,
                  "run_url": "https://example.invalid/run", "platform": "Test platform", "expected_tests": ["physics::case"]}
        for outcome, result in (("FAILED", "🔴 1 failing"), ("ignored", "⚪ Unchanged")):
            report["tests"] = {side: {"physics::case": outcome} for side in ("baseline", "candidate")}
            text = pr_report.render(report)
            self.assertIn(result, text)
            self.assertNotIn("🟢 Passed", text)


if __name__ == "__main__":
    unittest.main()
