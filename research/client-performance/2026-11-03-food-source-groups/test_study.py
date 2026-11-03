import copy
import json
import unittest
import recompute as s


class Study(unittest.TestCase):
    def setUp(self):
        self.r=s.rows('runs.csv')
        self.p=json.loads((s.ROOT/'parity.json').read_text(encoding='utf-8'))
        self.i=json.loads((s.ROOT/'identities.json').read_text(encoding='utf-8'))

    def test_retained_tables(self):
        s.validate(self.r,self.p,self.i)
        self.assertIn(s.tables(self.r,s.rows('tail-stages.csv')),(s.ROOT/'REPORT.md').read_text(encoding='utf-8'))

    def test_timing_identity_selection_and_completeness(self):
        for key,value in [('binary_sha256','0'*64),('callbacks','599'),('skips','1'),('sent_digest',''),('canonical_digest','d1fba762150c532c')]:
            r=copy.deepcopy(self.r);r[0][key]=value
            with self.subTest(key=key),self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
        r=copy.deepcopy(self.r)
        next(x for x in r if x['status']=='compile_overlap')['paired']='True'
        with self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
        with self.assertRaises(AssertionError):s.validate(self.r[1:],self.p,self.i)
        i=copy.deepcopy(self.i);i['candidate']['driver_sha256']='0'*64
        with self.assertRaises(AssertionError):s.validate(self.r,self.p,i)

    def test_both_image_arms_and_exact_configurations(self):
        key=next(iter(self.p))
        for arm in s.ARMS:
            for field,value in [('binary_sha256','0'*64),('callbacks',599),('skips',1),('image_sequence_sha256','')]:
                p=copy.deepcopy(self.p);p[key][arm][field]=value
                with self.subTest(arm=arm,field=field),self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
        p=copy.deepcopy(self.p);p['wrong-key']=p.pop(key)
        with self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
        p=copy.deepcopy(self.p);p[key]['candidate']['image_sequence_sha256']='0'*64
        with self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
        i=copy.deepcopy(self.i);i['candidate']['binary_sha256']=i['base']['binary_sha256']
        with self.assertRaises(AssertionError):s.validate(self.r,self.p,i)


if __name__=='__main__':unittest.main()
