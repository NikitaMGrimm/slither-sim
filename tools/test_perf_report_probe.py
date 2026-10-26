"""Selected metrics must be finite, balanced and independent of comparison pairs."""
import json
import math
import os
from pathlib import Path
import tempfile
import unittest

import perf_report_probe
import pr_report
from test_perf_baseline import catalog, row


class ProbeTests(unittest.TestCase):
    def test_residual_removes_order_and_rejects_partial_or_missing_cpu(self):
        pairs = [{label: {"workload": {"timing": {"cpuSeconds": value}}}
                  for label, value in (("baseline", 1), ("candidate", math.exp(.02 * order + noise)))}
                 for order, noise in zip(perf_report_probe.ORDER, [.01, -.01, .02, -.02, .01, -.01])]
        self.assertGreater(perf_report_probe.residual_sd(pairs, "cpuSeconds"), 0)
        self.assertIsNone(perf_report_probe.residual_sd(pairs[:5], "cpuSeconds"))
        pairs[0]["candidate"]["workload"]["timing"]["cpuSeconds"] = None
        self.assertIsNone(perf_report_probe.residual_sd(pairs, "cpuSeconds"))

    def test_head_configs_pin_world_and_restore_environment(self):
        listed = catalog([row("official-100-step", definition={"world": {"kind": "official", "snakes": 100},
                                                                "config": {"radius": 1234, "dynamicRadius": True}})])
        before = os.environ.get("SLITHER_PERF_CONFIGS")
        with tempfile.TemporaryDirectory() as temporary:
            with pr_report.head_configs(listed, Path(temporary)):
                data = json.loads(Path(os.environ["SLITHER_PERF_CONFIGS"]).read_text())
                self.assertEqual(data["world:Official(100)"], {"radius": 1234, "dynamicRadius": True})
            self.assertEqual(os.environ.get("SLITHER_PERF_CONFIGS"), before)


if __name__ == "__main__":
    unittest.main()
