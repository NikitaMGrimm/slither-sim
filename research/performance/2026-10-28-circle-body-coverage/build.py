"""Private full-host study build; one bounded target, no guest changes."""
import importlib.util,json,os,shutil,subprocess,sys
from pathlib import Path
here=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('shared',here/'shared/reproduce.py');shared=importlib.util.module_from_spec(spec);spec.loader.exec_module(shared)
root=here;arm=sys.argv[1];profile='--profile' in sys.argv
os.sched_setaffinity(0,{int(os.environ.get('STUDY_BUILD_CPU','0'))})
# The shared phase overlay supports both the original and indexed private
# coverage signatures. Keep its eligibility-vector measurement identical.
original_swap=shared.swap
def adapted_swap(path,before,after):
 if path.name=='broadcast.rs' and 'body_bounds,' in path.read_text():
  old='covered(head, snake, bounds[i], vision)'
  new='covered(head, snake, bounds[i], vision, body_bounds.for_snake(i))'
  if before=='            let eligible = snake.alive && (i == slot || '+old+');':
   before='            let eligible = snake.alive\n                && (i == slot || '+new+');'
  after=after.replace(old,new)
 original_swap(path,before,after)
shared.swap=adapted_swap
name,source,pin=shared.prepare(root,arm,profile)
if arm=='mutation':
 # Mutate only recipient coverage in the counted window, preserving the full
 # physical warmup and authentic food/connection history.
 shared.swap(source/'src/model.rs','let r2 = r * r * 0.98;','let r2 = r * r;')
 shared.swap(source/'src/client/simulation/snakes.rs',
  '    let reach = vision + 1.;\n    if head.x < x0 - reach',
  '    let vision=if crate::study_counter::MUTATE.load(std::sync::atomic::Ordering::Relaxed){vision*0.98}else{vision};\n    let reach = vision + 1.;\n    if head.x < x0 - reach')
# Reconstruct the frozen fixture payload's historical terminal CRLF while
# keeping the checked-in helper itself LF, as required by CONTRIBUTING.md.
payload=(here/'fixture.rs').read_bytes().rstrip(b'\r\n')+b'\r\n'
if arm=='mutation':
 needle=b'  crate::study_counter::enable();'
 assert payload.count(needle)==1
 payload=payload.replace(needle,b'  crate::study_counter::MUTATE.store(true,std::sync::atomic::Ordering::Relaxed);\n'+needle)
(source/'src/client/simulation/study_host.rs').write_bytes(payload)
f=source/'src/client/simulation.rs';f.write_text(f.read_text()+'\npub mod study_host;\n')
c=source/'src/study_counter.rs';s=c.read_text().replace('pub fn enter(id:usize)->Guard {','fn enter_active(id:usize)->Guard {')
s+='''
static ENABLED:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
pub fn enter(id:usize)->Option<Guard>{ENABLED.load(std::sync::atomic::Ordering::Relaxed).then(||enter_active(id))}
pub fn enable(){
 let p=unsafe{dlsym(std::ptr::null_mut(),c"study_begin".as_ptr().cast())};
 if !p.is_null(){unsafe{std::mem::transmute::<*mut c_void,unsafe extern "C" fn()>(p)()};}
 ENABLED.store(true,std::sync::atomic::Ordering::Relaxed);
}
pub fn disable(){ENABLED.store(false,std::sync::atomic::Ordering::Relaxed);}
''';c.write_text(s)
if arm=='mutation':
 c.write_text(c.read_text()+'\npub static MUTATE:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);\n')
if profile:
 f=source/'src/simulator.rs'
 shared.swap(f,'fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {','fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {\nlet _compact=crate::study_counter::enter(10);')
 shared.swap(f,'        // Stable-slot indexing avoids searching the living list for every pair.','        let collision_build=crate::study_counter::enter(11);\n        // Stable-slot indexing avoids searching the living list for every pair.')
 shared.swap(f,'        candidates.clear();\n        dead.clear();','        drop(collision_build);let collision_query=crate::study_counter::enter(12);\n        candidates.clear();\n        dead.clear();')
 shared.swap(f,'        if PROFILE {\n            profile.collision_seconds','        drop(collision_query);\n        if PROFILE {\n            profile.collision_seconds')
 f=source/'src/client/simulation/snakes.rs'
 if 'body_bounds: BodyBounds::new(snakes),' in f.read_text():
  shared.swap(f,'body_bounds: BodyBounds::new(snakes),','body_bounds: { let _build=crate::study_counter::enter(13); BodyBounds::new(snakes) },')
if profile:
 f=source/'src/client/simulation/broadcast.rs'
 shared.swap(f,'        let bounds: Vec<[f64; 4]> = self','        let whole_body_bounds=crate::study_counter::enter(14);\n        let bounds: Vec<[f64; 4]> = self')
 shared.swap(f,'        let grid = if let Some(mut grid) = self.coverage_grid.take() {','        drop(whole_body_bounds);let grid_update=crate::study_counter::enter(15);\n        let grid = if let Some(mut grid) = self.coverage_grid.take() {')
 shared.swap(f,'        let prey_bounds = PreyBounds::new','        drop(grid_update);\n        let prey_bounds = PreyBounds::new')
if profile and '        body_bounds.update(snakes);' in (source/'src/client/simulation/snakes.rs').read_text():
 shared.swap(source/'src/client/simulation/snakes.rs','        body_bounds.update(snakes);','        { let _build=crate::study_counter::enter(13); body_bounds.update(snakes); }')
(source/'examples/study_host.rs').write_text('''fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_host::run(a[1]=="1",a[2].parse().unwrap(),a[3].parse().unwrap(),a[4]=="1");}\n''')
for example in ['study_protocols','perf_baseline']:
 f=source/f'examples/{example}.rs';shared.swap(f,'let g=slither_sim::study_counter::enter(1);','slither_sim::study_counter::enable();let g=slither_sim::study_counter::enter(1);')
 if arm=='mutation':
  shared.swap(f,'slither_sim::study_counter::enable();','slither_sim::study_counter::MUTATE.store(true,std::sync::atomic::Ordering::Relaxed);slither_sim::study_counter::enable();')
if '--prepare-only' in sys.argv:
 print(json.dumps({'name':name,'sources':{str(f.relative_to(source)):shared.sha(f) for f in source.rglob('*.rs')}}))
 raise SystemExit(0)
compiler=subprocess.check_output(['rustup','which','rustc'],text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'),CARGO_BUILD_JOBS='1',CARGO_PROFILE_RELEASE_DEBUG='1',RAYON_NUM_THREADS='1',RUSTC=compiler)
with (root/name/'build.log').open('x') as log:subprocess.run(['cargo','build','--release','--locked','--offline','--example','study_host','--example','study_protocols','--example','perf_baseline'],cwd=source,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
(source.parent/'bin').mkdir()
for binary in ['study_host','study_protocols','perf_baseline']:shutil.copy2(root/'target/release/examples'/binary,source.parent/'bin'/binary)
archive_arm='candidate' if arm=='mutation' else arm
pins={'archiveCommit':pin,'archiveSha256':shared.sha(root/(archive_arm+'.tar')),'toolchain':subprocess.check_output([compiler,'--version'],text=True).strip(),'lockfileSha256':shared.sha(source/'Cargo.lock'),'command':['cargo','build','--release','--locked','--offline','--example','study_host','--example','study_protocols','--example','perf_baseline'],'binaries':{f.name:shared.sha(f) for f in (source.parent/'bin').iterdir()},'sources':{str(f.relative_to(source)):shared.sha(f) for f in source.rglob('*.rs')},'driverSha256':shared.sha(Path(__file__)),'fixtureSha256':shared.sha(source/'src/client/simulation/study_host.rs')}
(source.parent/'pins.json').write_text(json.dumps(pins,indent=2)+'\n')
print('BUILT',name,flush=True)
