"""Regression checks for the workload catalog, failure diagnostics and child cleanup."""

import contextlib
from pathlib import Path
import gzip
import hashlib
import io
import json
import re
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import Mock, patch

import perf_baseline
import perf_build
import perf_compare
import perf_observation_phases


def row(name, suite="ci", iterations=100, trace=4, profile=False, available=True, legacy=False, **extra):
    return {"available": available, "definition": {"config": {"radius": 1000}, "population": 4},
            "iterations": iterations, "legacy": legacy, "mode": "sandbox", "name": name,
            "operation": "step", "profile": profile, "suite": suite, "synthetic": None,
            "traceIterations": trace, **extra}


def catalog_line(rows=None, **fields):
    """Synthetic `perf_baseline --list` output; names and values are fixtures, not the real catalog."""
    rows = rows if rows is not None else [
        row("step-default", iterations=100000, trace=32, profile=True, legacy=True),
        row("observe", iterations=1000, trace=32, legacy=True),
        row("official-100-step", iterations=50, trace=8, profile=True),
        row("sandbox-16-bot", suite="full", iterations=200, trace=16),
        row("client-official-100", suite="full", available=False, trace=4)]
    value = {"format": perf_baseline.CATALOG_FORMAT, "kind": "catalog", "networking": False, "seed": 7,
             "workloads": rows, **fields}
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def catalog(rows=None, source="harness", **fields):
    return perf_baseline.parse_catalog(catalog_line(rows, **fields), source)


class Catalogs(unittest.TestCase):
    def test_parse_keeps_registration_order_and_fingerprints_definitions(self):
        parsed = catalog()
        self.assertEqual([w.name for w in parsed.workloads],
                         ["step-default", "observe", "official-100-step", "sandbox-16-bot", "client-official-100"])
        self.assertEqual(parsed.sha256, hashlib.sha256(catalog_line()).hexdigest())
        first = parsed.workloads[0]
        self.assertEqual((first.iterations, first.trace_iterations, first.legacy, first.profile),
                         (100000, 32, True, True))
        reordered = row("step-default", iterations=100000, trace=32, profile=True, legacy=True,
                        definition={"population": 4, "config": {"radius": 1000}})
        self.assertEqual(catalog([reordered]).workloads[0].fingerprint, first.fingerprint)
        changed = row("step-default", definition={"config": {"radius": 1001}, "population": 4})
        self.assertNotEqual(catalog([changed]).workloads[0].fingerprint, first.fingerprint)
        self.assertEqual(catalog([row("a", extra="ignored")]).workloads[0].name, "a")

    def test_invalid_catalogs_are_rejected_at_the_boundary(self):
        cases = {
            "not json": b"{",
            "format": catalog_line(format="slither-perf-catalog-v0"),
            "kind": catalog_line(kind="report"),
            "seed": catalog_line(seed=True),
            "empty": catalog_line([]),
            "duplicate": catalog_line([row("a"), row("a")]),
            "name": catalog_line([row("Step_Default")]),
            "suite": catalog_line([row("a", suite="nightly")]),
            "iterations": catalog_line([row("a", iterations=0)]),
            "iterations bool": catalog_line([row("a", iterations=True)]),
            "trace window": catalog_line([row("a", trace=1001)]),
            "flag": catalog_line([row("a", profile="yes")]),
            "synthetic": catalog_line([row("a", synthetic="")]),
            "definition": catalog_line([row("a", definition=None)]),
            "operation": catalog_line([row("a", operation="")]),
        }
        for name, line in cases.items():
            with self.subTest(name), self.assertRaisesRegex(ValueError, "invalid workload catalog from fixture"):
                perf_baseline.parse_catalog(line, "fixture")

    def test_selection_uses_suites_and_names_the_list_command_for_unknown_cases(self):
        parsed = catalog()
        self.assertEqual([w.name for w in parsed.select(suite="ci")],
                         ["step-default", "observe", "official-100-step"])
        self.assertEqual([w.name for w in parsed.select()],
                         ["step-default", "observe", "official-100-step", "sandbox-16-bot"])
        self.assertEqual([w.name for w in parsed.select(["sandbox-16-bot", "observe"])],
                         ["sandbox-16-bot", "observe"])
        with self.assertRaisesRegex(ValueError, r"unknown workload\(s\) step-bogus; `harness --list`"):
            parsed.select(["observe", "step-bogus"])
        with self.assertRaisesRegex(ValueError, "networking feature disabled"):
            parsed.select(["client-official-100"])
        with self.assertRaisesRegex(ValueError, "duplicate"):
            parsed.select(["observe", "observe"])

    def test_plans_must_match_names_iterations_and_trace_windows(self):
        base = catalog().select(suite="ci")
        self.assertEqual(perf_baseline.same_plan({"baseline": base, "candidate": base})[0],
                         {"name": "step-default", "iterations": 100000, "traceIterations": 32})
        for changed in ({"iterations": 99}, {"trace": 9}, {"name": "official-101-step"}):
            rows = [row("step-default", iterations=100000, trace=32, profile=True, legacy=True),
                    row("observe", iterations=1000, trace=32, legacy=True),
                    row(**{"name": "official-100-step", "iterations": 50, "trace": 8, **changed})]
            with self.subTest(changed), self.assertRaisesRegex(ValueError, "baseline and candidate workload plans differ"):
                perf_baseline.same_plan({"baseline": base, "candidate": catalog(rows).select(suite="ci")})

    def test_catalog_command_requires_one_line_and_respects_the_deadline(self):
        for stdout, code in ((catalog_line() + b"\n", 0), (b"", 2), (catalog_line() + b"\nextra\n", 0),
                             (catalog_line(), 1)):
            completed = subprocess.CompletedProcess([], code, stdout, b"usage witness")
            with self.subTest(stdout=stdout[:8], code=code), patch.object(
                    perf_baseline.subprocess, "run", return_value=completed) as run:
                if code == 0 and stdout.count(b"\n") == 1:
                    self.assertEqual(perf_baseline.load_catalog(Path("harness")).workloads[0].name, "step-default")
                else:
                    with self.assertRaisesRegex(RuntimeError, "--list. exited .*usage witness"):
                        perf_baseline.load_catalog(Path("harness"))
                self.assertEqual(run.call_args.args[0][1:], ["--list"])
                self.assertEqual(run.call_args.kwargs["timeout"], perf_baseline.COMMAND_TIMEOUT_SECONDS)
        with patch.object(perf_baseline.time, "perf_counter", return_value=10), patch.object(
                perf_baseline.subprocess, "run") as run:
            with self.assertRaises(TimeoutError):
                perf_baseline.load_catalog(Path("harness"), deadline=9)
            run.assert_not_called()

    def test_baseline_runner_takes_cases_counts_and_windows_from_the_catalog(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, cases, error in (("unknown", ["observe", "step-bogus"], "step-bogus"),
                                       ("defaults", None, None), ("named", ["official-100-step", "e2e-client"], None)):
                with self.subTest(name):
                    output = root / name
                    argv = ["perf_baseline", "--harness", sys.executable, "--cli", sys.executable,
                            "--output", str(output), "--repeats", "1", *(["--cases", *cases] if cases else [])]
                    traced, measured = [], []

                    def trace(executable, case, iterations, directory, *, terminal=False, deadline=None):
                        traced.append((case, iterations, terminal))
                        return {"case": case}

                    def run_native(executable, case, iterations, stderr_path, profile=False, **kwargs):
                        measured.append((case, iterations, profile))
                        return {"case": case, "process_wall_seconds": 0}

                    # Windows platform probes can call check_output(text=True),
                    # which must not consume the binary Git diff fixture below.
                    with patch.object(sys, "argv", argv), patch.object(
                            perf_baseline, "load_catalog", return_value=catalog()), patch.object(
                            perf_baseline, "command_output", return_value="fixture"), patch.object(
                            perf_baseline.platform, "platform", return_value="fixture-platform"), patch.object(
                            perf_baseline.platform, "processor", return_value="fixture-cpu"), patch.object(
                            perf_baseline, "capture_trace", side_effect=trace), patch.object(
                            perf_baseline, "native", side_effect=run_native), patch.object(
                            perf_baseline, "e2e", return_value={"wall_seconds": 0}) as e2e, patch.object(
                            perf_baseline.subprocess, "check_output", return_value=b""), \
                            contextlib.redirect_stdout(io.StringIO()):
                        if error:
                            with self.assertRaisesRegex(ValueError, f"{error}.*--list"):
                                perf_baseline.main()
                            self.assertEqual((traced, measured), ([], []))
                            self.assertIn("--list", json.loads((output / "failure.json").read_text())["error"])
                            continue
                        perf_baseline.main()
                    manifest = json.loads((output / "manifest.json").read_text())
                    self.assertEqual(manifest["catalog"]["sha256"], catalog().sha256)
                    self.assertEqual((manifest["platform"], manifest["cpu"]), ("fixture-platform", "fixture-cpu"))
                    if cases is None:
                        self.assertEqual(traced, [("step-default", 32, False), ("observe", 32, False),
                                                  ("official-100-step", 8, True), ("sandbox-16-bot", 16, True)])
                        self.assertEqual(measured[:2], [("step-default", 100000, True), ("official-100-step", 50, True)])
                        self.assertEqual(measured[2:6], [("step-default", 100000, False), ("observe", 1000, False),
                                                         ("official-100-step", 50, False), ("sandbox-16-bot", 200, False)])
                        self.assertEqual(e2e.call_count, 4)
                    else:
                        self.assertEqual(traced, [("official-100-step", 8, True)])
                        self.assertEqual(measured, [("official-100-step", 50, True)] + [("official-100-step", 50, False)] * 2)
                        self.assertEqual(e2e.call_count, 2)


class Comparisons(unittest.TestCase):
    def test_frozen_observation_table_matches_the_observation_harness(self):
        source = (perf_baseline.ROOT / perf_observation_phases.HARNESS).read_text(encoding="utf-8")
        declared = re.search(r'const USAGE: &str = ".*?; CASE = ([a-z|-]+);', source)[1].split("|")
        self.assertEqual(list(perf_compare.FROZEN_OBSERVATION), declared)
        # The pinned phase recipe predates observe-sparse and observe-clustered.
        phases = perf_observation_phases.CASES
        self.assertEqual(tuple(perf_compare.FROZEN_OBSERVATION)[:len(phases)], phases)
        self.assertTrue(all(1 <= n <= 100000 for n in perf_compare.FROZEN_OBSERVATION.values()))

    def test_observation_cases_resolve_without_a_catalog_and_never_mix(self):
        with patch.object(perf_compare, "load_catalog") as load:
            plan, catalogs, selections = perf_compare.resolve_plan(
                {"baseline": "a", "candidate": "b"}, ["observe-wide", "observe-empty"], "ci")
            self.assertEqual((plan, catalogs, selections), ({"observe-wide": 10000, "observe-empty": 100000}, None, None))
            with self.assertRaisesRegex(ValueError, "perf_observation executables"):
                perf_compare.resolve_plan({"baseline": "a", "candidate": "b"}, ["observe-wide", "observe"], "ci")
            load.assert_not_called()

    def copied(self, root, catalogs, *options):
        binary = root / "fixture.exe"
        binary.write_bytes(b"not an executable")
        output = root / "attempt"
        argv = ["--baseline", str(binary), "--candidate", str(binary), "--output", str(output), *options]
        with patch.object(perf_compare, "load_catalog", side_effect=catalogs), patch.object(
                perf_compare, "command_output", return_value="fixture"), patch.object(
                perf_compare, "native", side_effect=lambda *a, **k: {
                    "case": a[1], "cpu_seconds": 1, "workload": {"timing": {"wallSeconds": 1}}}) as native, \
                contextlib.redirect_stdout(io.StringIO()):
            try:
                return perf_compare.main(argv), output, native
            except Exception as error:
                return error, output, native

    def test_copied_mode_defaults_to_the_ci_suite_and_rejects_mismatched_or_unknown_plans(self):
        with tempfile.TemporaryDirectory() as temporary:
            code, output, native = self.copied(Path(temporary), [catalog(), catalog()], "--repeats", "1")
            self.assertEqual(code, 0)
            self.assertEqual(json.loads((output / "manifest.json").read_text())["workloads"],
                             {"step-default": 100000, "observe": 1000, "official-100-step": 50})
            self.assertEqual(native.call_count, 12)
            self.assertIn("| `observe` | 1000 | — |", (output / "summary.md").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as temporary:
            code, output, _ = self.copied(Path(temporary), [catalog(), catalog()], "--repeats", "1",
                                          "--suite", "full", "--cases", "sandbox-16-bot")
            self.assertEqual(code, 0)
            self.assertEqual(json.loads((output / "manifest.json").read_text())["workloads"], {"sandbox-16-bot": 200})
        rows = [row("step-default", iterations=1, trace=32, legacy=True)]
        for name, catalogs, options, message in (
                ("mismatch", [catalog(), catalog(rows)], ("--cases", "step-default"), "plans differ"),
                ("unknown", [catalog(), catalog()], ("--cases", "step-bogus"), "--list")):
            with self.subTest(name), tempfile.TemporaryDirectory() as temporary:
                error, output, native = self.copied(Path(temporary), catalogs, *options)
                self.assertIsInstance(error, ValueError)
                self.assertIn(message, str(error))
                native.assert_not_called()
                self.assertFalse((output / "manifest.json").exists())
                self.assertIn(message, json.loads((output / "failure.json").read_text())["error"])


class BaseComparisons(unittest.TestCase):
    """--base orchestration with fake builds and children; no Cargo or Git writes."""

    def run_base(self, root, traces, *, exists=False, trace_error=None, options=()):
        output = root / "outputs" / "run"
        if exists:
            output.mkdir(parents=True)
        bins = {}
        builds = {}
        for label in ("baseline", "candidate"):
            path = root / f"{label}.exe"
            path.write_bytes(label.encode())
            bins[label] = path
            builds[label] = perf_build.Build(label, None, {"perf_baseline": path},
                                             {path.name: perf_baseline.digest(path)},
                                             {"commit": label[0] * 40, "tree": label[1] * 40})
        calls = {"trace": [], "profile": []}

        def trace(binary, case, iterations, directory, *, terminal=False, deadline=None):
            label = "baseline" if binary == bins["baseline"] else "candidate"
            calls["trace"].append((label, case, iterations, terminal, deadline))
            if trace_error and (label, case) == trace_error:
                raise RuntimeError("invalid/incomplete trace witness")
            return {"case": case, "sha256": traces.get((label, case), "same")}

        def run_native(binary, case, iterations, stderr_path, profile=False, **kwargs):
            if profile:
                calls["profile"].append(case)
            return {"case": case, "cpu_seconds": 1, "workload": {"timing": {"wallSeconds": 1}}}

        argv = ["--base", "origin/main", "--output", str(output), "--repeats", "1", *options]
        with patch.object(perf_compare, "ROOT", root), patch.object(
                perf_compare.perf_build, "require_outside_snapshot") as location, patch.object(
                perf_compare.perf_build, "prepare_local", return_value=builds) as prepare, patch.object(
                perf_compare, "load_catalog", return_value=catalog()), patch.object(
                perf_compare, "command_output", return_value="fixture"), patch.object(
                perf_compare, "capture_trace", side_effect=trace), patch.object(
                perf_compare, "native", side_effect=run_native), contextlib.redirect_stdout(io.StringIO()):
            try:
                result = perf_compare.main(argv)
            except Exception as error:
                result = error
        location.assert_called_once_with(root, output.resolve())
        return result, output, calls, prepare

    def test_exit_codes_distinguish_equal_changed_and_incomplete_runs(self):
        with tempfile.TemporaryDirectory() as temporary:
            code, output, calls, _ = self.run_base(Path(temporary), {})
            self.assertEqual(code, 0)
            self.assertEqual([c[1:4] for c in calls["trace"] if c[0] == "candidate"],
                             [("step-default", 32, False), ("observe", 32, False), ("official-100-step", 8, True)])
            self.assertEqual(calls["profile"], ["step-default"] * 2 + ["official-100-step"] * 2)
            comparison = json.loads((output / "comparison.json").read_text())
            self.assertEqual((comparison["complete"], comparison["exit_code"], comparison["behavior"]),
                             (True, 0, "equal"))
            self.assertTrue((output / "summary.json").exists())
            self.assertFalse((output / "failure.json").exists())
        with tempfile.TemporaryDirectory() as temporary:
            code, output, calls, _ = self.run_base(Path(temporary), {("candidate", "observe"): "changed"},
                                                   options=("--no-profiles",))
            self.assertEqual(code, perf_compare.CHANGED)
            self.assertEqual(calls["profile"], [])
            self.assertIn("**Behavior changed** in `observe`", (output / "summary.md").read_text(encoding="utf-8"))
            self.assertTrue((output / "summary.json").exists())
        with tempfile.TemporaryDirectory() as temporary:
            error, output, calls, _ = self.run_base(Path(temporary), {}, trace_error=("baseline", "observe"))
            self.assertIsInstance(error, RuntimeError)
            self.assertIn("observe", json.loads((output / "failure.json").read_text())["error"])
            comparison = json.loads((output / "comparison.json").read_text())
            self.assertFalse(comparison["complete"])
            self.assertEqual(comparison["parity"][1]["reading"], "Unavailable")
            self.assertFalse((output / "summary.json").exists())

    def test_deadline_starts_after_builds_and_reaches_every_child(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(
                perf_compare.time, "perf_counter", return_value=100.0):
            code, _, calls, _ = self.run_base(Path(temporary), {}, options=("--max-wall-seconds", "5"))
            self.assertEqual(code, 0)
            self.assertEqual({c[4] for c in calls["trace"]}, {105.0})

    def test_existing_output_is_refused_before_any_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            error, output, _, prepare = self.run_base(Path(temporary), {}, exists=True)
            self.assertIsInstance(error, FileExistsError)
            prepare.assert_not_called()
            self.assertFalse((output / "failure.json").exists())


class ProcessFailures(unittest.TestCase):
    def test_e2e_workloads_pin_the_historical_world_without_launching_children(self):
        expected = {"radius": 1000, "dynamicRadius": False, "foodCount": 350,
                    "ambientReplenishPerSecond": 5, "prey": {"count": 0}}
        for hosted in (False, True):
            with self.subTest(hosted=hosted), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                client = Mock(stdin=Mock(), stdout=io.BytesIO(), stderr=None, returncode=0)
                host = Mock(stdin=None, stdout=None, stderr=io.BytesIO())
                host.poll.return_value = None
                replies = [json.dumps({"protocol": "slither-client-json-v1", "result": {}}).encode(),
                           json.dumps({"protocol": "slither-client-json-v1",
                                       "result": {"frame": {"status": "waiting"}}}).encode()]
                if hosted:
                    replies.insert(0, b"Listening on ws://127.0.0.1:1/slither")
                with patch.object(perf_baseline.subprocess, "Popen", side_effect=[host, client] if hosted else [client]), patch.object(
                        perf_baseline.Lines, "read", side_effect=replies), patch.object(
                        perf_baseline, "stop"), patch.object(perf_baseline, "process_metrics", return_value={}):
                    perf_baseline.e2e(Path(sys.executable), 1, hosted, root)
                request = json.loads(client.stdin.write.call_args_list[0].args[0])
                if hosted:
                    world = json.loads((root / "host-config.json").read_text())["simulation"]["world"]
                else:
                    world = request["config"]["world"]
                self.assertEqual(world, expected)

    def test_native_and_trace_close_handles_after_stop_or_pipe_failure(self):
        class CloseFailure(io.BytesIO):
            def close(self):
                if not self.closed:
                    super().close()
                    raise OSError("cleanup witness")

        for entry in ("native", "trace"):
            for failure in ("stop", "pipe"):
                with self.subTest(entry=entry, failure=failure), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary)
                    stdout = CloseFailure() if failure == "pipe" else io.BytesIO()
                    child = Mock(stdin=None, stdout=stdout, stderr=None)
                    opened_errors = []

                    def spawn(*args, **kwargs):
                        opened_errors.append(kwargs["stderr"])
                        return child

                    try:
                        with patch.object(perf_baseline.subprocess, "Popen", side_effect=spawn), patch.object(
                                perf_baseline.Lines, "read", side_effect=TimeoutError("read witness")), patch.object(
                                perf_baseline, "stop", side_effect=OSError("cleanup witness") if failure == "stop" else None):
                            with self.assertRaisesRegex(OSError, "cleanup witness"):
                                if entry == "native":
                                    perf_baseline.native(Path(sys.executable), "observe", 1, root / "stderr.txt")
                                else:
                                    perf_baseline.capture_trace(Path(sys.executable), "observe", 1, root)
                        self.assertEqual(len(opened_errors), 1)
                        self.assertEqual([stdout.closed, opened_errors[0].closed], [True, True])
                    finally:
                        stdout.close()
                        for errors in opened_errors:
                            errors.close()

    def test_native_and_trace_close_stderr_after_spawn_failure(self):
        for entry in ("native", "trace"):
            with self.subTest(entry=entry), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                opened_errors = []

                def spawn(*args, **kwargs):
                    opened_errors.append(kwargs["stderr"])
                    raise OSError("spawn witness")

                try:
                    with patch.object(perf_baseline.subprocess, "Popen", side_effect=spawn):
                        with self.assertRaisesRegex(OSError, "spawn witness"):
                            if entry == "native":
                                perf_baseline.native(Path(sys.executable), "observe", 1, root / "stderr.txt")
                            else:
                                perf_baseline.capture_trace(Path(sys.executable), "observe", 1, root)
                    self.assertEqual(len(opened_errors), 1)
                    self.assertTrue(opened_errors[0].closed)
                finally:
                    for errors in opened_errors:
                        errors.close()

    def test_host_cleanup_survives_client_stop_or_pipe_failure(self):
        for failure in ("stop", "pipe"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                host = Mock(stdin=None, stdout=None, stderr=io.BytesIO(b"fatal witness\n"))
                client = Mock(stdin=Mock(), stdout=Mock(), stderr=None)
                host.poll.return_value = 0
                if failure == "pipe":
                    client.stdin.close.side_effect = OSError("client cleanup witness")

                def stop(process):
                    if failure == "stop" and process is client:
                        raise OSError("client cleanup witness")

                with patch.object(perf_baseline.subprocess, "Popen", side_effect=[host, client]), patch.object(
                        perf_baseline.Lines, "read", side_effect=[b"Listening on ws://127.0.0.1:1/slither",
                                                                 TimeoutError("request witness")]), patch.object(
                        perf_baseline, "stop", side_effect=stop) as stopped:
                    with self.assertRaisesRegex(OSError, "client cleanup witness"):
                        perf_baseline.e2e(Path(sys.executable), 1, True, Path(temporary))
                self.assertEqual(stopped.call_args_list[-1].args, (host,))
                client.stdout.close.assert_called_once()
                self.assertTrue(host.stderr.closed)
                self.assertIn("fatal witness", (Path(temporary) / "host-stderr.txt").read_text())

    def test_manifest_command_timeout_is_enforced(self):
        with patch.object(perf_baseline, "COMMAND_TIMEOUT_SECONDS", .05):
            with self.assertRaises(subprocess.TimeoutExpired):
                perf_baseline.command_output(sys.executable, "-c", "import time; time.sleep(1)")

    def test_manifest_timeout_records_failure_in_both_runners(self):
        with tempfile.TemporaryDirectory() as temporary:
            for module, options in [
                    (perf_baseline, ["--harness", sys.executable, "--cli", sys.executable]),
                    (perf_compare, ["--baseline", sys.executable, "--candidate", sys.executable])]:
                with self.subTest(runner=module.__name__):
                    output = Path(temporary) / module.__name__
                    # Both runners read the catalog before metadata; the fixture keeps that offline.
                    with patch.object(sys, "argv", [module.__name__, *options, "--output", str(output)]), patch.object(
                            module, "command_output", side_effect=subprocess.TimeoutExpired("git", 30)), patch.object(
                            module, "load_catalog", return_value=catalog()):
                        with self.assertRaises(subprocess.TimeoutExpired):
                            module.main()
                    self.assertEqual(json.loads((output / "failure.json").read_text())["type"], "TimeoutExpired")

    def test_host_diagnostics_keep_bytes_read_with_announcement(self):
        witness = b"Listening on ws://127.0.0.1:1/slither\nfatal witness\n"
        with subprocess.Popen([sys.executable, "-c", f"import os; os.write(2, {witness!r})"],
                              stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as process:
            process.wait(timeout=10)
            reader = perf_baseline.Lines(process.stderr)
            announcement = reader.read(time.perf_counter() + 10)
            self.assertEqual(reader.pending, b"fatal witness\n")
            output = io.BytesIO()
            output.write(announcement + b"\n")
            reader.drain(output)
            self.assertEqual(output.getvalue(), witness)

    def test_early_exit_retains_stderr(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "fails.py"
            script.write_text("import sys\nprint('failure witness', file=sys.stderr)\nsys.exit(3)\n")
            errors = root / "stderr.txt"
            with self.assertRaises(EOFError):
                perf_baseline.native(sys.executable, str(script), 1, errors)
            self.assertIn("failure witness", errors.read_text())

    def test_trace_archive_preserves_child_bytes_and_retains_failures(self):
        # Synthetic child output, not simulator traces: spans several pipe reads and has UTF-8.
        emitted = b"".join(json.dumps({"tick": i, "state": "x" * (i % 97), "note": "\u00e9"},
                                      ensure_ascii=False).encode() + b"\n" for i in range(3000))
        line_end = emitted.index(b"\n") + 1
        children = {"valid": ("data", 0), "failed": (f"data[:{line_end}]", 3),
                    "incomplete": (f"data[:{line_end + 5}]", 0)}
        for name, (written, code) in children.items():
            with self.subTest(child=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "trace.bin").write_bytes(emitted)
                script = root / f"{name}.py"
                script.write_text("from pathlib import Path\nimport sys\n"
                                  "data = (Path(__file__).parent / 'trace.bin').read_bytes()\n"
                                  "sys.stderr.write('failure witness')\n"
                                  f"sys.stdout.buffer.write({written})\nsys.exit({code})\n")
                if name != "valid":
                    with self.assertRaisesRegex(RuntimeError, "invalid/incomplete"):
                        perf_baseline.capture_trace(sys.executable, str(script), 1, root)
                    self.assertEqual(Path(f"{script}-trace-stderr.txt").read_text(), "failure witness")
                    continue
                record = perf_baseline.capture_trace(sys.executable, str(script), 1, root)
                archive = Path(f"{script}.jsonl.gz").read_bytes()
                self.assertEqual(gzip.decompress(archive), emitted)
                self.assertEqual(archive[4:8], bytes(4))
                self.assertEqual(record, {"case": str(script), "iterations": 1, "lines": 3000,
                                          "uncompressed_bytes": len(emitted),
                                          "sha256": hashlib.sha256(emitted).hexdigest()})

    @staticmethod
    def scenario(case="CASE", n=3, kind="step", **changes):
        """Synthetic scenario record sequence shaped like the harness's; `changes` replaces parts."""
        line = lambda **fields: json.dumps(fields, sort_keys=True).encode()
        parts = {"initial": [line(kind="initial", case=case)],
                 "warmup": [line(kind="warmup", tick=t) for t in range(2)],
                 "warmed": [line(kind="warmed")],
                 "operations": [line(kind=kind, iteration=i) for i in range(n)],
                 "end": [line(kind="end", case=case, operations=n, physicsTicks=n)],
                 "report": [line(kind="report", case=case, iterations=n, mode="trace")]}
        parts.update(changes)
        return [record for name in parts for record in parts[name]]

    def test_scenario_sequence_requires_every_stage_in_order(self):
        line = lambda **fields: json.dumps(fields, sort_keys=True).encode()
        step = lambda i: line(kind="step", iteration=i)
        valid = {"step": self.scenario(), "bot": self.scenario(kind="bot"),
                 "decision": self.scenario(kind="decision"), "no warmup ticks": self.scenario(warmup=[]),
                 "epoch between operations": self.scenario(operations=[step(0), line(kind="epoch"), step(1), step(2)])}
        invalid = {
            "only end and report": (self.scenario(initial=[], warmup=[], warmed=[], operations=[]), "expected initial"),
            "missing initial": (self.scenario(initial=[]), "expected initial"),
            "missing warmed": (self.scenario(warmed=[]), "expected warmup or warmed"),
            "operation before warmed": (self.scenario(warmup=[step(0)]), "expected warmup or warmed"),
            "skipped iteration": (self.scenario(operations=[step(0), step(2), step(3)]), "expected operation 1"),
            "duplicated iteration": (self.scenario(operations=[step(0), step(0), step(1)]), "expected operation 1"),
            "too few operations": (self.scenario(operations=[step(0), step(1)]), "end after 2 of 3"),
            "too many operations": (self.scenario(operations=[step(0), step(1), step(2), step(3)]), "expected operation 3 of 3"),
            "end count": (self.scenario(end=[line(kind="end", case="CASE", operations=2)]), "end after 3 of 3"),
            "end case": (self.scenario(end=[line(kind="end", case="other", operations=3)]), "case 'other'"),
            "missing end": (self.scenario(end=[]), "unexpected 'report'"),
            "missing report": (self.scenario(report=[]), "while expecting report"),
            "report case": (self.scenario(report=[line(kind="report", case="other", iterations=3, mode="trace")]), "trace-mode report"),
            "report count": (self.scenario(report=[line(kind="report", case="CASE", iterations=2, mode="trace")]), "trace-mode report"),
            "report mode": (self.scenario(report=[line(kind="report", case="CASE", iterations=3, mode="timing")]), "trace-mode report"),
            "after report": (self.scenario() + [step(3)], "after the report"),
            "not json": (self.scenario(warmup=[b"{"]), "not JSON"),
        }
        for name, records in valid.items():
            with self.subTest(name):
                sequence = perf_baseline.TraceSequence("CASE", 3)
                for record in records:
                    sequence.feed(record)
                self.assertIsNone(sequence.finish())
                self.assertEqual(sequence.end["operations"], 3)
        for name, (records, message) in invalid.items():
            with self.subTest(name):
                sequence = perf_baseline.TraceSequence("CASE", 3)
                for record in records:
                    sequence.feed(record)
                self.assertIn(message, sequence.finish() or "")

    def test_capture_validates_scenarios_and_keeps_legacy_capture_unchanged(self):
        incomplete = self.scenario(initial=[], warmup=[], warmed=[], operations=[])
        for name, records, error in (("complete", self.scenario(n=2), None),
                                     ("end and report only", incomplete, "expected initial")):
            with self.subTest(name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                script = root / "child.py"
                case = str(script)
                emitted = b"".join(record + b"\n" for record in records)
                (root / "trace.bin").write_bytes(emitted.replace(b'"CASE"', json.dumps(case).encode()))
                script.write_text("from pathlib import Path\nimport sys\n"
                                  "sys.stdout.buffer.write((Path(__file__).parent / 'trace.bin').read_bytes())\n")
                if error:
                    with self.assertRaisesRegex(RuntimeError, f"invalid/incomplete .*{error}"):
                        perf_baseline.capture_trace(sys.executable, case, 3, root, terminal=True)
                else:
                    result = perf_baseline.capture_trace(sys.executable, case, 2, root, terminal=True)
                    self.assertEqual((result["lines"], result["end"]["operations"]), (len(records), 2))
                # Legacy rows have no scenario sequence; any complete child output is archived.
                self.assertNotIn("end", perf_baseline.capture_trace(sys.executable, case, 3, root))

    def test_multi_megabyte_lines_are_buffered_in_linear_time(self):
        # Synthetic 64 MiB line in 64 KiB reads. Growing one bytes object per chunk took
        # about 5 s here (32 GiB copied); joining once per line takes well under 0.1 s.
        size, chunk = 64 << 20, 64 << 10
        data = memoryview(b"x" * size + b"\ntail")
        offsets = iter(range(0, len(data), chunk))

        class Pipe:
            def fileno(self):
                return -1

        reader = perf_baseline.Lines(Pipe())
        reader.available = lambda: chunk
        with patch.object(perf_baseline.os, "read",
                          side_effect=lambda fd, count: bytes(data[next(offsets, len(data)):][:count])):
            started = time.perf_counter()
            line = reader.read(time.perf_counter() + 60)
            elapsed = time.perf_counter() - started
        self.assertEqual((len(line), reader.pending), (size, b"tail"))
        self.assertLess(elapsed, 1.5)

    def test_only_untimed_bulk_reads_yield_before_sleeping(self):
        class Pipe:
            def fileno(self):
                return -1

        for spin, expected in ((0.0, [0.0005, 0.0005]), (0.005, [0, 0])):
            with self.subTest(spin=spin):
                reader = perf_baseline.Lines(Pipe(), spin_seconds=spin)
                polls = iter([0, 0, 3])
                reader.available = lambda: next(polls)
                # A frozen clock keeps both idle polls inside the spin window even
                # if the test thread is descheduled between them.
                with patch.object(perf_baseline.os, "read", return_value=b"ok\n"), patch.object(
                        perf_baseline.time, "sleep") as sleep, patch.object(
                        perf_baseline.time, "perf_counter", return_value=0.0):
                    self.assertEqual(reader.read(60.0), b"ok")
                self.assertEqual([call.args[0] for call in sleep.call_args_list], expected)
        self.assertEqual(perf_baseline.Lines(Pipe()).spin_seconds, 0)

    def test_timeout_terminates_and_waits_for_child(self):
        processes = []
        real_popen = subprocess.Popen

        def spawn(*args, **kwargs):
            process = real_popen(*args, **kwargs)
            processes.append(process)
            return process

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "waits.py"
            script.write_text("import time\ntime.sleep(60)\n")
            with patch.object(perf_baseline.subprocess, "Popen", spawn), patch.object(
                    perf_baseline.Lines, "read", side_effect=TimeoutError("deadline witness")):
                with self.assertRaises(TimeoutError):
                    perf_baseline.native(sys.executable, str(script), 1, root / "stderr.txt")
            self.assertEqual(len(processes), 1)
            self.assertIsNotNone(processes[0].poll())


if __name__ == "__main__":
    unittest.main()
