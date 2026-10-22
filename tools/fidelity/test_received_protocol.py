"""Client receipt reconstruction and stock decoding checks."""

import json
import math
import statistics
import unittest
from pathlib import Path

import slither_protocol as sp

FIXTURE = Path(__file__).resolve().parents[2] / "tests/fixtures/server-body-snapshots.json"


class ReceivedProtocolTests(unittest.TestCase):
    def setUp(self):
        self.case = json.loads(FIXTURE.read_text())["cases"][0]
        self.sid = self.case["snake_id"]
        self.appearance = bytes.fromhex(self.case["initial_packet_hex"])
        self.world = sp.ReceivedWorld()
        self.world.feed_received(self.appearance, 0, 1000.0)

    def snake(self, now=1000.0, **kwargs):
        return self.world.metadata(now, **kwargs)["snakes"][0]

    def test_polling_and_server_extrapolation_do_not_refresh_receipts(self):
        first = self.snake(1042.0, extrapolation_server_ms=42)
        second = self.snake(1084.0, extrapolation_server_ms=84)
        self.assertAlmostEqual(first["position"]["ageSeconds"], 0.042)
        self.assertAlmostEqual(second["position"]["ageSeconds"], 0.084)
        self.assertTrue(first["position"]["predicted"])
        self.assertFalse(first["heading"]["predicted"])
        self.assertEqual(self.snake(1000.0)["body"]["ageSeconds"], 0.0)

    def test_equal_partial_reports_refresh_only_reported_fields(self):
        angle_byte = 4
        self.world.feed_received(bytes([ord("E"), angle_byte]), 70, 1070.0)
        self.world.feed_received(bytes([ord("E"), angle_byte]), 110, 1110.0)
        row = self.snake(1120.0)
        self.assertAlmostEqual(row["wantedHeading"]["ageSeconds"], 0.01)
        for name in ("heading", "speed", "position", "body", "mass", "appearance"):
            self.assertAlmostEqual(row[name]["ageSeconds"], 0.12)
        self.assertEqual(len(row["wantedHistory"]), 1)
        self.assertAlmostEqual(row["wantedHistory"][0]["ageSeconds"], 0.05)

    def test_body_tail_and_mass_updates_have_independent_receipts(self):
        self.world.feed_received(b"h" + self.sid.to_bytes(2, "big") + b"\x00\x00\x01", 20, 1020.0)
        row = self.snake(1030.0)
        self.assertAlmostEqual(row["mass"]["ageSeconds"], 0.01)
        self.assertAlmostEqual(row["body"]["ageSeconds"], 0.03)
        self.world.feed_received(b"r" + self.sid.to_bytes(2, "big"), 40, 1040.0)
        row = self.snake(1050.0)
        self.assertAlmostEqual(row["body"]["ageSeconds"], 0.01)
        self.assertAlmostEqual(row["position"]["ageSeconds"], 0.05)

    def test_histories_keep_actual_receipts_and_expire(self):
        for at, angle in ((1010.0, 3), (1030.0, 7), (1070.0, 9), (1110.0, 12)):
            self.world.feed_received(bytes([ord("E"), angle]), int(at) - 1000, at)
        # Arrival order rather than the server clock drives history.
        self.world.feed_received(bytes([ord("3"), 90]), 140, 1140.0)
        self.world.feed_received(bytes([ord("3"), 100]), 180, 1180.0)
        self.world.feed_received(bytes([ord("3"), 110]), 220, 1220.0)
        row = self.snake(1262.0)
        self.assertEqual(
            [round(h["ageSeconds"], 3) for h in row["wantedHistory"]], [0.152, 0.192, 0.232]
        )
        self.assertAlmostEqual(row["wantedHeading"]["ageSeconds"], 0.152)
        self.assertAlmostEqual(row["speedHistory"][0]["ageSeconds"], 0.082)
        self.assertAlmostEqual(row["speedHistory"][0]["value"], 100 / 18 / 0.032)
        self.assertAlmostEqual(row["speedHistory"][1]["ageSeconds"], 0.262)
        old = self.snake(1900.0)
        self.assertEqual(old["wantedHistory"], [])
        self.assertTrue(all(not h["valid"] for h in old["speedHistory"]))

    def test_reappearance_removal_reconnect_and_clock_regression_reset(self):
        original = self.snake()["incarnation"]
        self.world.feed_received(bytes([ord("E"), 42]), 10, 1010.0)
        self.world.feed_received(b"s" + self.sid.to_bytes(2, "big"), 20, 1020.0)
        self.assertEqual(self.world.metadata(1020.0)["snakes"], [])
        self.world.feed_received(self.appearance, 30, 1030.0)
        self.assertGreater(self.snake(1030.0)["incarnation"], original)
        self.assertEqual(self.snake(1030.0)["wantedHistory"], [])
        self.world.feed_frame(sp.Frame(2, 1100.0, 0, [self.appearance]))
        generation = self.world.connection_generation
        self.assertEqual(self.snake(1100.0)["wantedHistory"], [])
        self.world.feed_received(b"M\x00\x02\x40", 10, 1110.0)
        self.world.feed_received(self.appearance, 0, 100.0)
        self.assertGreater(self.world.connection_generation, generation)
        self.assertFalse(self.world.metadata(100.0)["minimap"]["valid"])

    def test_category_receipts_are_independent_and_do_not_claim_empty_space(self):
        self.world.feed_received(b"F\x01\x02\x00\x03\x04\x05", 20, 1020.0)
        self.world.feed_received(b"M\x00\x02\x40", 40, 1040.0)
        metadata = self.world.metadata(1080.0)
        self.assertAlmostEqual(metadata["food"]["ageSeconds"], 0.06)
        self.assertAlmostEqual(metadata["minimap"]["ageSeconds"], 0.04)
        self.assertFalse(metadata["prey"]["valid"])
        self.assertTrue(all(not metadata[name]["complete"] for name in ("food", "bodies", "prey")))

    def test_fixture_reconstruction_improves_error_without_changing_stock_decoder(self):
        canonical_errors, stock_errors = [], []
        for case in json.loads(FIXTURE.read_text())["cases"]:
            received, stock = sp.ReceivedWorld(), sp.World()
            packet = bytes.fromhex(case["initial_packet_hex"])
            received.feed_received(packet, 0, 1000.0)
            stock.feed(packet, 0)
            for i, step in enumerate(case["steps"], 1):
                packet = bytes.fromhex(step["packet_hex"])
                received.feed_received(packet, i, 1000.0 + i)
                stock.feed(packet, i)
            errors = lambda world: [
                math.dist(a, b)
                for a, b in zip(world.snakes[case["snake_id"]].pts, case["expected_body"])
            ]
            canonical, client = errors(received), errors(stock)
            self.assertEqual(len(received.snakes[case["snake_id"]].pts), len(case["expected_body"]))
            self.assertLessEqual(max(canonical), 1.25)
            canonical_errors.append(statistics.mean(canonical))
            stock_errors.append(statistics.mean(client))
        self.assertLess(statistics.mean(canonical_errors), 0.4)
        self.assertGreater(statistics.mean(stock_errors), 7.0)

    def test_server_only_feed_cannot_fabricate_receipt_age(self):
        with self.assertRaisesRegex(TypeError, "receipt clock"):
            self.world.feed(self.appearance, 20)


if __name__ == "__main__":
    unittest.main()
