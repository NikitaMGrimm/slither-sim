"""Bounded offline evidence checks; no recording, GPU or timing work."""
import hashlib
import json
from pathlib import Path
import runpy
import unittest

FOLDER = Path(__file__).resolve().parent
REPO = FOLDER.parents[2]


class Study(unittest.TestCase):
    def test_exact_measured_frame_source_and_recomputed_tables(self):
        identities = json.loads((FOLDER / 'identities.json').read_text(encoding='utf-8'))
        candidate = identities['arms']['candidate']
        self.assertEqual(candidate['frame_source_sha256'], candidate['reviewed_frame_source_sha256'])
        self.assertEqual(hashlib.sha256((REPO / 'client/apps/wgpu-slice/src/frame.rs').read_bytes()).hexdigest(),
                         candidate['frame_source_sha256'])
        module = runpy.run_path(str(FOLDER / 'recompute.py'))
        report = (FOLDER / 'REPORT.md').read_text(encoding='utf-8')
        self.assertEqual(report.split('<!-- BEGIN RESULTS -->\n', 1)[1].split('\n<!-- END RESULTS -->', 1)[0],
                         module['tables']())
        self.assertEqual(report.split('<!-- BEGIN DIAGNOSTICS -->\n', 1)[1].split('\n<!-- END DIAGNOSTICS -->', 1)[0],
                         module['diagnostics']())


if __name__ == '__main__':
    unittest.main()
