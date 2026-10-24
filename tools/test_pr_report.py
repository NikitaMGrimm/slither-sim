"""Regression coverage for unavailable data, paired interpretation and report text."""

import ast
import contextlib
import io
import json
import math
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

from perf_baseline import digest
import perf_baseline
import perf_compare
import perf_stats
import pr_report
from test_perf_baseline import catalog, row
from test_perf_stats import REFERENCE


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
                            log.write_text("synthetic clean result\n")
                            copied = work / f"bin-baseline/perf_baseline{suffix}"
                            if not copied.exists():
                                return 0  # The first label's clean of a possibly restored target.
                            self.assertEqual(copied.read_bytes(), b"baseline")
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
                        patch.object(pr_report, "measure") as measure, \
                        patch.dict(os.environ, environment), contextlib.redirect_stdout(io.StringIO()):
                    report = pr_report.collect(output, "a" * 40, time.monotonic())
                self.assertEqual(worktrees, [work / "baseline"])
                self.assertFalse(worktrees[0].exists())
                self.assertEqual(target_paths, {checkout / "target"})
                self.assertEqual(measure.called, not failure)
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
                self.assertEqual(events[:5], [("clean-release", "baseline"), ("clean-dev", "baseline"),
                                              ("build", "baseline"), ("test", "baseline"), ("clean-release", "baseline")])
                if failure != "clean-release":
                    self.assertEqual(events[5], ("clean-dev", "baseline"))
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
        collectors.append("tools/pr_report_progress.py")  # Workflow-owned checkpoint handshake.
        self.assertIn("tools/perf_build.py", collectors)
        measured += [*pr_report.HARNESS, "Cargo.toml", "Cargo.lock", ".github/workflows/pr-report.yml", *collectors,
                     # Cargo and rustup read these if added; none exist yet.
                     "build.rs", ".cargo/config.toml", "rust-toolchain", "rust-toolchain.toml"]
        self.assertEqual([name for name in measured if not matched(name)], [])
        self.assertEqual([p for p in patterns if p.startswith("tools/") and p not in collectors], [])
        # The publisher always runs default-branch code, so PR edits to it are not measured.
        unmeasured = ["README.md", "CHANGELOG.md", "docs/maintainers/pr-reports.md", "research/README.md",
                      "tests/spawn.rs", "examples/bot.mjs", "tools/pr_report_comment.cjs",
                      ".github/workflows/pr-report-comment.yml", ".github/workflows/checks.yml"]
        self.assertEqual([name for name in unmeasured if matched(name)], [])

    def test_publisher_identity_and_staleness_regressions(self):
        subprocess.run(["node", "--test", str(Path(__file__).with_name("pr_report_comment.test.cjs")),
                        str(Path(__file__).with_name("release_performance.test.cjs"))], check=True)

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

    def test_unchanged_failures_and_ignored_cases_are_not_green_passes(self):
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40,
                  "run_url": "https://example.invalid/run", "platform": "Test platform", "expected_tests": ["physics::case"]}
        for outcome, result in (("FAILED", "🔴 1 failing"), ("ignored", "⚪ Unchanged")):
            report["tests"] = {side: {"physics::case": outcome} for side in ("baseline", "candidate")}
            text = pr_report.render(report)
            self.assertIn(result, text)
            self.assertNotIn("🟢 Passed", text)

    def test_performance_rows_follow_the_comment_wording(self):
        def dataset(name):
            return next(x for x in REFERENCE["datasets"] if x["id"] == name)["pairs"]

        def state(name):
            return next(x for x in REFERENCE["state_vectors"] if x["id"] == name)

        def look(name, termination, complete=None):
            pairs = dataset(name)
            return perf_stats.final([perf_stats.analyze(pairs)], complete or len(pairs), termination)

        capped = state("cap_after_14_keep_12")
        rows = {"faster": look("faster_at_8", "resolved"), "slower": look("slower_at_12", "resolved"),
                "within": look("within_at_12", "resolved"), "noisy": look("undecided_at_26", "max_pairs"),
                "capped": perf_stats.final(capped["expected"]["history"], 14, "time_cap"),
                "flat": look("order_only_degenerate_at_8", "time_cap"),
                "crash": perf_stats.final([], 4, "measurement_failure", "candidate failed at pair 5"),
                "short": perf_stats.final([], 7, "time_cap"), "late": perf_stats.final([], 0, "not_started")}
        report = {"base": "a" * 40, "head": "b" * 40, "merge": "c" * 40, "run_url": "https://example.invalid/run",
                  "platform": "Test platform",
                  "performance": [{"case": case, **row} for case, row in rows.items()]}
        text = pr_report.render(report)
        visible, details = text.split("<details>", 1)
        self.assertIn("\nOperation wall time, ms/op. Negative change means less time.\n", visible)
        self.assertIn("| Workload | Base | PR | Change | Result |", visible)
        faster = rows["faster"]["result"]
        low, high = faster["interval_percent"]
        self.assertIn(f"| faster | {faster['baseline_ms_per_operation']:.5f} | {faster['candidate_ms_per_operation']:.5f} | "
                      f"−{-faster['change_percent']:.2f}% [−{-low:.2f}%, −{-high:.2f}%] | 🟢 Faster |", visible)
        for case, result in (("slower", "🔴 Slower"), ("within", "⚪ Within ±5%"), ("noisy", "🟡 Undecided"),
                             ("capped", "🟡 Undecided")):
            self.assertRegex(visible, rf"\| {case} \| \d+\.\d{{5}} \| \d+\.\d{{5}} \| [+−]?\d+\.\d\d% "
                                      rf"\[[+−]?\d+\.\d\d%, [+−]?\d+\.\d\d%\] \| {result} \|")
        self.assertRegex(visible, r"\| flat \| [\d.]+ \| [\d.]+ \| \+\d+\.\d\d% \[interval unavailable\] \| 🟡 Undecided \|")
        for case in ("crash", "short", "late"):
            self.assertIn(f"| {case} | — | — | — | ⚠️ Unavailable |", visible)
        rows_text = visible.split("| --- | ---: | ---: | ---: | --- |\n", 1)[1].split("### Regression", 1)[0]
        self.assertNotIn("-", rows_text, "negative numbers use a true minus sign")
        self.assertIn("\n- Paired operation wall time after warmup, with balanced order and covered checks at 8/12/26, "
                      "then doubling through 53,248 pairs (finite safety guard). "
                      "Base/PR are geometric means; Change shows estimate [interval] against a −5% to +5% band.\n"
                      "- V2 order-adjusted t intervals provide ≥95% simultaneous coverage across 10 workloads and 14 pre-budgeted looks "
                      "under independent normal log errors. Resolved rows stop; time-capped rows retain their last "
                      "completed look. Labels use unrounded bounds.\n"
                      "- Hosted model calibration is not established; performance labels are advisory and conditional on "
                      "the timing model.\n", details)
        for line in ("- `faster`: 8 pairs used; stopped when resolved.",
                     "- `noisy`: 26 pairs used; stopped at the last look.",
                     "- `capped`: 12 pairs used; time cap (14 complete pairs collected).",
                     "- `flat`: 8 pairs used; interval unavailable (degenerate residuals); time cap.",
                     "- `crash`: unavailable; candidate failed at pair 5 (see raw logs).",
                     "- `short`: unavailable; time cap before 8 complete pairs (7 collected).",
                     "- `late`: unavailable; time cap before sampling started."):
            self.assertIn("\n" + line + "\n", details)
        for retired in ("display heuristic", "median", "process CPU", "Inconclusive", "paired change range", "95% interval"):
            self.assertNotIn(retired, text)

    def test_percentages_use_a_true_minus_and_never_show_negative_zero(self):
        self.assertEqual([pr_report.percent(v) for v in (-8.3, 1.2, -0.004, 0.0, -0.0, 0.004, 0.005001)],
                         ["−8.30%", "+1.20%", "0.00%", "0.00%", "0.00%", "0.00%", "+0.01%"])

    def test_stage_deadline_keeps_the_job_reserve(self):
        listed = catalog([row("step-default", legacy=True), row("observe")])
        changed = catalog([row("step-default", legacy=True), row("observe", definition={"changed": True})])
        for elapsed, changed_suite, limit in ((100, False, "stage cap"), (400, True, "job reserve")):
            with self.subTest(elapsed=elapsed), tempfile.TemporaryDirectory() as name:
                captured = {}

                def sample(binaries, hashes, workloads, output, deadline, plan):
                    captured.update(workloads=[w.name for w in workloads], budget=deadline - time.perf_counter(), plan=plan)
                    return {w.name: perf_stats.final([], 0, "not_started") | {"n_attempted": 0} for w in workloads}

                report = {"errors": [], "phases_seconds_after_job_start": {}}
                catalogs = {"baseline": changed if changed_suite else listed, "candidate": listed}
                suites = {label: c.select(suite="ci") for label, c in catalogs.items()}
                (Path(name) / "out").mkdir()
                with patch.object(pr_report.perf_compare, "sequential", side_effect=sample), \
                        patch.object(pr_report, "digest", return_value="d" * 64), contextlib.redirect_stdout(io.StringIO()):
                    pr_report.measure(report, {}, {}, catalogs, suites, Path(name) / "out" / "performance",
                                      time.monotonic() - elapsed)
                expected = min(pr_report.STAGE_CAP_SECONDS,
                               pr_report.JOB_TIMEOUT_SECONDS - pr_report.RESERVE_SECONDS - elapsed)
                self.assertAlmostEqual(captured["budget"], expected, delta=1)
                self.assertEqual(report["budget"]["limited_by"], limit)
                self.assertEqual(captured["workloads"], ["step-default"] if changed_suite else ["step-default", "observe"])
                observe = report["performance"][1]
                self.assertEqual(observe["termination"], "measurement_failure" if changed_suite else "not_started")
                if changed_suite:
                    self.assertEqual(observe["failure"], "workload definitions differ between base and PR")
                self.assertEqual(captured["plan"]["order"], perf_stats.ORDER)

    def test_sampler_failure_after_published_checkpoint_withdraws_final_claims(self):
        with tempfile.TemporaryDirectory() as name:
            output = Path(name)
            listed = catalog([row("step-default", legacy=True)])
            suites = {label: listed.select(suite="ci") for label in ("baseline", "candidate")}
            report = {"base": "a" * 40, "merge": "b" * 40, "head": "c" * 40,
                      "run_url": "https://example.invalid", "platform": "synthetic",
                      "errors": [], "phases_seconds_after_job_start": {}}
            look = next(d["expected"] for d in REFERENCE["datasets"] if d["id"] == "faster_at_8")

            def sample(*args, checkpoint):
                checkpoint({"step-default": perf_stats.final([look], 8, "resolved") | {"n_attempted": 8}}, time.perf_counter())
                raise OSError("synthetic final binary digest read failure")

            with patch.dict(os.environ, {"PR_REPORT_CHECKPOINT_UPLOAD": "1"}), \
                    patch.object(pr_report.perf_compare, "sequential", side_effect=sample), \
                    patch.object(pr_report, "digest", return_value="d" * 64), contextlib.redirect_stdout(io.StringIO()):
                pr_report.measure(report, {}, {}, {label: listed for label in suites}, suites,
                                  output / "performance", time.monotonic())
            prior = json.loads((output / "progress/1/report.json").read_text())
            self.assertEqual(prior["performance"][0]["label"], "Faster")
            final = report["performance"][0]
            self.assertEqual((final["label"], final["n_used"], final["termination"]), ("Unavailable", 0, "measurement_failure"))
            self.assertIsNone(final["result"])
            self.assertEqual(final["history"], [look])
            self.assertTrue(report["errors"])

    def test_workflow_timeout_matches_the_deadline_and_pr_runs_never_save_the_cache(self):
        workflow = (pr_report.ROOT / ".github/workflows/pr-report.yml").read_text(encoding="utf-8")
        measure = workflow.split("\n  measure:\n", 1)[1].split("\n  cache:\n", 1)[0]
        self.assertIn(f"\n    timeout-minutes: {pr_report.JOB_TIMEOUT_SECONDS // 60}\n", measure)
        steps = measure.split("\n      - ")
        self.assertIn("PR_REPORT_JOB_START", steps[1], "the anchor must be the first step")
        self.assertIn("save-if: false", measure)
        self.assertIn("if: github.event_name == 'pull_request'", measure)
        self.assertLess(measure.index("name: Upload comment data"), measure.index("name: Upload results"))


class Clock:
    def __init__(self):
        self.now = 0.0

    def perf_counter(self):
        return self.now


class Sampling(unittest.TestCase):
    """perf_compare.sequential against scripted children (statistics contract §2 and §6)."""

    PATTERN = [-1.1, .7, -.4, 1.4, -.8, .2, 1.0, -1.0, .5, -.3, 1.2, -.6, -.9, .4, -.2, 1.3, -.7, .1, .9, -1.2,
               .6, -.5, 1.1, -.1, .3, -.8]

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.binaries = {}
        for label in ("baseline", "candidate"):
            self.binaries[label] = self.directory / label
            self.binaries[label].write_bytes(label.encode())
        self.hashes = {label: digest(path) for label, path in self.binaries.items()}
        self.clock = Clock()
        self.calls = []

    def run_sampler(self, workloads, behavior, deadline=1e9, checkpoint=None):
        """behavior(case, label, pair_index, position) -> seconds, or an exception to raise."""
        output = self.directory / f"out-{len(list(self.directory.iterdir()))}"
        output.mkdir()
        operations = {w.name: w.iterations for w in workloads}

        def native(executable, case, iterations, stderr_path, profile=False, *, deadline=None):
            label = Path(executable).name
            # Attempt logs are named CASE-(warmup|INDEX)-LABEL-stderr.txt.
            index = None if "-warmup-" in stderr_path.name else int(stderr_path.name.split("-")[-3])
            position = sum(1 for c in self.calls if c[:2] == (case, index))
            self.calls.append((case, index, label))
            self.clock.now += 1
            outcome = behavior(case, label, index, position)
            if isinstance(outcome, BaseException):
                raise outcome
            if self.clock.now > deadline:
                self.clock.now = deadline
                raise TimeoutError("performance run deadline exceeded during native child")
            return {"case": case, "exit_code": 0, "cpu_seconds": 0.015625,
                    # The harness reports its internal operation name, not the catalog's label.
                    "workload": {"kind": "report", "case": case, "mode": "timing", "operation": "internal_name",
                                 "iterations": iterations,
                                 "timing": {"wallSeconds": outcome, "operations": operations[case]}}}

        with patch.object(perf_compare, "native", side_effect=native), \
                patch.object(perf_compare, "time", SimpleNamespace(perf_counter=self.clock.perf_counter)), \
                contextlib.redirect_stdout(io.StringIO()):
            rows = perf_compare.sequential(self.binaries, self.hashes, workloads, output, deadline, {"plan": "test"}, checkpoint=checkpoint)
        return rows, output

    def scripted(self, effects):
        """Candidate time = factor * exp(noise * pattern); the first process of a pair runs 2% slower."""
        def behavior(case, label, index, position):
            factor, noise, fail_at = effects[case]
            if index is not None and index == fail_at and label == "candidate":
                return RuntimeError(f"{case} exited 101")
            seconds = 1.0 if label == "baseline" or index is None else factor * math.exp(noise * self.PATTERN[(index - 1) % len(self.PATTERN)])
            return seconds * (1.02 if position == 0 else 1.0)
        return behavior

    def workloads(self, *names):
        return catalog([row(name, iterations=1000) for name in names]).select(suite="ci")

    def test_orders_warmups_stopping_and_failures(self):
        workloads = self.workloads("fast", "same", "crash", "noisy")
        rows, output = self.run_sampler(workloads, self.scripted({
            "fast": (.8, .004, None), "same": (1.0, .03, None), "crash": (1.0, .01, 5), "noisy": (1.0, .1, None)}))
        warmups = [(case, label) for case, index, label in self.calls if index is None]
        self.assertEqual(warmups, [(w.name, label) for w in workloads for label in ("baseline", "candidate")])
        self.assertEqual(self.calls[:len(warmups)], [(c, None, l) for c, l in warmups])
        for case in ("fast", "same", "noisy"):
            pairs = [c for c in self.calls if c[0] == case and c[1] is not None]
            for index in range(1, len(pairs) // 2 + 1):
                first = next(label for c, i, label in pairs if i == index)
                self.assertEqual(first, "baseline" if perf_stats.ORDER[index - 1] == 1 else "candidate")
        # Rounds visit active rows in catalog order.
        self.assertEqual([c for c, i, _ in self.calls if i == 1][::2], ["fast", "same", "crash", "noisy"])
        self.assertEqual([c for c, i, _ in self.calls if i == 9][::2], ["same", "noisy"])
        fast, same, crash, noisy = (rows[name] for name in ("fast", "same", "crash", "noisy"))
        self.assertEqual((fast["label"], fast["n_used"], fast["termination"]), ("Faster", 8, "resolved"))
        self.assertEqual([look["label"] for look in same["history"]], ["Undecided", "Within"])
        self.assertEqual((same["n_used"], same["n_complete"]), (12, 12))
        self.assertEqual((crash["label"], crash["termination"], crash["n_complete"], crash["failure"]),
                         ("Unavailable", "measurement_failure", 4, "candidate failed at pair 5"))
        self.assertFalse([c for c in self.calls if c[0] == "crash" and (c[1] or 0) > 5])
        self.assertEqual((noisy["label"], noisy["termination"]), ("Within", "resolved"))
        self.assertGreater(noisy["n_used"], 26)
        self.assertEqual([look["n"] for look in noisy["history"]], [n for n in perf_stats.LOOKS if n <= noisy["n_used"]])
        # Every child attempt is retained; analysis uses only recorded pairs.
        attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
        self.assertEqual(len(attempts), len(self.calls))
        self.assertEqual(sum(a["warmup"] for a in attempts), 8)
        self.assertEqual([a["status"] for a in attempts if a["case"] == "crash" and a["pair_index"] == 5],
                         ["ok", "failed"] if perf_stats.ORDER[4] == 1 else ["failed"])
        pairs = [json.loads(line) for line in (output / "pairs.jsonl").read_text().splitlines()]
        noisy_pairs = [{k: p[k] for k in perf_stats.FIELDS} for p in pairs if p["workload_id"] == "noisy"]
        self.assertEqual(noisy["history"][-1], perf_stats.analyze(noisy_pairs))
        self.assertTrue((output / "plan.json").is_file())
        self.assertEqual(json.loads((output / "sequential.json").read_text())["fast"]["label"], "Faster")

    def test_time_cap_keeps_the_last_completed_look(self):
        workloads = self.workloads("first", "second")
        behavior = self.scripted({"first": (1.0, .1, None), "second": (1.0, .1, None)})
        # Each child takes one second: 4 warmup children, then 4 per round.
        rows, output = self.run_sampler(workloads, behavior, deadline=4 + 4 * 10 + 1.5)
        for name in ("first", "second"):
            row = rows[name]
            self.assertEqual((row["termination"], row["n_used"], row["last_look_n"], row["label"]),
                             ("time_cap", 8, 8, row["history"][0]["label"]))
        self.assertEqual((rows["first"]["n_complete"], rows["first"]["n_attempted"]), (10, 11))
        self.assertEqual((rows["second"]["n_complete"], rows["second"]["n_attempted"]), (10, 10))
        attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
        self.assertEqual(attempts[-1]["status"], "incomplete_due_to_cap")
        self.assertEqual(attempts[-1]["pair_index"], 11)

    def test_progress_only_at_covered_complete_rounds_and_pause_consumes_budget(self):
        snapshots = []

        def checkpoint(rows, deadline):
            snapshots.append(json.loads(json.dumps(rows)))
            self.assertTrue(all(r["n_complete"] in perf_stats.LOOKS for r in rows.values()))
            self.clock.now = deadline  # An upload pause cannot extend D.

        rows, _ = self.run_sampler(self.workloads("fast", "noisy"), self.scripted({
            "fast": (.8, .004, None), "noisy": (1.0, .1, None)}), deadline=200, checkpoint=checkpoint)
        self.assertEqual(len(snapshots), 1)
        self.assertEqual(snapshots[0]["fast"]["termination"], "resolved")
        self.assertEqual(snapshots[0]["noisy"]["termination"], "sampling")
        self.assertEqual(rows["noisy"]["n_used"], perf_stats.LOOKS[0])
        self.assertEqual(rows["noisy"]["termination"], "time_cap")
        self.assertFalse(any(index and index > perf_stats.LOOKS[0] for _, index, _ in self.calls))

    def test_no_child_starts_at_the_deadline_and_a_warmup_cap_leaves_rows_unstarted(self):
        workloads = self.workloads("first", "second")
        behavior = self.scripted({"first": (1.0, .1, None), "second": (1.0, .1, None)})
        rows, _ = self.run_sampler(workloads, behavior, deadline=4 + 4 * 7)
        self.assertEqual(len(self.calls), 4 + 4 * 7)
        self.assertEqual([(r["termination"], r["label"], r["reason"]) for r in rows.values()],
                         [("time_cap", "Unavailable", "insufficient_pairs")] * 2)
        self.calls.clear()
        self.clock.now = 0
        rows, _ = self.run_sampler(workloads, behavior, deadline=2.5)
        self.assertEqual([r["termination"] for r in rows.values()], ["not_started"] * 2)

    def test_v2_cap_after_45_keeps_26_and_safety_guard_is_not_resolution(self):
        behavior = self.scripted({"noisy": (1.05, .1, None)})
        rows, _ = self.run_sampler(self.workloads("noisy"), behavior, deadline=2 + 2 * 45)
        self.assertEqual((rows["noisy"]["n_complete"], rows["noisy"]["n_used"], rows["noisy"]["termination"]),
                         (45, 26, "time_cap"))
        # A deliberately small synthetic finite schedule exercises the safety
        # guard without writing 100,000 child records in a regression test.
        with patch.object(perf_stats, "LOOKS", (8, 12)), patch.object(perf_stats, "ORDER", perf_stats.ORDER[:12]):
            self.calls.clear()
            self.clock.now = 0
            rows, _ = self.run_sampler(self.workloads("noisy"), behavior)
        self.assertEqual((rows["noisy"]["label"], rows["noisy"]["termination"]), ("Undecided", "max_pairs"))

    def test_exit_wait_timeout_keeps_last_looks_or_only_fails_affected_workload(self):
        for capped in (False, True):
            with self.subTest(capped=capped):
                self.calls.clear()
                self.clock.now = 0
                normal = self.scripted({"first": (1.0, .1, None), "healthy": (1.0, .1, None)})

                def behavior(case, label, index, position):
                    if case != "first" or index != 10 or position != 0:
                        return normal(case, label, index, position)
                    if capped:
                        self.clock.now = 1000
                    child = Mock(stdin=None, stdout=io.BytesIO(), stderr=None, returncode=0)
                    child.poll.return_value = 0
                    child.wait.side_effect = subprocess.TimeoutExpired("fixture", 120)
                    with patch.object(perf_baseline.subprocess, "Popen", return_value=child), \
                            patch.object(perf_baseline.Lines, "read", return_value=b"{}"), \
                            patch.object(perf_baseline, "time", SimpleNamespace(perf_counter=lambda: 0)):
                        try:
                            perf_baseline.native("fixture", case, 1000, self.directory / "stderr.txt", deadline=120)
                        except TimeoutError as error:
                            return error

                rows, output = self.run_sampler(self.workloads("first", "healthy"), behavior, deadline=1000)
                self.assertEqual(rows["first"]["n_complete"], 9)
                self.assertEqual([look["n"] for look in rows["first"]["history"]], [8])
                if capped:
                    for result in rows.values():
                        self.assertEqual((result["termination"], result["n_used"]), ("time_cap", 8))
                else:
                    self.assertEqual((rows["first"]["termination"], rows["first"]["label"]),
                                     ("measurement_failure", "Unavailable"))
                    self.assertEqual((rows["healthy"]["termination"], rows["healthy"]["label"]),
                                     ("resolved", "Within"))
                attempts = [json.loads(line) for line in (output / "results.jsonl").read_text().splitlines()]
                failed = next(a for a in attempts if a["case"] == "first" and a["pair_index"] == 10)
                self.assertEqual(failed["status"], "incomplete_due_to_cap" if capped else "failed")
                self.assertEqual(json.loads((output / "sequential.json").read_text()), rows)

    def test_hung_children_invalid_records_and_changed_binaries_are_failures(self):
        workloads = self.workloads("hung", "count")

        def behavior(case, label, index, position):
            if case == "hung" and index == 2:
                return TimeoutError("child response deadline exceeded")
            if case == "count" and index == 3 and label == "baseline":
                return float("nan")
            return 1.0 + .01 * (index or 0) * (label == "candidate")

        rows, _ = self.run_sampler(workloads, behavior)
        self.assertEqual((rows["hung"]["termination"], rows["hung"]["n_complete"]), ("measurement_failure", 1))
        self.assertEqual((rows["count"]["termination"], rows["count"]["failure"]),
                         ("measurement_failure", "baseline failed at pair 3"))

        def corrupt(case, label, index, position):
            if index == 8:
                self.binaries["baseline"].write_bytes(b"changed")
            return 1.0 + .001 * self.PATTERN[(index or 1) - 1] * (label == "candidate")

        self.calls.clear()
        rows, _ = self.run_sampler(self.workloads("one"), corrupt)
        self.assertEqual((rows["one"]["label"], rows["one"]["failure"], len(rows["one"]["history"])),
                         ("Unavailable", "benchmark executable changed during sampling", 1))


if __name__ == "__main__":
    unittest.main()
