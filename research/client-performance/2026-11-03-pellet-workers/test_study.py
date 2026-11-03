import copy
import json
import unittest

import recompute as study


class Study(unittest.TestCase):
    def setUp(self):
        self.runs = study.rows('runs.csv')
        self.parity = json.loads((study.ROOT / 'parity.json').read_text(encoding='utf-8'))
        self.identities = json.loads((study.ROOT / 'identities.json').read_text(encoding='utf-8'))

    def test_retained_pairs_binaries_and_report(self):
        study.validate(self.runs, self.parity, self.identities)
        text = study.tables(self.runs, study.rows('idle.csv'), study.rows('density.csv'), study.rows('callback-stages.csv'))
        self.assertIn(text, (study.ROOT / 'REPORT.md').read_text(encoding='utf-8'))

    def test_both_timing_arm_identities_are_checked(self):
        for arm in study.ARMS:
            changed = copy.deepcopy(self.runs)
            next(r for r in changed if r['arm'] == arm)['binary_sha256'] = '0' * 64
            with self.assertRaises(AssertionError):
                study.validate(changed, self.parity, self.identities)

    def test_parity_arm_identity_count_and_configuration_are_checked(self):
        key = next(iter(self.parity))
        for field, value in [('binary_sha256', '0' * 64), ('baseline_binary_sha256', '0' * 64), ('callbacks', 599), ('baseline_callbacks', 599), ('skips', 1), ('baseline_skips', 1)]:
            changed = copy.deepcopy(self.parity)
            changed[key][field] = value
            with self.subTest(field=field), self.assertRaises(AssertionError):
                study.validate(self.runs, changed, self.identities)
        changed = copy.deepcopy(self.parity)
        changed['wrong/configuration'] = changed.pop(key)
        with self.assertRaises(AssertionError):
            study.validate(self.runs, changed, self.identities)

    def test_compile_overlap_cannot_be_included_or_reassigned(self):
        for field, value in [('paired', 'True'), ('status', 'accepted')]:
            changed = copy.deepcopy(self.runs)
            next(r for r in changed if r['status'] == 'compile_overlap')[field] = value
            with self.assertRaises(AssertionError):
                study.validate(changed, self.parity, self.identities)

    def test_incomplete_or_short_pairs_are_rejected(self):
        with self.assertRaises(AssertionError):
            study.validate(self.runs[1:], self.parity, self.identities)
        changed = copy.deepcopy(self.runs)
        changed[0]['callbacks'] = '599'
        with self.assertRaises(AssertionError):
            study.validate(changed, self.parity, self.identities)

if __name__ == '__main__':
    unittest.main()
