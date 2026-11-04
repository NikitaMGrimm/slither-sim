import copy
import gzip
import hashlib
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('continuous_zoom_study', Path(__file__).with_name('recompute.py'))
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)


class Study(unittest.TestCase):
    def test_retained_source_delta_without_git_or_current_source(self):
        root = Path(__file__).resolve().parent
        ids = __import__('json').loads((root / 'identities.json').read_text(encoding='utf-8'))
        source = gzip.decompress((root / 'food-painter-measured.rs.gz').read_bytes())
        path = 'client/src/presentation/food_painter.rs'
        self.assertEqual(hashlib.sha256(source).hexdigest(), ids['candidate']['runtime_source_sha256'][path])
        old = b'        pixels_per_unit: pixels_per_unit,\n'
        self.assertEqual(source.count(old), 1)
        reviewed = source.replace(old, b'        pixels_per_unit,\n')
        self.assertEqual(hashlib.sha256(reviewed).hexdigest(), ids['candidate']['reviewed_runtime_source_sha256'][path])

    def test_retained_controls_and_complete_blocks(self):
        study.tables(*study.load())

    def test_timing_identity_digest_and_admission_fail_closed(self):
        for field, bad in [('binary_sha256', '0' * 64), ('driver_sha256', '0' * 64), ('command_digest', ''), ('selected_numeric_sha256', 'absent'), ('callbacks', '599'), ('skips', '1'), ('block_accepted', 'wrong')]:
            with self.subTest(field=field):
                rows, ids, images, pop = copy.deepcopy(study.load())
                rows[0][field] = bad
                with self.assertRaises(AssertionError):
                    study.validate(rows, ids, images, pop)

    def test_image_identity_active_hash_and_exact_configuration_set(self):
        for failure in ['wrong_arm', 'same_binary', 'short', 'skip', 'inactive', 'config', 'digest']:
            with self.subTest(failure=failure):
                rows, ids, images, pop = copy.deepcopy(study.load())
                key = 'rtx/native/stock'; record = images[key]['candidate']
                if failure == 'wrong_arm': record['binary_sha256'] = ids['look']['binary_sha256']
                if failure == 'same_binary': record['binary_sha256'] = ids['base']['binary_sha256']
                if failure == 'short': record['callbacks'] = 599
                if failure == 'skip': record['skips'] = 1
                if failure == 'inactive': record['unique_frame_hashes'] = 1
                if failure == 'config': images['wrong'] = images.pop(key)
                if failure == 'digest': record['image_sequence_sha256'] = ''
                with self.assertRaises(AssertionError):
                    study.validate(rows, ids, images, pop)


if __name__ == '__main__':
    unittest.main()
