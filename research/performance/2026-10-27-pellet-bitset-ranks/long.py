"""Build frozen old-base/current-PR diagnostics and run 60k ticks per case."""
from pathlib import Path
import argparse,gzip,hashlib,json,os,shutil,subprocess,tarfile
if not __debug__:raise SystemExit('Run without optimization')
if os.environ.get('RUSTFLAGS') or os.environ.get('CARGO_ENCODED_RUSTFLAGS'):raise SystemExit('Use default target')
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('repository',type=Path);p.add_argument('output',type=Path)
p.add_argument('--cpu',type=int,default=2);p.add_argument('--ticks',type=int,default=60000)
p.add_argument('--target-root',type=Path)
p.add_argument('--reuse-builds',type=Path)
p.add_argument('--reuse-runs',type=Path)
p.add_argument('--base',default='4532a76f22958ae3abcca801e90edd30ed9f466f')
p.add_argument('--lazy',default='8a75634407d8a106aa0480eeb5aab51d218026d1')
p.add_argument('--candidate',default='9a3f6f5fe85184e42b32c2251480b94356c9210e')
a=p.parse_args();os.sched_setaffinity(0,{a.cpu})
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
repo=a.repository.resolve();study=Path(__file__).resolve().parent
env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for arm,ref in [('base',a.base),('lazy',a.lazy),('candidate',a.candidate)]:
 out=root/arm
 if arm!='candidate' and a.reuse_builds and (a.reuse_builds/arm/'pins.json').exists():
  previous=a.reuse_builds/arm;pins=json.loads((previous/'pins.json').read_text())
  if pins['runtime']!=ref or sha(previous/'binary')!=pins['binarySha256'] or sha(previous/'source.tar')!=pins['archiveSha256']:raise SystemExit('Reuse pin mismatch')
  for name,digest in pins['overlaySha256'].items():
   if sha(previous/'source'/name)!=digest:raise SystemExit('Reuse source mismatch')
  if sha(previous/'source/src/study_rebuild_probe.rs')!=sha(study/'long_counter.rs') or sha(previous/'source/src/client/simulation/study_long.rs')!=sha(study/'long_fixture.rs'):raise SystemExit('Reuse fixture mismatch')
  shutil.copytree(previous,out);print('REUSED',arm,flush=True);continue
 source=out/'source';source.mkdir(parents=True);archive=out/'source.tar'
 subprocess.run(['git','-C',str(repo),'archive','-o',str(archive),ref],check=True)
 with tarfile.open(archive) as f:f.extractall(source,filter='data')
 shutil.copy2(study/'long_counter.rs',source/'src/study_rebuild_probe.rs')
 shutil.copy2(study/'long_fixture.rs',source/'src/client/simulation/study_long.rs')
 for name,extra in [('src/lib.rs','\npub mod study_rebuild_probe;\n'),('src/client/simulation.rs','\npub mod study_long;\n'),('src/simulator.rs','\nimpl Simulator { pub(crate) fn study_retained(&self)->usize {self.food_cache.study_retained()} }\n')]:
  p=source/name;p.write_text(p.read_text()+extra)
 (source/'examples/study_long.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_long::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
 p=source/'src/food_grid.rs';s=p.read_text();start=s.index('    pub(crate) fn pellets(');end=s.index('        self.pellets.as_ref()',start)
 incremental='    growing: Option<Box<PelletIndex>>' in s
 hook=s.index('        if self.pellets.is_none()',start) if arm=='base' else s.index('        if let Some(index) = &mut self.pellets {',start) if incremental else s.index('        if self\n',start)
 extra='''        let study_old=self.pellets.as_ref().map_or(0,|p|p.slots.len());
        let study_rebuilding=self.pellets.is_none()'''+(';' if arm=='base' else ' || study_old>=food.len().max(4096).saturating_mul(2)' + (' || self.pellets.as_ref().is_some_and(|p|p.growing.is_some());' if incremental else ';'))+'''
        let study_start=if study_rebuilding {crate::study_rebuild_probe::read_now()} else {None};
'''
 active='self.pellets.as_ref().is_some_and(|p|p.growing.is_some())' if incremental else 'false'
 extra+=f'        let study_active={active};\n'
 progress='self.pellets.as_ref().and_then(|p|p.growing.as_ref()).map_or(food.len(),|p|p.food_slots.len())' if incremental else 'food.len()'
 tail=f'        if study_rebuilding {{crate::study_rebuild_probe::rebuild(study_start,study_old,food.len(),study_old>0 && !study_active,!({active}),{progress});}}\n'
 s=s[:end]+tail+s[end:]
 s=s[:hook]+extra+s[hook:]
 s+='\nimpl FoodCache {pub(crate) fn study_retained(&self)->usize {self.pellets.as_ref().map_or(0,|p|p.slots.len())}}\n'
 p.write_text(s)
 for p in source.rglob('*.rs'):os.utime(p,None)
 target=(a.target_root.resolve()/('native-'+('base' if arm=='base' else 'candidate'))/'target') if a.target_root else out/'target';env['CARGO_TARGET_DIR']=str(target)
 with (out/'build.log').open('w') as log:subprocess.run(['cargo','build','--release','--locked','--example','study_long'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(target/'release/examples/study_long',out/'binary')
 (out/'pins.json').write_text(json.dumps({'runtime':ref,'archiveSha256':sha(archive),'binarySha256':sha(out/'binary'),'overlaySha256':{n:sha(source/n) for n in ['src/food_grid.rs','src/simulator.rs','src/study_rebuild_probe.rs','src/client/simulation/study_long.rs']}},indent=2)+'\n')
 print('BUILT',arm,flush=True)
for case in ['official500','official500-orbit','battledome80','official500-shrink']:
 reports={}
 for arm in ['base','lazy','candidate']:
  prefix=root/f'{case}-{arm}'
  if arm!='candidate' and a.reuse_runs:
   previous=a.reuse_runs/f'{case}-{arm}'
   pin=json.loads((a.reuse_runs/arm/'pins.json').read_text())
   if pin['binarySha256']!=sha(root/arm/'binary'):raise SystemExit('Run reuse binary mismatch')
   for suffix in ['.log','.json','.bin.gz','.trace.json']:shutil.copy2(previous.with_suffix(suffix),prefix.with_suffix(suffix))
   print('REUSED RUN',case,arm,flush=True)
  else:
   digest=hashlib.sha256();size=0
   with prefix.with_suffix('.log').open('w') as log:
    child=subprocess.Popen([str(root/arm/'binary'),case,'--trace'],env=dict(env,LONG_COUNTS=str(prefix.with_suffix('.json')),LONG_TICKS=str(a.ticks)),stdout=subprocess.PIPE,stderr=log)
    with gzip.open(prefix.with_suffix('.bin.gz'),'wb',compresslevel=1) as output:
     while block:=child.stdout.read(1<<20):output.write(block);digest.update(block);size+=len(block)
    if child.wait()!=0:raise SystemExit('Long fixture failed')
   prefix.with_suffix('.trace.json').write_text(json.dumps({'sha256':digest.hexdigest(),'bytes':size})+'\n')
  reports[arm]=json.loads(prefix.with_suffix('.log').read_text().splitlines()[-1])
  data=json.loads(prefix.with_suffix('.json').read_text())
  if len(data['rows'])!=a.ticks:raise SystemExit('Wrong tick count')
  if any(r['counts'][1]<=0 or r['counts'][2]/r['counts'][1]<.99 for r in data['rows']):raise SystemExit('Tick counter scheduling below 99%')
  periodic=[r for r in data['rebuilds'] if r['old']]
  print('MEASURED',case,arm,'rebuilds',len(periodic),'maxRebuildInstructions',max((r['counts'][0] for r in periodic),default=0),flush=True)
 if reports['base']!=reports['candidate'] or reports['base']!=reports['lazy']:raise SystemExit('Endpoint/summary changed')
 for arm in ['lazy','candidate']:
  with gzip.open(root/f'{case}-base.bin.gz','rb') as left,gzip.open(root/f'{case}-{arm}.bin.gz','rb') as right:
   while True:
    x,y=left.read(1<<20),right.read(1<<20)
    if x!=y:raise SystemExit('Raw trace bytes changed')
    if not x:break
 print('MATCHED',case,flush=True)
