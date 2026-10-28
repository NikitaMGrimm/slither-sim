"""Build and run coarse/nested probes from one explicit post-merge main archive."""
import argparse,hashlib,json,os,shutil,statistics,subprocess,tarfile
from pathlib import Path
if not __debug__:raise SystemExit('Run without Python optimization')
p=argparse.ArgumentParser();p.add_argument('ref');p.add_argument('root',type=Path);p.add_argument('--cpu',type=int,default=3);p.add_argument('--target',type=Path,required=True);a=p.parse_args()
if os.environ.get('RUSTFLAGS') or os.environ.get('CARGO_ENCODED_RUSTFLAGS'):raise SystemExit('Use the default target')
os.sched_setaffinity(0,{a.cpu});root=a.root.resolve();root.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
env=dict(os.environ,CARGO_BUILD_JOBS='1',RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1',RUSTUP_TOOLCHAIN='1.99.0',CARGO_PROFILE_RELEASE_DEBUG='1',CARGO_TARGET_DIR=str(a.target.resolve()))

def build(path):
 with (path/'build.log').open('w') as log:subprocess.run(['cargo','build','--release','--locked','--example','study_protocols'],cwd=path/'source',env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 shutil.copy2(a.target/'release/examples/study_protocols',path/'binary')
 print('BUILT',path.name,flush=True)
control=root/'control';source=control/'source';source.mkdir(parents=True)
archive=control/'source.tar';subprocess.run(['git','archive','-o',str(archive),a.ref],check=True)
with tarfile.open(archive) as f:f.extractall(source,filter='data')
q=source/'src/client/simulation.rs';q.write_text(q.read_text()+'\npub mod study_protocols;\n')
fixture=source/'research/performance/2026-10-27-host-snake-candidates'
shutil.copy2(fixture/'protocol_fixture.rs',source/'src/client/simulation/study_protocols.rs')
child=source/'src/client/simulation/study_protocols';child.mkdir();shutil.copy2(fixture/'tick_counter.rs',child/'tick_counter.rs')
(source/'examples/study_protocols.rs').write_text('fn main(){let a:Vec<_>=std::env::args().collect();slither_sim::client::simulation::study_protocols::run(&a[1],a.get(2).is_some_and(|s|s=="--trace"));}\n')
build(control)
summary=None
with (control/'trace').open('xb') as out,(control/'trace.stderr').open('w') as err:
 subprocess.run([str(control/'binary'),'official500','--trace'],env=env,stdout=out,stderr=err,check=True)
summary=json.loads((control/'trace.stderr').read_text().splitlines()[-1])
(control/'pins.json').write_text(json.dumps({'runtime':a.ref,'archiveSha256':sha(archive),'binarySha256':sha(control/'binary'),'fixtureSha256':sha(fixture/'protocol_fixture.rs')},indent=2)+'\n')
phases=['validation','setup','movement_boost_body','collision_records','collision_order_index','collision_tests','death_drops','pickup_cache','prey','refill_admission','finish','pre_world']
nested=['index_build_collision_and_prey','collision_candidate_query','collision_exact','movement_math','append_contract_body','speed','kiosk','cache_remove','eater_grid_build','pellet_lookup_tests_sort','admission_has_room','cache_push','prey_all','update_mass','validation_all','prey_decisions','pickup_circle','prey_motion','unused','pellet_position_or_rank_refresh']
selected={}
for mode in ['coarse','nested']:
 path=root/mode
 command=['python3',str(Path(__file__).with_name('profile.py')),a.ref,str(path)]
 if mode=='nested':command+=['--nested']
 subprocess.run(command,check=True)
 build(path)
 with (path/'run.log').open('w') as log:
  subprocess.run([str(path/'binary'),'official500'],env=dict(env,PICKUP_PROFILE=str(path/'phases.json')),stdout=log,stderr=subprocess.STDOUT,check=True)
 assert json.loads((path/'run.log').read_text().splitlines()[-1])==summary
 with (path/'trace').open('xb') as out,(path/'trace.stderr').open('w') as err:
  subprocess.run([str(path/'binary'),'official500','--trace'],env=dict(env,PICKUP_PROFILE=str(path/'trace-phases.json')),stdout=out,stderr=err,check=True)
 assert json.loads((path/'trace.stderr').read_text().splitlines()[-1])==summary
 with (path/'trace').open('rb') as left,(control/'trace').open('rb') as right:
  while True:
   x,y=left.read(1<<20),right.read(1<<20);assert x==y
   if not x:break
 rows=json.loads((path/'phases.json').read_text());assert len(rows)==853 and len({r['tick'] for r in rows})==853 and all(r['tick']>=120 and r['tick']!=720 for r in rows)
 scheduled=[];means={}
 for i,name in enumerate(phases):
  values=[]
  for row in rows:
   entries=[x for x in row['phases'] if x[0]==i];assert len(entries)==1
   _,count,enabled,running=entries[0];assert enabled>0 and running/enabled>=.99
   scheduled.append(running/enabled);values.append(count*enabled/running)
  means[name]=statistics.fmean(values)
 selected[mode]={'runtime':a.ref,'ticks':len(rows),'minimumScheduledPct':100*min(scheduled),'phases':means,'worldTotal':sum(means[n] for n in phases[:-1]),
                 'nestedInstructions':{n:statistics.fmean(r['sub'][i] for r in rows) for i,n in enumerate(nested)},
                 'nestedCalls':{n:statistics.fmean(r['calls'][i] for r in rows) for i,n in enumerate(nested)},
                 'binarySha256':sha(path/'binary'),'controlSha256':sha(control/'binary'),'traceSha256':sha(path/'trace'),'traceEqual':True,
                 'pins':json.loads((path/'source-pins.json').read_text())}
 (root/'selected.json').write_text(json.dumps(selected,indent=2)+'\n')
 print('PROFILE',mode,json.dumps(selected[mode]),flush=True)
