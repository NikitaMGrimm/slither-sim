"""Use the retained run.sh, lock and validity gates; never repeat completed arms."""
from pathlib import Path
import json,os,statistics,subprocess,sys,time
if not __debug__:raise SystemExit('run without -O')
r=Path(__file__).resolve().parent
mode=sys.argv[1];assert mode in ['smoke','pairs']
base='4532a76f22958ae3abcca801e90edd30ed9f466f';candidate='e10a20abb4f2b2fc072560787e20d918954a31ba'
plan=[base,candidate] if mode=='smoke' else [base,candidate,candidate,base,base,candidate]
if mode=='pairs':assert len(json.loads((r/'smoke.json').read_text()))==2
dest=r/(mode+'.json');assert not dest.exists(),'inspect retained progress; do not rerun completed holds'
rows=[]
for i,revision in enumerate(plan):
    before=set((r/'raw').glob('*/attempt.json'))
    env=dict(os.environ)
    if mode=='smoke':env['STUDY_SMOKE']='1'
    else:env.pop('STUDY_SMOKE',None)
    print('START',mode,i,revision[:7],flush=True)
    with (r/(mode+'-'+str(i)+'.log')).open('x') as log:
        subprocess.run([str(r/'run.sh'),revision,'--preset','official','--count','8' if mode=='smoke' else '500'],cwd=r,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
    created=set((r/'raw').glob('*/attempt.json'))-before;assert len(created)==1
    path=created.pop().with_name('summary.json');v=json.loads(path.read_text())
    assert v['commit']==revision and v['validTraffic'] and v['validAffinity'] and v['generatorHasHeadroom']
    assert v['hostAffinity']['allowedCpuUnion']==v['hostAffinity']['lastCpuUnion']==[0]
    assert v['generatorAffinity']['allowedCpuUnion']==v['generatorAffinity']['lastCpuUnion']==[1]
    row={'revision':revision,'attempt':path.parent.name,'cpuMsPerTick':v['cpuMsPerExecutedTick']}
    rows.append(row);dest.write_text(json.dumps(rows,indent=2)+'\n')
    print('DONE',mode,i,row['cpuMsPerTick'],flush=True)
if mode=='pairs':
    pairs=[]
    for i in range(3):
        arm={v['revision']:v['cpuMsPerTick'] for v in rows[2*i:2*i+2]}
        pairs.append({'base':arm[base],'candidate':arm[candidate],'ratio':arm[candidate]/arm[base]})
    (r/'paired-summary.json').write_text(json.dumps({'pairs':pairs,'medianRatio':statistics.median(v['ratio'] for v in pairs),'consistentReduction':all(v['ratio']<1 for v in pairs)},indent=2)+'\n')
    print('PAIRS',json.dumps(pairs),flush=True)
