import copy
import json
import unittest
import recompute as s
class Study(unittest.TestCase):
 def setUp(self):
  self.r=s.rows('runs.csv');self.p=json.loads((s.ROOT/'parity.json').read_text(encoding='utf-8'));self.i=json.loads((s.ROOT/'identities.json').read_text(encoding='utf-8'))
 def test_retained_controls_and_tables(self):
  s.validate(self.r,self.p,self.i)
  self.assertIn(s.tables(self.r,s.rows('idle.csv'),s.rows('density.csv'),s.rows('callback-stages.csv')),(s.ROOT/'REPORT.md').read_text(encoding='utf-8'))
 def test_timing_binaries_and_pair_selection(self):
  for arm in s.ARMS:
   r=copy.deepcopy(self.r);next(x for x in r if x['cohort']=='final'and x['arm']==arm)['binary_sha256']='0'*64
   with self.subTest(arm=arm),self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
  for field,value in [('paired','True'),('status','accepted')]:
   r=copy.deepcopy(self.r);next(x for x in r if x['status']=='compile_overlap')[field]=value
   with self.subTest(field=field),self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
 def test_complete_replay_and_image_counts_are_required(self):
  r=copy.deepcopy(self.r);r[0]['callbacks']='599'
  with self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
  with self.assertRaises(AssertionError):s.validate(self.r[1:],self.p,self.i)
  key=next(iter(self.p['final']))
  for field,value in [('callbacks',599),('baseline_callbacks',599),('skips',1),('baseline_skips',1),('binary_sha256','0'*64),('baseline_binary_sha256','0'*64)]:
   p=copy.deepcopy(self.p);p['final'][key][field]=value
   with self.subTest(field=field),self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
  p=copy.deepcopy(self.p);p['final']['wrong/key']=p['final'].pop(key)
  with self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
 def test_active_controls_require_shaped_nonempty_digests(self):
  for value in ['', 'bad', '0'*15, 'G'*16]:
   r=copy.deepcopy(self.r);r[0]['command_digest']=value
   with self.subTest(value=value),self.assertRaises(AssertionError):s.validate(r,self.p,self.i)
  key=next(iter(self.p['final']))
  for value in ['', 'bad', '0'*63, 'G'*64]:
   p=copy.deepcopy(self.p);p['final'][key]['image_sequence_sha256']=p['final'][key]['baseline_image_sequence_sha256']=value
   with self.subTest(value=value),self.assertRaises(AssertionError):s.validate(self.r,p,self.i)
 def test_inactive_canonical_values_are_not_treated_as_evidence(self):
  r=copy.deepcopy(self.r)
  for row in r:row['inactive_quad_accumulator']='absent'
  s.validate(r,self.p,self.i)
if __name__=='__main__':unittest.main()