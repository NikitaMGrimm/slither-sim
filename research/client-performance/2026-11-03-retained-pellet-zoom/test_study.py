import copy,gzip,hashlib,json,re,subprocess,tempfile,unittest
from pathlib import Path
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

 def test_instrumentation_reproduces_measured_sources(self):
  verify_instrumentation(self.i,(s.ROOT/'measurement-delta.patch').read_bytes())
 def test_changed_instrumentation_is_rejected(self):
  patch=(s.ROOT/'measurement-delta.patch').read_bytes()
  changed=patch.replace(b'profile_begin=std::time::Instant::now();',b'profile_begin=std::time::Instant::now(); // changed',1)
  self.assertNotEqual(changed,patch)
  with self.assertRaises(AssertionError)as error:verify_instrumentation(self.i,changed)
  self.assertEqual(error.exception.args[0][:2],('base','client/src/presentation/food.rs'))

def verify_instrumentation(ids,patch):
 # Frozen source bases survive squash merge/shallow clones. Never read current
 # runtime files or depend on PR-branch objects for this historical study.
 bases=json.loads(gzip.decompress((s.ROOT/'source-bases.json.gz').read_bytes()))
 needed={h for arm in s.ARMS for p,h in ids[arm]['runtime_source_sha256'].items()if h!=ids[arm]['measured_source_sha256'][p]}
 assert set(bases)==needed
 for digest,source in bases.items():assert hashlib.sha256(source.encode('utf-8')).hexdigest()==digest
 for arm in s.ARMS:
  with tempfile.TemporaryDirectory()as temp:
   root=Path(temp)
   for path,digest in ids[arm]['runtime_source_sha256'].items():
    if digest in bases:
     target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(bases[digest].encode('utf-8'))
   selected=b''.join(b for b in re.split(rb'(?=^--- )',patch,flags=re.M)if b.startswith(b'--- a/'+arm.encode()+b'/'))
   assert selected
   command=['git','-c','core.autocrlf=false','-c','core.eol=lf','apply','--unidiff-zero','--verbose','-p2','-']
   result=subprocess.run(command,input=selected,cwd=root,capture_output=True)
   assert result.returncode==0,result.stderr.decode('utf-8')
   assert b'offset'not in result.stderr
   for path,expected in ids[arm]['measured_source_sha256'].items():
    source=root/path
    actual=hashlib.sha256(source.read_bytes()).hexdigest()if source.exists()else ids[arm]['runtime_source_sha256'][path]
    assert actual==expected,(arm,path,actual,expected)

if __name__=='__main__':unittest.main()
