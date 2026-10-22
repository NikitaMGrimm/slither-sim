"""End-to-end checks for the synthetic worked movement study.

Builds the simulator and replay example through Cargo on first use.
"""

import base64
import gzip
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

import capture_quality
import slither_protocol as sp
import worked_study as study


def rewrite(source, target, edit):
    lines = gzip.decompress(source.read_bytes()).splitlines(keepends=True)
    with target.open("xb") as raw, gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as gz:
        gz.write(b"".join(edit(lines)))


class WireEncodingTest(unittest.TestCase):
    def test_spawn_and_multi_packet_frame_decode_with_imported_protocol(self):
        snake = {"head": {"x": 2500.0, "y": 0.0}, "heading": 3.0, "speed": 180.9375, "fam": 0.25,
                 "body": [{"x": 2500.0, "y": 0.0}, {"x": 2542.0, "y": 0.0}]}
        world = sp.World()
        world.feed(study.setup_packet(), 0)
        world.feed(study.spawn_packet(snake), 0)
        me = world.snakes[study.SELF_ID]
        expected = [[2542.0 + study.CENTER, study.CENTER], [2500.0 + study.CENTER, study.CENTER]]
        for point, truth in zip(me.pts, expected, strict=True):
            self.assertAlmostEqual(point[0], truth[0], places=9)
            self.assertAlmostEqual(point[1], truth[1], places=9)
        self.assertAlmostEqual(me.sp / 0.032, snake["speed"])
        self.assertAlmostEqual(me.fam, 0.25, places=6)
        packets = [study.rotation_packet(snake, 1.0), b"G\x00\x00", b"p"]
        self.assertEqual(sp.split(study.frame(7, packets)), packets)
        self.assertEqual(sp.split(study.frame(7, packets[:1])), packets[:1])

    def test_plan_must_be_local_simulator_pipeline_test(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "plan.json"
            path.write_text(json.dumps({**study.DEFAULT_PLAN, "dataOrigin": "official-server"}))
            with self.assertRaises(ValueError):
                study.load_plan(path)


class WorkedStudyTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name)
        cls.output = cls.root / "study"
        cls.results = study.run(cls.output)
        cls.archive = cls.output / study.ARCHIVE

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_pipeline_checks_pass_with_synthetic_labels(self):
        failed = [c for c in self.results["checks"] if not c["passed"]]
        self.assertEqual(failed, [])
        self.assertEqual(self.results["outcome"], "pipeline-checks-passed")
        self.assertEqual(self.results["dataOrigin"], "local-simulator")
        report = (self.output / "REPORT.md").read_text(encoding="utf-8")
        self.assertIn("SYNTHETIC LOCAL-SIMULATOR DATA", report)
        self.assertIn("No official-server fidelity is established", report)
        for name in ("replay.json", "replay-reported.json", "control-known-state.json",
                     "control-timing-plus7ms.json", "control-heading-plus50mrad.json"):
            self.assertTrue((self.output / "fixtures" / name).is_file(), name)
        self.assertEqual(sorted(p.name for p in (self.output / "raw").iterdir()), ["movement.jsonl.gz"])

    def test_archive_is_verified_and_pins_plan_and_collector(self):
        audit = capture_quality.audit_capture(self.archive)
        self.assertEqual((audit["status"], audit["issues"], audit["warnings"]), ("verified", [], []))
        plan_bytes = (self.output / "plan.json").read_bytes()
        plan_sha = study.sha256(plan_bytes)
        self.assertEqual((self.output / "plan.sha256").read_text(encoding="ascii"), plan_sha + "\n")
        header = audit["header"]
        self.assertEqual(header["dataOrigin"], "local-simulator")
        self.assertTrue(header["synthetic"])
        self.assertEqual(header["captureProvenance"]["planSha256"], plan_sha)
        self.assertEqual(header["captureProvenance"]["collectorSha256"], study.file_sha256(study.__file__))
        sources = header["captureProvenance"]["sources"]["files"]
        self.assertEqual(sources["tools/fidelity/prepare-fidelity-replay.py"],
                         study.file_sha256(study.HERE / "prepare-fidelity-replay.py"))

    def test_ledger_has_one_completed_attempt(self):
        events = [json.loads(line) for line in (self.output / "attempts.jsonl").read_text().splitlines()]
        self.assertEqual([e["event"] for e in events], ["started", "finished"])
        plan_sha = study.sha256((self.output / "plan.json").read_bytes())
        self.assertTrue(all(e["planSha256"] == plan_sha and e["attemptId"] == "attempt-1" for e in events))
        self.assertEqual((events[1]["status"], events[1]["exitCode"], events[1]["captures"]),
                         ("completed", 0, [study.ARCHIVE]))

    def test_rerun_is_byte_identical_and_refuses_overwrite(self):
        before = {p: p.read_bytes() for p in self.output.rglob("*") if p.is_file()}
        with self.assertRaises(FileExistsError):
            study.run(self.output)
        self.assertEqual({p: p.read_bytes() for p in self.output.rglob("*") if p.is_file()}, before)
        again = self.root / "again"
        study.run(again)
        for path in before:
            relative = path.relative_to(self.output)
            if relative.as_posix() != "attempts.jsonl":  # wall-clock ledger times differ
                self.assertEqual((again / relative).read_bytes(), before[path], relative)

    def test_tampered_archives_are_rejected_before_export(self):
        def flip_frame(lines):
            frames = [i for i, line in enumerate(lines) if line.startswith(b'["i"')]
            record = json.loads(lines[frames[100]])
            data = bytearray(base64.b64decode(record[2]))
            data[-1] ^= 0x10
            record[2] = base64.b64encode(bytes(data)).decode()
            lines[frames[100]] = (json.dumps(record, separators=(",", ":")) + "\n").encode()
            return lines

        def retime_send(lines):
            outgoing = next(i for i, line in enumerate(lines) if line.startswith(b'["o"'))
            record = json.loads(lines[outgoing])
            record[1] += 0.5  # stays ordered, but no longer matches the receipt
            lines[outgoing] = (json.dumps(record, separators=(",", ":")) + "\n").encode()
            return lines

        cases = {"bit-flip": (flip_frame, "terminal_integrity_mismatch"),
                 "retimed-send": (retime_send, "terminal_integrity_mismatch"),
                 "no-terminal": (lambda lines: lines[:-1], "missing_terminal_receipt")}
        for label, (edit, issue) in cases.items():
            with self.subTest(label):
                path = self.root / f"{label}.jsonl.gz"
                rewrite(self.archive, path, edit)
                audit = capture_quality.audit_capture(path)
                self.assertEqual(audit["status"], "quarantine")
                self.assertIn(issue, audit["issues"])
                with self.assertRaisesRegex(ValueError, "not verified"):
                    study.verified_export(path, label)
        truncated = self.root / "truncated.jsonl.gz"
        truncated.write_bytes(self.archive.read_bytes()[:-20])
        with self.assertRaisesRegex(ValueError, "not verified"):
            study.verified_export(truncated, "truncated")
        # The ungated exporter accepts a bit-flipped archive, which is why the gate exists.
        self.assertEqual(self.results["tamperControls"]["tamperedFrame"]["exporterWithoutAuditGate"], "accepted")

    def test_negative_controls_separate_from_known_state(self):
        replays = self.results["replays"]
        baseline = replays["known-state"]["anchorMeanError"]
        self.assertLess(replays["known-state"]["anchorMaxError"], study.KNOWN_STATE_ANCHOR_TOLERANCE)
        for label in ("timing-plus7ms", "timing-minus7ms", "heading-plus50mrad"):
            self.assertGreater(replays[label]["anchorMeanError"], study.NEGATIVE_CONTROL_MIN_ERROR)
            self.assertGreater(replays[label]["anchorMeanError"], 100 * baseline)
        known = json.loads((self.output / "fixtures/control-known-state.json").read_text())
        self.assertIn("Unavailable from any real capture", known["provenance"]["control"])

    def test_initial_heading_uncertainty_is_reported_not_hidden(self):
        estimates = self.results["initialState"]["estimates"]
        self.assertEqual(set(estimates), {"anchors3", "anchors4", "reported"})
        # Exported fixtures carry real reconstruction error; only the control removes it.
        self.assertGreater(estimates["anchors3"]["absoluteError"], 0.01)
        self.assertGreater(self.results["replays"]["anchors3"]["anchorMeanError"],
                           self.results["replays"]["known-state"]["anchorMeanError"])
        self.assertNotEqual(estimates["reported"]["speedError"], 0.0)

    @unittest.skipUnless(importlib.util.find_spec("jsonschema"), "evidence.py needs the tools group")
    def test_parent_evidence_audit_accepts_output(self):
        import evidence

        self.assertEqual(evidence.read_plan(self.output)["kind"], "pipeline-test")
        self.assertEqual(evidence.audit_study(self.output),
                         {"status": "verified", "issues": [], "attempts": 1, "captures": 1})
        bundle = self.root / "worked-study.tar.gz"
        evidence.package_study(self.output, bundle)
        evidence.verify_bundle(bundle)


if __name__ == "__main__":
    unittest.main()
