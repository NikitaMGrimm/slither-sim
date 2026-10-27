import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('recheck',Path(__file__).with_name('analyze.py'))
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)


class TickTest(unittest.TestCase):
    def test_half_open_hold_and_nearest_rank(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            (root/'host.json').write_text(json.dumps({'holdStartUnix':11,'holdEndUnix':12}))
            (root/'ticks.json').write_text(json.dumps([10,[
                [999999999,999000000,999000000],
                [1000000000,2000000,7000000],
                [1500000000,3000000,8000000],
                [2000000000,888000000,888000000]]]))
            r=module.tick_stats(root)
            self.assertEqual(r['tickBusyMs']['count'],2)
            self.assertEqual(r['tickBusyMs']['p50'],2)
            self.assertEqual(r['tickBusyMs']['p99'],3)
            self.assertEqual(r['tickBusyMs']['max'],3)
            self.assertEqual(r['tickStartGapMs']['max'],8)


if __name__=='__main__': unittest.main()
