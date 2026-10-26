"""Offline source selection and frozen near-server reference regressions."""
from contextlib import redirect_stdout
import importlib.util
import io
import json
import unittest

import transport_parity as tp

STUDY = tp.ROOT / 'research/fidelity/2026-10-26-server-timing-references'
spec = importlib.util.spec_from_file_location('server_reference_builder', STUDY / 'build.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


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
        self.assertEqual(len(reference['captures']), 4)
        self.assertEqual(len(reference['heldoutCaptures']), 2)
        self.assertEqual(len(reference['unfilteredCaptures']), 6)
        for row in reference['captures'] + reference['heldoutCaptures']:
            self.assertEqual(row['sha256'], sources[row['path']])
            self.assertNotIn('packetFamilies', row['summary'])
            self.assertNotIn('headerDeltaHistogramMs', row['summary'])
            self.assertGreater(row['summary']['frames'], 1000)
        self.assertTrue(all(r['split'] == 'development' for r in reference['captures']))
        self.assertTrue(all(r['split'] == 'heldout' for r in reference['heldoutCaptures']))

    def test_heldout_envelope_exceedance_is_retained_without_retuning(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        tolerance = json.loads(tp.DEFAULT_TOLERANCES.read_text())
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


if __name__ == '__main__':
    unittest.main()
