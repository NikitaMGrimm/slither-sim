"""Authored offline regressions for selection, lifecycle and uncertainty."""
import copy
import math
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import analyze as a


def row(dx, dy=0, flight=True, key=(1, 1, 1)):
    return {'key': key, 'speed': 6.153 if flight else 3.8, 'flight': flight,
            'snakes': [{'dx': dx, 'dy': dy, 'd': math.hypot(dx, dy),
                        'box': max(abs(dx), abs(dy)), 'age': 0, 'speed': 5.79}],
            'previous_mode_changed': None}


class AnalysisTests(unittest.TestCase):
    def test_speed_units_and_frozen_controls(self):
        self.assertEqual(a.laws(5.79)['A'], 6.153)
        self.assertEqual(a.laws(14)['A'], 10.997)
        self.assertEqual(a.laws(4)['A'], 6.153)
        self.assertEqual(a.laws(15)['A'], 10.997)
        self.assertEqual(a.laws(13.499)['B'], 6.153)
        self.assertEqual(a.laws(13.5)['B'], 10.997)
        self.assertFalse(a.ordinary(6.060))
        self.assertFalse(a.ordinary(11))

    def test_known_head_filter_precedes_nearest(self):
        r = row(330)
        r['snakes'] += row(290, 290)['snakes']
        self.assertTrue(a.prediction(r, 'square320'))
        self.assertFalse(a.prediction(r, 'circle300'))
        self.assertFalse(a.prediction(row(320), 'square320'))
        self.assertTrue(a.prediction(row(319.999), 'square320'))

    def test_boxes_and_strict_boundary(self):
        self.assertIsNone(a.robust_prediction(row(317), 'square315', 0))
        self.assertIsNone(a.robust_prediction(row(317), 'square320', 0))
        self.assertTrue(a.robust_prediction(row(280, 280), 'square320', 0))
        self.assertFalse(a.robust_prediction(row(500), 'square320', 0))
        bounds = a.region_bounds(row(280, 280)['snakes'][0], 6.153, 0)
        self.assertLess(bounds['circle'][0], math.hypot(280, 280))
        self.assertGreater(bounds['circle'][1], math.hypot(280, 280))
        self.assertIsNone(a.robust_prediction(row(500), 'square320', 42, True))

    def test_missing_class_and_episode_weighting(self):
        self.assertIsNone(a.classification([row(100)], 'square320')['balanced_accuracy'])
        # One long all-correct flight episode and one short missed flight episode.
        rows = [row(100)] * 20 + [row(500, key=(1, 2, 1)), row(500, flight=False, key=(1, 3, 1))]
        result = a.activation_scores(rows)['models']['square320']
        self.assertAlmostEqual(result['episode_balanced_accuracy'], .75)
        self.assertGreater(result['balanced_accuracy'], .95)

    def test_history_is_causal_and_requires_left_coverage(self):
        history = [(100, 6), (600, 8), (650, 10), (900, 14)]
        self.assertEqual(a.history_range(history, 700), (6, 10))
        self.assertIsNone(a.history_range([(600, 8)], 700))
        self.assertEqual(a.history_range([(0, 14)], 1000), (14, 14))

    def test_head_estimate_independent_of_unused_body_contraction(self):
        snake = a.sp.Snake(1, [[0., 0.], [42., 5.], [84., 10.], [126., 15.], [168., 20.]],
                           .2, .2, 5.79, 0, base=(168., 20.), hist=[(0, .2, 5.79)])
        worlds = [a.sp.World(contract=c, self_id=1, snakes={1: copy.deepcopy(snake)})
                  for c in (False, True)]
        # Authored absolute non-growth anchor and subsequent speed/heading report.
        anchor = b'=' + (100).to_bytes(2, 'big') + (210).to_bytes(2, 'big') + (24).to_bytes(2, 'big')
        for world in worlds:
            world.feed(anchor, 200)
            world.feed(b'e\x20\x80', 220)
        self.assertNotEqual(worlds[0].snakes[1].pts, worlds[1].snakes[1].pts)
        for t in (200, 220, 350):
            self.assertEqual(worlds[0].snakes[1].head(t), worlds[1].snakes[1].head(t))

    def test_full_present_fields_exclude_inherited_speeds(self):
        setup = bytearray(25)
        setup[0], setup[23], setup[24] = ord('a'), 19, 42
        appearance = (b'y\x00\x01\x00' + (5000).to_bytes(3, 'big') * 2 + b'\x1e0'
                      + b'\x00' * 6 + (6153).to_bytes(2, 'big'))
        compact = b'j\x00\x01' + b'\x01\x00' * 2 + b'0' + b'\x00' * 6
        frames = [a.sp.Frame(1, 0, 0, [bytes(setup), appearance]),
                  a.sp.Frame(1, 10, 10, [compact]), {'event': 'capture-end'}]
        with patch.object(a.sp, 'read', return_value=iter(frames)):
            result = a.inspect(Path('authored-unused'))
        self.assertEqual(result['census']['j'], 1)
        self.assertEqual(result['census']['present_speed'], 0)
        self.assertEqual(result['flights'], [])
        self.assertEqual(result['activations'], [])

    def test_hash_and_capture_end_gates(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, 'Missing/mismatched'):
                a.run(root, root / 'out')
            name = next(iter(a.SOURCES))
            archive = root / 'recordings/headless' / name / 'bot0.jsonl.gz'
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b'authored wrong bytes')
            with self.assertRaisesRegex(ValueError, 'Missing/mismatched'):
                a.run(root, root / 'out-bad-hash')
        with patch.object(a.sp, 'read', return_value=iter([])):
            with self.assertRaisesRegex(ValueError, 'capture-end'):
                a.inspect(Path('authored-unused'))

    def test_historical_fixture_comparison_retains_errors(self):
        r = row(280, 280)
        r['S'] = 100
        case = {'conn': 1, 'pid': 1, 'episode': 1, 'S': 100, 'speed': 6.153,
                'flight': True, 'snakes': copy.deepcopy(r['snakes'])}
        case['snakes'][0]['sid'] = r['snakes'][0]['sid'] = 2
        self.assertTrue(a.activation_fixture_check([r], {'cases': [case]})['matched_exactly'])
        case['speed'] = 3.8
        result = a.activation_fixture_check([r], {'cases': [case]})
        self.assertFalse(result['matched_exactly'])
        self.assertEqual(result['failures'], {'speed': 1})

    def test_power_does_not_average_systematic_budgets(self):
        p = a.power()
        self.assertFalse(p['AF_capture_pass'])
        self.assertFalse(p['width_capture_pass'])
        self.assertAlmostEqual(p['AF_max_separation'], .00877)
        self.assertAlmostEqual(p['needed_snake_speed_full_cell_below'], .00573485811097)
        self.assertLess(a.speed_budget(0), a.speed_budget(7))
        self.assertFalse(p['capture_proposed'])


if __name__ == '__main__':
    a.one_cpu()
    unittest.main()
