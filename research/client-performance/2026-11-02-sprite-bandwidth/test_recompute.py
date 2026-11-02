"""Negative checks for the published study's completeness guard (stdlib)."""
import copy
import json
from pathlib import Path
import unittest

import recompute as study

HERE = Path(__file__).parent


class Completeness(unittest.TestCase):
    def setUp(self):
        provenance = json.loads((HERE / "provenance.json").read_text(encoding="utf-8"))
        self.binaries = provenance["frozen"]["binaries"]
        self.rows = study.read_csv(HERE / "frozen-runs.csv")
        self.diagnostics = study.read_csv(HERE / "diagnostics.csv")
        self.snapshots = study.read_csv(HERE / "diagnostic-snapshots.csv")

    def check_bad(self, rows):
        with self.assertRaises(ValueError):
            study.validate_runs(rows, "frozen", self.binaries)

    def test_retained_matrix_passes(self):
        study.validate_runs(self.rows, "frozen", self.binaries)

    def test_missing_row_and_entire_cell(self):
        self.check_bad(self.rows[1:])
        self.check_bad([r for r in self.rows if (r["workload"], r["zoom"], r["size"]) != study.CELLS[0]])

    def test_extra_row_and_duplicate_id(self):
        self.check_bad(self.rows + [self.rows[0]])
        rows = copy.deepcopy(self.rows)
        rows[1]["run_id"] = rows[0]["run_id"]
        self.check_bad(rows)

    def test_id_labels_binary_and_samples(self):
        for field, wrong in (("run_id", "primary-9-replay-0-960-original-before"),
                             ("binary_sha256", "0" * 64), ("samples", "59"),
                             ("launch_index", "120"), ("messages", "1")):
            with self.subTest(field=field):
                rows = copy.deepcopy(self.rows)
                rows[0][field] = wrong
                self.check_bad(rows)

    def test_missing_diagnostic_runs_and_snapshots(self):
        study.diagnostics_tables(self.diagnostics, self.snapshots)
        for runs, snapshots in ((self.diagnostics[1:], self.snapshots),
                                (self.diagnostics, self.snapshots[1:])):
            with self.assertRaises(ValueError):
                study.diagnostics_tables(runs, snapshots)

    def test_changed_or_missing_report_block(self):
        report = "<!-- BEGIN CHECK -->\nretained\n<!-- END CHECK -->"
        study.replace_or_check(report, "CHECK", "retained", True)
        for wrong in (report.replace("retained", "altered"), ""):
            with self.assertRaises(ValueError):
                study.replace_or_check(wrong, "CHECK", "retained", True)

    def test_parity_requires_all_cells_and_digests(self):
        prefix = "" if (HERE / "parity-runs.csv").exists() else "pre-mip-"
        series = "pre-mip" if prefix else "history"
        binaries = json.loads((HERE / "provenance.json").read_text(encoding="utf-8"))[series]["binaries"]
        runs = study.read_csv(HERE / (prefix + "parity-runs.csv"))
        pictures = study.read_csv(HERE / (prefix + "pixels.csv"))
        study.parity_table(runs, pictures, binaries)
        for bad_runs, bad_pictures in ((runs[1:], pictures), (runs, pictures[1:])):
            with self.assertRaises(ValueError):
                study.parity_table(bad_runs, bad_pictures, binaries)
        changed = copy.deepcopy(runs)
        changed[0]["digest"] = "0" * 16
        with self.assertRaises(ValueError):
            study.parity_table(changed, pictures, binaries)

    def test_profile_requires_all_stages_and_snapshots(self):
        rows = study.read_csv(HERE / "profile-stages.csv")
        binary = json.loads((HERE / "provenance.json").read_text(encoding="utf-8"))["history"]["binaries"]["after"]
        study.profile_table(rows, binary)
        for incomplete in (rows[1:], [r for r in rows if r["workload"] != "replay"]):
            with self.assertRaises(ValueError):
                study.profile_table(incomplete, binary)
        changed = copy.deepcopy(rows)
        changed[0]["binary_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            study.profile_table(changed, binary)


if __name__ == "__main__":
    unittest.main()
