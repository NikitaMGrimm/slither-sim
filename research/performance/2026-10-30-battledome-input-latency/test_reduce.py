import hashlib
import json
from pathlib import Path
import struct
import tempfile
import unittest

import reduce


def capture(path, parts):
    with path.open("wb") as stream:
        stream.write(struct.pack("<IHHIIII", 0xa1b23c4d, 2, 4, 0, 0, 65535, 1))
        for part in parts:
            offset, payload, at, *direction = part
            source, target = (40000, 8766) if direction and not direction[0] else (8766, 40000)
            tcp = struct.pack("!HHIIBBHHH", source, target, 100 + offset, 0, 80, 24, 65535, 0, 0) + payload
            ip = bytes([69, 0]) + struct.pack("!H", 20 + len(tcp)) + bytes(5) + bytes([6]) + bytes(10) + tcp
            packet = bytes(14) + ip
            stream.write(struct.pack("<IIII", int(at), round((at % 1) * 1e9), len(packet), len(packet)))
            stream.write(packet)


class ReductionTests(unittest.TestCase):
    def test_quantiles_and_empty(self):
        self.assertEqual(reduce.stats([]), {"n": 0})
        self.assertEqual(reduce.stats(range(1, 101))["p95"], 95)
        self.assertEqual(reduce.stats(range(1, 101))["p99"], 99)

    def test_control_ping_is_not_game_ping(self):
        intervals = [{"start": 1., "end": 1.1, "deferred": 0},
                     {"start": 1.1, "end": 1.2, "deferred": 3}]
        self.assertEqual(reduce.classify(1.02, 1.04, intervals), "normal-bucket")
        self.assertEqual(reduce.classify(1.09, 1.12, intervals), "deferred-bucket")
        self.assertEqual(reduce.classify(.99, 1.02, intervals), "uncovered")
        self.assertIsNone(reduce.nearest([{"unix": 1., "milliseconds": .1}], 4.))

    def test_split_reordered_packets_use_completion_time_and_hash(self):
        upgrade = b"HTTP/1.1 101 Switching Protocols\r\n\r\n"
        payload = b"\x00\x07E\x64"
        wire = upgrade + bytes([130, len(payload)]) + payload
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "probe.pcap"
            split = len(upgrade) + 3
            capture(path, [(0, wire[:split], 10.003), (split + 1, wire[split + 1:], 10.004),
                           (split, wire[split:split + 1], 10.006)])
            frames, audit = reduce.pcap_frames(path)
            self.assertEqual(frames, [{"sha256": hashlib.sha256(payload).hexdigest(), "unix": 10.006}])
            self.assertEqual(audit["overlappingSegments"], 0)

    def test_missing_packet_and_truncation_fail(self):
        upgrade = b"HTTP/1.1 101 Switching Protocols\r\n\r\n"
        wire = upgrade + bytes([130, 4]) + b"\x00\x07E\x64"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "probe.pcap"
            capture(path, [(0, wire[:10], 10.), (11, wire[11:], 10.01)])
            with self.assertRaises(AssertionError):
                reduce.pcap_frames(path)
            capture(path, [(0, wire, 10.)])
            path.write_bytes(path.read_bytes()[:-1])
            with self.assertRaises(AssertionError):
                reduce.pcap_frames(path)

    def test_censor_and_echo_are_distinct_and_rtt_subtraction_keeps_negative(self):
        payload = b"\x00\x07E\x64"
        wire = b"HTTP/1.1 101 Switching Protocols\r\n\r\n" + bytes([130, len(payload)]) + payload
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = b"GET /slither HTTP/1.1\r\n\r\n" + bytes([130, 129, 1, 2, 3, 4, 98 ^ 1])
            capture(root / "capture-1.pcap", [(0, request, 10.0012, False), (0, wire, 10.002)])
            (root / "capture-1.log").write_text("0 packets dropped by kernel\n")
            (root / "window.json").write_text(json.dumps({"startUnix": 10., "endUnix": 11.,
                                                          "label": "synthetic", "seats": 1}))
            records = [
                {"type": "send", "id": 1, "life": 1, "at": .001, "unix": 10.001,
                 "command": 98, "expected": 100, "callSeconds": 0., "lateSeconds": 0.},
                {"type": "frame", "life": 1, "seq": 1, "at": .003, "unix": 10.003,
                 "sha256": hashlib.sha256(payload).hexdigest(), "headerMs": 7,
                 "wasPlaying": True, "playing": True},
                {"type": "echo", "id": 1, "life": 1, "at": .003, "unix": 10.003,
                 "wanted": 100, "frame": 1, "milliseconds": 2.},
                {"type": "send", "id": 2, "life": 1, "at": .1, "unix": 10.1,
                 "expected": 120, "callSeconds": 0., "lateSeconds": 0.},
                {"type": "censored", "id": 2, "life": 1, "unix": 10.2, "reason": "death"},
                {"type": "complete"},
            ]
            (root / "probe.jsonl").write_text("".join(json.dumps(v) + "\n" for v in records))
            result = reduce.summarize_row(root, [{"start": 9.9, "end": 11.1, "deferred": 0}],
                                          [{"unix": 10.001, "milliseconds": 3.}], [])
            self.assertEqual(result["confirmed"], 1)
            self.assertEqual(result["censored"], {"death": 1})
            self.assertEqual(result["latencyMinusRttMs"]["icmp"]["p50"], -1.)
            self.assertFalse(result["minimumSampleQuotaPass"])

    def test_challenge_before_game_epoch_does_not_turn_admission_into_excess(self):
        # These Playing flags are synthetic decoder metadata. The fixture tests
        # the measurement epoch, not the challenge solver or a valid snake login.
        payloads = [b"\x00\x006x", b"\x00\x00Ex", b"\x00\x07Ed"]
        pieces = [b"HTTP/1.1 101 Switching Protocols\r\n\r\n"]
        pieces[0] += bytes([130, len(payloads[0])]) + payloads[0]
        pieces.extend(bytes([130, len(payload)]) + payload for payload in payloads[1:])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            parts = [(0, b"GET /slither HTTP/1.1\r\n\r\n", 9.99, False)]
            offset = 0
            for piece, at in zip(pieces, [10., 10.5, 10.507]):
                parts.append((offset, piece, at))
                offset += len(piece)
            capture(root / "capture-1.pcap", parts)
            (root / "capture-1.log").write_text("0 packets dropped by kernel\n")
            (root / "window.json").write_text(json.dumps({"startUnix": 9.9, "endUnix": 11.,
                                                          "label": "synthetic-epoch", "seats": 1}))
            records = []
            for index, (payload, at, playing, was, header) in enumerate(zip(
                    payloads, [.0001, .5001, .5071], [False, True, True], [False, False, True], [0, 0, 7]), 1):
                records.append({"type": "frame", "life": 1, "seq": index, "at": at,
                                "unix": 10. + at, "sha256": hashlib.sha256(payload).hexdigest(),
                                "headerMs": header, "wasPlaying": was, "playing": playing})
            records.append({"type": "complete"})
            (root / "probe.jsonl").write_text("".join(json.dumps(value) + "\n" for value in records))
            result = reduce.summarize_row(root, [], [], [])
            self.assertAlmostEqual(result["frameMetricsMs"]["excess"]["max"], 0., places=7)
            self.assertLess(result["frameMetricsMs"]["packetExcess"]["max"], .001)


if __name__ == "__main__":
    unittest.main()
