"""Offline source/summary checks, without recordings, GPU or timing work."""
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


class Study(unittest.TestCase):
    def test_measured_sources_and_tables(self):
        data = json.loads((ROOT / 'identities.json').read_text(encoding='utf-8'))
        for arm in data['arms'].values():
            revision = arm['source_reference_revision']
            self.assertTrue(len(revision) == 40 and all(c in '0123456789abcdef' for c in revision))
            source = subprocess.check_output(['git', 'show', f'{revision}:client/apps/wgpu-slice/src/materials.rs'], cwd=REPO)
            self.assertEqual(hashlib.sha256(source).hexdigest(), arm['materials_source_sha256'])
        self.assertEqual(hashlib.sha256((REPO / 'client/apps/wgpu-slice/src/materials.rs').read_bytes()).hexdigest(),
                         data['arms']['candidate']['materials_source_sha256'])
        module = runpy.run_path(str(ROOT / 'recompute.py'))
        report = (ROOT / 'REPORT.md').read_text(encoding='utf-8')
        for section, function in [('RESULTS', 'tables'), ('DIAGNOSTICS', 'diagnostics'), ('REJECTED', 'rejected')]:
            self.assertEqual(report.split(f'<!-- BEGIN {section} -->\n', 1)[1].split(f'\n<!-- END {section} -->', 1)[0],
                             module[function]())


if __name__ == '__main__':
    unittest.main()
