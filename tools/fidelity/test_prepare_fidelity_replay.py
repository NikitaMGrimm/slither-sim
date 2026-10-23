"""Contract tests for the bounded headless replay exporter."""

import base64
import gzip
import hashlib
import importlib.util
import json
import math
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import slither_protocol as sp

SPEC = importlib.util.spec_from_file_location(
    "prepare_fidelity_replay", Path(__file__).with_name("prepare-fidelity-replay.py")
)
replay = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(replay)


class FakeWorld:
    """Small deterministic protocol state with real Frame/Out archive records."""

    def __init__(self):
        self.self_id = None
        self.snakes = {}
        self.foods = {}
        self.events = []

    def feed(self, packet, server):
        kind = packet[:1]
        if kind == b"a":
            self.self_id = 1
            self.snakes[1] = sp.Snake(1, [[-42.0, 0.0], [0.0, 0.0]], 0.0, 0.0, 5.76, 0.5)
            self.foods[7] = (20.0, 30.0, 5.0)
            self.events.append(("init", server, 21600, 21000))
            self.events.append(("spawn", server, 1, self.snakes[1]))
        elif kind == b"A":
            x, y = packet[1], packet[2]
            snake = self.snakes[1]
            snake.pts.pop(0)
            snake.pts.append([float(x), float(y)])
            self.events.append(("anchor", server, 1, float(x), float(y), False, "G"))
        elif kind == b"B":
            self.foods[7] = (21.0, 31.0, 6.0)
            self.events.append(("food", server, 7, 21.0, 31.0, 6.0, "b"))
        elif kind == b"F":
            # Bulk food packets update state without an explicit 'food' event.
            self.foods[7] = (20.0, 30.0, 8.0)
        elif kind == b"U":
            self.foods[7] = (20.0, 30.0, 5.0)
        elif kind in (b"E", b"O") and len(packet) == 1:
            food = self.foods.pop(7, None)
            self.events.append(("eat", server, 7, food, 1 if kind == b"E" else 2))
        elif kind == b"X":
            self.events.append(("eat", server, 9, None, 1))
        elif kind == b"P":
            self.events.append(("prey_eaten", server, 9, None, 1))
        elif kind == b"S":
            self.snakes[1].fam = 1.0
        elif kind == b"L":
            self.snakes[1].fam = 0.3
        elif kind == b"V":
            self.events.append(("game_over", server, 1))
        elif kind == b"s":
            snake = self.snakes.pop(1, None)
            self.events.append(("remove", server, 1, packet[3] == 1, snake))
        elif kind == b"Q":
            self.snakes[1] = sp.Snake(1, [[-42.0, 0.0], [0.0, 0.0]], 0.0, 0.0, 5.76, 0.5)
            self.events.append(("spawn", server, 1, self.snakes[1]))
        elif kind == b"e":
            snake = self.snakes[1]
            before = (snake.ang, snake.wang, snake.sp)
            snake.ang, snake.wang = 0.5, 1.5
            self.events.append(("rot", server, 1, before, (snake.ang, snake.wang, snake.sp)))
        elif kind == b"5":
            snake = self.snakes[1]
            before = (snake.ang, snake.wang, snake.sp)
            snake.ang, snake.wang = 0.5, 0.55
            self.events.append(("rot", server, 1, before, (snake.ang, snake.wang, snake.sp)))
        elif kind == b"E":
            snake = self.snakes[1]
            before = (snake.ang, snake.wang, snake.sp)
            snake.wang = 1.6
            self.events.append(("rot", server, 1, before, (snake.ang, snake.wang, snake.sp)))
        elif kind == b"J":
            snake = self.snakes[1]
            snake.pts.append(list(snake.pts[-1]))
        elif kind == b"T":
            snake = self.snakes[1]
            snake.pts.pop(0)


def archive_records(*, replacement=True, death=False, footer=True, last=600):
    records = [{"type": "header"}]
    for server in range(0, last + 1, 100):
        packets = (
            [b"a", b"A\x00\x00"] if server == 0 else [b"A" + bytes([server // 4, server // 20])]
        )
        if server == 300 and replacement:
            packets.append(b"B")
        if server == 400:
            packets.append(b"E")
        if server == 500 and death:
            packets.append(b"V")
        records.append(sp.Frame(1, float(server + 100), server, packets))
    if footer:
        records.append({"type": "meta", "event": "capture-end"})
    return records


class PrepareReplayTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "capture.jsonl.gz"
        self.path.write_bytes(b"synthetic test source")

    def export(
        self, records, *, start=0.2, duration=0.25, heading_history=3, heading_source="anchors"
    ):
        with (
            patch.object(replay.sp, "read", return_value=iter(records)),
            patch.object(replay.sp, "World", FakeWorld),
        ):
            return replay.export(
                self.path,
                1,
                start,
                duration,
                "test",
                heading_history=heading_history,
                heading_source=heading_source,
            )

    def test_saturated_wire_mass_cannot_initialize_replay(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 200).packets.append(b"S")
        with self.assertRaisesRegex(ValueError, "saturated"):
            self.export(records)

    def test_saturated_later_mass_is_censored_until_report_is_exact(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 300).packets.append(b"S")
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 400).packets.append(b"L")
        fixture = self.export(records)
        self.assertEqual(fixture["provenance"]["censored_mass_times"], [0.1])
        self.assertFalse(any(e["time"] == 0.1 for e in fixture["mass_events"]))
        self.assertNotIn("fam", next(e for e in fixture["observations"] if e["time"] == 0.1))
        self.assertEqual(fixture["mass_events"][-1]["fam"], 0.3)

    def test_affine_clock_and_ping_shift_outgoing_controls(self):
        frames = [sp.Frame(1, float(s + 100), s, []) for s in (0, 100, 200, 300)]
        slope, intercept, residual = replay.clock_fit(frames)
        self.assertAlmostEqual(slope, 1.0)
        self.assertAlmostEqual(intercept, -100.0)
        self.assertAlmostEqual(residual, 0.0)
        records = archive_records()
        records.insert(3, sp.Out(1, 200.0, b"\xfb"))  # ping: reply at local 300 ms
        records.insert(5, sp.Out(1, 330.0, bytes([64])))
        records[4].packets.append(b"p")
        clip = self.export(records)
        self.assertEqual(clip["provenance"]["ping_rtt_median_ms"], 100)
        self.assertEqual(clip["provenance"]["ping_samples"], 1)
        self.assertEqual(len(clip["controls"]), 1)
        self.assertAlmostEqual(clip["controls"][0]["time"], 0.13)
        self.assertAlmostEqual(clip["controls"][0]["action"]["heading"], 64 / 251 * math.tau)

    def test_pregame_challenge_is_excluded_from_affine_clock_fit(self):
        records = archive_records()
        records.insert(1, sp.Frame(1, -10000.0, 0, [b"6"]))
        clip = self.export(records)
        self.assertEqual(clip["provenance"]["clock_ignored_pregame_frames"], 1)
        self.assertAlmostEqual(clip["provenance"]["clock_slope"], 1.0)
        self.assertAlmostEqual(clip["provenance"]["clock_intercept_ms"], -100.0)

    def test_initial_heading_uses_only_preceding_anchor_chords(self):
        base = archive_records()
        changed_future = archive_records()
        changed_future[5].packets[0] = b"A\x01\xff"
        first = self.export(base)
        second = self.export(changed_future)
        self.assertEqual(
            first["initial"]["snake"]["heading"], second["initial"]["snake"]["heading"]
        )
        self.assertEqual(
            first["provenance"]["initial_heading_method"], "preceding-three-anchor tangent"
        )
        self.assertNotEqual(first["observations"], second["observations"])

    def test_longer_heading_history_uses_only_preceding_anchors(self):
        base = archive_records(last=900)
        changed_future = archive_records(last=900)
        next(r for r in changed_future if isinstance(r, sp.Frame) and r.server == 600).packets[
            0
        ] = b"A\x01\xff"
        first = self.export(base, start=0.4, duration=0.25, heading_history=5)
        second = self.export(changed_future, start=0.4, duration=0.25, heading_history=5)
        self.assertEqual(
            first["initial"]["snake"]["heading"], second["initial"]["snake"]["heading"]
        )
        self.assertEqual(first["provenance"]["initial_heading_history"], 5)
        self.assertEqual(
            first["provenance"]["initial_heading_method"],
            "preceding-5-anchor least-squares tangent",
        )
        self.assertNotEqual(first["observations"], second["observations"])

    def test_longer_heading_fit_recovers_straight_and_curved_motion(self):
        times = [0, 100, 230, 310, 440, 590]
        heading = 0.3
        straight = [(t, 100 + t * math.cos(heading), -50 + t * math.sin(heading)) for t in times]
        estimated, method = replay.tangent(straight, -1.0, history=6)
        self.assertAlmostEqual(estimated, heading, places=10)
        self.assertEqual(method, "preceding-6-anchor least-squares tangent")

        omega, radius, start_angle = 0.8, 110.0, -0.4
        curved = [
            (
                t,
                radius * math.cos(start_angle + omega * t / 1000),
                radius * math.sin(start_angle + omega * t / 1000),
            )
            for t in times
        ]
        estimated, _ = replay.tangent(curved, -1.0, history=6)
        expected = start_angle + omega * times[-1] / 1000 + math.pi / 2 - omega * 0.007 / 2
        self.assertAlmostEqual(estimated, expected, places=10)

    def test_heading_history_rejects_too_few_anchors(self):
        with self.assertRaisesRegex(ValueError, "at least 3"):
            replay.tangent([], 0, history=2)
        with self.assertRaisesRegex(ValueError, "at least 3"):
            self.export(archive_records(), heading_history=2)

    def test_reported_heading_advances_from_last_rotation_even_after_new_anchor(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 100).packets.append(
            b"e\0"
        )
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 200).packets.append(
            b"E\0"
        )
        clip = self.export(records, heading_source="reported")
        expected = replay.reported_heading(replay.reported_heading(0.5, 1.5, 2, 100), 1.6, 2, 0)
        self.assertAlmostEqual(clip["initial"]["snake"]["heading"], expected)
        self.assertEqual(clip["provenance"]["initial_reported_heading_age_ms"], 100)
        self.assertEqual(clip["provenance"]["initial_heading_source"], "reported")
        self.assertEqual(
            clip["provenance"]["initial_heading_method"],
            "server-reported heading advanced to start",
        )

    def test_reported_heading_rejects_new_unacknowledged_target(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 100).packets.append(
            b"e\0"
        )
        records.insert(3, sp.Out(1, 250.0, bytes([64])))  # mapped to server 150 ms
        with self.assertRaisesRegex(ValueError, "unacknowledged steering"):
            self.export(records, heading_source="reported")

    def test_reported_heading_integrates_wanted_only_change_after_actual_report(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 100).packets.append(
            b"5\0\0"
        )
        position = next(
            i for i, r in enumerate(records) if isinstance(r, sp.Frame) and r.server == 200
        )
        records.insert(position, sp.Frame(1, 250.0, 150, [b"E\0"]))
        clip = self.export(records, heading_source="reported")
        expected = replay.reported_heading(replay.reported_heading(0.5, 0.55, 2, 50), 1.6, 2, 50)
        self.assertAlmostEqual(clip["initial"]["snake"]["heading"], expected)
        self.assertNotAlmostEqual(expected, replay.reported_heading(0.5, 1.6, 2, 100))

    def test_reported_heading_integrates_segment_change_after_actual_report(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 100).packets.append(
            b"e\0"
        )
        position = next(
            i for i, r in enumerate(records) if isinstance(r, sp.Frame) and r.server == 200
        )
        records.insert(position, sp.Frame(1, 250.0, 150, [b"J"]))
        clip = self.export(records, heading_source="reported")
        expected = replay.reported_heading(replay.reported_heading(0.5, 1.5, 2, 50), 1.5, 3, 50)
        self.assertAlmostEqual(clip["initial"]["snake"]["heading"], expected)
        self.assertNotAlmostEqual(expected, replay.reported_heading(0.5, 1.5, 3, 100))

    def test_reported_heading_rejects_unacknowledged_command_after_wanted_report(self):
        records = archive_records()
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 100).packets.append(
            b"5\0\0"
        )
        position = next(
            i for i, r in enumerate(records) if isinstance(r, sp.Frame) and r.server == 200
        )
        records.insert(position, sp.Frame(1, 250.0, 150, [b"E\0"]))
        records.insert(position + 1, sp.Out(1, 260.0, bytes([80])))
        with self.assertRaisesRegex(ValueError, "unacknowledged steering"):
            self.export(records, heading_source="reported")

    def test_reported_heading_turn_limit_and_target_clamp(self):
        turn = replay.reported_heading(1.0, 2.0, 2, 100)
        self.assertAlmostEqual(turn, 1.0 + 0.033 / 0.008 * 0.1)
        self.assertAlmostEqual(replay.reported_heading(1.0, 1.01, 2, 100), 1.01)
        self.assertAlmostEqual(replay.reported_heading(1.0, 0.0, 2, 100), 1.0 - 0.033 / 0.008 * 0.1)

    def test_rotation_packet_types_refresh_actual_heading_only_when_present(self):
        self.assertTrue(replay.reports_actual_heading(b"e\0"))
        self.assertTrue(replay.reports_actual_heading(b"d\0"))
        self.assertTrue(replay.reports_actual_heading(b"E\0\0\0\0\0"))
        self.assertFalse(replay.reports_actual_heading(b"E\0"))
        self.assertFalse(replay.reports_actual_heading(b"4\0"))
        self.assertFalse(replay.reports_actual_heading(b"3\0"))
        self.assertTrue(replay.reports_wanted_heading(b"E\0"))
        self.assertTrue(replay.reports_wanted_heading(b"5\0\0"))
        self.assertFalse(replay.reports_wanted_heading(b"e\0"))
        self.assertFalse(replay.reports_wanted_heading(b"3\0"))

        # Exercise the same short protocol-19 variants through the real codec.
        world = sp.World()
        world.self_id = 1
        world.snakes[1] = sp.Snake(1, [[0.0, 0.0], [42.0, 0.0]], 0.0, 0.0, 5.76, 0.5)
        world.feed(b"E\x10", 10)
        self.assertAlmostEqual(world.snakes[1].wang, 16 * math.tau / 256)
        self.assertEqual(world.snakes[1].ang, 0.0)
        world.feed(b"5\x20\x30", 20)
        self.assertAlmostEqual(world.snakes[1].ang, 32 * math.tau / 256)
        self.assertAlmostEqual(world.snakes[1].wang, 48 * math.tau / 256)

    def test_coordinate_replacement_gets_new_id_and_self_pickup_remains_target(self):
        clip = self.export(archive_records())
        initial_id = clip["initial"]["food"][0]["id"]
        self.assertEqual(initial_id, "1/7/1")
        self.assertEqual(
            [(e["kind"], e["id"], e["reason"]) for e in clip["food_events"]],
            [
                ("remove", "1/7/1", "replacement"),
                ("add", "1/7/2", "observed-appearance"),
            ],
        )
        self.assertEqual(clip["expected_pickups"], [{"time": 0.2, "id": "1/7/2"}])
        self.assertFalse(
            any(e["id"] == "1/7/2" and e["kind"] == "remove" for e in clip["food_events"])
        )

    def test_bulk_food_changed_size_replaces_generation(self):
        records = archive_records(replacement=False)
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 300).packets.append(b"F")
        clip = self.export(records)
        self.assertEqual(
            [(e["kind"], e["id"]) for e in clip["food_events"]],
            [
                ("remove", "1/7/1"),
                ("add", "1/7/2"),
            ],
        )
        self.assertEqual(clip["food_events"][0]["reason"], "replacement")
        self.assertEqual(clip["food_events"][1]["food"]["size"], 8.0)
        self.assertEqual(clip["expected_pickups"], [{"time": 0.2, "id": "1/7/2"}])

    def test_bulk_food_unchanged_snapshot_keeps_generation(self):
        records = archive_records(replacement=False)
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 300).packets.append(b"U")
        clip = self.export(records)
        self.assertEqual(clip["food_events"], [])
        self.assertEqual(clip["expected_pickups"], [{"time": 0.2, "id": "1/7/1"}])

    def test_rejects_partial_archive_and_clip_without_live_frame_beyond_end(self):
        with self.assertRaisesRegex(ValueError, "capture-end"):
            self.export(archive_records(footer=False))
        with self.assertRaisesRegex(ValueError, "not fully covered"):
            self.export(archive_records(last=400))

    def test_rejects_death_inside_clip(self):
        with self.assertRaisesRegex(ValueError, "includes death"):
            self.export(archive_records(death=True), duration=0.35)

    def test_self_removal_rejects_a_clip_even_without_game_over(self):
        for reappear in (False, True):
            with self.subTest(reappear=reappear):
                records = archive_records(replacement=False)
                for record in records:
                    if isinstance(record, sp.Frame) and record.server >= 300:
                        record.packets = [b"p"]
                        if record.server == 300:
                            record.packets = [b"s\x00\x01\x01"]
                            if reappear:
                                record.packets.append(b"Q")
                with self.assertRaisesRegex(ValueError, "self removal"):
                    self.export(records)

    def test_rejects_self_pickup_without_observed_food_identity(self):
        records = archive_records(replacement=False)
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 300).packets.append(b"X")
        with self.assertRaisesRegex(ValueError, "Self pickup of unobserved food"):
            self.export(records)

    def test_self_prey_pickup_is_reported_as_unmodeled(self):
        records = archive_records(replacement=False)
        next(r for r in records if isinstance(r, sp.Frame) and r.server == 300).packets.append(b"P")
        clip = self.export(records)
        self.assertEqual(
            clip["provenance"]["unmodeled_self_prey_pickups"], [{"time": 0.1, "id": 9}]
        )
        self.assertEqual(clip["expected_pickups"], [{"time": 0.2, "id": "1/7/1"}])


class PrepareReplayArchiveTest(unittest.TestCase):
    """Small complete gzip archives through the actual reader and world codec."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "capture.jsonl.gz"
        setup = bytearray(30)
        setup[:8] = (
            b"a" + (21600).to_bytes(3, "big") + (430).to_bytes(2, "big") + (300).to_bytes(2, "big")
        )
        setup[21:25] = (430).to_bytes(2, "big") + bytes([19, 42])
        setup[27:30] = (21000).to_bytes(3, "big")
        # Two straight body anchors, zero headings and unsaturated mass.
        spawn = (
            b"s\0\1"
            + bytes(7)
            + (5760).to_bytes(2, "big")
            + (8388607).to_bytes(3, "big")
            + b"\0"
            + (420).to_bytes(3, "big")
            + bytes(3)
            + bytes(3)
            + bytes(6)
            + b"\0\0"
        )
        self.records = [
            {"type": "meta", "event": "connect"},
            self.frame(0, setup, 100.0),
            self.frame(0, spawn, 100.0),
        ]
        self.records.extend(
            self.frame(100, b"G\0\0", float(server + 100)) for server in range(100, 801, 100)
        )
        self.records.insert(5, ["o", 350.0, base64.b64encode(b"\x40").decode()])
        self.records.append({"type": "meta", "event": "capture-end"})

    @staticmethod
    def frame(gap, packet, timestamp):
        return ["i", timestamp, base64.b64encode(gap.to_bytes(2, "big") + packet).decode()]

    def write_archive(self):
        with gzip.GzipFile(filename=str(self.path), mode="wb", mtime=0) as stream:
            for record in self.records:
                stream.write((json.dumps(record) + "\n").encode())

    def export(self):
        self.write_archive()
        return replay.export(self.path, 1, 0.2, 0.25, "test")

    def test_valid_archive_maps_control_and_allows_equal_receipt_times(self):
        clip = self.export()  # Setup and spawn both arrive at 100 ms.
        self.assertAlmostEqual(clip["controls"][0]["time"], 0.05)
        self.assertEqual([row["time"] for row in clip["observations"]], [0.1, 0.2])

    def test_provenance_pins_protocol_tools_and_audit_limit(self):
        clip = self.export()
        provenance = clip["provenance"]
        self.assertEqual(clip["schema_version"], 1)
        self.assertEqual(provenance["wire_protocol"], 19)
        self.assertEqual(
            provenance["decoder_sha256"], hashlib.sha256(Path(sp.__file__).read_bytes()).hexdigest()
        )
        self.assertEqual(
            provenance["exporter_sha256"],
            hashlib.sha256(Path(replay.__file__).read_bytes()).hexdigest(),
        )
        self.assertEqual(provenance["capture_audit"], "not-performed-by-exporter")

    def test_other_protocol_is_rejected_instead_of_labelled_as_19(self):
        setup = bytearray(base64.b64decode(self.records[1][2])[2:])
        setup[23] = 11
        self.records[1] = self.frame(0, setup, 100.0)
        with self.assertRaisesRegex(ValueError, "Unsupported protocol version 11"):
            self.export()

    def test_source_digest_identifies_decoded_bytes_if_original_is_replaced(self):
        self.write_archive()
        source = self.path.read_bytes()
        original_read = sp.read

        def replace_after_read(snapshot):
            yield from original_read(snapshot)
            self.path.write_bytes(b"replacement after archive decode")

        with patch.object(replay.sp, "read", side_effect=replace_after_read):
            clip = replay.export(self.path, 1, 0.2, 0.25, "test")
        self.assertEqual(clip["provenance"]["sha256"], hashlib.sha256(source).hexdigest())
        self.assertNotEqual(self.path.read_bytes(), source)

    def test_rejects_regressed_incoming_receipt_clock(self):
        self.records[6][1] = 250.0
        with self.assertRaisesRegex(ValueError, "receipt clock regressed"):
            self.export()

    def test_rejects_regressed_outgoing_clock_before_it_changes_initial_action(self):
        self.records[5][1] = 150.0
        with self.assertRaisesRegex(ValueError, "receipt clock regressed"):
            self.export()

    def test_rejects_invalid_receipt_clock_in_both_directions(self):
        for index in (5, 6):
            for timestamp in (math.nan, math.inf, -math.inf, True, None, "400"):
                with self.subTest(index=index, timestamp=timestamp):
                    previous = self.records[index][1]
                    self.records[index][1] = timestamp
                    try:
                        with self.assertRaisesRegex(ValueError, "invalid receipt clock"):
                            self.export()
                    finally:
                        self.records[index][1] = previous

    def test_rejects_second_setup_epoch_in_selected_connection(self):
        setup = base64.b64decode(self.records[1][2])[2:]
        self.records.insert(6, self.frame(65535, setup, 390.0))
        with self.assertRaisesRegex(ValueError, "multiple game setup packets"):
            self.export()

    def test_rejects_duplicate_setup_even_before_server_clock_advances(self):
        self.records.insert(2, list(self.records[1]))
        with self.assertRaisesRegex(ValueError, "multiple game setup packets"):
            self.export()

    def test_rejects_two_setup_packets_in_one_frame(self):
        setup = base64.b64decode(self.records[1][2])[2:]
        payload = bytes(2) + len(setup).to_bytes(2, "big") + setup
        payload += bytes([len(setup) + 32]) + setup
        self.records[1][2] = base64.b64encode(payload).decode()
        with self.assertRaisesRegex(ValueError, "multiple game setup packets"):
            self.export()

    def test_other_connection_clock_reset_does_not_change_selected_clip(self):
        baseline = self.export()
        self.records.insert(-1, {"type": "meta", "event": "connect"})
        self.records.insert(-1, self.frame(65535, base64.b64decode(self.records[1][2])[2:], 950.0))
        self.records.insert(-1, ["o", 50.0, base64.b64encode(b"\x80").decode()])
        clip = self.export()
        for key in ("initial", "controls", "observations"):
            self.assertEqual(clip[key], baseline[key])

    def test_equal_time_duplicate_anchor_is_preserved_without_sequence_identity(self):
        self.records.insert(7, list(self.records[6]))
        clip = self.export()
        self.assertEqual([row["anchor_index"] for row in clip["observations"]], [1, 2])
        first, second = [row["anchor"]["x"] for row in clip["observations"]]
        self.assertAlmostEqual(second - first, 42.0)

    def test_rejects_records_after_terminal_capture_receipt(self):
        self.records.insert(5, self.records.pop())
        with self.assertRaisesRegex(ValueError, "records after terminal"):
            self.export()

    def test_rejects_interrupted_receipt_followed_by_completion(self):
        self.records.insert(-1, {"type": "meta", "event": "capture-interrupted"})
        with self.assertRaisesRegex(ValueError, "records after terminal"):
            self.export()

    def test_rejects_truncated_gzip_even_with_completed_clip_and_receipt(self):
        self.write_archive()
        self.path.write_bytes(self.path.read_bytes()[:-8])
        with self.assertRaises(EOFError):
            replay.export(self.path, 1, 0.2, 0.25, "test")


if __name__ == "__main__":
    unittest.main()
