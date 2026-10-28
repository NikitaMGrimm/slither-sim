"""Synthetic negative controls for the evidence selector, not gameplay data."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("scaling_select", Path(__file__).with_name("select_results.py"))
SELECT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SELECT)
PREP_SPEC = importlib.util.spec_from_file_location("scaling_prepare", Path(__file__).with_name("reproduce.py"))
PREP = importlib.util.module_from_spec(PREP_SPEC)
PREP_SPEC.loader.exec_module(PREP)


class WindowGuards(unittest.TestCase):
    def setUp(self):
        self.rows = [{"tick": n, "step": [100, 100, 100], "spawn": [100, 100, 100]} for n in range(1000)]

    def test_complete_synthetic_window(self):
        SELECT.validate_rows(self.rows)

    def test_missing_tick_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "incomplete"):
            SELECT.validate_rows(self.rows[:-1])

    def test_duplicate_tick_is_rejected(self):
        self.rows[500]["tick"] = 499
        with self.assertRaisesRegex(RuntimeError, "noncontiguous"):
            SELECT.validate_rows(self.rows)

    def test_pmu_starvation_is_rejected(self):
        for field in ["step", "spawn"]:
            with self.subTest(field=field):
                rows = copy.deepcopy(self.rows)
                rows[500][field][2] = 98
                with self.assertRaisesRegex(RuntimeError, "invalid PMU"):
                    SELECT.validate_rows(rows)

    def test_censored_control_cannot_become_steady(self):
        with self.assertRaisesRegex(RuntimeError, "unsettled"):
            SELECT.validate_censor({"status": "settled"})

    def test_control_budget_is_aggregate(self):
        status = {"status": "unsettled", "priorChargedSeconds": 3403.74,
                  "isolatedDiagnosticCpuSeconds": 180, "cpuBudgetSeconds": 3600}
        SELECT.validate_censor(status)
        status["isolatedDiagnosticCpuSeconds"] = 200
        with self.assertRaisesRegex(RuntimeError, "budget"):
            SELECT.validate_censor(status)

    def test_invalid_admission_rng_endpoint_is_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "RNG endpoint"):
            SELECT.admission_census({"poses": 50, "rng": 0})

    def test_death_census_handles_boundaries_and_escaped_strings(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "synthetic.json"
            path.write_text(json.dumps({"noise": '\"victim\":not a key',
                "state": {"deaths": [{"victim": "a"}, {"victim": "b"}]}}, separators=(",", ":")))
            self.assertEqual(SELECT.death_record_count(path, chunk_size=5), 2)

    def test_exact_tick_radius_events_expose_late_warmup_update(self):
        updates = [{"time": i*.007} for i in range(100001)]
        PREP.validate_radius_events(updates, 71100)
        with self.assertRaisesRegex(RuntimeError, "tick 98477"):
            PREP.validate_radius_events(updates, 100000)

    def test_radius_event_margin_covers_full_warmup_and_count(self):
        updates = [{"time": 0. if i == 0 else (i-.5)*.007} for i in range(260001)]
        PREP.validate_radius_events(updates, 241100)

    def test_admission_preparation_must_preserve_every_tick_lifecycle(self):
        reference = [{"tick": 1, "attempts": 1, "refusals": 0, "births": 1, "deaths": 1, "alive": 499}]
        SELECT.validate_reference_census(reference, copy.deepcopy(reference))
        for key in ["attempts", "refusals", "births", "deaths", "alive"]:
            measured = copy.deepcopy(reference)
            measured[0][key] += 1
            with self.subTest(key=key), self.assertRaisesRegex(RuntimeError, "population/lifecycle"):
                SELECT.validate_reference_census(reference, measured)


class PublicationGuards(unittest.TestCase):
    def test_nested_metadata_keeps_relative_paths_hashes_and_measurements(self):
        raw = {"kernel": "6.18.54-private", "nested": ["/private/study/results/a.json",
               "/home/example/artifacts/run/rows.json", "/mnt/c/Users/example/Source/slither-sim/src/lib.rs"],
               "sha256": "abc123", "instructions": 1234,
               "reason": "owner CPU-time budget"}
        published = SELECT.publication_view(raw, Path("/private/study"))
        self.assertEqual(published, {"kernel": "6.18.54", "nested": ["<vm-root>/results/a.json",
            "<wsl-artifacts>/run/rows.json", "<repo>/src/lib.rs"], "sha256": "abc123",
            "instructions": 1234, "reason": "maintaining agent CPU-time budget"})
        self.assertEqual(raw["kernel"], "6.18.54-private")

    def test_only_exact_attribution_edit_can_match_historical_probe_pin(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "bounded_probe.rs"
            original = b'let reason = "owner CPU-time budget";'
            expected = hashlib.sha256(original).hexdigest()
            path.write_bytes(original.replace(b"owner CPU-time budget", b"maintaining agent CPU-time budget"))
            SELECT.validate_probe_source(path, expected)
            path.write_bytes(path.read_bytes() + b"unexpected change")
            with self.assertRaisesRegex(RuntimeError, "probe harness drift"):
                SELECT.validate_probe_source(path, expected)


if __name__ == "__main__":
    unittest.main()
