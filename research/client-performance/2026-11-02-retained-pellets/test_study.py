"""Historical summaries remain testable without the measured branch or working files."""
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

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
        self.assertEqual(report.split('<!-- BEGIN REVIEW RESULTS -->\n')[1].split('\n<!-- END REVIEW RESULTS -->')[0], study.review_tables())
        self.assertEqual(report.split('<!-- BEGIN MENU RESULTS -->\n')[1].split('\n<!-- END MENU RESULTS -->')[0], study.menu_tables())
        self.assertEqual(report.split('<!-- BEGIN ZOOM RESULTS -->\n')[1].split('\n<!-- END ZOOM RESULTS -->')[0], study.zoom_tables())
        self.assertEqual(report.split('<!-- BEGIN SERIAL RESULTS -->\n')[1].split('\n<!-- END SERIAL RESULTS -->')[0], study.serial_tables())

    def test_serial_image_claim_rejects_short_or_skipped_runs_in_either_arm(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ['serial-identities.json', 'serial-runs.csv', 'serial-rejected-hint.patch', 'serial-rejected-packing.patch']:
                shutil.copyfile(ROOT / name, root / name)
            original = (root / 'serial-identities.json').read_text(encoding='utf-8')
            for arm in ['baseline', 'candidate']:
                for field, value in [('callbacks', 599), ('skips', 1)]:
                    identity = json.loads(original)
                    record = next(iter(identity['image_parity'].values()))[arm]
                    record[field] = value
                    (root / 'serial-identities.json').write_text(json.dumps(identity), encoding='utf-8')
                    with patch.object(study, 'ROOT', root), self.assertRaises(AssertionError):
                        study.serial_tables()


if __name__ == '__main__':
    unittest.main()
