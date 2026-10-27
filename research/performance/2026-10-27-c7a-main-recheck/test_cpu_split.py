import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('cpu_split', Path(__file__).with_name('cpu_split.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class CpuTest(unittest.TestCase):
    def test_aligned_process_and_thread_counters(self):
        def row(at, boundary, user, system, nice):
            counters = {'userSeconds': user, 'systemSeconds': system, 'nice': nice,
                        'name': 'slither-sockets'}
            return {'unix': at, 'boundary': boundary,
                    'processes': {'10': counters}, 'threads': {'10': {'11': counters}}}

        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'telemetry.json'
            path.write_text(json.dumps({'rows': [row(0, None, 0, 0, 0),
                row(10, 'start', 2, 4, 5), row(20, 'end', 3, 7, 5),
                row(30, None, 100, 100, 5)]}))
            result = module.split(path)
        self.assertEqual(result['processPercentOneCore'], {'user': 10, 'system': 30})
        self.assertEqual(result['threadPercentOneCore']['slither-sockets'],
                         {'user': 10, 'system': 30, 'niceStart': 5, 'niceEnd': 5})


if __name__ == '__main__':
    unittest.main()
