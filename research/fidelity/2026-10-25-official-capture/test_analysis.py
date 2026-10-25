import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('official_capture_analysis', Path(__file__).with_name('analyze.py'))
study = importlib.util.module_from_spec(spec)
spec.loader.exec_module(study)
np = study.np


class AnalysisTests(unittest.TestCase):
    def test_parent_floor_matches_brute_force_at_boundaries(self):
        rng = np.random.default_rng(3)
        t = np.cumsum(rng.uniform(1, 9, 1400))
        s = np.arange(len(t)) * 5
        x = t - s
        expected = [x[i] - min(x[max(0, i-600):i+600]) for i in range(len(x))]
        np.testing.assert_array_equal(study.frame_floor_excess(t, s), expected)

    def test_masks_include_endpoints(self):
        np.testing.assert_array_equal(study.clean_mask(np.array([0, 50, 100, 150, 200]), [[50, 150]]),
                                      [True, False, False, False, True])

    def test_tail_censoring_and_separated_runs(self):
        t = np.arange(8.) * 10
        e = np.array([11, 12, 0, 21, 0, 0, 0, 22.])
        row = study.tails(t, e, np.ones(8, dtype=bool), 10)
        self.assertEqual(row['episodes'], 3)
        self.assertEqual(row['censoredEpisodes'], 2)
        self.assertEqual(row['surroundingBracketMs']['minimum'], 20)
        self.assertEqual(row['peakMs']['maximum'], 22)

    def test_clean_bursts_do_not_bridge_excluded_receipts(self):
        t = np.arange(200.) * 10
        t[80] = t[79] + .1
        mask = np.ones(len(t), dtype=bool)
        mask[80] = False
        row = study.features([{'t': t, 'e': np.sin(t/100)+2, 'clean': mask}], True)
        self.assertEqual(row['burstLt1'], 0)
        self.assertEqual(row['adjacentPairs'], 197)


if __name__ == '__main__':
    unittest.main()
