import copy,csv,json,unittest
import recompute

class Study(unittest.TestCase):
    def setUp(self):
        with (recompute.ROOT/'runs.csv').open(newline='',encoding='utf-8') as stream:
            self.rows=list(csv.DictReader(stream))
        self.parity=json.loads((recompute.ROOT/'parity.json').read_text())
    def test_retained_records_and_report(self):
        recompute.validate(self.rows,self.parity)
        self.assertIn(recompute.table(self.rows),(recompute.ROOT/'REPORT.md').read_text(encoding='utf-8'))
    def test_short_or_skipped_parity_is_rejected(self):
        for field,value in [('callbacks',599),('skips',1)]:
            changed=copy.deepcopy(self.parity);next(iter(changed.values()))[field]=value
            with self.assertRaises(AssertionError):recompute.validate(self.rows,changed)
    def test_arm_reassignment_and_short_timing_are_rejected(self):
        for field,value in [('arm','candidate'),('callbacks','599')]:
            changed=copy.deepcopy(self.rows);changed[0][field]=value
            with self.assertRaises(AssertionError):recompute.validate(changed,self.parity)
if __name__=='__main__':unittest.main()
