"""Historical study checks need neither branch revisions nor current source bytes."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('mirror_study', ROOT / 'recompute.py')
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)


class StudyTests(unittest.TestCase):
    def test_tables_and_parity(self):
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        self.assertEqual(report.split('<!-- BEGIN RESULTS -->\n')[1].split('\n<!-- END RESULTS -->')[0],
                         study.results())


if __name__ == '__main__':
    unittest.main()
