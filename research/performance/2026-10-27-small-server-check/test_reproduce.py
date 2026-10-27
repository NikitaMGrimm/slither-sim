"""Focused validity checks for the study reducer, also run with python -O."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("study", Path(__file__).with_name("reproduce.py"))
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)


class ReducerTests(unittest.TestCase):
    def test_nearest_rank_includes_extreme_tick(self):
        result = study.quantiles(list(range(1, 101)))
        self.assertEqual(result, {"n": 100, "p50": 50, "p99": 99, "max": 100})
        with self.assertRaises(RuntimeError):
            study.quantiles([])

    def test_scheduling_gate_cannot_accept_unscheduled_counter(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "counter.json"
            for enabled, running, valid in [(100, 99, True), (100, 98, False), (0, 0, False)]:
                path.write_text(json.dumps({"instructions": 1000, "enabled": enabled, "running": running}))
                if valid:
                    self.assertEqual(study.counter(path)["instructions"], 1000)
                else:
                    with self.assertRaises(RuntimeError):
                        study.counter(path)

    def test_pairing_retains_warmup_but_does_not_select_it(self):
        rows = [{"pair": i, "base": {"instructions": b}, "candidate": {"instructions": c}}
                for i, b, c in [(-1, 1, 999), (0, 100, 80), (1, 200, 180), (2, 400, 400)]]
        result = study.paired(rows)
        self.assertEqual(result["ratioMedian"], .9)
        self.assertEqual(result["ratioRange"], [.8, 1.])
        self.assertEqual(len(result["pairs"]), 4)


if __name__ == "__main__":
    unittest.main()
