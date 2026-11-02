"""Negative controls for retained complete matrices; stdlib only."""
import copy
import csv
from pathlib import Path
import shutil
import tempfile
import unittest

import recompute as r


class MatrixTests(unittest.TestCase):
    def setUp(self):
        self.history = r.rows('combined-trial-runs.csv')
        self.profile = r.rows('profile-runs.csv')

    def test_valid_selected_matrices(self):
        r.history(self.history, 'combined-trial')
        r.profile(self.profile)

    def test_missing_cell_and_missing_run_are_rejected(self):
        for data in (self.history[:-1], [x for x in self.history if x['zoom'] != '-120']):
            with self.assertRaises(ValueError):
                r.history(data, 'combined-trial')
        with self.assertRaises(ValueError):
            r.profile([x for x in self.profile if x['diagnostic'] != 'no-boost'])

    def test_duplicate_labels_and_ids_are_rejected(self):
        for mutate in (lambda d: d.append(d[0]), lambda d: d[1].update(run_id=d[0]['run_id']),
                       lambda d: d[0].update(zoom='-120')):
            data = copy.deepcopy(self.history)
            mutate(data)
            with self.assertRaises(ValueError):
                r.history(data, 'combined-trial')

    def test_incomplete_samples_mixed_binaries_nonfinite_and_arm_order_fail(self):
        for mutate in (lambda d: d[0].update(samples='140'),
                       lambda d: d[0].update(binary_sha256='0'*64),
                       lambda d: d[0].update(gpu_p95_ms='nan'),
                       lambda d: d.reverse()):
            data = copy.deepcopy(self.history)
            mutate(data)
            with self.assertRaises(ValueError):
                r.history(data, 'combined-trial')
        data = copy.deepcopy(self.profile)
        data[0]['samples'] = '9'
        with self.assertRaises(ValueError):
            r.profile(data)

    def test_every_additional_evidence_matrix_is_required_before_tables(self):
        for filename in ('sparse-runs.csv', 'frozen-runs.csv', 'parity.csv', 'pixels.csv'):
            with self.subTest(filename=filename), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                for file in r.ROOT.iterdir():
                    if file.suffix in ('.csv', '.json'):
                        shutil.copyfile(file, root/file.name)
                data = r.rows(filename, root)
                with (root/filename).open('w', encoding='utf-8', newline='') as stream:
                    writer = csv.DictWriter(stream, fieldnames=list(data[0]))
                    writer.writeheader()
                    writer.writerows(data[1:])
                with self.assertRaises(ValueError):
                    r.tables(root)


if __name__ == '__main__':
    unittest.main()
