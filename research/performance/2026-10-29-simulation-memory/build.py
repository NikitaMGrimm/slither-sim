import os,subprocess,tarfile,json,hashlib,shutil
from pathlib import Path
r=Path(__file__).resolve().parent
s=r/'source';s.mkdir()
with tarfile.open(r/'main.tar') as t:t.extractall(s,filter='data')
subprocess.run(['git','apply',str(r/'instrumentation.patch')],cwd=s,check=True)
def swap(p,a,b):
 text=p.read_text();assert text.count(a)==1,(p,a);p.write_text(text.replace(a,b))
def append(p,text):p.write_text(p.read_text()+'\n'+text+'\n')
fixture=(s/'research/performance/2026-10-28-mature-covered-bounds/fixture.rs').read_text()
needle='  let guard=measured.then(||crate::study_counter::enter(2));self.sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();drop(guard);'
assert fixture.count(needle)==1
fixture=fixture.replace(needle,'''  let epoch=self.sim.backend.food_epoch;let start=if measured{crate::study_counter::begin();cpu()}else{0.};
  self.sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();
  if measured{let ns=((cpu()-start)*1e9).round() as u64;crate::study_counter::end(self.sim.backend.tick,epoch!=self.sim.backend.food_epoch,ns);}
''')
fixture=fixture.replace('fn window(&mut self,ticks:usize,path:&Path,trace:bool)', 'fn window(&mut self,ticks:usize,path:&Path,trace:bool,event:i32,sample:bool)')
fixture=fixture.replace('  crate::study_counter::enable();','  crate::study_counter::configure(event,sample,path.to_str().unwrap());\n  fs::write(path.with_extension("maps"),fs::read("/proc/self/maps").unwrap()).unwrap();')
fixture=fixture.replace('  crate::study_counter::disable();','')
fixture=fixture.replace('f.window(ticks,path,trace);','f.window(ticks,path,trace,words[2].parse().unwrap(),words.get(3)==Some(&"sample"));')
fixture=fixture.replace(' drop(output);',' eprintln!("FOOTPRINT {}",footprint(&f.sim.backend));\n drop(output);')
fixture+='''
fn footprint(b:&super::SimulatorBackend)->serde_json::Value{
use std::mem::size_of;
let connections:Vec<_>=b.slots.iter().map(|s|&s.connection).collect();
serde_json::json!({
"sizes":{"point":size_of::<crate::model::Point>(),"snake":size_of::<crate::model::Snake>(),"food":size_of::<crate::model::Food>(),"slot":size_of::<super::connection::Slot>(),"connection":size_of::<super::connection::Connection>(),"knownSnake":size_of::<super::snakes::KnownSnake>(),"optionKnownSnake":size_of::<Option<super::snakes::KnownSnake>>(),"knownFood":size_of::<super::food::KnownFood>(),"listingEntry":size_of::<([u8;2],super::food::KnownFood)>(),"knownSectorEntry":size_of::<([u8;2],std::sync::Arc<super::food::SectorFood>)>()},
"bodies":{"len":b.world.snakes().iter().map(|s|s.body.len()).sum::<usize>(),"capacity":b.world.snakes().iter().map(|s|s.body.capacity()).sum::<usize>()},
"bounds":b.coverage_bounds.study_footprint(),
"knownSnakes":connections.iter().map(|c|c.snakes.study_footprint()).collect::<Vec<_>>(),
"interests":connections.iter().map(|c|c.food_interest.study_footprint()).collect::<Vec<_>>(),
"food":b.study_food_footprint(),
"worldFoodLen":b.world.food().len(),"slotsCapacity":b.slots.capacity(),"connections":connections.len()
})}
'''
(s/'src/client/simulation/study_host.rs').write_text(fixture)
(s/'examples/study_host.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_host::run(a[1]=="1",a[2].parse().unwrap(),a[3].parse().unwrap(),false);}\n')
if '--prepare-only' in __import__('sys').argv:raise SystemExit(0)
subprocess.run(['cc','-O2','-c',str(s/'src/study_events.c'),'-o',str(r/'events.o')],check=True)
subprocess.run(['ar','rcs',str(r/'libmemdiag.a'),str(r/'events.o')],check=True)
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR='/tmp/slither-simulation-memory-target',CARGO_BUILD_JOBS='2',CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_INCREMENTAL='0',RUSTC=compiler,RUSTDOC=str(Path(compiler).with_name('rustdoc')),RUSTFLAGS=f'-L native={r} -l static=memdiag',CARGO_NET_OFFLINE='true')
env.pop('CARGO_ENCODED_RUSTFLAGS',None)
cmd=['cargo','build','--release','--locked','--offline','-j2','--example','study_host']
with (r/'build.log').open('w') as log:subprocess.run(cmd,cwd=s,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
shutil.copy2('/tmp/slither-simulation-memory-target/release/examples/study_host',r/'study_host')
(r/'build.json').write_text(json.dumps({'runtime':'539b37f3be969827b5eac67c2978e604f9e814e8','archiveSha256':hashlib.file_digest((r/'main.tar').open('rb'),'sha256').hexdigest(),'binarySha256':hashlib.file_digest((r/'study_host').open('rb'),'sha256').hexdigest(),'command':cmd,'rustflags':env['RUSTFLAGS'],'compiler':subprocess.check_output([compiler,'--version'],text=True).strip()},indent=2))
print('BUILT',flush=True)
