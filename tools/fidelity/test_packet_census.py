import base64
from contextlib import redirect_stderr, redirect_stdout
import gzip
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import packet_census as pc


def setup(version=19):
    packet = bytearray(30)
    packet[0], packet[23] = ord("a"), version
    return bytes(packet)


def appearance(snake_id):
    return b"s" + snake_id.to_bytes(2, "big") + bytes(29)


def prey(prey_id, length=20):
    return b"y" + prey_id.to_bytes(2, "big") + bytes(length - 3)


def update(prey_id):
    return b"j" + prey_id.to_bytes(2, "big") + bytes(4)


def incoming(receipt, *packets, elapsed=0, header_free=False):
    payload = packets[0] if len(packets) == 1 else b"".join(
        len(packet).to_bytes(2, "big") + packet for packet in packets)
    data = payload if header_free else elapsed.to_bytes(2, "big") + payload
    return ["i", receipt, base64.b64encode(data).decode("ascii")]


def command(segment, name, length=None):
    key = "commands" if length is None else "families"
    return next(row for row in segment[key] if row["command"] == name
                and (length is None or row["length"] == length))


class PacketCensusTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)

    def archive(self, records, terminal="capture-end", name="capture.jsonl.gz"):
        rows = [{"type": "header", "schema": 1, "t": 0,
                 "server": {"ip": "127.0.0.1", "port": 444}}, *records]
        if terminal:
            rows.append(dict(type="meta", event=terminal, t=1000,
                             frames=sum(isinstance(row, list) and row[0] == "i" for row in rows)))
        path = self.directory / name
        path.write_bytes(gzip.compress(b"".join(
            (json.dumps(row) + "\n").encode() for row in rows)))
        return path

    def connect(self, receipt):
        return dict(type="meta", event="connect", t=receipt)

    def test_bundled_counts_exact_gaps_and_reconnect_clocks(self):
        path = self.archive([
            self.connect(1), incoming(10, setup(), appearance(42), b"p", b"p", elapsed=65535),
            incoming(30, b"p", elapsed=7), self.connect(40),
            incoming(50, setup(), appearance(7), b"p", elapsed=65535),
            incoming(80, b"p", elapsed=14),
        ])
        before = path.read_bytes()
        report = pc.census(path)
        self.assertEqual(report["schemaVersion"], 1)
        self.assertEqual(report["wireVersion"], 19)
        self.assertEqual(report["status"], "complete")
        self.assertEqual(report["archiveSha256"], hashlib.sha256(before).hexdigest())
        self.assertEqual(report["decoderSha256"], pc.sha256(Path(pc.sp.__file__)))
        self.assertEqual(report["incomingFrames"], 4)
        self.assertEqual(report["incomingPackets"], 9)
        first, second = report["connections"]
        self.assertEqual(first["durationMs"], dict(server=7, receipt=20))
        self.assertEqual(second["durationMs"], dict(server=14, receipt=30))
        pong = command(first, "p", 1)
        self.assertEqual((pong["frames"], pong["packets"], pong["bytes"]), (2, 3, 3))
        self.assertEqual(pong["gapsMs"]["server"], dict(n=2, min=0, median=3.5, p95=7, max=7))
        self.assertEqual(pong["gapsMs"]["receipt"], dict(n=2, min=0, median=10, p95=20, max=20))
        self.assertEqual(command(second, "p")["gapsMs"]["server"]["n"], 1)
        self.assertEqual(command(second, "p")["gapsMs"]["server"]["median"], 14)
        self.assertEqual(first["ownId"], 42)
        self.assertEqual(second["ownId"], 7)
        self.assertNotIn("127.0.0.1", json.dumps(report))
        self.assertEqual(path.read_bytes(), before)

    def test_setup_resets_gaps_without_a_reconnect(self):
        report = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), appearance(4), b"p"),
            incoming(30, b"p", elapsed=9),
            incoming(50, setup(), appearance(8), b"p", elapsed=65535),
            incoming(90, b"p", elapsed=17),
        ]))
        first, second = report["connections"]
        self.assertEqual((first["connection"], second["connection"]), (1, 1))
        self.assertEqual((first["setupIndex"], second["setupIndex"]), (1, 2))
        self.assertEqual(command(first, "p")["gapsMs"]["server"]["median"], 9)
        self.assertEqual(command(second, "p")["gapsMs"]["server"]["median"], 17)

    def test_first_full_snake_and_packet_order_around_game_over(self):
        report = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), b"p", appearance(42), appearance(0),
                                      b"p", b"v\0", b"p"),
        ]))["connections"][0]
        self.assertEqual(report["ownId"], 42)
        p = command(report, "p")["byOwnState"]
        self.assertEqual((p["unknown_own"], p["active"], p["post_own_death"]), (1, 1, 1))
        self.assertEqual(command(report, "v")["byOwnState"]["active"], 1)
        self.assertEqual(report["finalOwnState"], "post_own_death")

    def test_unknown_own_game_over_and_own_removal_states(self):
        unknown = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), b"p", b"v\0", b"p"),
        ]))["connections"][0]
        self.assertIsNone(unknown["ownId"])
        self.assertEqual(command(unknown, "v")["byOwnState"]["unknown_own"], 1)
        self.assertEqual(command(unknown, "p")["byOwnState"]["post_own_death"], 1)
        known = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), appearance(42), b"s\0\0\1", b"p",
                                      b"s\0*", b"p", b"s\0*\1", b"p"),
        ]))["connections"][0]
        p = command(known, "p")["byOwnState"]
        self.assertEqual((p["active"], p["unknown_after_own_removal"], p["post_own_death"]), (1, 1, 1))

    def test_prey_reused_id_known_update_gaps_and_exposure_denominators(self):
        segment = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), appearance(42), prey(7)),
            incoming(20, update(7), elapsed=100), incoming(30, update(7), elapsed=100),
            incoming(40, prey(7, 3), elapsed=100), incoming(50, update(7), elapsed=100),
            incoming(60, prey(7), update(7), elapsed=100),
            incoming(70, update(7), elapsed=200), incoming(80, prey(7, 5), elapsed=100),
            incoming(90, prey(7), elapsed=100), incoming(100, b"p", elapsed=100),
        ]))["connections"][0]
        report = segment["prey"]
        entity, = report["entities"]
        self.assertEqual((entity["preyId"], entity["entries"], entity["knownUpdates"]), (7, 3, 4))
        self.assertEqual((entity["genericRemovals"], entity["eatenRemovals"], entity["censoredEpisodes"]), (1, 1, 1))
        self.assertAlmostEqual(entity["exposureSeconds"]["server"], .7)
        self.assertAlmostEqual(entity["exposureSeconds"]["receipt"], .06)
        self.assertAlmostEqual(entity["updatesPerKnownPreySecond"]["server"], 4 / .7)
        self.assertEqual(report["counts"]["unknownUpdates"], 1)
        self.assertEqual(report["updateGapsMs"]["server"], dict(n=2, min=100, median=150, p95=200, max=200))
        self.assertEqual(report["updateGapsMs"]["receipt"]["n"], 2)
        self.assertEqual(command(segment, "y")["gapsMs"]["server"]["n"], 4)
        self.assertEqual(command(segment, "y", 20)["gapsMs"]["server"]["n"], 2)

    def test_exact_weighted_median_and_nearest_rank_p95(self):
        gaps = pc.Gaps(pc.Budget())
        for value in range(1, 101):
            gaps.add(value)
            gaps.add(value)
        self.assertEqual(gaps.summary(), dict(n=200, min=1, median=50.5, p95=95, max=100))

    def test_repeated_entry_censors_and_never_joins_update_gaps(self):
        report = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), prey(7)), incoming(20, update(7), elapsed=100),
            incoming(30, prey(7), elapsed=100), incoming(40, update(7), elapsed=100),
        ]))["connections"][0]["prey"]
        self.assertEqual(report["knownEpisodes"]["entries"], 2)
        self.assertEqual(report["knownEpisodes"]["censoredEpisodes"], 2)
        self.assertEqual(report["updateGapsMs"]["server"]["n"], 0)
        self.assertAlmostEqual(report["knownEpisodes"]["exposureSeconds"]["server"], .3)

    def test_prey_episodes_reset_on_reconnect_setup_and_close(self):
        report = pc.census(self.archive([
            self.connect(1), incoming(10, setup(), prey(7)), incoming(20, update(7), elapsed=100),
            dict(type="meta", event="close", t=25), self.connect(30),
            incoming(40, setup(), update(7)), incoming(50, prey(7), update(7), elapsed=100),
            incoming(60, setup(), update(7)),
        ]))["connections"]
        self.assertEqual(len(report), 3)
        self.assertEqual(report[0]["prey"]["knownEpisodes"]["censoredEpisodes"], 1)
        self.assertAlmostEqual(report[0]["prey"]["knownEpisodes"]["exposureSeconds"]["server"], .1)
        self.assertEqual(report[1]["prey"]["counts"]["unknownUpdates"], 1)
        self.assertEqual(report[2]["prey"]["counts"]["unknownUpdates"], 1)
        self.assertTrue(all(s["prey"]["updateGapsMs"]["receipt"]["n"] == 0 for s in report))

    def test_header_free_archive_keeps_server_time_unavailable(self):
        report = pc.census(self.archive([
            self.connect(1), incoming(5, b"6synthetic", header_free=True),
            incoming(10, setup(), appearance(42), prey(7), header_free=True),
            incoming(20, update(7), header_free=True), incoming(30, update(7), header_free=True),
        ]))["connections"]
        self.assertEqual(len(report), 2)
        gameplay = report[1]
        self.assertEqual(gameplay["durationMs"], dict(server=None, receipt=20))
        self.assertEqual(gameplay["prey"]["updateGapsMs"]["server"]["n"], 0)
        self.assertIsNone(gameplay["prey"]["knownEpisodes"]["exposureSeconds"]["server"])
        self.assertEqual(gameplay["prey"]["updateGapsMs"]["receipt"]["median"], 10)

    def test_incomplete_interrupted_truncated_and_non19_fail_without_output(self):
        records = [self.connect(1), incoming(10, setup())]
        for terminal in (None, "capture-interrupted"):
            path = self.archive(records, terminal=terminal)
            with self.assertRaises(ValueError):
                pc.census(path)
            output = self.directory / "partial.json"
            with redirect_stderr(io.StringIO()), redirect_stdout(io.StringIO()) as stdout:
                self.assertEqual(pc.main([str(path), "--output", str(output)]), 2)
            self.assertFalse(output.exists())
            self.assertEqual(stdout.getvalue(), "")
        path = self.archive(records)
        path.write_bytes(path.read_bytes()[:-4])
        with self.assertRaises((EOFError, OSError)):
            pc.census(path)
        with self.assertRaisesRegex(ValueError, "literal wire protocol 19"):
            pc.census(self.archive([self.connect(1), incoming(10, setup(11))]))
        with patch.object(pc.sp, "PROTOCOL_VERSION", 20):
            self.assertEqual(pc.census(self.archive(records))["wireVersion"], 19)

    def test_later_non19_setup_and_missing_setup_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "literal wire protocol 19"):
            pc.census(self.archive([self.connect(1), incoming(10, setup()), incoming(20, setup(18))]))
        with self.assertRaisesRegex(ValueError, "missing literal wire protocol 19"):
            pc.census(self.archive([self.connect(1), incoming(10, b"p")]))

    def test_output_is_machine_readable_and_never_overwritten(self):
        path = self.archive([self.connect(1), incoming(10, setup(), prey(7))])
        output = self.directory / "census.json"
        self.assertEqual(pc.main([str(path), "--output", str(output)]), 0)
        original = output.read_bytes()
        report = json.loads(original)
        self.assertIn("limits", report)
        self.assertIsNone(report["connections"][0]["prey"]["knownEpisodes"]["updatesPerKnownPreySecond"]["server"])
        with redirect_stderr(io.StringIO()) as stderr:
            self.assertEqual(pc.main([str(path), "--output", str(output)]), 2)
        self.assertIn("already exists", stderr.getvalue())
        self.assertEqual(output.read_bytes(), original)

    def test_entity_rows_are_bounded_without_dropping_exposure_totals(self):
        with patch.object(pc, "MAX_ENTITY_ROWS", 1):
            report = pc.census(self.archive([
                self.connect(1), incoming(10, setup(), prey(1), prey(2)),
                incoming(20, update(1), update(2), elapsed=100),
            ]))["connections"][0]["prey"]
        self.assertEqual(len(report["entities"]), 1)
        self.assertEqual(report["omittedEntityRows"], 1)
        self.assertEqual(report["knownEpisodes"]["entries"], 2)
        self.assertAlmostEqual(report["knownEpisodes"]["exposureSeconds"]["server"], .2)
        self.assertEqual(report["knownEpisodes"]["knownUpdates"], 2)

    def test_bad_framing_clock_order_limits_and_trailing_records_fail(self):
        with self.assertRaisesRegex(ValueError, "clock"):
            pc.census(self.archive([self.connect(1), incoming(20, setup()), incoming(10, b"p")]))
        path = self.archive([self.connect(1), incoming(10, setup())])
        path.write_bytes(gzip.compress(gzip.decompress(path.read_bytes()) + b'{}\n'))
        with self.assertRaisesRegex(ValueError, "records after capture-end"):
            pc.census(path)
        with patch.object(pc, "MAX_GAP_VALUES", 0):
            with self.assertRaisesRegex(ValueError, "gap value limit"):
                pc.census(self.archive([self.connect(1), incoming(10, setup(), b"p", b"p")]))
        with patch.object(pc, "MAX_ENTITY_EPOCHS", 0):
            with self.assertRaisesRegex(ValueError, "prey ID/setup epoch limit"):
                pc.census(self.archive([self.connect(1), incoming(10, setup(), prey(1))]))
        malformed = ["i", 20, base64.b64encode(b"\0\1\0\x10p").decode()]
        with self.assertRaisesRegex(ValueError, "packet length"):
            pc.census(self.archive([self.connect(1), incoming(10, setup()), malformed]))


if __name__ == "__main__":
    unittest.main()
