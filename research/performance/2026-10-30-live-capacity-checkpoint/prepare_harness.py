"""Adapt the linked #256 harness in an external output directory, once.

Run after applying #256 harness.patch and providing shared248 plus three archives.
Requires Python 3.11.4+ for tarfile's data filter. Production files are untouched.
"""
from pathlib import Path
import sys
r=Path(sys.argv[1]).resolve()
def change(name,old,new):
 p=r/name;s=p.read_text();assert s.count(old)==1,(name,old,s.count(old));p.write_text(s.replace(old,new),newline='\n')
for name in ['build.py','client_build.py']:
 p=r/name;s=p.read_text().replace("r/'official-density-target'","Path('/tmp/slither-live-checkpoint-target')").replace("r/'official-density-target/release/examples/capacity_clients'","Path('/tmp/slither-live-checkpoint-target/release/examples/capacity_clients')")
 p.write_text(s,newline='\n')
change('build.py',"'--bin','slither-sim','--example','capacity_clients'","'--bin','slither-sim'")
change('build.py',"[('host','release/slither-sim'),('clients','release/examples/capacity_clients')]","[('host','release/slither-sim')]")
change('build.py',"(source/'examples/capacity_clients.rs').write_bytes", "p.swap(source/'src/client/simulation/broadcast.rs','        if epoch > self.food_epoch {','        if epoch > self.food_epoch {\\n            crate::study_observe::maintenance();')\n(source/'examples/capacity_clients.rs').write_bytes")
change('client_build.py',"tarfile.open(r/'base.tar')","tarfile.open(r/'client-base.tar')")
change('client_build.py',"target.resolve().parent==r","target.resolve()==Path('/tmp/slither-live-checkpoint-target')")
change('reconstruct.py',"tarfile.open(r/'base.tar')","tarfile.open(r/'client-base.tar')")
change('timing.rs','static ACTIVE:AtomicBool=AtomicBool::new(false);','static ACTIVE:AtomicBool=AtomicBool::new(false);\nstatic DETAIL:AtomicBool=AtomicBool::new(false);')
change('timing.rs','rows:Vec<[u64;3]>','rows:Vec<[u64;3]>,last_stages:[u64;16],phase_rows:Vec<[u64;16]>,food_rows:Vec<bool>,food:bool')
change('timing.rs','if tick%100==0{ACTIVE.store','if tick%100==0{DETAIL.store(std::env::var_os("CAPACITY_DETAIL").is_some_and(|p|std::path::Path::new(&p).exists()),Ordering::Relaxed);ACTIVE.store')
change('timing.rs','  if tick%100==0{','  if DETAIL.load(Ordering::Relaxed){s.phase_rows.push(std::array::from_fn(|i|s.stages[i]-s.last_stages[i]));s.food_rows.push(s.food);s.last_stages=s.stages;s.food=false;}\n  if tick%100==0{')
change('timing.rs','   eprintln!("CAPACITY {}",serde_json::json!({"unix":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),"tick":tick,"rows":s.rows,"stages":s.stages}));s.rows.clear();s.stages=[0;16];', """   let mut report=serde_json::json!({"unix":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),"tick":tick,"rows":s.rows,"stages":s.stages});
   if !s.phase_rows.is_empty(){report["phaseRows"]=serde_json::json!(s.phase_rows);report["foodFlags"]=serde_json::json!(s.food_rows);}
   eprintln!("CAPACITY {}",report);s.rows.clear();s.stages=[0;16];s.last_stages=[0;16];s.phase_rows.clear();s.food_rows.clear();""")
p=r/'timing.rs';p.write_text(p.read_text()+'\npub fn maintenance(){if DETAIL.load(Ordering::Relaxed){CLOCK.with_borrow_mut(|s|s.food=true);}}\n',newline='\n')
change('run.py',"CAPACITY_DIAGNOSTIC=str(root/'DIAGNOSTIC')","CAPACITY_DIAGNOSTIC=str(root/'DIAGNOSTIC'),CAPACITY_DETAIL=str(root/'DETAIL')")
change('run.py','hold=None;profile=None','hold=None;profile=None;coarse=None')
change('run.py',"    if profile is None:\n     (root/'DIAGNOSTIC').touch();profile=now+diagnostic_seconds;meta['diagnosticStartUnix']=time.time();save()\n    if now>=profile:meta['diagnosticEndUnix']=time.time();break", "    if coarse is None:\n     (root/'DETAIL').touch();coarse=now+(5 if smoke else 20);meta['coarseStartUnix']=time.time();save()\n    if now>=coarse and profile is None:\n     (root/'DETAIL').unlink();meta['coarseEndUnix']=time.time();(root/'DIAGNOSTIC').touch();profile=now+diagnostic_seconds;meta['diagnosticStartUnix']=time.time();save()\n    if profile is not None and now>=profile:\n     meta['diagnosticEndUnix']=time.time();time.sleep(1.1)\n     telemetry.write(json.dumps({'server':check_affinity(server.pid),'client':prior.snapshot(clients.pid)})+'\\n');telemetry.flush();break")
change('select.py',"diagnostic_windows='--diagnostic-windows' in sys.argv[3:];names=", "diagnostic_windows='--diagnostic-windows' in sys.argv[3:];coarse_window='--coarse-window' in sys.argv[3:];names=")
change('select.py',"['--preflight','--diagnostic-windows']]","['--preflight','--diagnostic-windows','--coarse-window']]")
change('select.py'," census={};capacity={};sockets=[];clients=[]", " if coarse_window:meta['holds']=[{'startUnix':meta['coarseStartUnix'],'endUnix':meta['coarseEndUnix']}]\n census={};capacity={};sockets=[];clients=[]")
change('select.py','  assert len(ticks)>=10','  assert len(ticks)>=(4 if coarse_window else 10)')
change('select.py',"   threshold=3*(end-start)/300", "   gate=json.loads((r/'isolation-gate.json').read_text());threshold=(end-start)*(gate['idleEnvelopeRate']+gate['excessRateAllowance'])+gate['endpointQuantumCount']*gate['counterQuantumSeconds']")
change('select.py',"'thresholdSeconds':threshold,'passes'", "'thresholdSeconds':threshold,'idleEnvelopeAllowanceSeconds':(end-start)*gate['idleEnvelopeRate'],'excessAllowanceSeconds':(end-start)*gate['excessRateAllowance'],'counterAllowanceSeconds':gate['endpointQuantumCount']*gate['counterQuantumSeconds'],'passes'")
# Evaluate both retryable gates before naming a failure class. Predicates stand.
change('select.py',"  if not preflight and meta.get('shortMature',False):\n   assert all(v['living']>=490 and 45<=v['meanSegments']<=65 and 156<=v['p90Segments']<=196 and v['p99Segments']>=200 for v in lengths),'mature length class drift'", "  class_ok=True\n  if not preflight and meta.get('shortMature',False):\n   class_ok=all(v['living']>=490 and 45<=v['meanSegments']<=65 and 156<=v['p90Segments']<=196 and v['p99Segments']>=200 for v in lengths)")
change('select.py',"   assert row['backgroundProcessScreen']['passes'],'competing process CPU'", "   if not class_ok and not row['backgroundProcessScreen']['passes']:raise AssertionError('multiple quality gates failed: class and isolation')\n   assert class_ok,'mature length class drift'\n   assert row['backgroundProcessScreen']['passes'],'competing process CPU'")
print('ADAPTED',flush=True)
