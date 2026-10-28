"""Authored analysis and client-probe regressions; never opens a game socket."""
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import unittest

spec = importlib.util.spec_from_file_location("turn_analysis", Path(__file__).with_name("analyze.py"))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Models(unittest.TestCase):
    def test_equivalent_count_is_size_dependent(self):
        self.assertEqual(m.rate(2), 4.125)
        for n in (9, 37, 100, 174, 244, 293):
            self.assertAlmostEqual(m.rate(n, 111), m.rate(2 + 106 * (n - 2) / 111), places=14)
        self.assertGreater(m.rate(174, 111), m.rate(174, 106))

    def test_conditional_budget_boundary(self):
        q = m.rate(174, 111)
        inside = q * 10 + m.A - 1e-5
        outside = q * 10 + m.A + q * .001 + 1e-5
        self.assertTrue(m.score(inside, 10, q)["admitted_at_jitter_ms"]["0"])
        self.assertFalse(m.score(outside, 10, q)["admitted_at_jitter_ms"]["0"])
        self.assertTrue(m.score(outside, 10, q)["admitted_at_jitter_ms"]["7"])

    def test_epoch_offset_cancels_midpoint_duration(self):
        times = [100, 220, 350, 480]
        midpoint_span = lambda ts: (ts[-1] + ts[-2] - ts[1] - ts[0]) / 2000
        self.assertEqual(midpoint_span(times), midpoint_span([t + 65535 for t in times]))

    def test_declared_geometry_budget(self):
        self.assertAlmostEqual(m.A, 2 * math.asin(math.sqrt(2) / 39) + math.tau / 65536)
        self.assertGreater(m.A, .07)

    def test_point_fit_does_not_reject_all_fixed_offsets(self):
        fixture = json.loads((m.REPO / "tests/fixtures/movement-turn-runs.json").read_text())
        result = m.calculate(fixture)
        self.assertEqual(result["fitted_global_offset"], 9)
        self.assertEqual(result["global_offsets_admitted_all_runs_by_jitter_ms"]["7"], [7, 8])


@unittest.skipUnless(os.environ.get("TURN_COUNT_BINARY"), "set TURN_COUNT_BINARY for the authored native probes")
class ClientGeometry(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        m.limit_cpu()
        cls.probe = json.loads(subprocess.check_output([os.environ["TURN_COUNT_BINARY"], "geometry"], text=True))

    def test_insertion_order_has_distinct_geometry(self):
        self.assertEqual(len(self.probe["cases"]), 4)
        for case in self.probe["cases"]:
            self.assertTrue(case["client_insert_contract_exact"])
            self.assertGreater(case["live_point_max_vs_contract_insert"], 8)
            self.assertLess(case["live_point_max_vs_contract_insert"], 9)

    def test_logical_count_excludes_dying_retention(self):
        for case in self.probe["cases"]:
            after = case["after"]
            self.assertEqual(after["logical"], case["segments"] + int(case["grew"]))
            self.assertEqual(after["physical"] - after["dying"], after["logical"])
            self.assertEqual(after["logical"], case["server_physical_count"])

    def test_mass_scale_and_displayed_length_have_separate_clocks(self):
        for case in self.probe["lifecycle"]:
            rows = {r["action"]: r["count"] for r in case["rows"]}
            self.assertEqual(rows["appearance"]["scang"], rows["mass_only"]["scang"])
            self.assertEqual(rows["appearance"]["displayed_length"], rows["mass_only"]["displayed_length"])
            self.assertEqual(rows["mass_only"]["displayed_length"], rows["advance_8ms"]["displayed_length"])
            self.assertNotEqual(rows["mass_only"]["displayed_length"], rows["advance_16ms"]["displayed_length"])
            self.assertNotEqual(rows["mass_only"]["scang"], rows["growth"]["scang"])
            self.assertAlmostEqual(rows["advance_to_6000ms"]["displayed_length"], case["initial_segments"] + .3, places=12)


if __name__ == "__main__":
    unittest.main()
