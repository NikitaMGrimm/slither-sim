import copy,csv,json,unittest
import recompute

class Study(unittest.TestCase):
    def setUp(self):
        with (recompute.ROOT/'runs.csv').open(newline='',encoding='utf-8') as stream:
            self.rows=list(csv.DictReader(stream))
        self.parity=json.loads((recompute.ROOT/'parity.json').read_text(encoding='utf-8'))
        self.identities=json.loads((recompute.ROOT/'identities.json').read_text(encoding='utf-8'))
    def test_retained_records_and_report(self):
        recompute.validate(self.rows,self.parity,self.identities)
        self.assertIn(recompute.table(self.rows),(recompute.ROOT/'REPORT.md').read_text(encoding='utf-8'))
    def test_short_or_skipped_parity_is_rejected(self):
        for field,value in [('callbacks',599),('skips',1)]:
            changed=copy.deepcopy(self.parity);next(iter(changed.values()))[field]=value
            with self.assertRaises(AssertionError):recompute.validate(self.rows,changed,self.identities)
    def test_arm_reassignment_and_short_timing_are_rejected(self):
        for field,value in [('arm','candidate'),('callbacks','599')]:
            changed=copy.deepcopy(self.rows);changed[0][field]=value
            with self.assertRaises(AssertionError):recompute.validate(changed,self.parity,self.identities)
    def test_mismatched_arm_binary_is_rejected(self):
        for arm in ['baseline','candidate']:
            with self.subTest(arm=arm):
                changed=copy.deepcopy(self.rows)
                next(row for row in changed if row['arm']==arm)['binary_sha256']='0'*64
                with self.assertRaisesRegex(AssertionError,'timing binary'):
                    recompute.validate(changed,self.parity,self.identities)
    def test_replaced_parity_configuration_is_rejected(self):
        changed=copy.deepcopy(self.parity)
        changed['unexpected/configuration']=changed.pop(next(iter(changed)))
        self.assertEqual(len(changed),10)
        with self.assertRaisesRegex(AssertionError,'parity configurations'):
            recompute.validate(self.rows,changed,self.identities)
if __name__=='__main__':unittest.main()
