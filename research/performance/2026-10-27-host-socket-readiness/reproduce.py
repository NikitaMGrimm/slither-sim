"""Linux x86-64 frozen host builds or fixed-work idle instruction pairs."""
import argparse,hashlib,json,os,shutil,statistics,subprocess,tarfile
from pathlib import Path
if not __debug__:raise SystemExit('run without -O: the checks are asserts')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('mode',choices=['build','idle']);p.add_argument('repository',type=Path);p.add_argument('output',type=Path)
p.add_argument('--base',default='46e00fae6f2c70abff6868e59f40ab74365b3a2d')
p.add_argument('--candidate',default='5b46f0b511f95f8e53c0258e3b770da55f0419d9')
p.add_argument('--archives',type=Path,help='Existing base.tar/candidate.tar from git archive, pinned by full SHA')
p.add_argument('--perf');a=p.parse_args();root=a.output.resolve();study=Path(__file__).resolve().parent
os.sched_setaffinity(0,{0});env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(cmd,log,cwd=root,extra=None):
 with Path(log).open('w') as f:subprocess.run(cmd,cwd=cwd,env=dict(env,**(extra or {})),stdout=f,stderr=subprocess.STDOUT,check=True)
if a.mode=='build':
 root.mkdir(parents=True,exist_ok=False)
 for arm,ref in [('base',a.base),('candidate',a.candidate)]:
  out=root/arm;source=out/'source';source.mkdir(parents=True)
  archive=out/'source.tar'
  if a.archives:shutil.copy2(a.archives/(arm+'.tar'),archive)
  else:subprocess.run(['git','-C',str(a.repository.resolve()),'archive','--format=tar','-o',str(archive),ref],check=True)
  with tarfile.open(archive) as f:
   if a.archives:assert f.pax_headers.get('comment')==ref,'Archive commit mismatch'
   f.extractall(source,filter='data')
  def edit(name,before,after):
   path=source/name;s=path.read_text();assert s.count(before)==1,(name,before);path.write_text(s.replace(before,after))
  edit('src/lib.rs','pub mod profiling;','pub mod profiling;\npub mod socket_study;')
  shutil.copy2(study/'socket_study.rs',source/'src/socket_study.rs')
  server='src/client/server.rs';old='        let read = self.inner.read(&mut bytes[..limit])?;'
  if old in (source/server).read_text():edit(server,old,'        let result = self.inner.read(&mut bytes[..limit]);\n        let read = result?;')
  edit(server,'        let read = result?;','        crate::socket_study::read(&result);\n        let read = result?;')
  edit(server,'        self.inner.write(bytes)','        let result = self.inner.write(bytes);\n        crate::socket_study::write(&result);\n        result')
  edit(server,'        self.inner.flush()','        crate::socket_study::event(8,1);\n        self.inner.flush()')
  edit(server,'    fn poll(&mut self) -> Result<(), String> {','    fn poll(&mut self) -> Result<(), String> {\n        crate::socket_study::event(11,1);')
  edit(server,'    fn deliver(&mut self, delivery: Delivery, now: Instant) {','    fn deliver(&mut self, delivery: Delivery, now: Instant) {\n        crate::socket_study::event(12,1);')
  edit(server,'    fn send(&mut self, mut framed: Vec<u8>) -> Result<(), String> {','    fn send(&mut self, mut framed: Vec<u8>) -> Result<(), String> {\n        crate::socket_study::event(9,1);\n        crate::socket_study::event(10,framed.len() as u64);')
  ready=(source/'src/client/server/readiness.rs').exists()
  if ready:edit('src/client/server/readiness.rs','        self.fds.clear();','        crate::socket_study::event(14,1);\n        self.fds.clear();')
  shutil.copy2(source/'research/performance/2026-10-26-host-capacity/host_probe.rs',source/'examples/socket_probe.rs')
  edit('examples/socket_probe.rs','        if now >= due {','        if now >= due {\n            if boundary==0 && now>=start {slither_sim::socket_study::active(true);}\n            if now>=end {slither_sim::socket_study::active(false);}')
  (source/'src/client/server').mkdir(exist_ok=True)
  (source/server).write_text((source/server).read_text()+'\npub mod study_idle;\n')
  (source/'src/client/server/study_idle.rs').write_text((study/'idle.rs').read_text().replace('/* READINESS */','readiness: readiness::Readiness::default(),' if ready else ''))
  (source/'examples/study_idle.rs').write_text('fn main(){slither_sim::client::server::study_idle::run();}\n')
  load=source/'examples/host_capacity_load.rs'
  run(['patch','-o',str(load),str(source/'research/performance/2026-10-26-host-capacity/host_capacity_load.rs'),str(source/'research/performance/2026-10-26-host-known-slots/host-players.patch')],out/'load-patch.log')
  run(['python3',str(study/'load_overlay.py'),str(load)],out/'load-overlay.log')
  target=out/'target';names=['socket_probe','host_capacity_load','study_idle']
  run(['cargo','build','--release','--locked',*[x for n in names for x in ['--example',n]]],out/'build.log',source,{'CARGO_TARGET_DIR':str(target)})
  (out/'bin').mkdir()
  for n in names:shutil.copy2(target/'release/examples'/n,out/'bin'/n)
  (out/'pins.json').write_text(json.dumps({'source':ref,'archiveSha256':sha(archive),'binaries':{n:sha(out/'bin'/n) for n in names}},indent=2)+'\n')
  print('BUILT',arm,flush=True)
if a.mode=='idle':
 assert a.perf;out=root/'idle';out.mkdir();rows=[]
 def rehash():
  for arm in ['base','candidate']:
   for n,v in json.loads((root/arm/'pins.json').read_text())['binaries'].items():assert sha(root/arm/'bin'/n)==v
 rehash()
 for i in range(-1,5):
  row={}
  for arm in (['base','candidate'] if i<0 or i%2==0 else ['candidate','base']):
   prefix=out/f'{i}-{arm}'
   run([a.perf,'stat','-x;','-e','instructions:u','-o',str(prefix.with_suffix('.stat')),'--',str(root/arm/'bin/study_idle')],prefix.with_suffix('.log'),extra={'STUDY_SOCKET_COUNTS':str(prefix.with_suffix('.counts.json'))})
   fields=next(l.split(';') for l in prefix.with_suffix('.stat').read_text().splitlines() if 'instructions:u' in l);assert float(fields[4])>=99
   row[arm]=int(float(fields[0]));v=json.loads(prefix.with_suffix('.counts.json').read_text());v=dict(zip(v['names'],v['values']))
   assert v['gateway_polls']==4000 and v['read_attempts']==(2_000_000 if arm=='base' else 0)
   assert v['readiness_polls']==(4000 if arm=='candidate' else 0) and v['write_attempts']==v['frames_offered']==0
  row['ratio']=row['candidate']/row['base'];rows.append(row);print(i,row,flush=True)
 v=rows[1:];(out/'selected.json').write_text(json.dumps({'baseMedian':statistics.median(r['base'] for r in v),'candidateMedian':statistics.median(r['candidate'] for r in v),'pairedMedian':statistics.median(r['ratio'] for r in v),'pairedRange':[min(r['ratio'] for r in v),max(r['ratio'] for r in v)]},indent=2)+'\n')
 rehash()
