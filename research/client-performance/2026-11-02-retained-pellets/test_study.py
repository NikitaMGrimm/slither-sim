"""Historical summaries remain testable without the measured branch or working files."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('retained_study', ROOT / 'recompute.py')
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)


class StudyTests(unittest.TestCase):
    def test_retained_results_and_parity(self):
        output = study.tables()
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        self.assertEqual(report.split('<!-- BEGIN RESULTS -->\n')[1].split('\n<!-- END RESULTS -->')[0], output)
        self.assertEqual(report.split('<!-- BEGIN DIAGNOSTICS -->\n')[1].split('\n<!-- END DIAGNOSTICS -->')[0], study.diagnostics())


if __name__ == '__main__':
    unittest.main()
