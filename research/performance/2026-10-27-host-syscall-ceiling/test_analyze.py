import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('ceiling', Path(__file__).with_name('analyze.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CeilingTest(unittest.TestCase):
    def test_units_and_thread_scope(self):
        counts = {'slither-sockets': {'sendto': 200, 'recvfrom': 100, 'futex': 50},
                  'slither-simulat': {'futex': 100}}
        result = module.ceilings(counts, 10, {'median': 1000, 'max': 2000})
        self.assertEqual(result['socketNetwork']['callsPerTick'], 30)
        self.assertAlmostEqual(result['socketNetwork']['medianProxyMsPerTick'], .03)
        self.assertAlmostEqual(result['socketNetwork']['medianProxyCorePercent'], .03/7*100)
        self.assertEqual(result['socketAll']['calls'], 350)
        self.assertEqual(result['hostAll']['calls'], 450)

    def test_perf_rows_strip_identifiers_and_keep_scope(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp)/'perf.csv'
            p.write_text('slither-sockets-101;200;;syscalls:sys_enter_sendto;1000000;100.00;;\n'
                         'slither-simulat-102;100;;syscalls:sys_enter_futex;1000000;100.00;;\n')
            result = module.perf_counts(p, {'101': 'slither-sockets', '102': 'slither-simulat'})
            self.assertEqual(result, {'slither-sockets': {'sendto': 200}, 'slither-simulat': {'futex': 100}})
            p.write_text('slither-sockets-101;<not counted>;;syscalls:sys_enter_sendto;0;0;;\n')
            with self.assertRaises(ValueError):
                module.perf_counts(p, {'101': 'slither-sockets'})


if __name__ == '__main__':
    unittest.main()
