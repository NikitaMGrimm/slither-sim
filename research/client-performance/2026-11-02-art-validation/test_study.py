"""Bounded offline evidence checks; no recording, GPU or timing work."""
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import unittest

FOLDER = Path(__file__).resolve().parent


class Study(unittest.TestCase):
    def test_exact_measured_frame_source_and_recomputed_tables(self):
        identities = json.loads((FOLDER / 'identities.json').read_text(encoding='utf-8'))
        sources = json.loads(gzip.decompress((FOLDER / identities['source_snapshot']).read_bytes()))
        self.assertEqual(set(sources), {'current', 'earlier-composition'})
        for composition, folder in [('current', FOLDER), ('earlier-composition', FOLDER / 'earlier-composition')]:
            data = json.loads((folder / 'identities.json').read_text(encoding='utf-8'))
            self.assertEqual(set(sources[composition]), set(data['arms']))
            for name, arm in data['arms'].items():
                source = sources[composition][name].encode('utf-8')
                self.assertEqual(hashlib.sha256(source).hexdigest(), arm['frame_source_sha256'])
                self.assertEqual(arm['frame_source_sha256'], arm['reviewed_frame_source_sha256'])
        module = runpy.run_path(str(FOLDER / 'recompute.py'))
        report = (FOLDER / 'REPORT.md').read_text(encoding='utf-8')
        self.assertEqual(report.split('<!-- BEGIN RESULTS -->\n', 1)[1].split('\n<!-- END RESULTS -->', 1)[0],
                         module['tables']())
        self.assertEqual(report.split('<!-- BEGIN DIAGNOSTICS -->\n', 1)[1].split('\n<!-- END DIAGNOSTICS -->', 1)[0],
                         module['diagnostics']())


if __name__ == '__main__':
    unittest.main()
