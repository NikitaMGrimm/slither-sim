"""Four predeclared interleaved pairs; retain all attempts, replace none."""
from pathlib import Path
import hashlib,json,os,subprocess,sys
if not __debug__:raise SystemExit('run without -O')
r=Path(__file__).resolve().parent;os.sched_setaffinity(0,{1})
a='4532a76f22958ae3abcca801e90edd30ed9f466f';b='e10a20abb4f2b2fc072560787e20d918954a31ba'
mode=sys.argv[1]
if mode not in ['smoke','pairs']:raise SystemExit('smoke or pairs')
plan=[a,b] if mode=='smoke' else [b,a,a,b,b,a,a,b]
if mode=='pairs' and not (r/'followup-smoke.json').exists():raise SystemExit('smokes required')
dest=r/('followup-'+mode+'.json')
if dest.exists():raise SystemExit('preserve completed/partial series; inspect before recovery')
rows=[]
for i,ref in enumerate(plan):
 marker=r/f'{mode}-{i}.started';marker.write_text(ref+'\n') if not marker.exists() else (_ for _ in ()).throw(RuntimeError('attempt already reserved'))
 before=set((r/'raw').glob('*/attempt.json'));env=dict(os.environ)
 if mode=='smoke':env['STUDY_SMOKE']='1'
 else:env.pop('STUDY_SMOKE',None)  # pairs must never inherit the smoke hold
 print('START',mode,i,ref[:7],flush=True)
 with (r/f'followup-{mode}-{i}.log').open('x') as log:
  subprocess.run([str(r/'run.sh'),ref,'--preset','official','--count','8' if mode=='smoke' else '500'],cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 created=set((r/'raw').glob('*/attempt.json'))-before
 if len(created)!=1:raise RuntimeError('expected one retained attempt')
 p=created.pop().parent;v=json.loads((p/'summary.json').read_text());world=json.loads((p/'host/world.json').read_text());h=json.loads((p/'host/host.json').read_text())
 if not (v['validTraffic'] and v['validAffinity'] and v['generatorHasHeadroom']):raise RuntimeError('validity failed')
 if v['hostAffinity']['allowedCpuUnion']!=[0] or v['generatorAffinity']['allowedCpuUnion']!=[1]:raise RuntimeError('wrong affinity')
 post=[x for x in world if x['unix']>=h['holdEndUnix']]
 if len(post)<29 or post[-1]['unix']<h['holdEndUnix']+29:raise RuntimeError('missing live post window')
 if not all(x['alive']>0 and x['food']>0 for x in post[:29]):raise RuntimeError('post world not live')
 row={'revision':ref,'attempt':p.name,'cpuMsPerTick':v['cpuMsPerExecutedTick'],'excessP99Ms':v['excessMs']['p99'],'deferred':v['deferredTicks'],'summarySha256':hashlib.sha256((p/'summary.json').read_bytes()).hexdigest()}
 rows.append(row);dest.write_text(json.dumps(rows,indent=2)+'\n');print('DONE',mode,i,json.dumps(row),flush=True)
