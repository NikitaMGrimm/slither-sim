"""Synthetic boundary controls for the new gate and background accounting."""
from copy import deepcopy
import unittest
from confirmation_gate import evaluate
from confirmation_background import finish


def fixture():
    rows = []
    for name in ['mature-stock', 'mature-legacy', 'short-500-stock', 'short-500-legacy']:
        pairs = []
        for pair in range(-1, 10):
            base = {'all': {'ticks': 1080, 'cpuTotalNs': 1080000000},
                    'maintenance': {'ticks': 15, 'cpuTotalNs': 15000000}}
            candidate = deepcopy(base)
            candidate['all']['cpuTotalNs'] -= 100000000
            candidate['maintenance']['cpuTotalNs'] -= 1000000
            pairs.append({'pair': pair, 'discardedAB': pair == -1,
                          'base': base, 'candidate': candidate})
        rows.append({'name': name, 'pairs': pairs})
    return rows


class Controls(unittest.TestCase):
    def test_eight_improving_pairs_pass_with_two_unfavorable_pairs(self):
        rows = fixture()
        for p in rows[0]['pairs'][1:3]:
            p['candidate']['all']['cpuTotalNs'] = 3 * p['base']['all']['cpuTotalNs']
        decision = evaluate(rows)
        self.assertTrue(decision['runtimeQualifies'])
        self.assertEqual(decision['mature'][0]['improvingWholeWindowPairs'], 8)

    def test_seven_improving_pairs_fail_despite_a_lower_median(self):
        rows = fixture()
        for p in rows[0]['pairs'][1:4]:
            p['candidate']['all']['cpuTotalNs'] = p['base']['all']['cpuTotalNs'] + 1
        decision = evaluate(rows)
        self.assertLess(decision['mature'][0]['medianPairedWholeCpuRatio'], 1)
        self.assertFalse(decision['runtimeQualifies'])

    def test_exactly_equal_even_maintenance_median_fails(self):
        rows = fixture()
        for index, p in enumerate(rows[0]['pairs'][1:]):
            p['candidate']['maintenance']['cpuTotalNs'] = 14000000 if index < 5 else 16000000
        self.assertEqual(evaluate(rows)['mature'][0]['medianPairedMaintenanceCpuRatio'], 1)
        self.assertFalse(evaluate(rows)['runtimeQualifies'])

    def test_absolute_ten_us_passes_but_one_more_ns_per_window_fails(self):
        rows = fixture()
        for p in rows[3]['pairs'][1:]:
            p['candidate']['all']['cpuTotalNs'] = p['base']['all']['cpuTotalNs'] + 10800000
        self.assertTrue(evaluate(rows)['runtimeQualifies'])
        for p in rows[3]['pairs'][1:]:
            p['candidate']['all']['cpuTotalNs'] += 1
        self.assertFalse(evaluate(rows)['runtimeQualifies'])

    def test_original_five_pair_series_cannot_enter(self):
        rows = fixture()
        rows[0]['pairs'] = rows[0]['pairs'][:6]
        with self.assertRaises(AssertionError):
            evaluate(rows)

    def test_discarded_ab_does_not_vote(self):
        rows = fixture()
        for row in rows:
            row['pairs'][0]['candidate']['all']['cpuTotalNs'] *= 1000
        self.assertTrue(evaluate(rows)['runtimeQualifies'])

    def snapshots(self):
        before = {'hz': 100, 'monotonic': 1., 'busy': 500, 'steal': 10,
                  'processes': {1: {'start': 1, 'own': 100, 'waited': 20, 'study': True},
                                2: {'start': 1, 'own': 200, 'waited': 0, 'study': False}}}
        after = deepcopy(before)
        after.update(monotonic=2., busy=545)
        after['processes'][1].update(own=110, waited=50)
        after['processes'][2]['own'] = 205
        return before, after

    def test_reaped_child_cpu_remains_in_study_total(self):
        before, after = self.snapshots()
        value = finish(before, after)
        self.assertEqual(value['studyTreeCpuMs'], 400)
        self.assertEqual(value['guestBusyMinusStudyCpuMsRaw'], 50)
        self.assertEqual(value['visibleNonStudyCpuMs'], 50)
        self.assertEqual(value['counterQuantumMs'], 10)
        self.assertFalse(value['exclusionApplied'])

    def test_negative_quantized_residual_is_preserved(self):
        before, after = self.snapshots()
        after['busy'] = 539
        self.assertEqual(finish(before, after)['guestBusyMinusStudyCpuMsRaw'], -10)

    def test_pid_reuse_is_new_and_exited_rather_than_negative_cpu(self):
        before, after = self.snapshots()
        after['processes'][2].update(start=2, own=1)
        value = finish(before, after)
        self.assertEqual(value['visibleNonStudyCpuMs'], 10)
        self.assertEqual(value['newNonStudyProcesses'], 1)
        self.assertEqual(value['exitedNonStudyProcesses'], 1)


if __name__ == '__main__':
    unittest.main()
