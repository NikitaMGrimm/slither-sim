import base64
import gzip
import hashlib
import json
import tempfile
import unittest
import uuid
from pathlib import Path

from capture_quality import audit_capture, audit_captures, packets


class CaptureQualityTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def header(self, batch=None, index=0, bots=1):
        return {
            "type": "header",
            "schema": 1,
            "t": 0,
            "wall": 1000,
            "server": {"ip": "127.0.0.1", "port": 444},
            "captureProvenance": {
                "version": 1,
                "captureId": str(uuid.uuid4()),
                "captureBatchId": batch or str(uuid.uuid4()),
                "botIndex": index,
                "botCount": bots,
                "collectorSha256": "a" * 64,
                "planSha256": "b" * 64,
                "timeOrigin": 1000,
                "options": {"scenario": "forage", "bots": bots},
            },
        }

    def setup(self, radius=100, msl=42):
        packet = bytearray(30)
        packet[0] = ord("a")
        packet[23], packet[24] = 19, msl
        packet[27:30] = radius.to_bytes(3, "big")
        return b"\xff\xff" + packet

    def frame(self, direction, timestamp, data):
        return [direction, timestamp, base64.b64encode(data).decode("ascii")]

    def records(self, header=None):
        return [
            header or self.header(),
            {"type": "meta", "event": "connect", "t": 1},
            self.frame("i", 2, self.setup()),
            self.frame("o", 5, b"\xfb"),
            self.frame("i", 30, b"\x00\x0ap"),
        ]

    def write(self, rows=None, name="capture.gz", legacy=False, event="capture-end", mutate=None):
        rows = rows or self.records()
        if legacy:
            rows[0].pop("captureProvenance", None)
        prefix = b"".join((json.dumps(row, separators=(",", ":")) + "\n").encode() for row in rows)
        counts = {"i": {"frames": 0, "bytes": 0}, "o": {"frames": 0, "bytes": 0}}
        for row in rows:
            if isinstance(row, list):
                counts[row[0]]["frames"] += 1
                counts[row[0]]["bytes"] += len(base64.b64decode(row[2]))
        receipt = {
            "type": "meta",
            "event": event,
            "t": 31,
            "frames": counts["i"]["frames"],
            "joins": 1,
            "collectorEndSha256": "a" * 64,
        }
        if not legacy:
            receipt["integrity"] = {
                "format": "sha256-jsonl-prefix-v1",
                "sha256": hashlib.sha256(prefix).hexdigest(),
                "lines": len(rows),
                "payloadBytes": len(prefix),
                "counts": counts,
            }
        if mutate:
            mutate(receipt)
        payload = prefix + (json.dumps(receipt) + "\n").encode()
        file = self.directory / name
        file.write_bytes(gzip.compress(payload))
        return file

    def test_verified_integrity_and_wire_rtt_not_one_way_latency(self):
        result = audit_capture(self.write())
        self.assertEqual(result["status"], "verified")
        self.assertTrue(result["completeGzip"])
        self.assertTrue(result["integrityVerified"])
        self.assertEqual(result["network"]["gamePingRttMs"]["mean"], 25)
        self.assertEqual(result["network"]["arrivalHeaderAbsoluteDifferenceMs"]["mean"], 18)
        self.assertEqual(result["network"]["receivedServerElapsedMs"], 10)

    def test_unsupported_protocol_is_quarantined_with_raw_bytes_preserved(self):
        rows = self.records()
        data = bytearray(base64.b64decode(rows[2][2]))
        data[2 + 23] = 11
        rows[2][2] = base64.b64encode(data).decode("ascii")
        file = self.write(rows=rows)
        original = file.read_bytes()
        result = audit_capture(file)
        self.assertEqual(result["status"], "quarantine")
        self.assertIn("unsupported_protocol_version:11", result["issues"])
        self.assertEqual(result["setupSignatures"][0]["protocolVersion"], 11)
        self.assertTrue(result["integrityVerified"])
        self.assertEqual(file.read_bytes(), original)

    def test_header_free_bundles_keep_setup_and_pong_without_inventing_elapsed_time(self):
        setup = self.setup()[2:]
        rows = [self.header(), {"type": "meta", "event": "connect", "t": 1},
                self.frame("i", 2, b"6synthetic"), self.frame("o", 5, b"\xfb"),
                self.frame("i", 30, len(setup).to_bytes(2, "big") + setup + b"\x21p")]
        result = audit_capture(self.write(rows=rows))
        self.assertEqual(result["status"], "verified", result["issues"])
        self.assertEqual(result["joins"], 1)
        self.assertEqual(result["network"]["gamePingRttMs"]["mean"], 25)
        self.assertEqual(result["network"]["receivedServerElapsedMs"], 0)

    def test_legacy_data_remains_readable_not_silently_verified(self):
        result = audit_capture(self.write(legacy=True))
        self.assertEqual(result["status"], "legacy_unknown")
        self.assertFalse(result["integrityVerified"])

    def test_altered_prefix_is_quarantined_even_with_valid_gzip(self):
        file = self.write()
        payload = gzip.decompress(file.read_bytes()).replace(b'"forage"', b'"boost"')
        file.write_bytes(gzip.compress(payload))
        result = audit_capture(file)
        self.assertEqual(result["status"], "quarantine")
        self.assertIn("terminal_integrity_mismatch", result["issues"])

    def test_crc_corruption_and_truncated_gzip_are_quarantined(self):
        for name, transform in (
            ("truncated.gz", lambda data: data[:-5]),
            ("crc.gz", lambda data: data[:-8] + b"\0\0\0\0" + data[-4:]),
        ):
            file = self.write(name=name)
            file.write_bytes(transform(file.read_bytes()))
            result = audit_capture(file)
            self.assertEqual(result["status"], "quarantine")
            self.assertFalse(result["completeGzip"])
            self.assertEqual(result["archiveSha256"], hashlib.sha256(file.read_bytes()).hexdigest())

    def test_missing_terminal_and_interruption_never_claim_complete_capture(self):
        file = self.write(event="capture-interrupted")
        self.assertIn("interrupted_capture", audit_capture(file)["issues"])
        payload = gzip.decompress(file.read_bytes()).splitlines(keepends=True)
        file.write_bytes(gzip.compress(b"".join(payload[:-1])))
        self.assertIn("missing_terminal_receipt", audit_capture(file)["issues"])

    def test_duplicate_compressed_and_recompressed_content_are_not_replicates(self):
        original = self.write()
        copy = self.directory / "copy.gz"
        copy.write_bytes(gzip.compress(gzip.decompress(original.read_bytes()), mtime=1))
        result = audit_captures([original, copy])
        self.assertEqual(len(result["exclusions"]), 2)
        self.assertTrue(
            all("duplicate_payloadSha256" in capture["issues"] for capture in result["captures"])
        )

    def test_simultaneous_bots_are_one_complete_batch_and_missing_member_ineligible(self):
        batch = str(uuid.uuid4())
        files = [
            self.write(rows=self.records(self.header(batch, index, 2)), name=f"bot{index}.gz")
            for index in range(2)
        ]
        complete = audit_captures(files)["batches"]
        self.assertEqual(len(complete), 1)
        self.assertTrue(complete[0]["eligibleForPlannedInference"])
        self.assertFalse(audit_captures(files[:1])["batches"][0]["eligibleForPlannedInference"])

    def test_setup_change_is_hard_mismatch_radius_change_is_not(self):
        for radius, msl, incompatible in ((200, 42, False), (100, 43, True)):
            rows = self.records()
            rows.append(self.frame("i", 31, self.setup(radius, msl)))
            file = self.write(
                rows=rows, name=f"setup-{msl}.gz", mutate=lambda receipt: receipt.update(joins=2)
            )
            self.assertEqual(
                "setup_signature_changed_within_capture" in audit_capture(file)["issues"],
                incompatible,
            )

    def test_clock_regression_and_malformed_framing_fail_closed(self):
        rows = self.records()
        rows[-1][1] = 4
        self.assertIn(
            "record clock regressed", " ".join(audit_capture(self.write(rows=rows))["issues"])
        )
        for data in (b"\0", b"\0\0\0", b"\0\0\0\0", b"\0\0\0\x08a"):
            with self.assertRaises(ValueError):
                packets(data)

    def test_pending_ping_is_censored_not_labeled_packet_loss(self):
        rows = self.records()[:-1]
        result = audit_capture(self.write(rows=rows))
        self.assertEqual(result["network"]["censoredPings"], 1)
        self.assertEqual(result["network"]["gamePingRttMs"]["count"], 0)

    def test_changed_source_is_warning_not_server_update(self):
        result = audit_capture(
            self.write(mutate=lambda receipt: receipt.update(collectorEndSha256="c" * 64))
        )
        self.assertEqual(result["status"], "verified")
        self.assertIn("collector_source_changed_on_disk", result["warnings"])

    def test_malformed_provenance_and_integrity_return_quarantine_not_crash(self):
        for key, value in (
            ("captureBatchId", []),
            ("botCount", "2"),
            ("planSha256", "not-a-digest"),
        ):
            header = self.header()
            header["captureProvenance"][key] = value
            report = audit_captures([self.write(rows=self.records(header), name=f"{key}.gz")])
            self.assertEqual(report["captures"][0]["status"], "quarantine")
            self.assertEqual(report["batches"], [])
        result = audit_capture(self.write(mutate=lambda receipt: receipt.update(integrity=[])))
        self.assertEqual(result["status"], "quarantine")
        self.assertIn("terminal_integrity_mismatch", result["issues"])

    def test_invalid_deflate_and_deep_json_return_serializable_quarantine(self):
        malformed = self.directory / "deflate.gz"
        malformed.write_bytes(b"\x1f\x8b\x08\0" + b"\0" * 6 + b"\x07" + b"\0" * 8)
        nested = self.directory / "nested.gz"
        nested.write_bytes(gzip.compress(("[" * 10000 + "]" * 10000 + "\n").encode()))
        for file in (malformed, nested):
            result = audit_capture(file)
            self.assertEqual(result["status"], "quarantine")
            json.dumps(result, allow_nan=False)

    def test_nonfinite_json_numbers_cannot_contaminate_metrics_or_output(self):
        for field in (b'["i",30', b'"wall":1000'):
            file = self.write(name=f"infinite-{len(field)}.gz")
            payload = gzip.decompress(file.read_bytes()).replace(
                field, field.split(b":")[0] + b":1e999" if b":" in field else b'["i",1e999'
            )
            file.write_bytes(gzip.compress(payload))
            result = audit_capture(file)
            self.assertEqual(result["status"], "quarantine")
            self.assertEqual(result["network"]["gamePingRttMs"]["count"], 0)
            json.dumps(result, allow_nan=False)

    def test_submillisecond_clock_regression_cannot_produce_negative_rtt(self):
        rows = self.records()
        rows[-1][1] = 4.999
        result = audit_capture(self.write(rows=rows))
        self.assertEqual(result["status"], "quarantine")
        self.assertEqual(result["network"]["gamePingRttMs"]["count"], 0)

    def test_batch_membership_inconsistency_is_order_invariant_and_ineligible(self):
        batch = str(uuid.uuid4())
        first, second = self.header(batch, 0, 2), self.header(batch, 1, 2)
        second["captureProvenance"]["botCount"] = 3
        files = [
            self.write(rows=self.records(first), name="first.gz"),
            self.write(rows=self.records(second), name="second.gz"),
        ]
        for ordering in (files, list(reversed(files))):
            self.assertFalse(audit_captures(ordering)["batches"][0]["eligibleForPlannedInference"])

    def test_paired_batch_requires_shared_clock_and_actual_server(self):
        for mismatch in ("timeOrigin", "actualServer"):
            batch = str(uuid.uuid4())
            first, second = self.header(batch, 0, 2), self.header(batch, 1, 2)
            rows = self.records(second)
            if mismatch == "timeOrigin":
                second["captureProvenance"]["timeOrigin"] = 2000
            else:
                rows[1]["server"] = {"ip": "127.0.0.2", "port": 444}
            files = [
                self.write(rows=self.records(first), name=f"first-{mismatch}.gz"),
                self.write(rows=rows, name=f"second-{mismatch}.gz"),
            ]
            result = audit_captures(files)
            self.assertTrue(result["batches"][0]["completeMembership"])
            self.assertFalse(result["batches"][0]["matchedProvenance"])
            self.assertFalse(result["batches"][0]["eligibleForPlannedInference"])

    def test_overlapping_pings_do_not_manufacture_rtt(self):
        rows = self.records()[:-1]
        rows.extend(
            [
                self.frame("o", 6, b"\xfb"),
                self.frame("o", 7, b"\xfb"),
                self.frame("i", 8, b"\x00\x01p"),
            ]
        )
        result = audit_capture(self.write(rows=rows))
        self.assertEqual(result["network"]["gamePingRttMs"]["count"], 0)
        self.assertEqual(result["network"]["ambiguousPingRequests"], 3)
        self.assertIn("overlapping_game_pings", result["warnings"])


if __name__ == "__main__":
    unittest.main()
