"""Selected metrics must be finite, balanced and independent of comparison pairs."""
import json
import math
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import perf_report_probe
import pr_report
import perf_compare
from test_perf_baseline import catalog, row


class ProbeTests(unittest.TestCase):
    def test_probe_retains_early_eof_as_failed_cell_without_retry(self):
        listed = catalog([row("socket", mode="socket-host")])
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(perf_report_probe, "digest", return_value="d" * 64), \
                patch.object(perf_report_probe, "counter_probe", return_value={}), \
                patch.object(perf_report_probe, "native", side_effect=EOFError("child pipe closed")) as native:
            result = perf_report_probe.probe(Path("synthetic"), listed.select(), Path(temporary) / "probe",
                                            deadline=time.perf_counter() + 10)
        native.assert_called_once()
        self.assertEqual(result["workloads"]["socket"]["complete_pairs"], 0)
        self.assertEqual(result["workloads"]["socket"]["failure"], "child pipe closed")
        self.assertEqual(result["workloads"]["socket"]["selected"], "wallSeconds")

    def test_cpu_inference_keeps_true_wall_values_in_raw_pairs(self):
        listed = catalog([row("socket", mode="socket-host")])
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            binaries = {label: output / label for label in ("baseline", "candidate")}
            for binary in binaries.values():
                binary.write_bytes(b"synthetic immutable executable")
            calls = {label: 0 for label in binaries}

            def native(binary, case, iterations, stderr, **kwargs):
                label = binary.name
                index = calls[label]
                calls[label] += 1
                cpu = 1. if label == "baseline" else .8 * math.exp([.001, -.002, .003, -.004][index % 4])
                wall = 1. if label == "baseline" else 2.
                return {"workload": {"kind": "report", "case": case, "mode": "timing", "iterations": iterations,
                                     "timing": {"operations": iterations, "wallSeconds": wall, "cpuSeconds": cpu}}}

            with patch.object(perf_compare, "native", side_effect=native):
                results = perf_compare.sequential(binaries, {k: perf_report_probe.digest(v) for k, v in binaries.items()},
                                                 listed.select(), output, time.perf_counter() + 10,
                                                 {"metrics": {"socket": "cpuSeconds"}})
            result = results["socket"]
            self.assertEqual(result["label"], "Faster")
            self.assertEqual(result["result"]["metric"], "cpuSeconds")
            self.assertAlmostEqual(result["result"]["candidate_busy_wall_ms"], 20.)
            raw = [json.loads(line) for line in (output / "pairs.jsonl").read_text().splitlines()]
            self.assertTrue(all(pair["candidate_wall_seconds"] == 2. for pair in raw))

    def test_changed_definition_retains_head_value_without_a_comparison_claim(self):
        listed = catalog([row("changed")])
        old = catalog([row("changed", definition={"config": {"radius": 2000}})])
        for baseline in (old.select(), ()):
            suites = {"candidate": listed.select(), "baseline": baseline}
            report = {"errors": [], "phases_seconds_after_job_start": {}}
            with tempfile.TemporaryDirectory() as temporary, \
                    patch("perf_baseline.native", return_value={"workload": {"kind": "report", "case": "changed", "mode": "timing",
                             "iterations": 100, "timing": {"operations": 100, "wallSeconds": .25}}}), \
                    patch.object(perf_compare, "sequential", return_value={}), patch.object(pr_report, "digest", return_value="d" * 64):
                pr_report.measure(report, {"candidate": Path("synthetic")}, {},
                                  {"candidate": listed, "baseline": old}, suites, Path(temporary) / "performance", time.monotonic())
            result = report["performance"][0]
            self.assertEqual(result["label"], "Redefined")
            self.assertIsNone(result["result"])
            self.assertEqual(result["head_ms_per_operation"], 2.5)
            self.assertFalse(report["errors"])

    def test_socket_local_suite_does_not_attempt_a_nondeterministic_trace(self):
        listed = catalog([row("socket", mode="socket-host")])
        with patch.object(perf_compare, "capture_trace") as trace:
            rows = perf_compare.parity({}, {"candidate": listed.select(), "baseline": listed.select()}, Path("unused"))
        trace.assert_not_called()
        self.assertIn("Not applicable", rows[0]["reading"])

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
