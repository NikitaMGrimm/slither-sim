import tempfile
from pathlib import Path
import unittest
from analyze import stats, timing

class TimingTests(unittest.TestCase):
    def test_nearest_rank(self):
        self.assertEqual(stats(range(1,101))['p99'],99)
        self.assertEqual(stats([]),{'count':0})

    def test_each_connection_has_its_own_minimum(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/'load-0.frames.csv').write_text('client,connection,receipt_ms,server_ms,bytes\n0,1,100,0,3\n0,1,112,10,3\n0,2,1000,0,3\n0,2,1015,10,3\n1,1,50,0,3\n1,1,61,10,3\n')
            (root/'load-0.intervals.csv').write_text('client,connection,receipt_ms,interval_ms,header_interval_ms\n0,1,112,12,10\n0,2,1015,15,10\n1,1,61,11,10\n')
            result=timing(root)
            self.assertEqual(result['excessMs']['count'],6)
            self.assertEqual(result['excessMs']['p50'],0)
            self.assertEqual(result['excessMs']['max'],5)
            self.assertEqual(result['perClient']['1']['excessMs']['max'],1)
            self.assertEqual(result['ownUpdateGapMs']['p50'],12)

if __name__=='__main__': unittest.main()
