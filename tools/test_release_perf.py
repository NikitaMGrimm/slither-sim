"""Direct-release regressions with synthetic files and mocked builds/sampling only."""

import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import zipfile

import perf_build
import perf_compare
import perf_stats
import release_perf
import release_perf_archive
from test_perf_baseline import catalog, row


def request():
    measurement = {"repository": "NikitaMGrimm/slither-sim", "protocol": perf_stats.PLAN,
                   "baseline": {"tag": "v0.3.1", "commit": "a" * 40, "tree": "b" * 40},
                   "candidate": {"commit": "c" * 40, "tree": "d" * 40},
                   "harness": {"commit": "e" * 40, "tree": "f" * 40},
                   "config": {"runner": "windows-latest", "toolchain": "stable", "features": list(release_perf.FEATURES),
                              "profile": "release", "release_settings": release_perf.RELEASE_SETTINGS,
                              "jobs": 2, "suite": "ci", "blocks": 1, "job_seconds": 1800, "stage_seconds": 1500, "reserve_seconds": 90}}
    return {"schema": release_perf.SCHEMA, "measurement": measurement, "key": release_perf.request_key(measurement),
            "run": 42, "attempt": 1, "published_release_id": 7, "target": {"number": 90, "head": "1" * 40}}


class DirectComparison(unittest.TestCase):
    def test_cross_language_input_key_and_publisher_regressions(self):
        result = subprocess.check_output(["node", "-e",
            "const x=require('./tools/release_perf_request.cjs');process.stdout.write(x.key(JSON.parse(process.argv[1])));",
            json.dumps(request()["measurement"])], cwd=release_perf.ROOT, text=True)
        self.assertEqual(result, request()["key"])
        subprocess.run(["node", "--test", "tools/release_perf.test.cjs"], cwd=release_perf.ROOT, check=True)

    def test_request_key_changes_with_measurement_inputs_but_not_publication_target(self):
        pinned = request()
        for label, name, value in (("baseline", "commit", "2" * 40), ("baseline", "tag", "v0.3.0"),
                                   ("candidate", "commit", "3" * 40), ("harness", "commit", "4" * 40),
                                   ("config", "stage_seconds", 7200)):
            changed = copy.deepcopy(pinned["measurement"])
            changed[label][name] = value
            self.assertNotEqual(release_perf.request_key(changed), pinned["key"])
        pinned["target"] = {"number": 99, "head": "9" * 40}
        self.assertEqual(release_perf.request_key(pinned["measurement"]), pinned["key"])

    def test_exact_commits_shared_overlay_and_incompatible_builds(self):
        for incompatible in (False, True):
            with self.subTest(incompatible=incompatible), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                output = root / "out"
                output.mkdir()
                pinned = request()
                calls = []

                def resolve(_root, ref):
                    pin = next(p for p in pinned["measurement"].values() if isinstance(p, dict) and p.get("commit") == ref) if ref != "HEAD" else pinned["measurement"]["harness"]
                    return pin["commit"], pin["tree"]

                def command_output(args, cwd, environment=None, timeout=120):
                    args = list(map(str, args))
                    calls.append(args)
                    if args[:3] == ["git", "worktree", "add"]:
                        checkout = Path(args[4])
                        checkout.mkdir()
                        for filename in (*perf_build.HARNESS, "tests/fixtures/synthetic.json"):
                            path = checkout / filename
                            path.parent.mkdir(parents=True, exist_ok=True)
                            path.write_text('#[test]\nfn synthetic() {}\n' if filename.endswith(".rs") else "{}\n", encoding="utf-8")
                        (checkout / "Cargo.lock").write_text("# fixture\n", encoding="utf-8")
                    elif args[:3] == ["git", "worktree", "remove"]:
                        shutil.rmtree(args[4])
                    return "" if args[:3] == ["git", "status", "--porcelain"] else "synthetic tool"

                def command(args, cwd, log, environment=None, timeout=600):
                    args = list(map(str, args))
                    calls.append(args)
                    if args[:2] == ["cargo", "build"]:
                        self.assertEqual(args[4:7], list(release_perf.FEATURES))
                        if incompatible and Path(cwd).name == "baseline":
                            log.write_text("old source does not support the harness\n", encoding="utf-8")
                            return 1
                        for example in ("perf_baseline", "fidelity_metrics"):
                            binary = Path(environment["CARGO_TARGET_DIR"]) / "release/examples" / (example + perf_build.SUFFIX)
                            binary.parent.mkdir(parents=True, exist_ok=True)
                            binary.write_bytes(b"synthetic, never executable")
                    if args[:2] == ["cargo", "test"]:
                        self.assertIn("--no-default-features", args)
                        log.write_text("\n".join(f"Running tests/{name}.rs\ntest synthetic ... ok" for name in perf_build.TESTS), encoding="utf-8")
                    elif "fidelity_metrics" in args[0]:
                        log.write_text('{"schema":1,"metrics":[]}\n', encoding="utf-8")
                    else:
                        log.write_text("synthetic build log\n", encoding="utf-8")
                    return 0

                with patch.object(perf_build, "resolve", side_effect=resolve), patch.object(
                        perf_build, "output", side_effect=command_output), patch.object(
                        perf_build, "command", side_effect=command), patch.object(release_perf, "measure") as sample:
                    report = release_perf.collect(pinned, output, 0, root)
                self.assertEqual(report["state"], "unavailable" if incompatible else "complete")
                self.assertEqual(sample.call_count, 0 if incompatible else 1)
                self.assertEqual(report["request"], pinned)
                self.assertEqual(report["tests"]["baseline"]["physics::synthetic"], "ok")
                self.assertTrue((output / "baseline-build.log").exists())
                self.assertFalse(any("^2" in part for args in calls for part in args))
                self.assertFalse((output / "work").exists())
                if not incompatible:
                    a, b = (report["identities"][label] for label in ("baseline", "candidate"))
                    self.assertEqual(a["overlay_inputs"], b["overlay_inputs"])
                    self.assertNotEqual(a["target"], b["target"])
                    self.assertTrue((output / "binaries/bin-baseline").is_dir())

    def test_shared_sampler_uses_v2_deadline_identity_and_excludes_unavailable_parity(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            binary = output / "fake"
            binary.write_bytes(b"never execute")
            builds = {label: SimpleNamespace(binaries={"perf_baseline": binary}) for label in ("baseline", "candidate")}
            c = catalog([row("one"), row("two")])
            report = {"request": request()}

            def sequential(binaries, hashes, workloads, path, deadline, plan):
                self.assertEqual([w.name for w in workloads], ["one"])
                self.assertEqual(deadline, 810)
                self.assertEqual(plan["comparison"], request())
                self.assertEqual(plan["looks"], perf_stats.LOOKS)
                self.assertEqual(len(plan["looks"]), 14)
                self.assertEqual(plan["workloads"][0]["operations_per_block"], 100)
                self.assertEqual(plan["workloads"][0]["definitionSha256"], c.workloads[0].fingerprint)
                perf_compare.write_json(path / "plan.json", plan)
                return {"one": {**perf_stats.final([], 0, "not_started"), "n_attempted": 0}}

            with patch.object(release_perf, "load_catalog", return_value=c), patch.object(
                    perf_compare, "parity", return_value=[{"case": "one", "reading": "Equal"}, {"case": "two", "reading": "Unavailable"}]), patch.object(
                    perf_compare, "sequential", side_effect=sequential), patch.object(
                    release_perf.time, "perf_counter", return_value=100), patch.object(release_perf.time, "monotonic", return_value=1000):
                release_perf.measure(report, builds, output, 0)
            self.assertEqual(report["performance"][1]["label"], "Unavailable")

    def test_definition_seed_operation_and_slot_mismatches_never_sample(self):
        base = catalog([row("one")])
        for candidate in (catalog([row("one", definition={"different": True})]), catalog([row("one")], seed=8),
                          catalog([row("one", operation="observe")]), catalog([row(f"case{i}") for i in range(11)])):
            with tempfile.TemporaryDirectory() as temporary:
                binary = Path(temporary) / "fake"
                binary.write_bytes(b"never execute")
                builds = {label: SimpleNamespace(binaries={"perf_baseline": binary}) for label in ("baseline", "candidate")}
                with patch.object(release_perf, "load_catalog", side_effect=[base, candidate]), patch.object(
                        perf_compare, "sequential") as sample, self.assertRaises(ValueError):
                    release_perf.measure({"request": request()}, builds, Path(temporary), 0)
                sample.assert_not_called()

    def test_request_archive_rejects_paths_duplicates_links_and_oversized_data(self):
        for entries in ([('request.json', b'{}')], [('../request.json', b'{}')],
                        [('request.json', b'{}'), ('request.json', b'{}')], [('request.json', b'x' * 20001)]):
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with zipfile.ZipFile(root / "request.zip", "w") as archive:
                    for name, data in entries:
                        archive.writestr(name, data)
                if len(entries) == 1 and entries[0] == ('request.json', b'{}'):
                    release_perf_archive.request(root / "request.zip", root / "data")
                    self.assertEqual((root / "data/request.json").read_bytes(), b'{}')
                else:
                    with self.assertRaises(ValueError):
                        release_perf_archive.request(root / "request.zip", root / "data")


if __name__ == "__main__":
    unittest.main()
