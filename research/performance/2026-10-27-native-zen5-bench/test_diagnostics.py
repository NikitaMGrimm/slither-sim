import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("diagnostics", Path(__file__).with_name("diagnostics.py"))
d = importlib.util.module_from_spec(spec)
spec.loader.exec_module(d)


class CpuClockTests(unittest.TestCase):
    def test_aws_field_screen_does_not_treat_client_time_as_system_time(self):
        self.assertEqual(d.cpu_split_fields({"clientsTimeWeighted": {}, "hostCpuPercent": 50}), [])
        self.assertEqual(d.cpu_split_fields({"nested": [{"userCpuSeconds": 2, "stime": 3}]}),
                         ["stime", "userCpuSeconds"])

    def fixture(self, root, split=True):
        (root / "host").mkdir()
        (root / "host/host.json").write_text(json.dumps({"rows": [
            {"boundary": "start", "ticks": 100}, {"boundary": "end", "ticks": 120}]}))
        rows = []
        for i, cpu in enumerate([10, 10.14]):
            process = {"cpuSeconds": cpu}
            if split:
                process.update(userCpuSeconds=8 + i * .10, systemCpuSeconds=2 + i * .04)
            rows.append({"boundary": ["start", "end"][i], "unix": 10 + i,
                         "processes": {"1": process}, "diagnostic": {"cpu0Mhz": 2600 + i,
                         "cpu0Counters": f"cpu0 {100+i*9} 0 0 0 0 0 0 {i}"}})
        (root / "host/telemetry.json").write_text(json.dumps({"rows": rows}))

    def test_separate_cpu_counters_use_executed_ticks_and_same_boundaries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            r = d.cpu_clock(root)
            self.assertAlmostEqual(r["userMsPerExecutedTick"], 5)
            self.assertAlmostEqual(r["systemMsPerExecutedTick"], 2)
            self.assertAlmostEqual(r["totalMsPerExecutedTick"], 7)
            self.assertEqual(r["cpu0StealPercent"], 10)
            self.assertEqual(r["observedCpu0Mhz"]["min"], 2600)
            self.assertEqual(r["observedCpu0Mhz"]["max"], 2601)

    def test_combined_historical_counter_is_not_invented_as_a_split(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root, split=False)
            r = d.cpu_clock(root)
            self.assertEqual(r["status"], "unavailable")
            self.assertIsNone(r["userMsPerExecutedTick"])
            self.assertIsNone(r["systemMsPerExecutedTick"])

    def test_inconsistent_split_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            p = root / "host/telemetry.json"
            data = json.loads(p.read_text())
            data["rows"][1]["processes"]["1"]["systemCpuSeconds"] += 1
            p.write_text(json.dumps(data))
            with self.assertRaisesRegex(RuntimeError, "do not sum"):
                d.cpu_clock(root)


class AnomalyTests(unittest.TestCase):
    def test_increment_census_windows_and_half_open_tick_boundaries(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for role in ["host", "load"]:
                (root / role).mkdir()
            counters, resources = [], []
            for i in range(31):
                counters.append({"unix": 100 + i, "deferred": 0 if i < 10 else 2 if i < 11 else 4})
                resources.append({"unix": 100 + i,
                    "processes": {"1": {"cpuSeconds": i / 2, "rssBytes": 1024**2}},
                    "cpu": f"cpu {1000+i*10} 0 0 {1000+i*10} 0 0 0 0 0 0",
                    "threads": {"1": {"1": {"name": "host", "cpuSeconds": i / 2, "cpus": [0]}}}})
            (root / "host/host.json").write_text(json.dumps({
                "holdStartUnix": 100, "holdEndUnix": 130, "rows": counters}))
            for role in ["host", "load"]:
                (root / role / "telemetry.json").write_text(json.dumps({"rows": resources}))
            (root / "host/ticks.json").write_text(json.dumps([100, [
                [8999000000, 1000000, 7000000], [9000000000, 2000000, 7000000],
                [10999000000, 3000000, 7000000], [11000000000, 4000000, 7000000]]]))
            r = d.anomaly(root)
            self.assertEqual(r["deferredTicks"], 4)
            self.assertEqual([x["deferredTicks"] for x in r["sampledDeferralIncrements"]], [2, 2])
            before, episode, after = r["windows"]
            self.assertEqual((episode["startSeconds"], episode["endSeconds"]), (9, 11))
            self.assertEqual(before["tickBusyMs"]["count"], 1)
            self.assertEqual(episode["tickBusyMs"]["count"], 2)
            self.assertEqual(episode["tickBusyMs"]["max"], 3)
            self.assertEqual(after["tickBusyMs"]["min"], 4)
            self.assertEqual(episode["host"]["cpuPercentOneCore"], 50)
            self.assertEqual(episode["host"]["processCpuPercentByPinnedCore"], {"0": 50})


if __name__ == "__main__":
    unittest.main()
