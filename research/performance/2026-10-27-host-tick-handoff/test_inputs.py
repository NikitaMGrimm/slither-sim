import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('inputs', Path(__file__).with_name('analyze_inputs.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class InputTests(unittest.TestCase):
    def test_fifo_repeated_bytes_and_next_tick(self):
        samples = [(0, 1, 7, 10.001), (0, 1, 7, 10.101)]
        reads = [(0, 0, 7, 10.002, 0, 1), (0, 0, 7, 10.102, 0, 1)]
        ingests = [(1, 0, 7, 10.004, .007, 0), (1, 0, 7, 10.105, .014, 0)]
        result = module.match_input(samples, reads, ingests, [10.003, 10.104])
        self.assertEqual(result['matchedSamples'], 2)
        self.assertEqual(result['ingestedNoLaterThanNextStartedTick'], 2)
        self.assertAlmostEqual(result['clientEnqueueToIngestMs']['p99'], 4)
        self.assertAlmostEqual(result['serverReadToIngestMs']['p50'], 2)

    def test_missing_sample_is_counted_and_does_not_steal_next_byte(self):
        result = module.match_input(
            [(0, 1, 7, 1), (0, 1, 8, 1.1)], [(0, 0, 8, 1.101, 0, 1)],
            [(1, 0, 8, 1.103, .007, 0)], [1.102])
        self.assertEqual(result['unmatchedRead'], 1)
        self.assertEqual(result['matchedSamples'], 1)

    def test_waiting_for_read_pass_is_not_hidden_by_read_to_ingest(self):
        result = module.match_input(
            [(0, 1, 7, 10.001)], [(0, 0, 7, 10.005, 0, 1)],
            [(1, 0, 7, 10.011, .014, 0)], [10.003, 10.010])
        self.assertEqual(result['ingestedNoLaterThanNextStartedTick'], 1)
        self.assertEqual(result['ingestedNoLaterThanNextStartedTickSinceClientEnqueue'], 0)
        self.assertEqual(result['extraTicksAfterFirstStartedTickSinceClientEnqueue']['p99'], 1)


if __name__ == '__main__':
    unittest.main()
