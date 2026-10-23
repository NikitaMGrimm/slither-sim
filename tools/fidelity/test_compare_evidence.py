import copy
import base64
import gzip
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import compare_evidence as ce
import evidence
import test_evidence as evidence_fixture
import test_capture_quality as capture_fixture


def submission(n=1):
    return dict(schemaVersion=1, comparisonId="synthetic-comparison", independence="Synthetic test batches; not real evidence.",
                context="Matched synthetic scenario, size and controls.", alpha=.05, minUnits=4, resamples=4095, seed=7,
                metrics=[dict(id="prey-gap", name="preyServerGapMs", minEffect=1., minSamples=2)],
                **{group: [dict(source=f"{group}-{i}.tar.gz", sha256=hashlib.sha256(f"{group}-{i}".encode()).hexdigest())
                           for i in range(n)] for group in ("baseline", "candidate")})


def unit(key, value=100., start=0., profile="same"):
    return dict(bundleSha256=hashlib.sha256(key.encode()).hexdigest(), studyId=key, planSha256="0" * 64,
                profile=profile, identities=[dict(archiveSha256=key, payloadSha256=key, captureId=key, captureBatchId=key)],
                intervals=[dict(server={"ip": key, "port": 444}, start=start, end=start + 1000.)], metrics={"prey-gap": value})


class ComparisonTests(unittest.TestCase):
    def run_comparison(self, value, units):
        paths = {item["source"]: Path(item["source"]) for group in ("baseline", "candidate") for item in value[group]}
        with patch.object(ce, "summarize_bundle", side_effect=lambda p, *_: units[p.name]):
            return ce.compare_submission(value, paths, submission_sha256="a" * 64)

    def cohort(self, n=6, candidate=130.):
        value = submission(n)
        units = {f"{group}-{i}.tar.gz": unit(f"{group}-{i}", 100. if group == "baseline" else candidate)
                 for group in ("baseline", "candidate") for i in range(n)}
        return value, units

    def test_independent_batch_shift_and_equal_negative_control(self):
        value, units = self.cohort()
        result = self.run_comparison(value, units)
        self.assertEqual(result["status"], "drift_detected", result["metrics"])
        self.assertEqual(result["metrics"][0]["test"]["difference"], 30.)
        self.assertAlmostEqual(result["metrics"][0]["adjustedPValue"], 2 / 924)
        self.assertEqual(result["scientificStatus"], "unreviewed")
        value, units = self.cohort(candidate=100.)
        result = self.run_comparison(value, units)
        self.assertEqual(result["status"], "no_drift_detected")
        self.assertTrue(any("equivalence" in s for s in result["limits"]))

    def test_effect_gate_and_missing_data_do_not_become_passes(self):
        value, units = self.cohort()
        value["metrics"][0]["minEffect"] = 40.
        self.assertEqual(self.run_comparison(value, units)["status"], "no_drift_detected")
        units["candidate-0.tar.gz"]["metrics"]["prey-gap"] = None
        result = self.run_comparison(value, units)
        self.assertEqual(result["status"], "inconclusive")
        self.assertNotIn("test", result["metrics"][0])
        value, units = self.cohort(n=3)
        self.assertEqual(self.run_comparison(value, units)["status"], "inconclusive")

    def test_all_predeclared_endpoints_stay_in_holm_family(self):
        value, units = self.cohort(n=4)
        value["metrics"].append(dict(id="map-gap", name="minimapServerGapMs", minEffect=1., minSamples=2))
        for row in units.values():
            row["metrics"]["map-gap"] = None
        result = self.run_comparison(value, units)
        self.assertAlmostEqual(result["metrics"][0]["test"]["pValue"], 2 / 70)
        self.assertAlmostEqual(result["metrics"][0]["adjustedPValue"], 4 / 70)
        self.assertEqual(result["status"], "inconclusive")

    def test_reused_payload_batch_identity_and_changed_profile_are_incomparable(self):
        for key in ("archiveSha256", "payloadSha256", "captureId", "captureBatchId"):
            value, units = self.cohort()
            units["candidate-0.tar.gz"]["identities"][0][key] = "baseline-0"
            result = self.run_comparison(value, units)
            self.assertEqual(result["status"], "incomparable")
            self.assertIn("reused_" + key, result["reasons"])
        value, units = self.cohort()
        units["candidate-0.tar.gz"]["profile"] = "different-collector-or-settings"
        self.assertEqual(self.run_comparison(value, units)["status"], "incomparable")

    def test_concurrent_same_server_units_are_not_independent(self):
        value, units = self.cohort()
        units["candidate-0.tar.gz"]["intervals"][0]["server"] = dict(ip="baseline-0", port=444)
        result = self.run_comparison(value, units)
        self.assertIn("overlapping_same_server_acquisitions", result["reasons"])
        self.assertEqual(result["status"], "incomparable")

    def test_submission_schema_rejects_unknowns_reuse_and_unbounded_inputs(self):
        for key, bad in (("schemaVersion", True), ("schemaVersion", 2), ("alpha", 0), ("alpha", float("nan")),
                         ("minUnits", True), ("minUnits", 3), ("resamples", 65536), ("seed", -1), ("context", "")):
            value = submission(); value[key] = bad
            with self.subTest(key=key, value=bad), self.assertRaises(ValueError):
                ce.validate_submission(value)
        for mutate in (lambda v: v.update(unexpected=1),
                       lambda v: v["candidate"].extend(copy.deepcopy(v["baseline"])),
                       lambda v: v["metrics"].append(copy.deepcopy(v["metrics"][0])),
                       lambda v: v["metrics"][0].update(name="untrusted-code")):
            value = submission(); mutate(value)
            with self.assertRaises(ValueError):
                ce.validate_submission(value)

    def test_minimap_metric_uses_joint_full_delta_clock_and_exposure_gate(self):
        epoch = dict(wireVersion=19, minimapUpdateGapsMs=dict(server=dict(n=3, median=2500)),
                     commands=[dict(command="M", gapsMs=dict(server=dict(n=1, median=5000)))])
        self.assertEqual(ce._metric(dict(connections=[epoch]), {}, "minimapServerGapMs", 3), 2500)
        self.assertIsNone(ce._metric(dict(connections=[epoch]), {}, "minimapServerGapMs", 4))

    def test_fresh_output_only_and_report_keeps_source_identities(self):
        value, units = self.cohort()
        report = self.run_comparison(value, units)
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "report"
            ce.write_report(output, report)
            self.assertEqual(json.loads((output / "report.json").read_text(encoding="utf-8")), report)
            self.assertIn("not equivalence", (output / "REPORT.md").read_text(encoding="utf-8"))
            with self.assertRaises(FileExistsError):
                ce.write_report(output, report)

    def test_real_verified_bundle_snapshot_and_hash_rejection(self):
        helper = evidence_fixture.EvidenceTests()
        helper.setUp()
        self.addCleanup(helper.doCleanups)
        helper.complete()
        bundle = helper.directory / "study.tar.gz"
        evidence.package_study(helper.study, bundle)
        digest = ce.pc.sha256(bundle)
        with evidence.verified_bundle(bundle) as (root, manifest):
            self.assertEqual(manifest["audit"]["status"], "verified")
            self.assertTrue((root / "plan.json").is_file())
        self.assertFalse(root.exists())
        result = ce.summarize_bundle(bundle, digest, submission()["metrics"])
        self.assertEqual(result["bundleSha256"], digest)
        self.assertEqual(result["metrics"]["prey-gap"], None)
        self.assertEqual(len(result["identities"]), 1)
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            ce.summarize_bundle(bundle, "0" * 64, submission()["metrics"])

    def test_complete_capture_to_bundle_to_batch_comparison_detects_synthetic_change(self):
        # Independent synthetic invocations, not actual server evidence. Unlike
        # the scalar unit tests above, every value here is decoded from raw j.
        value = submission(6)
        paths = {}
        for index, (group, item) in enumerate((g, row) for g in ("baseline", "candidate") for row in value[g]):
            helper = evidence_fixture.EvidenceTests()
            helper.setUp()
            self.addCleanup(helper.doCleanups)
            helper.complete()
            archive = helper.study / "raw/known.jsonl.gz"
            rows = [json.loads(line) for line in gzip.decompress(archive.read_bytes()).splitlines()][:-1]
            rows[0]["wall"] += index * 10000
            gap = 100 if group == "baseline" else 200
            def incoming(t, elapsed, packet):
                return ["i", t, base64.b64encode(elapsed.to_bytes(2, "big") + packet).decode()]
            rows += [incoming(40, 10, b"y\x00\x01" + bytes(17)),
                     incoming(40 + gap, gap, b"j\x00\x01" + bytes(4)),
                     incoming(40 + 2 * gap, gap, b"j\x00\x01" + bytes(4)),
                     incoming(40 + 3 * gap, gap, b"j\x00\x01" + bytes(4))]
            writer = capture_fixture.CaptureQualityTests()
            writer.setUp()
            self.addCleanup(writer.doCleanups)
            collector = rows[0]["captureProvenance"]["collectorSha256"]
            made = writer.write(rows=rows, mutate=lambda r: r.update(t=41 + 3 * gap, collectorEndSha256=collector))
            archive.write_bytes(made.read_bytes())
            bundle = helper.directory / "unit.tar.gz"
            evidence.package_study(helper.study, bundle)
            item["sha256"] = ce.pc.sha256(bundle)
            paths[item["source"]] = bundle
        raw = json.dumps(value).encode()
        result = ce.compare_submission(value, paths, submission_sha256=hashlib.sha256(raw).hexdigest())
        self.assertEqual(result["status"], "drift_detected")
        self.assertEqual(result["metrics"][0]["samples"], dict(baseline=[100.] * 6, candidate=[200.] * 6))
        self.assertAlmostEqual(result["metrics"][0]["test"]["pValue"], 2 / 924)


if __name__ == "__main__":
    unittest.main()
