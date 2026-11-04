import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('continuous_zoom_study', Path(__file__).with_name('recompute.py'))
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)


class Study(unittest.TestCase):
    def test_retained_source_delta_without_git_or_current_source(self):
        root = Path(__file__).resolve().parent
        ids = json.loads((root / 'identities.json').read_text(encoding='utf-8'))
        source = gzip.decompress((root / 'food-painter-measured.rs.gz').read_bytes())
        path = 'client/src/presentation/food_painter.rs'
        self.assertEqual(hashlib.sha256(source).hexdigest(), ids['candidate']['runtime_source_sha256'][path])
        with tempfile.TemporaryDirectory(prefix='zoom-source-study-') as temporary:
            target = Path(temporary).resolve() / path
            self.assertTrue(target.is_relative_to(Path(temporary).resolve()))
            target.parent.mkdir(parents=True)
            target.write_bytes(source)
            subprocess.run(['git', '-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'apply', '-p1', str(root / 'source-only-delta.patch')], cwd=temporary, check=True, capture_output=True)
            self.assertEqual(hashlib.sha256(target.read_bytes()).hexdigest(), ids['candidate']['reviewed_runtime_source_sha256'][path])

    def test_retained_controls_and_complete_blocks(self):
        study.tables(*study.load())

    def test_review_factor_one_image_controls(self):
        records = json.loads((Path(__file__).with_name('review-image-controls.json')).read_text(encoding='utf-8'))
        ids = study.load()[1]
        candidate = records['candidate']
        controls = records['controls']
        self.assertEqual(set(controls), {f'{a}/{q}/stock' for a in ['rtx', 'igpu'] for q in ['native', 'original']})
        baseline = study.load()[2]
        for key, value in controls.items():
            self.assertEqual(value['binary_sha256'], candidate['binary_sha256'])
            self.assertNotEqual(value['binary_sha256'], ids['base']['binary_sha256'])
            self.assertEqual(value['driver_sha256'], ids['base']['driver_sha256'])
            self.assertEqual(value['driver_sha256'], candidate['driver_sha256'])
            self.assertEqual(value['callbacks'], 600)
            self.assertEqual(value['skips'], 0)
            self.assertGreater(value['unique_frame_hashes'], 1)
            study.digest(value['image_sequence_sha256'])
            self.assertEqual(value['image_sequence_sha256'], baseline[key]['base']['image_sequence_sha256'])

    def test_timing_identity_digest_and_admission_fail_closed(self):
        for field, bad in [('binary_sha256', '0' * 64), ('driver_sha256', '0' * 64), ('command_digest', ''), ('selected_numeric_sha256', 'absent'), ('callbacks', '599'), ('skips', '1'), ('block_accepted', 'wrong')]:
            with self.subTest(field=field):
                rows, ids, images, pop, capacity = copy.deepcopy(study.load())
                rows[0][field] = bad
                with self.assertRaises(AssertionError):
                    study.validate(rows, ids, images, pop, capacity)

    def test_image_identity_active_hash_and_exact_configuration_set(self):
        for failure in ['wrong_arm', 'same_binary', 'short', 'skip', 'inactive', 'config', 'digest']:
            with self.subTest(failure=failure):
                rows, ids, images, pop, capacity = copy.deepcopy(study.load())
                key = 'rtx/native/stock'; record = images[key]['candidate']
                if failure == 'wrong_arm': record['binary_sha256'] = ids['look']['binary_sha256']
                if failure == 'same_binary': record['binary_sha256'] = ids['base']['binary_sha256']
                if failure == 'short': record['callbacks'] = 599
                if failure == 'skip': record['skips'] = 1
                if failure == 'inactive': record['unique_frame_hashes'] = 1
                if failure == 'config': images['wrong'] = images.pop(key)
                if failure == 'digest': record['image_sequence_sha256'] = ''
                with self.assertRaises(AssertionError):
                    study.validate(rows, ids, images, pop, capacity)


if __name__ == '__main__':
    unittest.main()
