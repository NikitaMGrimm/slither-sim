"""Coarse diagnostic user-instruction brackets on one frozen runtime archive."""
import argparse,hashlib,json,os,shutil,statistics,subprocess,tarfile
from pathlib import Path
if not __debug__:raise SystemExit('run without -O')
folder=Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('archive',type=Path);p.add_argument('output',type=Path);p.add_argument('--cpu',type=int,default=1);p.add_argument('--control',type=Path,required=True);a=p.parse_args()
out=a.output.resolve();source=out/'source'
os.sched_setaffinity(0,{a.cpu});source.mkdir(parents=True)
archive=a.archive.resolve()
with tarfile.open(archive) as f:
 pin=f.pax_headers['comment'];f.extractall(source,filter='data')
def edit(name,before,after,count=1):
 p=source/name;s=p.read_text();assert s.count(before)==count,(name,before,s.count(before));p.write_text(s.replace(before,after))
def scope(name,signature,i):edit(name,signature,signature+f'\n        let _study_scope=crate::recipient_profile::Scope::new({i});')
edit('src/lib.rs','pub mod profiling;','pub mod profiling;\nmod recipient_profile;')
shutil.copy2(folder/'profile.rs',source/'src/recipient_profile.rs')
p=source/'src/client/simulation.rs';p.write_text(p.read_text()+'\npub mod study_protocols;\n')
study=source/'research/performance/2026-10-27-host-snake-candidates'
fixture=(study/'protocol_fixture.rs').read_text()
fixture=fixture.replace('        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();','        let study_tick=crate::recipient_profile::Tick::start();\n        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();')
fixture=fixture.replace('        population_sum += population;','        study_tick.finish(tick,population);\n        population_sum += population;')
(source/'src/client/simulation/study_protocols.rs').write_text(fixture)
child=source/'src/client/simulation/study_protocols';child.mkdir();shutil.copy2(study/'tick_counter.rs',child/'tick_counter.rs')
(source/'examples/study_protocols.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
b='src/client/simulation/broadcast.rs';s='src/client/simulation/snakes.rs';f='src/client/simulation/food.rs'
before='        self.world\n            .step(&actions)\n            .map_err(|error| error.to_string())?;'
edit(b,before,'        let study_physics=crate::recipient_profile::Scope::new(0);\n'+before+'\n        drop(study_physics);')
scope(f,'    pub(super) fn sync_food_sectors(&mut self) -> Result<FoodChanges, String> {',1)
scope(b,'    fn update_reports(&mut self) -> Result<(), String> {',2)
edit(b,'        let bounds: Vec<[f64; 4]> = self','        let study_bounds=crate::recipient_profile::Scope::new(3);\n        let bounds: Vec<[f64; 4]> = self')
line='        let grid = CoverageGrid::new(self.world.snakes(), &bounds, self.world.config().vision);'
edit(b,line,'        drop(study_bounds);\n        let study_grid=crate::recipient_profile::Scope::new(4);\n'+line+'\n        drop(study_grid);')
line='        let prey_bounds = PreyBounds::new(self.world.prey(), self.world.config().vision);'
edit(b,line,'        let study_prey_bounds=crate::recipient_profile::Scope::new(5);\n'+line+'\n        drop(study_prey_bounds);')
edit(b,'        let known = &self.slots[slot].connection.snakes;','        let study_candidates=crate::recipient_profile::Scope::new(6);\n        let known = &self.slots[slot].connection.snakes;')
edit(b,'        for i in order {','        drop(study_candidates);\n        let study_snakes=crate::recipient_profile::Scope::new(7);\n        for i in order {')
edit(b,'        let food_start = packets.len();','        drop(study_snakes);\n        let food_start = packets.len();')
edit(f,'    let subscribed = interest.subscribed();','    let _study_food=crate::recipient_profile::Scope::new(8);\n    let subscribed = interest.subscribed();')
edit(b,'        let mut visible = BTreeMap::new();','        let study_prey=crate::recipient_profile::Scope::new(9);\n        let mut visible = BTreeMap::new();')
edit(b,'        Ok((packets, presented))','        drop(study_prey);\n        Ok((packets, presented))')
edit('src/client/simulation/connection.rs','        if self.slots[slot].connection.output_fault.is_some() {','        let _study_frame=crate::recipient_profile::Scope::new(10);\n        if self.slots[slot].connection.output_fault.is_some() {')
edit('src/client/encode13.rs','        let mut out = Vec::with_capacity(packets.len());','        let _study_legacy=crate::recipient_profile::Scope::new(11);\n        let mut out = Vec::with_capacity(packets.len());')
scope(b,'    pub(super) fn new(world: &Simulator, profiles: &[PlayerAppearance]) -> Result<Self, String> {',12)
edit(b,'    ) -> Result<Option<Vec<u8>>, String> {','    ) -> Result<Option<Vec<u8>>, String> {\n        let _study_map=crate::recipient_profile::Scope::new(13);')
scope(b,'    fn hud(&mut self, slot: usize, board: Option<&Board>) -> Result<Vec<Vec<u8>>, String> {',14)
scope(b,'    fn broadcast_with(&mut self, early: usize, hand_over: &mut EarlyFrames) -> Result<(), String> {',15)
env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_TARGET_DIR=str(out/'target'))
with (out/'build.log').open('w') as log:subprocess.run(['cargo','build','--release','--locked','--example','study_protocols'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
(out/'bin').mkdir();binary=out/'bin/study_protocols';shutil.copy2(out/'target/release/examples/study_protocols',binary)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(out/'pins.json').write_text(json.dumps({'runtime':pin,'archiveSha256':sha(archive),'binarySha256':sha(binary),'moduleSha256':sha(source/'src/recipient_profile.rs'),'controlSha256':sha(a.control)},indent=2)+'\n')
selected={};ticks=None
for case in ['official500','official500-half','official500-legacy']:
 path=out/(case+'.phases.json')
 with (out/(case+'.log')).open('w') as log:subprocess.run([str(binary),case],env=dict(env,STUDY_PHASES=str(path)),stdout=log,stderr=subprocess.STDOUT,check=True)
 data=json.loads(path.read_text());rows=[r for r in data['rows'] if r['tick']>=120 and r['tick']!=720 and r['playing']==500]
 ids=[r['tick'] for r in rows];ticks=ticks or ids;assert ids==ticks and len(ids)==853
 assert all(r['enabled']>0 and r['running']/r['enabled']>=.99 for r in rows)
 summary=json.loads((out/(case+'.log')).read_text().splitlines()[-1])
 trace_paths=[]
 for name,exe in [('profile',binary),('control',a.control)]:
  path=out/(case+'-'+name+'.trace');trace_paths.append(path)
  with path.open('xb') as trace,(out/(case+'-'+name+'.stderr')).open('x') as err:
   subprocess.run([str(exe),case,'--trace'],env=dict(env,STUDY_PHASES=str(out/(case+'-control-phases.json'))),stdout=trace,stderr=err,check=True)
  assert json.loads((out/(case+'-'+name+'.stderr')).read_text().splitlines()[-1])==summary
 with trace_paths[0].open('rb') as left,trace_paths[1].open('rb') as right:
  while True:
   x,y=left.read(1<<20),right.read(1<<20);assert x==y
   if not x:break
 assert sha(a.control)==json.loads((out/'pins.json').read_text())['controlSha256']
 selected[case]={'traceEqual':True,'ticks':len(rows),'instructions':statistics.mean(r['instructions'] for r in rows),'phases':dict(zip(data['phases'],[statistics.mean(r['phases'][i] for r in rows) for i in range(16)])),'calls':dict(zip(data['phases'],[statistics.mean(r['calls'][i] for r in rows) for i in range(16)]))}
 (out/'selected.json').write_text(json.dumps(selected,indent=2)+'\n');print('PROFILE',case,json.dumps(selected[case]),flush=True)
