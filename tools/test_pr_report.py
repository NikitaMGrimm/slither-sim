"""Regression coverage for unavailable data, paired interpretation and report text."""

import ast
import contextlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import pr_report
from test_perf_baseline import catalog, row


def local_imports(directory, module):
    """The collector and every tools/ module it imports, transitively, without executing them."""
    found, pending = set(), [module]
    while pending:
        name = pending.pop()
        if name in found or not (directory / f"{name}.py").is_file():
            continue
        found.add(name)
        tree = ast.parse((directory / f"{name}.py").read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                pending += [alias.name.split(".")[0] for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and node.module and not node.level:
                pending.append(node.module.split(".")[0])
    return found


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
                events, target_paths, worktrees = [], set(), []

                def git_or_tool(args, cwd, environment=None, timeout=None):
                    args = [str(arg) for arg in args]
                    if args[:3] == ["git", "worktree", "add"]:
                        worktrees.append(Path(args[4]))
                        shutil.copytree(checkout, args[4])
                    elif args[:3] == ["git", "worktree", "remove"]:
                        shutil.rmtree(args[4])
                    elif args[:2] == ["git", "rev-parse"]:
                        return ("t" if args[-1].endswith("{tree}") else "b") * 40
                    return "synthetic tool"

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
                listed = catalog([row("step-default", trace=32, legacy=True), row("official-100-step", trace=8),
                                  row("official-500-step", suite="full")])
                traced = []

                def trace(binary, case, iterations, directory, *, terminal=False, deadline=None):
                    traced.append((Path(binary).parent.name, case, iterations, terminal))
                    return {"sha256": case}

                with patch.object(pr_report, "ROOT", checkout), patch.object(pr_report, "git", side_effect=["c" * 40, "b" * 40, "a" * 40]), \
                        patch.object(pr_report.perf_build, "output", side_effect=git_or_tool), \
                        patch.object(pr_report.subprocess, "check_output", return_value="synthetic rustc"), \
                        patch.object(pr_report.perf_build, "command", side_effect=command), \
                        patch.object(pr_report, "load_catalog", return_value=listed), \
                        patch.object(pr_report, "capture_trace", side_effect=trace), \
                        patch.object(pr_report, "performance_rows", return_value=[]), \
                        patch.dict(os.environ, environment), contextlib.redirect_stdout(io.StringIO()):
                    report = pr_report.collect(output, "a" * 40)
                self.assertEqual(worktrees, [work / "baseline"])
                self.assertFalse(worktrees[0].exists())
                self.assertEqual(target_paths, {work / "target"})
                self.assertEqual(report["identities"]["baseline"]["commit"], "b" * 40)
                if failure:
                    self.assertEqual([t["reading"] for t in report["traces"]], ["Unavailable"] * 2)
                else:
                    self.assertEqual(traced, [(f"bin-{label}", case, window, terminal)
                                              for case, window, terminal in (("step-default", 32, False),
                                                                             ("official-100-step", 8, True))
                                              for label in ("baseline", "candidate")])
                    self.assertEqual([(t["case"], t["iterations"], t["reading"]) for t in report["traces"]],
                                     [("step-default", 32, "Equal"), ("official-100-step", 8, "Equal")])
                    self.assertIn("`official-100-step` 8", pr_report.render({**report, "run_url": "https://example.invalid"}))
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
        return pr_report.performance_rows(directory, {"observe"})[0]

    def test_mixed_pair_directions_and_small_shifts_stay_inconclusive(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            self.assertEqual(self.performance(directory, [.8, .8, 1.1])["reading"], "Inconclusive")
            self.assertEqual(self.performance(directory, [.98, .99, .99])["reading"], "Inconclusive")
            self.assertEqual(self.performance(directory, [.8, .85, .9])["reading"], "Faster")
            self.assertEqual(self.performance(directory, [1.1, 1.2, 1.3])["reading"], "Slower")
            self.assertEqual(self.performance(directory, [.8, .85, .9], cpu=False)["metric"], "operation wall")

    def test_legacy_rows_use_process_cpu_and_scenario_rows_use_operation_wall(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            # CPU ratios stay near 1 (setup-dominated); the timed operation got 20% faster.
            pairs = [{"repetition": r, "cpu_ratio_candidate_over_baseline": .99,
                      "wall_ratio_candidate_over_baseline": .8} for r in range(3)]
            rows = [{"case": case, "label": label, "repetition": r, "cpu_seconds": 2.0,
                     "workload": {"timing": {"wallSeconds": 1.0 if label == "baseline" else .8}}}
                    for case in ("observe", "official-100-bot") for r in range(3)
                    for label in ("baseline", "candidate")]
            summary = {case: {"pairs": pairs, "iterations": 100} for case in ("observe", "official-100-bot")}
            (directory / "summary.json").write_text(json.dumps(summary))
            (directory / "results.jsonl").write_text("\n".join(json.dumps(row) for row in rows))
            legacy, scenario = pr_report.performance_rows(directory, {"observe"})
            self.assertEqual((legacy["metric"], legacy["ratio"], legacy["reading"]), ("process CPU", .99, "Inconclusive"))
            self.assertEqual((scenario["metric"], scenario["ratio"], scenario["reading"]), ("operation wall", .8, "Faster"))
            self.assertEqual((scenario["baseline"], scenario["candidate"]), (10.0, 8.0))
            report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40, "run_url": "https://example.invalid/run",
                      "platform": "Test platform", "performance": [legacy, scenario]}
            text = pr_report.render(report)
            self.assertIn("Time per operation, ms.", text)
            self.assertIn("| observe (process CPU) |", text)
            self.assertIn("| official-100-bot (operation wall) |", text)
            self.assertIn("scenario rows show operation wall time, which excludes placement and warmup", text)
            # Without legacy rows, no row uses process CPU.
            self.assertEqual({row["metric"] for row in pr_report.performance_rows(directory, set())}, {"operation wall"})

    def test_single_metric_headers_use_explicit_labels(self):
        for metric_name, header in (("process CPU", "Process CPU, ms/op."), ("operation wall", "Operation wall, ms/op.")):
            row = {"case": "observe", "metric": metric_name, "baseline": 1, "candidate": 1, "ratio": 1,
                   "ratio_min": 1, "ratio_max": 1, "reading": "Inconclusive"}
            report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40, "run_url": "https://example.invalid/run",
                      "platform": "Test platform", "performance": [row]}
            with self.subTest(metric_name):
                text = pr_report.render(report)
                self.assertIn(header, text)
                self.assertNotIn("Process cpu", text)

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
            row = pr_report.performance_rows(directory, {"observe"})[0]
            self.assertEqual(row["baseline"], 100)
            self.assertEqual(row["candidate"], 100)
            self.assertEqual(row["ratio"], row["candidate"] / row["baseline"])

    def test_workflow_paths_cover_every_measured_input_and_skip_documentation(self):
        root = pr_report.ROOT
        workflow = (root / ".github/workflows/pr-report.yml").read_text(encoding="utf-8")
        block = re.search(r"^    paths:\n((?:      - .+\n)+)", workflow, re.MULTILINE)[1]
        patterns = [line.removeprefix("      - ") for line in block.splitlines()]
        # Only GitHub's * (one path segment) and ** (any depth) are used here.
        self.assertFalse([p for p in patterns if re.search(r"[?\[\]!+]", p)])
        expressions = [re.compile(re.escape(p).replace(r"\*\*", ".*").replace(r"\*", "[^/]*")) for p in patterns]
        matched = lambda name: any(e.fullmatch(name) for e in expressions)
        measured = [p.relative_to(root).as_posix() for directory in ("src", "tests/fixtures")
                    for p in (root / directory).rglob("*") if p.is_file()]
        collectors = [f"tools/{name}.py" for name in sorted(local_imports(root / "tools", "pr_report"))]
        self.assertIn("tools/perf_build.py", collectors)
        measured += [*pr_report.HARNESS, "Cargo.toml", "Cargo.lock", ".github/workflows/pr-report.yml", *collectors,
                     # Cargo and rustup read these if added; none exist yet.
                     "build.rs", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"]
        self.assertEqual([name for name in measured if not matched(name)], [])
        self.assertEqual([p for p in patterns if p.startswith("tools/") and p not in collectors], [])
        # The publisher always runs default-branch code, so PR edits to it are not measured.
        unmeasured = ["README.md", "CHANGELOG.md", "docs/pr-reports.md", "research/README.md",
                      "tests/spawn.rs", "examples/bot.mjs", "tools/pr_report_comment.cjs",
                      ".github/workflows/pr-report-comment.yml", ".github/workflows/checks.yml"]
        self.assertEqual([name for name in unmeasured if matched(name)], [])

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
