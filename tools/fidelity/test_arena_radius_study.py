"""Synthetic regressions for observation gaps and causal forecast initialization."""
import unittest
import importlib.util
from pathlib import Path

STUDY = Path(__file__).resolve().parents[2] / 'research/fidelity/2026-10-26-arena-radius'
spec = importlib.util.spec_from_file_location('arena_radius_study', STUDY / 'analyze.py')
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)
connection_summary, free_run = analysis.connection_summary, analysis.free_run


def connection(times, radii):
    return {'conn': 1, 'protocol': 19, 'last_server_ms': times[-1],
            'deaths': [], 'visibility_removals': 0,
            'radius': [{'S': t, 'receipt_ms': t, 'radius': r, 'players': None,
                        'kind': 'a' if i == 0 else 'z'}
                       for i, (t, r) in enumerate(zip(times, radii))]}


class AnalysisTests(unittest.TestCase):
    def test_sparse_gap_splits_runs_but_preserves_endpoint_totals(self):
        c = connection([0, 128, 256, 67963, 68091, 68219], [1000, 999, 998, 900, 899, 898])
        summary, intervals = connection_summary(c)
        runs = [r for r in summary['runs'] if r['direction']]
        self.assertEqual(runs, [{'direction': -1, 'seconds': .256, 'delta': -2}] * 2)
        self.assertEqual(summary['nonzero_run_seconds']['max'], .256)
        self.assertEqual(summary['total_variation'], 102)
        self.assertEqual(sum(i['dr'] for i in intervals), -102)
        self.assertEqual(intervals[2]['dt'], 67707)

    def test_terminal_gap_is_not_an_observed_run(self):
        summary, _ = connection_summary(connection([0, 128, 67835], [1000, 999, 900]))
        self.assertEqual(summary['nonzero_run_seconds']['n'], 1)
        self.assertEqual(summary['nonzero_run_seconds']['max'], .128)
        self.assertEqual(summary['total_variation'], 100)

    def test_direction_changes_and_one_second_intervals_keep_their_durations(self):
        summary, _ = connection_summary(connection([0, 1000, 2000, 2128], [1000, 999, 998, 999]))
        self.assertEqual(summary['runs'], [
            {'direction': -1, 'seconds': 2., 'delta': -2},
            {'direction': 1, 'seconds': .128, 'delta': 1},
        ])

    def test_holding_uses_forecast_origin_after_the_first_usable_count(self):
        c = connection([0, 2000, 62000], [1000, 900, 800])
        c['leaderboards'] = [{'S': 1000, 'players': 4}]
        for row in c['radius'][1:]:
            row.update(players=4, population_age_ms=row['S'] - 1000)
        result = free_run(c)
        self.assertEqual(result['forecast_origin_ms'], 2000)
        self.assertEqual(result['forecast_origin_radius'], 900)
        self.assertEqual(result['forecast_origin_hold_mae'], 50)
        self.assertTrue(all(r['forecast_origin_hold'] == 900 for r in result['rows']))


if __name__ == '__main__':
    unittest.main()
