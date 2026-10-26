"""Offline source selection and frozen near-server reference regressions."""
from contextlib import redirect_stdout
import copy
import importlib.util
import io
import json
import unittest

import transport_parity as tp

STUDY = tp.ROOT / 'research/fidelity/2026-10-26-server-timing-references'
spec = importlib.util.spec_from_file_location('server_reference_builder', STUDY / 'build.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
spec = importlib.util.spec_from_file_location('server_reference_calibrator', STUDY / 'calibrate.py')
calibrator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(calibrator)


class ServerTimingReferencesTest(unittest.TestCase):
    def test_mask_boundaries_and_uncovered_time_are_excluded(self):
        clean = builder.Clean([[20, 30], [50, 60]], (10, 100))
        for a, b in [(0, 15), (90, 101), (15, 20), (30, 40), (35, 70)]:
            self.assertFalse(clean.interval(a, b))
        for a, b in [(10, 19), (31, 49), (61, 100)]:
            self.assertTrue(clean.interval(a, b))

    def test_prey_cadence_does_not_bridge_removal_reappearance_or_mask(self):
        def frame(t, *packets):
            return tp.sp.Frame(1, t, t, list(packets))
        y = b'y\x00\x01' + bytes(17)
        j = b'j\x00\x01' + bytes(4)
        frames = [frame(0, b'a', y), frame(100, j), frame(400, j),
                  frame(500, b'y\x00\x01'), frame(600, j), frame(700, y),
                  frame(800, j), frame(1100, j), frame(1200, b'a', y),
                  frame(1300, j), frame(1600, j)]
        result = builder.cadence(frames, builder.Clean([[900, 1000]], (0, 2000)))
        self.assertEqual(result['prey'], [300, 300])

    def test_reference_sources_and_calibration_split_match_completed_study(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        pins = json.loads((builder.STUDY / 'source-pins.json').read_text())['secondContinuation']
        sources = {r['path']: r['sha256'] for r in pins['vpsFiles']}
        self.assertEqual(reference['wireVersion'], 19)
        self.assertIsNone(reference['clientContract'])
        self.assertEqual(len(reference['captures']), 6)
        self.assertEqual(len(reference['maskedDevelopmentCaptures']), 4)
        self.assertEqual(len(reference['heldoutCaptures']), 2)
        for row in reference['captures'] + reference['maskedDevelopmentCaptures'] + reference['heldoutCaptures']:
            self.assertEqual(row['sha256'], sources[row['path']])
            self.assertNotIn('packetFamilies', row['summary'])
            self.assertNotIn('headerDeltaHistogramMs', row['summary'])
            self.assertGreater(row['summary']['frames'], 1000)
        self.assertEqual({r['split'] for r in reference['captures']}, {'development', 'heldout'})
        self.assertTrue(all(r['split'] == 'development' for r in reference['maskedDevelopmentCaptures']))
        self.assertTrue(all(r['split'] == 'heldout' for r in reference['heldoutCaptures']))

    def test_heldout_envelope_exceedance_is_retained_without_retuning(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        # Keep the rejected first attempt's zero-allowance dev-only envelope.
        reference['captures'] = reference['maskedDevelopmentCaptures']
        for metric in tolerance['metrics'].values():
            metric['spreadAllowance'] = 0
        outcomes = []
        for row in reference['heldoutCaptures']:
            candidate = dict(schemaVersion=1, wireVersion=19, status='complete',
                             summary=row['summary'], captures=[row])
            with redirect_stdout(io.StringIO()):
                outcomes.append(tp.compare(reference, candidate, tolerance))
        self.assertEqual(outcomes, [0, 1])
        limit = max(r['summary']['excessDelayMs']['p90'] for r in reference['captures'])
        self.assertAlmostEqual(limit, 1.090, places=6)
        self.assertAlmostEqual(reference['heldoutCaptures'][1]['summary']['excessDelayMs']['p90'], 1.096, places=6)

    def test_every_official_capture_passes_default_against_other_five(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        for i, capture in enumerate(reference['captures']):
            with self.subTest(batch=capture['batch']):
                others = {**reference, 'captures': reference['captures'][:i] + reference['captures'][i+1:]}
                candidate = {**reference, 'summary': capture['summary'], 'captures': [capture]}
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(tp.compare(others, candidate, tolerance), 0)

    def test_default_rejects_clearly_out_of_envelope_synthetic_candidate(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        for name, (path, _) in tp.GATES.items():
            with self.subTest(metric=name):
                candidate = copy.deepcopy(reference)
                candidate['dataOrigin'] = 'synthetic'
                candidate['summary'] = copy.deepcopy(reference['captures'][0]['summary'])
                values = [tp.lookup(row['summary'], path) for row in reference['captures']]
                metric = tolerance['metrics'][name]
                upper = max(values) + metric['spreadAllowance'] * max(max(values)-min(values), metric.get('spreadFloor', 0))
                target = candidate['summary']
                parts = path.split('.')
                for part in parts[:-1]:
                    target = target[part]
                target[parts[-1]] = 10 * upper + 1
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(tp.compare(reference, candidate, tolerance), 1)

    def test_default_allowance_is_smallest_passing_fixed_rule_and_matches_record(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        result = calibrator.evaluate(reference['captures'])
        record = json.loads((STUDY / 'loo-results.json').read_text())
        self.assertEqual(result['selectedAllowance'], 1)
        self.assertFalse(result['trials'][0]['allPassed'])
        self.assertTrue(result['trials'][1]['allPassed'])
        self.assertEqual({m['spreadAllowance'] for m in tolerance['metrics'].values()}, {result['selectedAllowance']})
        self.assertEqual({name: m.get('spreadFloor', 0) for name, m in tolerance['metrics'].items()},
                         {name: 1 if name == 'frame_server_p50' else 0 for name in tp.GATES})
        for key in result:
            self.assertEqual(result[key], record[key])

    def test_header_resolution_floor_is_required_and_not_added_to_other_metrics(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        no_floor = calibrator.evaluate(reference['captures'], frame_spread_floor=0)
        self.assertIsNone(no_floor['selectedAllowance'])
        self.assertIsNone(no_floor['finalLimits'])
        self.assertTrue(all(not trial['allPassed'] for trial in no_floor['trials']))
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        candidate = next(c for c in reference['captures'] if c['batch'] == 'low-2')
        others = {**reference, 'captures': [c for c in reference['captures'] if c is not candidate]}
        report = {**reference, 'summary': candidate['summary'], 'captures': [candidate]}
        with redirect_stdout(io.StringIO()):
            self.assertEqual(tp.compare(others, report, tolerance), 0)
            tolerance['metrics']['frame_server_p50']['spreadFloor'] = 0
            self.assertEqual(tp.compare(others, report, tolerance), 1)


if __name__ == '__main__':
    unittest.main()
