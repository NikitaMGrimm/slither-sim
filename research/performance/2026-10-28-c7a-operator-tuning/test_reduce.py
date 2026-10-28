import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name+'.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TuningTest(unittest.TestCase):
    def test_arm_requires_both_valid_repetitions(self):
        grade = load('reduce').grade_arm
        rows = [{'rep': 1, 'valid': True, 'gatePass': True}, {'rep': 2, 'valid': True, 'gatePass': True}]
        self.assertEqual(grade(rows), 'pass')
        rows[1]['gatePass'] = False
        self.assertEqual(grade(rows), 'miss')
        rows[1].update(gatePass=True, valid=False)
        self.assertEqual(grade(rows), 'miss')
        self.assertEqual(grade(rows[:1]), 'incomplete')

    def test_readback_and_irq_scope(self):
        probe = load('tuning_probe')
        self.assertEqual(probe.parse_coalesce('Adaptive RX: on  TX: n/a\nrx-usecs:\t64\ntx-usecs:\t64\n'),
                         {'adaptiveRx': 'on', 'rxUsecs': 64, 'txUsecs': 64})
        text = '  CPU0 CPU1\n11: 10 20 PCI-MSI nic-Tx-Rx-0\n12: 100 200 timer\n13: 3 4 PCI-MSI nic-Tx-Rx-1\n'
        self.assertEqual(probe.network_irq(text), 37)
        with self.assertRaises(ValueError):
            probe.network_irq('CPU0\n12: 100 timer\n')

    def test_resource_boundaries_and_priority(self):
        def row(at, boundary, user, system, irq):
            values = {'userSeconds': user, 'systemSeconds': system, 'nice': 5, 'name': 'slither-sockets'}
            return {'unix': at, 'boundary': boundary, 'networkIrqs': irq, 'totalIrqs': irq*2,
                    'processes': {'1': values}, 'threads': {'1': {'2': values}}}
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp)/'telemetry.json'
            p.write_text(json.dumps({'rows': [row(0, None, 0, 0, 0),
                row(10, 'start', 2, 4, 100), row(12, 'end', 2.1, 4.2, 110),
                row(20, None, 100, 100, 10000)]}))
            r = load('reduce').resources(p)
        self.assertAlmostEqual(r['processCpuPercent']['user'], 5)
        self.assertAlmostEqual(r['processCpuPercent']['system'], 10)
        self.assertEqual(r['networkIrqsPerSecond'], 5)
        self.assertEqual(r['totalIrqsPerSecond'], 10)
        self.assertEqual(r['threadCpuPercent']['slither-sockets']['niceEnd'], 5)


if __name__ == '__main__':
    unittest.main()
