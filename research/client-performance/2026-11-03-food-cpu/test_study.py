import copy,csv,json,unittest
import recompute

class Study(unittest.TestCase):
    def setUp(self):
        with (recompute.ROOT/'runs.csv').open(newline='',encoding='utf-8') as stream:
            self.rows=list(csv.DictReader(stream))
        self.parity=json.loads((recompute.ROOT/'parity.json').read_text(encoding='utf-8'))
        self.identities=json.loads((recompute.ROOT/'identities.json').read_text(encoding='utf-8'))
        self.canonical=json.loads((recompute.ROOT/'canonical-parity.json').read_text(encoding='utf-8'))
    def test_retained_records_and_report(self):
        recompute.validate(self.rows,self.parity,self.identities,self.canonical)
        self.assertIn(recompute.table(self.rows),(recompute.ROOT/'REPORT.md').read_text(encoding='utf-8'))
    def test_short_or_skipped_parity_is_rejected(self):
        for field,value in [('callbacks',599),('skips',1)]:
            changed=copy.deepcopy(self.parity);next(iter(changed.values()))[field]=value
            with self.assertRaises(AssertionError):recompute.validate(self.rows,changed,self.identities,self.canonical)
    def test_arm_reassignment_and_short_timing_are_rejected(self):
        for field,value in [('arm','candidate'),('callbacks','599')]:
            changed=copy.deepcopy(self.rows);changed[0][field]=value
            with self.assertRaises(AssertionError):recompute.validate(changed,self.parity,self.identities,self.canonical)
    def test_mismatched_arm_binary_is_rejected(self):
        for arm in ['baseline','candidate']:
            with self.subTest(arm=arm):
                changed=copy.deepcopy(self.rows)
                next(row for row in changed if row['arm']==arm)['binary_sha256']='0'*64
                with self.assertRaisesRegex(AssertionError,'timing binary'):
                    recompute.validate(changed,self.parity,self.identities,self.canonical)
    def test_replaced_parity_configuration_is_rejected(self):
        changed=copy.deepcopy(self.parity)
        changed['unexpected/configuration']=changed.pop(next(iter(changed)))
        self.assertEqual(len(changed),10)
        with self.assertRaisesRegex(AssertionError,'parity configurations'):
            recompute.validate(self.rows,changed,self.identities,self.canonical)
    def test_wrong_parity_binary_in_either_arm_is_rejected(self):
        for field in ['baseline_binary_sha256','binary_sha256']:
            with self.subTest(field=field):
                changed=copy.deepcopy(self.parity)
                next(iter(changed.values()))[field]='0'*64
                with self.assertRaisesRegex(AssertionError,'parity binary'):
                    recompute.validate(self.rows,changed,self.identities,self.canonical)
    def test_baseline_binary_for_both_parity_arms_is_rejected(self):
        changed=copy.deepcopy(self.parity)
        for record in changed.values():
            record['binary_sha256']=record['baseline_binary_sha256']
        with self.assertRaisesRegex(AssertionError,'candidate parity binary'):
            recompute.validate(self.rows,changed,self.identities,self.canonical)
        identities=copy.deepcopy(self.identities)
        identities['candidate_image_binary_sha256']=identities['baseline_image_binary_sha256']
        with self.assertRaisesRegex(AssertionError,'distinct binaries'):
            recompute.validate(self.rows,changed,identities,self.canonical)
    def test_mismatched_image_drivers_are_rejected(self):
        changed=copy.deepcopy(self.identities)
        changed['baseline_image_driver_sha256']='0'*64
        with self.assertRaisesRegex(AssertionError,'image drivers differ'):
            recompute.validate(self.rows,self.parity,changed,self.canonical)
    def test_inactive_or_empty_canonical_gate_cannot_pass(self):
        for arm in ['baseline','candidate']:
            for field,value in [('active',False),('quad_digest',recompute.EMPTY_QUAD_DIGEST),('callbacks',599)]:
                changed=copy.deepcopy(self.canonical)
                changed[arm][field]=value
                with self.subTest(arm=arm,field=field), self.assertRaises(AssertionError):
                    recompute.validate(self.rows,self.parity,self.identities,changed)
        changed=copy.deepcopy(self.canonical)
        for arm in changed:changed[arm]['quad_digest']=recompute.EMPTY_QUAD_DIGEST
        with self.assertRaisesRegex(AssertionError,'no active digest'):
            recompute.validate(self.rows,self.parity,self.identities,changed)
if __name__=='__main__':unittest.main()
