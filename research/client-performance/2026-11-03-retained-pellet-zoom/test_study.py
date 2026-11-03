import copy,json,unittest
import recompute as s
class Study(unittest.TestCase):
 def setUp(self):
  self.r=s.rows('runs.csv');self.p=json.loads((s.ROOT/'parity.json').read_text(encoding='utf-8'));self.c=json.loads((s.ROOT/'canonical-controls.json').read_text(encoding='utf-8'));self.i=json.loads((s.ROOT/'identities.json').read_text(encoding='utf-8'))
 def test_retained_tables(self):
  s.validate(self.r,self.p,self.c,self.i)
  self.assertIn(s.tables(self.r),(s.ROOT/'REPORT.md').read_text(encoding='utf-8'))
 def test_bad_run_and_binary_values_fail(self):
  for key,value in [('binary_sha256','0'*64),('callbacks','599'),('skips','1'),('sent_digest',''),('retained_art_callbacks','299')]:
   r=copy.deepcopy(self.r);r[0][key]=value
   with self.subTest(key=key),self.assertRaises(AssertionError):s.validate(r,self.p,self.c,self.i)
  r=copy.deepcopy(self.r);r[0].update(status='compile_overlap',compile_samples='1',reason='observed rustc/link overlap',paired='True')
  with self.assertRaises(AssertionError):s.validate(r,self.p,self.c,self.i)
 def test_both_image_arms_and_configs_are_bound(self):
  key=next(iter(self.p))
  for arm in s.ARMS:
   for field,value in [('binary_sha256','0'*64),('callbacks',599),('skips',1),('image_sequence_sha256','')]:
    p=copy.deepcopy(self.p);p[key]['arms'][arm][field]=value
    with self.subTest(arm=arm,field=field),self.assertRaises(AssertionError):s.validate(self.r,p,self.c,self.i)
  p=copy.deepcopy(self.p);p['wrong']=p.pop(key)
  with self.assertRaises(AssertionError):s.validate(self.r,p,self.c,self.i)
  p=copy.deepcopy(self.p);k=next(k for k,r in p.items()if not r['exact']);p[k]['canonical_max_rgb']=3
  with self.assertRaises(AssertionError):s.validate(self.r,p,self.c,self.i)
 def test_inactive_or_mismatched_canonical_values_fail(self):
  for field,value in [('quad_digest','d1fba762150c532c'),('quad_digest',''),('canonical_and_send_digest',''),('canonical_quads',0),('callbacks',599),('skips',1),('binary_sha256','0'*64)]:
   c=copy.deepcopy(self.c);c['1920x1080']['candidate'][field]=value
   with self.subTest(field=field),self.assertRaises(AssertionError):s.validate(self.r,self.p,c,self.i)
if __name__=='__main__':unittest.main()
