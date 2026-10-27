"""Authored synthetic regressions: corrections, event clocks, identity and rejection."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import unittest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("replay", HERE / "replay.py")
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)


def u24(value):
    return int(value).to_bytes(3, "big")


def setup():
    p = b"a" + u24(10000)
    p += b"".join(n.to_bytes(2, "big") for n in (430, 100, 200))
    p += bytes([48])
    p += b"".join(n.to_bytes(2, "big") for n in (539, 40, 1400, 33, 29, 430))
    return p + bytes([19, 42])


def snake(identity, x):
    # Synthetic front at x; body encoded tail-to-front, last edge by direction.
    p = b"s" + identity.to_bytes(2, "big") + u24(0) + b"0" + u24(0)
    p += (5790).to_bytes(2, "big") + u24(0) + bytes([0])
    p += u24(x * 5) + u24(50000) + bytes([0, 0, 255])
    return p + u24((x - 84) * 5) + u24(50000) + bytes([211, 127, 0, 0])


def anchor(identity, x):
    return b"=" + identity.to_bytes(2, "big") + bytes([0, 0]) + x.to_bytes(2, "big") + (10000).to_bytes(2, "big")


def frame(commands, elapsed=7):
    return list(elapsed.to_bytes(2, "big") + b"".join(len(p).to_bytes(2, "big") + p for p in commands))


@unittest.skipUnless(os.environ.get("CLIENT_VIEW_BINARY"), "set CLIENT_VIEW_BINARY to the built research executable")
class Integration(unittest.TestCase):
    def run_rows(self, records, ok=True):
        replay.limit_cpus()
        result = subprocess.run([os.environ["CLIENT_VIEW_BINARY"], "receipt", "8"],
            input="".join(json.dumps(r) + "\n" for r in records), text=True, capture_output=True)
        self.assertEqual(result.returncode == 0, ok, result.stderr)
        return [json.loads(line) for line in result.stdout.splitlines()]

    def initial(self, connection=1):
        return dict(line=1, connection=connection, t_ms=0,
                    bytes=frame([setup(), snake(1, 10000), snake(2, 10100)]))

    def test_anchor_smoothing_and_front_identity(self):
        rows = self.run_rows([self.initial(), dict(line=2, connection=1, t_ms=32,
            bytes=frame([anchor(2, 10200)]))])
        r = next(r for r in rows if r["kind"] == "snake_anchor")
        # Four 8ms updates travel exactly the announced 5.79 units.
        self.assertAlmostEqual(r["head_innovation"], 100 - 5.79, places=7)
        self.assertLess(r["head_immediate_display_shift"], 1e-4)
        self.assertEqual(r["body_existing_points"], 3)
        self.assertTrue(r["stratum"]["visible"])
        self.assertEqual(r["id"], 2)  # Own anchors must never enter peer metrics.

    def test_connection_reuse_cannot_pair_old_identity(self):
        rows = self.run_rows([self.initial(), dict(line=2, connection=2, t_ms=10,
            bytes=frame([anchor(2, 10200)])), dict(line=3, connection=2, t_ms=20,
            bytes=frame([setup(), snake(2, 10200)])),
            dict(line=4, connection=2, t_ms=52, bytes=frame([anchor(2, 10300)]))])
        self.assertFalse(any(r["kind"] == "snake_anchor" for r in rows))
        self.assertEqual(rows[-1]["setups"], 2)

    def test_bad_trailing_packet_rejects_whole_frame(self):
        rows = self.run_rows([self.initial(), dict(line=2, connection=1, t_ms=32,
            bytes=frame([anchor(2, 10200), b"g"]))], ok=False)
        self.assertFalse(any(r["kind"] == "snake_anchor" for r in rows))

    def test_regressing_receipt_rejected(self):
        self.run_rows([self.initial(), dict(line=2, connection=1, t_ms=-1,
            bytes=frame([anchor(2, 10200)]))], ok=False)

    def test_game_over_gates_following_bundled_samples(self):
        rows = self.run_rows([self.initial(), dict(line=2, connection=1, t_ms=32,
            bytes=frame([b"v\x00", anchor(2, 10200)]))])
        r = next(r for r in rows if r["kind"] == "snake_anchor")
        self.assertFalse(r["own_alive"])

    def test_non_death_own_removal_gates_retained_object(self):
        removal = b"s" + (1).to_bytes(2, "big") + bytes([0])
        rows = self.run_rows([self.initial(), dict(line=2, connection=1, t_ms=32,
            bytes=frame([removal, anchor(2, 10200)])),
            dict(line=3, connection=1, t_ms=64, bytes=frame([anchor(2, 10300)]))])
        peers = [r for r in rows if r["kind"] == "snake_anchor"]
        self.assertEqual(len(peers), 2)
        self.assertTrue(all(not r["own_alive"] for r in peers))

    def test_prey_prediction_and_present_speed_only(self):
        spawn = (b"y" + (8).to_bytes(2, "big") + bytes([0]) + u24(50500)
                 + u24(50000) + bytes([20]) + b"0" + u24(0) + u24(0)
                 + (3800).to_bytes(2, "big"))
        motion = (b"j" + (8).to_bytes(2, "big") + (3370).to_bytes(2, "big")
                  + (3333).to_bytes(2, "big") + (6153).to_bytes(2, "big"))
        first = self.initial()
        first["bytes"] = frame([setup(), snake(1, 10000), spawn])
        rows = self.run_rows([first, dict(line=2, connection=1, t_ms=32,
            bytes=frame([motion]))])
        r = next(r for r in rows if r["kind"] == "prey_motion")
        self.assertAlmostEqual(r["innovation"], 7.2, places=7)
        self.assertAlmostEqual(r["speed_error_per_32ms"], 2.353, places=9)
        self.assertNotIn("heading_error_rad", r)
        self.assertLess(r["immediate_display_shift"], 1e-4)


class Statistics(unittest.TestCase):
    def test_nearest_rank_and_missing_exposure(self):
        self.assertEqual(replay.quantiles([3, 1, 2])["p50"], 2)
        self.assertIsNone(replay.quantiles([])["p95"])
        self.assertEqual(replay.quantiles([1, 2, 3])["p95"], 3)


if __name__ == "__main__":
    unittest.main()
