import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

import evidence
import test_capture_quality as capture_fixture


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.plan = json.loads((evidence.ROOT / "evidence/templates/movement-plan.json").read_text())
        self.plan.update(kind="pipeline-test", dataOrigin="local-simulator", studyId="test-study")
        self.plan_source = self.directory / "source-plan.json"
        self.plan_source.write_text(json.dumps(self.plan), encoding="utf-8")
        self.study = evidence.create_study(self.plan_source, self.directory / "study")
        self.base = {"schemaVersion": 1, "attemptId": "attempt-1", "studyId": "test-study",
                     "planSha256": evidence.sha256((self.study / "plan.json").read_bytes())}

    def event(self, kind, **extra):
        event = {**self.base, "event": kind, "time": "2026-10-22T12:00:00Z"}
        event.update({"command": ["offline-fixture"]} if kind == "started" else
                     {"status": "completed", "exitCode": 0, "captures": ["raw/known.jsonl.gz"]})
        return {**event, **extra}

    def complete(self):
        helper = capture_fixture.CaptureQualityTests()
        helper.setUp()
        self.addCleanup(helper.doCleanups)
        header = helper.header()
        header["dataOrigin"] = "local-simulator"
        digest = hashlib.sha256((evidence.ROOT / "tools/fidelity/slither-headless.mjs").read_bytes()).hexdigest()
        header["captureProvenance"].update(
            collectorSha256=digest, planSha256=self.base["planSha256"],
            options={"scenario": "movement", "seconds": 60, "bots": 1},
        )
        archive = helper.write(helper.records(header), mutate=lambda receipt: receipt.update(collectorEndSha256=digest))
        (self.study / "raw/known.jsonl.gz").write_bytes(archive.read_bytes())
        with evidence.study_lock(self.study):
            evidence.append_event(self.study, self.event("started"))
            evidence.append_event(self.study, self.event("finished"))

    def rewrite_capture_header(self, edit):
        path = self.study / "raw/known.jsonl.gz"
        records = [json.loads(line) for line in gzip.decompress(path.read_bytes()).splitlines()]
        edit(records[0])
        prefix = b"".join((json.dumps(row, separators=(",", ":")) + "\n").encode() for row in records[:-1])
        records[-1]["integrity"].update(sha256=evidence.sha256(prefix), payloadBytes=len(prefix))
        path.write_bytes(gzip.compress(prefix + (json.dumps(records[-1]) + "\n").encode()))

    def test_plan_bytes_are_frozen_and_no_existing_output_is_overwritten(self):
        self.assertEqual((self.study / "plan.json").read_bytes(), self.plan_source.read_bytes())
        digest = (self.study / "plan.sha256").read_bytes()
        self.assertEqual(len(digest), 65)
        self.assertTrue(digest.endswith(b"\n"))
        self.assertNotIn(b"\r", digest)
        with self.assertRaises(FileExistsError):
            evidence.create_study(self.plan_source, self.study)
        with (self.study / "plan.json").open("a") as stream:
            stream.write("\n")
        with self.assertRaisesRegex(ValueError, "frozen plan"):
            evidence.read_plan(self.study)

    def test_strict_json_and_plan_schema_reject_ambiguous_metadata(self):
        for text in ('{"schemaVersion":1,"schemaVersion":2}', '{"number":NaN}'):
            self.plan_source.write_text(text)
            with self.assertRaises(ValueError):
                evidence.read_json(self.plan_source)
        for field, value in (("schemaVersion", 2), ("question", ""), ("unexpected", True)):
            bad = {**self.plan, field: value}
            with self.assertRaises(ValueError):
                evidence.validate(bad, "study-plan")

    def test_unfinished_attempt_and_missing_planned_attempt_remain_incomplete(self):
        self.assertEqual(evidence.audit_study(self.study)["status"], "incomplete")
        with evidence.study_lock(self.study):
            evidence.append_event(self.study, self.event("started"))
        self.assertIn("failed_or_unfinished_attempts", evidence.audit_study(self.study)["issues"])
        with self.assertRaisesRegex(ValueError, "unplanned"):
            evidence.append_event(self.study, self.event("started", attemptId="replacement"))

    def test_invalid_event_is_rejected_before_appending(self):
        evidence.append_event(self.study, self.event("started"))
        previous = (self.study / "attempts.jsonl").read_bytes()
        for extra in ({"exitCode": 1}, {"captures": ["../escape"]}, {"time": "2026-10-21T12:00:00Z"}):
            with self.assertRaises(ValueError):
                evidence.append_event(self.study, self.event("finished", **extra))
            self.assertEqual((self.study / "attempts.jsonl").read_bytes(), previous)
        with (self.study / "attempts.jsonl").open("ab") as stream:
            stream.write(b"{")
        with self.assertRaisesRegex(ValueError, "truncated"):
            evidence.audit_study(self.study)

    def test_capture_origin_plan_and_raw_membership_are_checked(self):
        self.complete()
        self.assertEqual(evidence.audit_study(self.study)["status"], "verified")
        path = self.study / "raw/known.jsonl.gz"
        original = path.read_bytes()
        path.write_bytes(gzip.compress(gzip.decompress(original).replace(b'"local-simulator"', b'"official-server"')))
        self.assertEqual(evidence.audit_study(self.study)["status"], "quarantined")
        path.write_bytes(original)
        (self.study / "raw/extra.jsonl.gz").write_bytes(original)
        result = evidence.audit_study(self.study)
        self.assertIn("raw_capture_membership_mismatch", result["issues"])

    def test_completed_study_roundtrips_with_source_snapshots(self):
        self.complete()
        output = self.directory / "bundle.tar.gz"
        manifest = evidence.package_study(self.study, output)
        self.assertEqual(manifest["audit"]["status"], "verified")
        self.assertEqual(manifest["scientificStatus"], "unreviewed")
        self.assertEqual(manifest, evidence.verify_bundle(output))
        self.assertTrue(any(entry["path"] == "software/tools/fidelity/evidence.py" for entry in manifest["files"]))
        before = output.read_bytes()
        with self.assertRaises(FileExistsError):
            evidence.package_study(self.study, output)
        self.assertEqual(output.read_bytes(), before)

    def test_request_origin_must_match_the_frozen_plan(self):
        self.plan["capture"]["origin"] = "http://localhost:8080"
        self.plan_source.write_text(json.dumps(self.plan), encoding="utf-8")
        self.study = evidence.create_study(self.plan_source, self.directory / "with-origin")
        self.base["planSha256"] = evidence.sha256((self.study / "plan.json").read_bytes())
        self.complete()
        self.assertIn("capture_config_mismatch:raw/known.jsonl.gz", evidence.audit_study(self.study)["issues"])
        self.rewrite_capture_header(lambda header: header["captureProvenance"]["options"].update(
            origin="http://localhost:8080"))
        self.assertEqual(evidence.audit_study(self.study)["status"], "verified")
        self.rewrite_capture_header(lambda header: header["captureProvenance"]["options"].update(
            origin="http://localhost:8081"))
        self.assertEqual(evidence.audit_study(self.study)["status"], "quarantined")

    def test_bundle_cannot_replace_missing_software_with_the_verifiers_checkout(self):
        self.complete()
        output = self.directory / "bundle.tar.gz"
        evidence.package_study(self.study, output)
        with tarfile.open(output, "r:gz") as source:
            files = {member.name: source.extractfile(member).read() for member in source
                     if not member.name.startswith("software/")}
        manifest = json.loads(files["manifest.json"])
        manifest["files"] = [entry for entry in manifest["files"]
                             if not entry["path"].startswith("software/")]
        files["manifest.json"] = json.dumps(manifest).encode()
        changed = self.directory / "missing-software.tar.gz"
        with tarfile.open(changed, "w:gz") as archive:
            for name, data in files.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        with self.assertRaisesRegex(ValueError, "source snapshot"):
            evidence.verify_bundle(changed)

    def test_corrupt_data_can_be_preserved_without_a_verified_label(self):
        self.complete()
        (self.study / "raw/known.jsonl.gz").write_bytes(b"interrupted data")
        output = self.directory / "failed.tar.gz"
        manifest = evidence.package_study(self.study, output)
        self.assertEqual(manifest["audit"]["status"], "quarantined")
        self.assertEqual(manifest, evidence.verify_bundle(output))

    def test_malformed_deflate_stream_is_quarantined(self):
        self.complete()
        path = self.study / "raw/known.jsonl.gz"
        path.write_bytes(b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x00\xff\xff" + bytes(20))
        result = evidence.audit_study(self.study)
        self.assertEqual(result["status"], "quarantined")
        self.assertTrue(any(issue.startswith("invalid_capture:") for issue in result["issues"]))

    def test_tampered_bundle_is_rejected_even_if_it_is_valid_gzip(self):
        self.complete()
        output = self.directory / "bundle.tar.gz"
        evidence.package_study(self.study, output)
        changed = self.directory / "changed.tar.gz"
        with tarfile.open(output, "r:gz") as source, tarfile.open(changed, "w:gz") as target:
            for member in source:
                data = source.extractfile(member).read()
                if member.name == "REPORT.md":
                    data += b"tampered\n"
                info = tarfile.TarInfo(member.name)
                info.size = len(data)
                target.addfile(info, io.BytesIO(data))
        with self.assertRaisesRegex(ValueError, "hash/size mismatch"):
            evidence.verify_bundle(changed)

    def test_concatenated_tar_cannot_hide_extra_members_after_padding(self):
        self.complete()
        output = self.directory / "bundle.tar.gz"
        evidence.package_study(self.study, output)
        extra = io.BytesIO()
        with tarfile.open(fileobj=extra, mode="w") as archive:
            info = tarfile.TarInfo("unlisted.txt")
            info.size = 1
            archive.addfile(info, io.BytesIO(b"x"))
        changed = self.directory / "concatenated.tar.gz"
        changed.write_bytes(gzip.compress(gzip.decompress(output.read_bytes()) + extra.getvalue()))
        with self.assertRaisesRegex(ValueError, "exactly the archive members"):
            evidence.verify_bundle(changed)

    def test_declared_core_source_pins_survive_a_rewritten_bundle_inventory(self):
        self.complete()
        pins = {"src/physics.rs": evidence.sha256((evidence.ROOT / "src/physics.rs").read_bytes())}
        self.rewrite_capture_header(lambda header: header["captureProvenance"].update(
            sources={"files": pins, "combinedSha256": evidence.source_map_hash(pins)}))
        output = self.directory / "bundle.tar.gz"
        evidence.package_study(self.study, output)
        with tarfile.open(output, "r:gz") as source:
            files = {member.name: source.extractfile(member).read() for member in source}
        files["software/src/physics.rs"] += b"\n// changed source\n"
        manifest = json.loads(files["manifest.json"])
        entry = next(entry for entry in manifest["files"] if entry["path"] == "software/src/physics.rs")
        entry.update(sha256=evidence.sha256(files[entry["path"]]), bytes=len(files[entry["path"]]))
        files["manifest.json"] = json.dumps(manifest).encode()
        changed = self.directory / "changed-source.tar.gz"
        with tarfile.open(changed, "w:gz") as archive:
            for name, data in files.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        with self.assertRaisesRegex(ValueError, "recomputed capture checks"):
            evidence.verify_bundle(changed)

    def test_malformed_provenance_is_preserved_as_quarantined_evidence(self):
        self.complete()
        path = self.study / "raw/known.jsonl.gz"
        original = path.read_bytes()
        edits = [
            lambda header: header.update(captureProvenance=["malformed"]),
            lambda header: header["captureProvenance"].update(collectorSha256=[]),
            lambda header: header["captureProvenance"].update(options=["malformed"]),
            lambda header: header.update(server=["malformed"]),
            lambda header: header["captureProvenance"].update(sources={"files": []}),
            lambda header: header["captureProvenance"].update(sources={"files": {"../escape": "a" * 64}, "combinedSha256": "b" * 64}),
            lambda header: header["captureProvenance"].update(sources={"files": {"src/Physics.rs": evidence._digest_file(evidence.ROOT / "src/physics.rs")}, "combinedSha256": "b" * 64}),
            lambda header: header["captureProvenance"].update(sources={"files": {"src/physics.rs": evidence._digest_file(evidence.ROOT / "src/physics.rs")}, "combinedSha256": "b" * 64}),
        ]
        for index, edit in enumerate(edits):
            with self.subTest(index=index):
                path.write_bytes(original)
                self.rewrite_capture_header(edit)
                output = self.directory / f"malformed-{index}.tar.gz"
                manifest = evidence.package_study(self.study, output)
                self.assertEqual(manifest["audit"]["status"], "quarantined")
                self.assertEqual(manifest, evidence.verify_bundle(output))

    def test_unsafe_archive_paths_links_aliases_and_case_collisions_are_rejected(self):
        for members in (("../escape",), ("/absolute",), ("C:alternate",), ("NUL.txt",),
                        ("folder./a",), ("A/one", "a/two"), ("same", "same"), ("file", "file/child")):
            with self.subTest(members=members):
                path = self.directory / "unsafe.tar.gz"
                with tarfile.open(path, "w:gz") as archive:
                    for name in members:
                        info = tarfile.TarInfo(name)
                        info.size = 1
                        archive.addfile(info, io.BytesIO(b"x"))
                with self.assertRaises(ValueError):
                    evidence.verify_bundle(path)
        path = self.directory / "link.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            info = tarfile.TarInfo("link")
            info.type, info.linkname = tarfile.SYMTYPE, "outside"
            archive.addfile(info)
        with self.assertRaises(ValueError):
            evidence.verify_bundle(path)

    def test_study_lock_preserves_an_existing_owner(self):
        with evidence.study_lock(self.study):
            before = (self.study / ".study.lock").read_bytes()
            with self.assertRaises(FileExistsError):
                with evidence.study_lock(self.study):
                    self.fail("lock unexpectedly acquired")
            self.assertEqual((self.study / ".study.lock").read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
