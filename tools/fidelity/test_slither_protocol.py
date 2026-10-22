import base64
import gzip
import json
import math
import tempfile
import unittest
from pathlib import Path

import slither_protocol as sp


class ProtocolTest(unittest.TestCase):
    def test_captured_setup_uses_protocol_19_and_preserves_wire_constants(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/protocol19-setup.json").read_text())
        world = sp.World()
        world.feed(bytes.fromhex(fixture["setupPrefixHex"]), 0)
        expected = fixture["expected"]
        self.assertEqual(world.grd, expected["grd"])
        self.assertEqual(world.mscps, expected["mscps"])
        self.assertEqual(world.sector_size, expected["sectorSize"])
        self.assertEqual(world.cst, expected["cstWire"] / 1000)
        self.assertEqual(world.msl, expected["msl"])
        self.assertEqual(world.radius, expected["radius"])

    def test_other_protocols_are_rejected_before_setup_changes_state(self):
        fixture = json.loads((Path(__file__).parent / "fixtures/protocol19-setup.json").read_text())
        for version in (11, 18, 20):
            with self.subTest(version=version):
                setup = bytearray.fromhex(fixture["setupPrefixHex"])
                setup[23] = version
                world = sp.World()
                previous = vars(world).copy()
                with self.assertRaisesRegex(ValueError, f"Unsupported protocol version {version}"):
                    world.feed(setup, 0)
                self.assertEqual(vars(world), previous)

    def test_short_setup_is_rejected_without_partial_state_changes(self):
        world = sp.World()
        previous = vars(world).copy()
        with self.assertRaisesRegex(ValueError, "short setup packet"):
            world.feed(b"a\x00\x00\x01", 0)
        self.assertEqual(vars(world), previous)

    def test_setup_header_starts_game_clock_instead_of_adding_challenge_gap(self):
        for setup_header in (3255, 65535):
            with self.subTest(setup_header=setup_header), tempfile.TemporaryDirectory() as temp:
                path = Path(temp) / "wire.jsonl.gz"
                wire = lambda gap, packet: base64.b64encode(
                    gap.to_bytes(2, "big") + packet
                ).decode()
                records = [
                    {"type": "meta", "event": "connect"},
                    ["i", 100.0, wire(0, b"6")],
                    ["i", 112.0, wire(setup_header, b"a")],
                    ["i", 119.0, wire(7, b"e")],
                ]
                with gzip.open(path, "wt") as archive:
                    for row in records:
                        archive.write(json.dumps(row) + "\n")
                frames = [row for row in sp.read(path) if isinstance(row, sp.Frame)]
                self.assertEqual([row.server for row in frames], [0, 0, 7])

    def test_split_uses_elapsed_header_and_length_prefixes(self):
        self.assertEqual(
            sp.split(bytes([0, 7, 0, 3, 101, 1, 2, 35, 104, 0, 9])),
            [bytes([101, 1, 2]), bytes([104, 0, 9])],
        )
        self.assertEqual(sp.split(bytes([0, 7, 112])), [bytes([112])])

    def test_split_rejects_truncated_and_empty_packets(self):
        for data in (
            b"",
            b"\x00\x07",
            b"\x00\x07\x00",
            b"\x00\x07\x00\x00",
            b"\x00\x07\x00\x03e",
            b"\x00\x07\x00\x01p\x23e",
        ):
            with self.subTest(data=data), self.assertRaises(ValueError):
                sp.split(data)

    def test_read_rejects_invalid_base64_and_frame_directions(self):
        for row in (["i", 1, "AAAHcA==!"], ["x", 1, "AAAHcA=="]):
            with self.subTest(row=row), tempfile.TemporaryDirectory() as temp:
                path = Path(temp) / "wire.jsonl.gz"
                with gzip.open(path, "wt") as archive:
                    archive.write(json.dumps(row) + "\n")
                with self.assertRaises(ValueError):
                    list(sp.read(path))

    def test_self_anchors_follow_msl_and_growth_carries_fam(self):
        w = sp.World()
        w.self_id = 1
        w.snakes[1] = sp.Snake(1, [[0.0, 0.0], [42.0, 0.0], [84.0, 0.0]], 0.0, 0.0, 5.79, 0.0)
        quarter = 1 << 14  # 90 degrees in 16-bit anchor angles
        w.feed(bytes([ord("G"), quarter >> 8, quarter & 255]), 100)
        o = w.snakes[1]
        self.assertEqual(o.sct, 3)
        self.assertAlmostEqual(o.pts[-1][0], 84.0)
        self.assertAlmostEqual(o.pts[-1][1], 42.0)
        fam = int(0.25 * 16777215)
        w.feed(
            bytes([ord("N"), quarter >> 8, quarter & 255, fam >> 16, (fam >> 8) & 255, fam & 255]),
            200,
        )
        self.assertEqual(o.sct, 4)
        self.assertAlmostEqual(o.fam, 0.25, places=6)
        # The oldest anchor was pulled toward its neighbor by the client's cst contraction.
        self.assertAlmostEqual(o.pts[0][0], 42.0 + 42.0 * 0.43 / 4)

    def test_head_advances_from_the_last_anchor_on_the_server_clock(self):
        o = sp.Snake(
            1, [[0.0, 0.0]], 0.0, 0.0, 6.4, 0.0, anchor_server=0, hist=[(0, math.pi / 2, 6.4)]
        )
        x, y = o.head(100)
        self.assertAlmostEqual(x, 0.0)
        self.assertAlmostEqual(y, 6.4 / 32 * 100)

    def test_rsc_packet_replaces_only_own_value_and_resets_on_spawn(self):
        w = sp.World(self_id=1)
        own = sp.Snake(1, [[1.0, 2.0]], 0.0, 0.0, 5.7, 0.25)
        other = sp.Snake(2, [[3.0, 4.0]], 0.0, 0.0, 5.7, 0.5)
        w.snakes.update({1: own, 2: other})
        geometry = [point[:] for point in own.pts]
        for clock, value in ((100, 9), (110, 3), (120, 0)):
            w.feed(bytes([ord("R"), value]), clock)
            self.assertEqual(own.rsc, value)
            self.assertEqual(own.pts, geometry)
            self.assertEqual(own.sct, 1)
            self.assertEqual(own.fam, 0.25)
            self.assertEqual(other.rsc, 0)
        self.assertEqual(
            w.events[-3:], [("rsc", 100, 1, 0, 9), ("rsc", 110, 1, 9, 3), ("rsc", 120, 1, 3, 0)]
        )
        w.feed(b"R", 130)
        w.feed(b"R\x08\x09", 140)
        self.assertEqual(own.rsc, 0)
        self.assertEqual(len(w.events), 3)
        w.snakes.pop(1)
        w.feed(b"R\x07", 150)
        self.assertEqual(len(w.events), 3)
        spawn = bytearray(31)
        spawn[0:3] = b"s\x00\x01"
        w.feed(spawn, 160)
        self.assertEqual(w.snakes[1].rsc, 0)


if __name__ == "__main__":
    unittest.main()
