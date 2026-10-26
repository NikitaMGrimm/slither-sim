"""Synthetic checks for decisions that determine the published grades/verdict."""
import json
from pathlib import Path
import tempfile
import unittest

from analyze import comparison, reduce_trial, stats


class AnalysisTests(unittest.TestCase):
    def test_exact_nearest_rank(self):
        self.assertEqual(stats(range(1, 101)), {'count': 100, 'p50': 50, 'p99': 99, 'max': 100})

    def test_regression_must_exceed_spread_strictly(self):
        def trials(b):
            return [{'name': f'{rev}{i}', 'config': {'bots': {'threads': 2}}, 'valid': True,
                     'simulationCpuPercentOneCore': v, 'excessMs': {'p99': v}}
                    for rev, values in [('A', [10, 11, 12]), ('B', b)]
                    for i, v in enumerate(values, 1)]
        result = comparison(trials([12, 13, 14]))
        self.assertFalse(result['simulationCpuPercentOneCore']['regressionBeyondSpread'])
        result = comparison(trials([12.1, 13.1, 14.1]))
        self.assertTrue(result['excessP99Ms']['regressionBeyondSpread'])
        self.assertAlmostEqual(result['excessP99Ms']['bMinusAMedian'], 2.1)
        self.assertEqual(result['excessP99Ms']['spread'], 2)
        with_smoke = trials([12, 13, 14])
        with_smoke.append({**with_smoke[0], 'name': 'Asmoke', 'simulationCpuPercentOneCore': 10000})
        self.assertEqual(comparison(with_smoke)['simulationCpuPercentOneCore']['aMedian'], 11)

    def fixture(self, root, excess=3, deferred=0):
        def write(name, obj):
            (root / name).write_text(json.dumps(obj))
        write('ready.json', {'startUnix': 100, 'endUnix': 400, 'config': {'bots': {'threads': 2}},
                             'policy': 'synthetic', 'parameterManifestSha256': 'synthetic'})
        write('host.json', {'rows': [
            {'boundary': 'start', 'unix': 100, 'ticks': 1000, 'deferred': 7, 'simulationSeconds': 5},
            {'boundary': 'end', 'unix': 400, 'ticks': 43857, 'deferred': 7 + deferred, 'simulationSeconds': 155}]})
        names = [('slither-simulat', 0, 150), ('slither-sockets', 0, 3), ('study-watcher', 1, 1),
                 ('slither-bots-0', 2, 30), ('slither-bots-1', 3, 60)]
        rows = [{'boundary': label, 'unix': epoch,
                 'systemCpu': f'cpu {100 + int(last) * 200} 0 0 {100 + int(last) * 200} 0 0 0 0',
                 'threads': {str(i): {'name': name, 'cpus': [cpu], 'cpuSeconds': cost if last else 0}
                             for i, (name, cpu, cost) in enumerate(names)}}
                for label, epoch, last in [('start', 100, False), ('end', 400, True)]]
        write('telemetry.json', {'rows': rows})
        write('watcher.json', {'readOrSendFailures': 0, 'playingSeconds': 300})
        # Two connection baselines; offsets cancel, and one long-hold tail stays.
        (root / 'frames.csv').write_text('connection,receipt_ms,server_ms,type\n1,100,0,103\n'
                                         f'1,{100 + excess},0,103\n2,500,0,103\n2,501,0,103\n')

    def test_grades_cpu_and_connection_normalization(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            result = reduce_trial(root)
            self.assertEqual(result['excessMs']['p99'], 3)
            self.assertEqual(result['simulationCpuPercentOneCore'], 50)
            self.assertEqual(result['hostCpuPercentOneCore'], 51)
            self.assertEqual(result['botCpuCores'], .3)
            self.assertTrue(result['officialLike'])
            self.fixture(root, excess=10)
            self.assertTrue(reduce_trial(root)['realTime'])
            self.assertFalse(reduce_trial(root)['officialLike'])
            self.fixture(root, excess=10.01)
            self.assertFalse(reduce_trial(root)['realTime'])
            self.fixture(root, deferred=1)
            self.assertFalse(reduce_trial(root)['realTime'])

    def test_missing_or_wrong_pins_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.fixture(root)
            path = root / 'telemetry.json'
            data = json.loads(path.read_text())
            for row in data['rows']:
                row['threads']['0']['cpus'] = [1]
            path.write_text(json.dumps(data))
            with self.assertRaisesRegex(ValueError, 'CPU placement'):
                reduce_trial(root)


if __name__ == '__main__':
    unittest.main()
