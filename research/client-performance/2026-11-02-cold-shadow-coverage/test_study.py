"""Offline source/summary checks, without recordings, GPU or timing work."""
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parent


class Study(unittest.TestCase):
    def test_measured_sources_and_tables(self):
        data = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
        sources = json.loads(gzip.decompress((ROOT / data['source_snapshot']).read_bytes()))
        self.assertEqual(set(sources), set(data['arms']))
        for name, arm in data['arms'].items():
            source = sources[name].encode('utf-8')
            self.assertEqual(hashlib.sha256(source).hexdigest(), arm['materials_source_sha256'])
        module = runpy.run_path(str(ROOT / 'recompute.py'))
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        for section, function in [('RESULTS', 'tables'), ('DIAGNOSTICS', 'diagnostics'), ('REJECTED', 'rejected')]:
            self.assertEqual(report.split(f'<!-- BEGIN {section} -->\n', 1)[1].split(f'\n<!-- END {section} -->', 1)[0],
                             module[function]())


if __name__ == '__main__':
    unittest.main()
