import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('diagnostic', Path(__file__).with_name('analyze.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AnalysisTests(unittest.TestCase):
    def test_tick_lag_clips_starts_and_removes_constant_offset(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'host.json').write_text(json.dumps({'holdStartUnix': 10.014, 'holdEndUnix': 10.030}))
            (root/'ticks.json').write_text(json.dumps([10, [
                [7_000_000, 1_000_000, 0],
                [15_000_000, 2_000_000, 0],
                [23_000_000, 3_000_000, 0],
                [30_000_000, 99_000_000, 0],
            ]]))
            result = module.tick_lag(root)
            self.assertEqual(result['tickStartLagNormalizedMs']['count'], 2)
            self.assertAlmostEqual(result['tickStartLagNormalizedMs']['max'], 1)
            self.assertAlmostEqual(result['tickCompletionLagNormalizedMs']['max'], 2)

    def test_duplicate_stage_keys_match_in_fifo_order(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rows = [{'unix': t, 'threads': {}, 'softirqs': f'CPU0\nNET_RX: {n}\n'}
                    for t, n in [(101, 1), (102, 3)]]
            (root/'scheduler.json').write_text(json.dumps({'start': 101, 'end': 102, 'hz': 100, 'rows': rows}))
            records = [[0, 100, 0, 0, 0], [3, 101.3, 1, 9, 0], [4, 101.31, 1, 9, 0],
                       [3, 101.4, 1, 9, 0], [4, 101.42, 1, 9, 0]]
            (root/'stages.json').write_text(json.dumps(records))
            (root/'stages.json.simulation').write_text(json.dumps([
                [8, 101.1, 1, 9, 0], [8, 101.2, 1, 9, 0]]))
            result = module.reduce(root)
            self.assertEqual(result['softirqCounts']['NET_RX'], 2)
            self.assertEqual(result['slot0EncodeToQueueMs']['count'], 2)
            self.assertAlmostEqual(result['slot0EncodeToQueueMs']['p99'], 200)
            self.assertAlmostEqual(result['slot0QueueToSendPopMs']['p50'], 10)
            self.assertAlmostEqual(result['slot0QueueToSendPopMs']['p99'], 20)

    def test_stack_groups_count_each_period_once(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'stacks.txt'
            path.write_text('''slither-sockets 1/2 [000] 1.1: 10 cpu-clock:
  aaa tcp_sendmsg ([kernel.kallsyms])
  bbb handle_softirqs ([kernel.kallsyms])

swapper 0/0 [000] 1.2: 30 cpu-clock:
  aaa cpuidle_enter_state ([kernel.kallsyms])

''')
            self.assertEqual(module.kernel_groups(path), {'softirq': 25})


if __name__ == '__main__':
    unittest.main()
