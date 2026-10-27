"""Linux wrapper validity checks; synthetic telemetry, no sockets or server."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("bench", Path(__file__).with_name("bench.py"))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class AffinityTests(unittest.TestCase):
    def sample(self, allowed, last):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "telemetry.json"
            path.write_text(json.dumps({"epoch": 0, "rows": [{"unix": 1, "threads": {"10": {"11": {
                "cpus": allowed, "lastCpu": last}}}}]}))
            return bench.affinity(path, {0})

    def test_thread_pinned_and_observed_on_host_core(self):
        self.assertEqual(self.sample([0], 0), {"threadSamples": 1,
            "allowedCpuUnion": [0], "lastCpuUnion": [0], "violations": 0})

    def test_wider_affinity_fails_even_if_last_cpu_was_zero(self):
        self.assertEqual(self.sample([0, 1], 0)["violations"], 1)

    def test_wrong_observed_cpu_fails_even_with_correct_mask(self):
        self.assertEqual(self.sample([0], 1)["violations"], 1)

    def test_pre_epoch_taskset_sample_is_outside_workload(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "telemetry.json"
            path.write_text(json.dumps({"epoch": 10, "rows": [
                {"unix": 9, "threads": {"10": {"10": {"cpus": [1, 2, 3], "lastCpu": 1}}}},
                {"unix": 10, "threads": {"10": {"10": {"cpus": [0], "lastCpu": 0}}}}]}))
            result = bench.affinity(path, {0})
            self.assertEqual(result["threadSamples"], 1)
            self.assertEqual(result["violations"], 0)


if __name__ == "__main__":
    unittest.main()
